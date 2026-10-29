// Responses wire adapter for the existing ChatGPT OAuth connection.
fn tool_failure(error: String, provider: &str, model: &str) -> ProviderToolTurn {
    ProviderToolTurn {
        text: format!("Provider error: {error}"),
        thinking: String::new(),
        tool_calls: Vec::new(),
        response_items: Vec::new(),
        provider_name: provider.into(),
        model_id: model.into(),
    }
}

fn responses_tool_payload(messages: &[Value], tools: &Value, model: &str) -> Value {
    let mut input = Vec::new();
    let mut instructions = Vec::new();
    for message in messages {
        let role = message["role"].as_str().unwrap_or("user");
        if role == "system" {
            instructions.push(message["content"].as_str().unwrap_or("").to_string());
        } else if role == "tool" {
            input.push(json!({"type":"function_call_output", "call_id":message["tool_call_id"], "output":message["content"]}));
        } else if let Some(items) = message["_responses_output"]
            .as_array()
            .filter(|items| !items.is_empty())
        {
            // Preserve reasoning items alongside function calls across tool rounds.
            input.extend(items.iter().cloned());
        } else {
            if let Some(text) = message["content"].as_str().filter(|text| !text.is_empty()) {
                input.push(json!({"type":"message", "role":role, "content":[{"type":if role == "assistant" {"output_text"} else {"input_text"}, "text":text}]}));
            }
            if let Some(parts) = message["content"].as_array() {
                let content: Vec<Value> = parts.iter().filter_map(|p| match p["type"].as_str() {
                    Some("text") => Some(json!({"type":"input_text","text":p["text"]})),
                    Some("image_url") => Some(json!({"type":"input_image","image_url":p["image_url"]["url"]})),
                    _ => None,
                }).collect();
                if !content.is_empty() { input.push(json!({"type":"message","role":"user","content":content})); }
            }
            if let Some(calls) = message["tool_calls"].as_array() {
                for call in calls {
                    input.push(json!({"type":"function_call", "call_id":call["id"], "name":call["function"]["name"], "arguments":call["function"]["arguments"]}));
                }
            }
        }
    }
    let tools: Vec<Value> = tools
        .as_array()
        .into_iter()
        .flatten()
        .map(|tool| {
            let mut function = tool["function"].clone();
            function["type"] = json!("function");
            function["strict"] = json!(false);
            function
        })
        .collect();
    json!({"model":model,"instructions":instructions.join("\n"),"input":input,"tools":tools,
        "tool_choice":"auto","store":false,"stream":true,"include":["reasoning.encrypted_content"]})
}

fn parse_responses_tool_stream<R: std::io::BufRead, F: FnMut(StreamChunk)>(
    reader: R,
    mut on_chunk: F,
) -> Result<(String, String, Vec<ProviderToolCall>, Vec<Value>), String> {
    let mut text = String::new();
    let mut thinking = String::new();
    for line in reader.lines() {
        let line = line.map_err(|error| format!("Interrupted provider stream: {error}"))?;
        let Some(data) = line.trim().strip_prefix("data:") else {
            continue;
        };
        if data.trim() == "[DONE]" {
            continue;
        }
        let ev: Value = serde_json::from_str(data).map_err(|error| error.to_string())?;
        match ev["type"].as_str().unwrap_or("") {
            "response.output_text.delta" => {
                let delta = ev["delta"].as_str().unwrap_or("");
                text.push_str(delta);
                on_chunk(StreamChunk::Text(delta.into()));
            }
            "response.reasoning_summary_text.delta" | "response.reasoning_text.delta" => {
                let delta = ev["delta"].as_str().unwrap_or("");
                thinking.push_str(delta);
                on_chunk(StreamChunk::Thinking(delta.into()));
            }
            "response.completed" => {
                if ev["response"]["status"] != "completed" {
                    return Err("Incomplete provider response".into());
                }
                let items = ev["response"]["output"]
                    .as_array()
                    .ok_or("Missing response output")?
                    .clone();
                let mut calls = Vec::new();
                for item in &items {
                    if item["type"] == "function_call" {
                        let name = item["name"].as_str().ok_or("Missing function name")?;
                        let arguments = item["arguments"]
                            .as_str()
                            .ok_or("Missing function arguments")?;
                        let id = item["call_id"]
                            .as_str()
                            .filter(|id| !id.is_empty())
                            .ok_or("Missing call ID")?;
                        let value = serde_json::from_str(arguments)
                            .map_err(|error| format!("Invalid tool arguments: {error}"))?;
                        crate::agent_tools::validate_input(name, &value)?;
                        calls.push(ProviderToolCall {
                            id: id.into(),
                            name: name.into(),
                            arguments: arguments.into(),
                        });
                    }
                }
                if text.is_empty() {
                    for item in &items {
                        if let Some(parts) = item["content"].as_array() {
                            for part in parts {
                                if let Some(t) = part["text"].as_str() {
                                    text.push_str(t);
                                }
                            }
                        }
                    }
                    if !text.is_empty() {
                        on_chunk(StreamChunk::Text(text.clone()));
                    }
                }
                return Ok((text, thinking, calls, items));
            }
            "error" | "response.failed" | "response.incomplete" => {
                return Err(format!(
                    "Provider response failed: {}",
                    ev.get("error").unwrap_or(&ev)
                ))
            }
            _ => {}
        }
    }
    Err("Provider stream ended before response.completed; no tools were executed.".into())
}

impl ProviderManager {
    fn complete_codex_tool_turn<F: FnMut(StreamChunk)>(
        &self,
        messages: &[Value],
        tools: &Value,
        model: &str,
        token: &str,
        on_chunk: &mut F,
    ) -> ProviderToolTurn {
        let result = (|| -> Result<_, String> {
            let http = reqwest::blocking::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(15))
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .map_err(|e| e.to_string())?;
            let payload = responses_tool_payload(messages, tools, model);
            let mut credential = token.to_string();
            for attempt in 0..2 {
                let account = Self::extract_chatgpt_account_id(&credential)
                    .ok_or("OpenAI OAuth account unavailable; reconnect OpenAI.")?;
                let response = http
                    .post("https://chatgpt.com/backend-api/codex/responses")
                    .bearer_auth(&credential)
                    .header("ChatGPT-Account-Id", account)
                    .header("originator", "codex_cli_rs")
                    .header("User-Agent", "codex_cli_rs/0.1.0")
                    .header("Accept", "text/event-stream")
                    .json(&payload)
                    .send()
                    .map_err(|e| e.to_string())?;
                if response.status().as_u16() == 401 && attempt == 0 {
                    if let Some(fresh) = self.refresh_codex_token(&http) {
                        credential = fresh;
                        continue;
                    }
                }
                if !response.status().is_success() {
                    let status = response.status();
                    return Err(format!(
                        "HTTP {status}: {}",
                        response
                            .text()
                            .unwrap_or_default()
                            .chars()
                            .take(400)
                            .collect::<String>()
                    ));
                }
                return parse_responses_tool_stream(std::io::BufReader::new(response), on_chunk);
            }
            Err("OpenAI OAuth authentication failed".into())
        })();
        match result {
            Ok((text, thinking, tool_calls, response_items)) => ProviderToolTurn {
                text,
                thinking,
                tool_calls,
                response_items,
                provider_name: "OpenAI".into(),
                model_id: model.into(),
            },
            Err(error) => tool_failure(error, "OpenAI", model),
        }
    }
}

#[cfg(test)]
mod responses_tool_tests {
    use super::*;
    #[test]
    fn retains_reasoning_and_matches_call_results() {
        let reasoning = json!({"type":"reasoning","id":"r1","encrypted_content":"opaque"});
        let call = json!({"type":"function_call","call_id":"c1","name":"Read","arguments":"{\"file_path\":\"a\"}"});
        let payload = responses_tool_payload(
            &[
                json!({"role":"system","content":"instructions"}),
                json!({"role":"user","content":"read a"}),
                json!({"role":"assistant","_responses_output":[reasoning,call]}),
                json!({"role":"tool","tool_call_id":"c1","content":"contents"}),
            ],
            &crate::agent_tools::tool_schemas(),
            "test-model",
        );
        assert_eq!(payload["input"][1]["type"], "reasoning");
        assert_eq!(payload["input"][2]["call_id"], "c1");
        assert_eq!(payload["input"][3]["type"], "function_call_output");
        assert_eq!(payload["tools"][0]["name"], "Bash");
    }
    #[test]
    fn screenshot_observations_use_responses_image_blocks() {
        let payload=responses_tool_payload(&[json!({"role":"user","content":[{"type":"text","text":"Screenshot"},{"type":"image_url","image_url":{"url":"data:image/png;base64,aGVsbG8="}}]})],&json!([]),"fixture");
        assert_eq!(payload["input"][0]["content"][1]["type"],"input_image");
        assert_eq!(payload["input"][0]["content"][1]["image_url"],"data:image/png;base64,aGVsbG8=");
    }
    #[test]
    fn only_completed_valid_responses_can_execute_tools() {
        let complete = json!({"type":"response.completed","response":{"status":"completed","output":[{"type":"function_call","call_id":"c1","name":"Read","arguments":"{\"file_path\":\"a\"}"}]}});
        let body = format!("data: {complete}\n\n");
        assert_eq!(
            parse_responses_tool_stream(std::io::Cursor::new(body), |_| {})
                .unwrap()
                .2
                .len(),
            1
        );
        for ev in [
            json!({"type":"response.function_call_arguments.done","arguments":"{}"}),
            json!({"type":"response.incomplete"}),
            json!({"type":"response.completed","response":{"status":"completed","output":[{"type":"function_call","call_id":"c1","name":"Write","arguments":"{}"}]}}),
        ] {
            assert!(parse_responses_tool_stream(
                std::io::Cursor::new(format!("data: {ev}\n\n")),
                |_| {}
            )
            .is_err());
        }
    }
}
