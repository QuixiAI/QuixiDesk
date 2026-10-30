// Native Messages adapter. Schemas stay intact; tool calls execute only after a complete stream.
// Protocol: https://platform.claude.com/docs/en/build-with-claude/streaming
fn anthropic_payload(messages: &[Value], tools: &Value, model: &str) -> Result<Value, String> {
    let mut system = Vec::new();
    let mut converted: Vec<Value> = Vec::new();
    for message in messages {
        let role = message["role"].as_str().unwrap_or("user");
        if role == "system" {
            system.push(message["content"].as_str().unwrap_or("").to_owned());
            continue;
        }
        let mut content = Vec::new();
        if role == "tool" {
            content.push(json!({"type":"tool_result","tool_use_id":message["tool_call_id"],"content":message["content"],"is_error":message["is_error"].as_bool().unwrap_or(false)}));
        } else if let Some(native) = message["_responses_output"].as_array().filter(|v| {
            !v.is_empty()
                && v.iter().all(|b| {
                    matches!(
                        b["type"].as_str(),
                        Some("text" | "thinking" | "redacted_thinking" | "tool_use")
                    )
                })
        }) {
            content.extend(native.iter().cloned());
        } else {
            if let Some(text) = message["content"].as_str().filter(|v| !v.is_empty()) {
                content.push(json!({"type":"text","text":text}));
            }
            if let Some(blocks) = message["content"].as_array() {
                for block in blocks {
                    match block["type"].as_str() {
                        Some("text") => content.push(block.clone()),
                        Some("image_url") => {
                            let url = block["image_url"]["url"]
                                .as_str()
                                .ok_or("Image has no URL")?;
                            let source = if let Some(data) = url.strip_prefix("data:") {
                                let (mime, data) = data
                                    .split_once(";base64,")
                                    .ok_or("Unsupported image data URL")?;
                                json!({"type":"base64","media_type":mime,"data":data})
                            } else {
                                json!({"type":"url","url":url})
                            };
                            content.push(json!({"type":"image","source":source}));
                        }
                        _ => {
                            return Err("Unsupported content block in Anthropic conversation".into())
                        }
                    }
                }
            }
            if let Some(calls) = message["tool_calls"].as_array() {
                for call in calls {
                    let input: Value = serde_json::from_str(
                        call["function"]["arguments"].as_str().unwrap_or("{}"),
                    )
                    .map_err(|e| format!("Invalid stored tool arguments: {e}"))?;
                    content.push(json!({"type":"tool_use","id":call["id"],"name":call["function"]["name"],"input":input}));
                }
            }
        }
        if content.is_empty() {
            continue;
        }
        let role = if role == "assistant" {
            "assistant"
        } else {
            "user"
        };
        if converted.last().is_some_and(|v| v["role"] == role) {
            converted.last_mut().unwrap()["content"]
                .as_array_mut()
                .unwrap()
                .extend(content);
        } else {
            converted.push(json!({"role":role,"content":content}));
        }
    }
    let tools: Vec<Value> = tools
        .as_array()
        .ok_or("Invalid tool definitions")?
        .iter()
        .map(|tool| {
            let f = &tool["function"];
            json!({"name":f["name"],"description":f["description"],"input_schema":f["parameters"]})
        })
        .collect();
    Ok(
        json!({"model":model,"max_tokens":4096,"system":system.join("\n\n"),"messages":converted,"tools":tools,"tool_choice":{"type":"auto"},"stream":true}),
    )
}

fn parse_anthropic_tool_stream<R: std::io::BufRead, F: FnMut(StreamChunk)>(
    reader: R,
    on_chunk: &mut F,
) -> Result<(String, String, Vec<ProviderToolCall>, Vec<Value>), String> {
    use std::collections::{BTreeMap, HashSet};
    let mut blocks: BTreeMap<usize, (Value, String, bool)> = BTreeMap::new();
    let mut started = false;
    let mut stopped = false;
    let mut stop_reason = String::new();
    let mut bytes = 0usize;
    for line in reader.lines() {
        let line = line.map_err(|e| format!("Anthropic stream read failed: {e}"))?;
        bytes += line.len();
        if bytes > 16 * 1024 * 1024 {
            return Err("Anthropic stream exceeded size limit".into());
        }
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let event: Value = serde_json::from_str(data.trim())
            .map_err(|e| format!("Invalid Anthropic event: {e}"))?;
        match event["type"].as_str() {
            Some("message_start") => {
                if started {
                    return Err("Duplicate Anthropic message start".into());
                }
                started = true;
            }
            Some("content_block_start") => {
                let i = event["index"].as_u64().ok_or("Missing block index")? as usize;
                if !started
                    || blocks
                        .insert(i, (event["content_block"].clone(), String::new(), false))
                        .is_some()
                {
                    return Err("Invalid Anthropic block start".into());
                }
            }
            Some("content_block_delta") => {
                let i = event["index"].as_u64().ok_or("Missing block index")? as usize;
                let (block, json, closed) = blocks.get_mut(&i).ok_or("Delta without a block")?;
                if *closed {
                    return Err("Delta after closed block".into());
                }
                let d = &event["delta"];
                match d["type"].as_str() {
                    Some("input_json_delta") if block["type"] == "tool_use" => {
                        json.push_str(d["partial_json"].as_str().ok_or("Invalid JSON delta")?)
                    }
                    Some("text_delta") if block["type"] == "text" => {
                        let s = d["text"].as_str().ok_or("Invalid text delta")?;
                        let current = block["text"].as_str().unwrap_or("");
                        block["text"] = json!(format!("{current}{s}"));
                        on_chunk(StreamChunk::Text(s.into()));
                    }
                    Some("thinking_delta") if block["type"] == "thinking" => {
                        let s = d["thinking"].as_str().ok_or("Invalid thinking delta")?;
                        let current = block["thinking"].as_str().unwrap_or("");
                        block["thinking"] = json!(format!("{current}{s}"));
                        on_chunk(StreamChunk::Thinking(s.into()));
                    }
                    Some("signature_delta") if block["type"] == "thinking" => {
                        let current = block["signature"].as_str().unwrap_or("");
                        block["signature"] = json!(format!(
                            "{current}{}",
                            d["signature"].as_str().ok_or("Invalid signature")?
                        ));
                    }
                    _ => return Err("Unsupported Anthropic content delta".into()),
                }
            }
            Some("content_block_stop") => {
                let i = event["index"].as_u64().ok_or("Missing block index")? as usize;
                let (block, partial, closed) = blocks.get_mut(&i).ok_or("Stop without a block")?;
                if *closed {
                    return Err("Duplicate block stop".into());
                }
                if block["type"] == "tool_use" && !partial.is_empty() {
                    block["input"] = serde_json::from_str(partial)
                        .map_err(|e| format!("Invalid tool arguments: {e}"))?;
                }
                *closed = true;
            }
            Some("message_delta") => {
                if let Some(reason) = event["delta"]["stop_reason"].as_str() {
                    stop_reason = reason.into();
                }
            }
            Some("message_stop") => {
                stopped = true;
                break;
            }
            Some("ping") => {}
            Some("error") => return Err(format!("Anthropic stream error: {}", event["error"])),
            _ => return Err("Unsupported Anthropic stream event".into()),
        }
    }
    if !started
        || !stopped
        || blocks.values().any(|(_, _, closed)| !*closed)
        || !matches!(
            stop_reason.as_str(),
            "tool_use" | "end_turn" | "stop_sequence"
        )
    {
        return Err(format!(
            "Incomplete Anthropic turn (stop_reason={stop_reason})"
        ));
    }
    let mut calls = Vec::new();
    let mut ids = HashSet::new();
    let mut text = String::new();
    let mut thinking = String::new();
    let mut native = Vec::new();
    for (_, (block, _, _)) in blocks {
        match block["type"].as_str() {
            Some("tool_use") => {
                let id = block["id"]
                    .as_str()
                    .filter(|v| !v.is_empty())
                    .ok_or("Missing tool id")?;
                let name = block["name"]
                    .as_str()
                    .filter(|v| !v.is_empty())
                    .ok_or("Missing tool name")?;
                if !ids.insert(id.to_owned()) || !block["input"].is_object() {
                    return Err("Invalid tool arguments or duplicate id".into());
                }
                crate::agent_tools::validate_provider_input(name, &block["input"])?;
                calls.push(ProviderToolCall {
                    id: id.into(),
                    name: name.into(),
                    arguments: block["input"].to_string(),
                });
            }
            Some("text") => text.push_str(block["text"].as_str().unwrap_or("")),
            Some("thinking") => thinking.push_str(block["thinking"].as_str().unwrap_or("")),
            Some("redacted_thinking") => {}
            _ => return Err("Unsupported Anthropic content block".into()),
        }
        native.push(block);
    }
    if (stop_reason == "tool_use") != !calls.is_empty() {
        return Err("Anthropic stop reason does not match tool calls".into());
    }
    Ok((text, thinking, calls, native))
}

impl ProviderManager {
    fn complete_anthropic_tool_turn<F: FnMut(StreamChunk)>(
        &self,
        messages: &[Value],
        tools: &Value,
        model: &str,
        base: &str,
        key: &str,
        provider: &str,
        on_chunk: &mut F,
    ) -> ProviderToolTurn {
        let run = || -> Result<reqwest::blocking::Response, String> {
            let payload = anthropic_payload(messages, tools, model)?;
            let http = reqwest::blocking::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(15))
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .map_err(|e| e.to_string())?;
            let response = http
                .post(format!("{base}/messages"))
                .header("x-api-key", key)
                .header("anthropic-version", "2023-06-01")
                .header("accept", "text/event-stream")
                .json(&payload)
                .send()
                .map_err(|e| e.to_string())?;
            if !response.status().is_success() {
                return Err(format!(
                    "Anthropic HTTP {}: {}",
                    response.status(),
                    response
                        .text()
                        .unwrap_or_default()
                        .chars()
                        .take(400)
                        .collect::<String>()
                ));
            }
            Ok(response)
        };
        let response = match run() {
            Ok(v) => v,
            Err(e) => return tool_failure(e, provider, model),
        };
        match parse_anthropic_tool_stream(std::io::BufReader::new(response), on_chunk) {
            Ok((text, thinking, tool_calls, response_items)) => ProviderToolTurn {
                text,
                thinking,
                tool_calls,
                response_items,
                provider_name: provider.into(),
                model_id: model.into(),
                stream_error: None,
            },
            Err(e) => tool_failure(e, provider, model),
        }
    }
}

#[cfg(test)]
mod anthropic_tool_tests {
    use super::*;
    fn stream(events: Vec<Value>) -> String {
        events.iter().map(|e| format!("data: {e}\n\n")).collect()
    }
    #[test]
    fn incomplete_error_and_truncated_streams_never_return_executable_calls() {
        let events = vec![
            json!({"type":"message_start"}),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"call","name":"McpCall","input":{}}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"server\":\"s\"}"}}),
            json!({"type":"content_block_stop","index":0}),
        ];
        for ending in [
            vec![],
            vec![json!({"type":"error","error":{"message":"overloaded"}})],
            vec![
                json!({"type":"message_delta","delta":{"stop_reason":"max_tokens"}}),
                json!({"type":"message_stop"}),
            ],
        ] {
            let mut all = events.clone();
            all.extend(ending);
            assert!(
                parse_anthropic_tool_stream(std::io::Cursor::new(stream(all)), &mut |_| {})
                    .is_err()
            );
        }
    }
    #[test]
    fn native_adapter_preserves_schema_signed_thinking_errors_and_images() {
        let schema = json!({"type":"object","properties":{"x":{"oneOf":[{"type":"number","minimum":3},{"type":"null"}]}},"additionalProperties":false});
        let signed = json!({"type":"thinking","thinking":"reason","signature":"signed"});
        let payload=anthropic_payload(&[
            json!({"role":"system","content":"system"}),
            json!({"role":"assistant","_responses_output":[signed.clone(),{"type":"tool_use","id":"a","name":"McpCall","input":{}}]}),
            json!({"role":"tool","tool_call_id":"a","content":"Denied","is_error":true}),
            json!({"role":"user","content":[{"type":"image_url","image_url":{"url":"data:image/png;base64,YQ=="}}]}),
        ],&json!([{"function":{"name":"native","description":"test","parameters":schema}}]),"model").unwrap();
        assert_eq!(payload["tools"][0]["input_schema"], schema);
        assert_eq!(payload["messages"][0]["content"][0], signed);
        assert_eq!(payload["messages"][1]["content"][0]["is_error"], true);
        assert_eq!(
            payload["messages"][1]["content"][1]["source"]["data"],
            "YQ=="
        );
    }
}
