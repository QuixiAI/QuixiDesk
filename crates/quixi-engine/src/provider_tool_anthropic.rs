// Native Messages adapter. Schemas stay intact; tool calls execute only after a complete stream.
// Protocol: https://platform.claude.com/docs/en/build-with-claude/streaming
// Bounded defaults within known model limits; unknown models stay conservative.
pub(crate) fn anthropic_output_budget(model: &str) -> u32 {
    if model.starts_with("deepseek-") {
        65_536
    } else if ["claude-sonnet-4", "claude-opus-4", "claude-haiku-4", "claude-3-7-sonnet", "claude-fable-5", "claude-opus-5", "claude-sonnet-5", "claude-haiku-5"].iter().any(|prefix| model.contains(prefix)) {
        16_384
    } else if model.contains("claude-3-5-sonnet") || model.contains("claude-3-5-haiku") {
        8_192
    } else { 4_096 }
}

#[cfg(test)]
fn anthropic_payload(messages: &[Value], tools: &Value, model: &str) -> Result<Value, String> {
    anthropic_payload_with_effort(messages, tools, model, None)
}

fn anthropic_payload_with_effort(
    messages: &[Value],
    tools: &Value,
    model: &str,
    effort_override: Option<&str>,
) -> Result<Value, String> {
    let is_deepseek = model.starts_with("deepseek-");
    let in_history_system = model == "deepseek-flash";
    let mut system = Vec::new();
    let mut converted: Vec<Value> = Vec::new();
    for message in messages {
        let role = message["role"].as_str().unwrap_or("user");
        if role == "system" {
            let text = message["content"].as_str().unwrap_or("").to_owned();
            if in_history_system && !converted.is_empty() && !text.is_empty() {
                converted.push(json!({"role":"system","content":[{"type":"text","text":text}]}));
            } else {
                system.push(text);
            }
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
            let mut def = json!({"name":f["name"],"description":f["description"],"input_schema":f["parameters"]});
            if f.get("defer_loading").and_then(Value::as_bool) == Some(true) {
                def["defer_loading"] = json!(true);
            }
            def
        })
        .collect();
    let mut out = json!({
        "model": model,
        "max_tokens": anthropic_output_budget(model),
        "system": system.join("\n\n"),
        "messages": converted,
        "tools": tools,
        "tool_choice": {"type": "auto"},
        "stream": true
    });
    let eff = effort_override
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty() && s != "default" && s != "auto");
    if is_deepseek {
        let disabled = eff.as_deref() == Some("off") || model == "deepseek-chat";
        out["thinking"] = json!({ "type": if disabled { "disabled" } else { "enabled" } });
        if !disabled {
            let mapped = match eff.as_deref() {
                Some("low") => "low",
                Some("max") => "max",
                _ => "high",
            };
            out["output_config"] = json!({ "effort": mapped });
            out.as_object_mut().unwrap().remove("tool_choice");
        }
    } else if let Some(ref level) = eff {
        if matches!(level.as_str(), "low" | "medium" | "high" | "max")
            && (model.contains("claude-sonnet-4")
                || model.contains("claude-opus-4")
                || model.contains("claude-3-7-sonnet")
                || model.contains("claude-opus-5")
                || model.contains("claude-sonnet-5")
                || model.contains("claude-fable-5"))
        {
            let budget = match level.as_str() {
                "low" => 2_048,
                "medium" => 4_096,
                "high" => 8_192,
                _ => 12_288,
            };
            out["max_tokens"] = json!(anthropic_output_budget(model).max(budget + 4_096));
            out["thinking"] = json!({ "type": "enabled", "budget_tokens": budget });
            // Anthropic requires tool_choice auto without forced overrides when thinking is enabled.
        }
    }
    Ok(out)
}

fn parse_anthropic_tool_stream<R: std::io::BufRead, F: FnMut(StreamChunk)>(
    reader: R,
    on_chunk: &mut F,
) -> Result<(String, String, Vec<ProviderToolCall>, Vec<Value>, Option<ProviderTokenUsage>), String> {
    use std::collections::{BTreeMap, HashSet};
    let mut blocks: BTreeMap<usize, (Value, String, bool)> = BTreeMap::new();
    let mut started = false;
    let mut stopped = false;
    let mut stop_reason = String::new();
    let mut bytes = 0usize;
    let mut usage = ProviderTokenUsage::default();
    let mut saw_usage = false;
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
                if let Some(u) = event.pointer("/message/usage").and_then(Value::as_object) {
                    if let Some(v) = u.get("input_tokens").and_then(Value::as_u64) { usage.input_tokens = v; saw_usage = true; }
                    if let Some(v) = u.get("output_tokens").and_then(Value::as_u64) { usage.output_tokens = v; saw_usage = true; }
                    if let Some(v) = u.get("cache_read_input_tokens").and_then(Value::as_u64) { usage.cache_read_tokens = v; saw_usage = true; }
                    if let Some(v) = u.get("cache_creation_input_tokens").and_then(Value::as_u64) { usage.cache_write_tokens = v; saw_usage = true; }
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
                let (_, _, closed) = blocks.get_mut(&i).ok_or("Stop without a block")?;
                if *closed {
                    return Err("Duplicate block stop".into());
                }
                // Defer JSON validation until stop reason is known.
                *closed = true;
            }
            Some("message_delta") => {
                if let Some(reason) = event["delta"]["stop_reason"].as_str() {
                    stop_reason = reason.into();
                }
                if let Some(u) = event.get("usage").and_then(Value::as_object) {
                    if let Some(v) = u.get("input_tokens").and_then(Value::as_u64) { usage.input_tokens = v; saw_usage = true; }
                    if let Some(v) = u.get("output_tokens").and_then(Value::as_u64) { usage.output_tokens = v; saw_usage = true; }
                    if let Some(v) = u.get("cache_read_input_tokens").and_then(Value::as_u64) { usage.cache_read_tokens = v; saw_usage = true; }
                    if let Some(v) = u.get("cache_creation_input_tokens").and_then(Value::as_u64) { usage.cache_write_tokens = v; saw_usage = true; }
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
    for (_, (mut block, partial, _)) in blocks {
        match block["type"].as_str() {
            Some("tool_use") => {
                if !partial.is_empty() {
                    block["input"] = serde_json::from_str(&partial)
                        .map_err(|e| format!("Invalid tool arguments: {e}"))?;
                }
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
                    extra_content: None,
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
    Ok((text, thinking, calls, native, saw_usage.then_some(usage)))
}

fn resolve_messages_root(base: &str) -> String {
    let trimmed = base.trim_end_matches('/');
    if trimmed.ends_with("/v1") || trimmed.ends_with("/anthropic") {
        trimmed.to_string()
    } else if trimmed.contains("api.deepseek.com") || trimmed.contains("api.anthropic.com") {
        format!("{trimmed}/v1")
    } else {
        trimmed.to_string()
    }
}

impl ProviderManager {
    #[allow(dead_code)]
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
        self.complete_anthropic_tool_turn_with_effort(messages, tools, model, base, key, provider, None, on_chunk)
    }

    #[allow(clippy::too_many_arguments)]
    fn complete_anthropic_tool_turn_with_effort<F: FnMut(StreamChunk)>(
        &self,
        messages: &[Value],
        tools: &Value,
        model: &str,
        base: &str,
        key: &str,
        provider: &str,
        effort_override: Option<&str>,
        on_chunk: &mut F,
    ) -> ProviderToolTurn {
        let run = || -> Result<reqwest::blocking::Response, String> {
            let payload = anthropic_payload_with_effort(messages, tools, model, effort_override)?;
            let http = reqwest::blocking::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(15))
                .timeout(std::time::Duration::from_secs(600))
                .build()
                .map_err(|e| e.to_string())?;
            let root = resolve_messages_root(base);
            let response = http
                .post(format!("{root}/messages"))
                .header("User-Agent", concat!("quixi-desk/", env!("CARGO_PKG_VERSION")))
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
        anthropic_stream_turn(std::io::BufReader::new(response), provider, model, on_chunk)
    }
}

fn anthropic_stream_turn<R: std::io::BufRead, F: FnMut(StreamChunk)>(
    reader: R, provider: &str, model: &str, on_chunk: &mut F,
) -> ProviderToolTurn {
        let mut partial_text = String::new();
        let mut partial_thinking = String::new();
        let result = parse_anthropic_tool_stream(reader, &mut |chunk| {
            match &chunk {
                StreamChunk::Text(s) => partial_text.push_str(s),
                StreamChunk::Thinking(s) => partial_thinking.push_str(s),
            }
            on_chunk(chunk);
        });
        match result {
            Ok((text, thinking, tool_calls, response_items, usage)) => ProviderToolTurn {
                text,
                thinking,
                reasoning_details: None,
                tool_calls,
                response_items,
                provider_name: provider.into(),
                model_id: model.into(),
                stream_error: None,
                usage,
            },
            Err(e) => {
                // The caller retries certain parser errors before retaining text.
                // Surface this as a terminal partial response instead, with no calls.
                let error = if e.contains("stop_reason=max_tokens") {
                    "Anthropic output truncated (max_tokens); request a continuation or smaller output. No tools were executed.".to_owned()
                } else {
                    format!("Anthropic response incomplete: {}; no tools were executed.", e.replace("Invalid tool arguments", "Malformed tool input"))
                };
                let notice = format!("

[{error}]");
                on_chunk(StreamChunk::Text(notice.clone()));
                partial_text.push_str(&notice);
                ProviderToolTurn {
                    text: partial_text, thinking: partial_thinking,
                    reasoning_details: None,
                    tool_calls: Vec::new(), response_items: Vec::new(),
                    provider_name: provider.into(), model_id: model.into(),
                    stream_error: Some(error),
                    usage: None,
                }
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
    fn budgets_are_model_aware_and_bounded() {
        for (model, budget) in [("claude-sonnet-4-5", 16384), ("claude-opus-4-1", 16384), ("claude-3-5-sonnet-latest", 8192), ("claude-3-haiku", 4096), ("unknown", 4096)] {
            let payload = anthropic_payload(&[], &json!([]), model).unwrap();
            assert_eq!(payload["max_tokens"], budget);
        }
    }

    #[test]
    fn max_tokens_preserves_long_text_and_never_executes_tools() {
        // More than 4096 tokens even for tokenizers that merge repeated words.
        let text = "This is a long output sentence.
".repeat(5000);
        for input in [None, Some("{\"path\":"), Some("{}" )] {
            let mut events = vec![
                json!({"type":"message_start"}),
                json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
                json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":text}}),
                json!({"type":"content_block_stop","index":0}),
            ];
            if let Some(input) = input {
                events.extend([
                    json!({"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"call","name":"Write","input":{}}}),
                    json!({"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":input}}),
                    json!({"type":"content_block_stop","index":1}),
                ]);
            }
            events.extend([
                json!({"type":"message_delta","delta":{"stop_reason":"max_tokens"}}),
                json!({"type":"message_stop"}),
            ]);
            let mut streamed = String::new();
            let turn = anthropic_stream_turn(std::io::Cursor::new(stream(events)), "Anthropic", "claude-sonnet-4-5", &mut |chunk| {
                if let StreamChunk::Text(s) = chunk { streamed.push_str(&s); }
            });
            assert!(turn.text.starts_with(&text));
            assert_eq!(turn.text, streamed);
            assert!(turn.text.contains("output truncated (max_tokens)"));
            assert!(turn.tool_calls.is_empty());
            assert!(turn.response_items.is_empty());
            assert!(turn.stream_error.unwrap().contains("max_tokens"));
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

// The legacy non-streaming fallback must surface truncation too.
pub(crate) fn anthropic_response_text(response: &Value) -> Option<String> {
    let mut text = response["content"].as_array()?.iter()
        .filter(|block| block["type"] == "text")
        .filter_map(|block| block["text"].as_str())
        .collect::<String>();
    if response["stop_reason"] == "max_tokens" {
        text.push_str("

[Anthropic output truncated (max_tokens); request a continuation or smaller output.]");
    }
    if text.is_empty() { None } else { Some(text) }
}

#[cfg(test)]
mod anthropic_fallback_tests {
    use super::*;
    #[test]
    fn fallback_retains_all_text_and_reports_truncation() {
        let response = json!({"content":[{"type":"thinking","thinking":"hidden"},{"type":"text","text":"one"},{"type":"text","text":"two"}],"stop_reason":"max_tokens"});
        let text = anthropic_response_text(&response).unwrap();
        assert!(text.starts_with("onetwo"));
        assert!(text.contains("truncated (max_tokens)"));
        assert_eq!(anthropic_response_text(&json!({"content":[{"type":"text","text":"complete"}],"stop_reason":"end_turn"})).as_deref(), Some("complete"));
    }
}
