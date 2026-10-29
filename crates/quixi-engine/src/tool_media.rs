//! Preserve image observations separately from textual tool output.
use base64::Engine;
use serde_json::{json, Value};

pub(crate) fn encode_result(value: &Value) -> String {
    fn walk(v: &Value, images: &mut Vec<Value>) -> Value {
        if let Some(map) = v.as_object() {
            let mime = map
                .get("mimeType")
                .or_else(|| map.get("mime_type"))
                .and_then(Value::as_str)
                .unwrap_or("image/png");
            for key in ["data", "base64", "image", "image_data"] {
                if let Some(data) = map.get(key).and_then(Value::as_str) {
                    if (v["type"] == "image" || key != "data") && data.len() > 512 {
                        let valid = matches!(
                            mime,
                            "image/png" | "image/jpeg" | "image/webp" | "image/gif"
                        ) && data.len() <= 8_000_000
                            && base64::engine::general_purpose::STANDARD
                                .decode(data)
                                .is_ok();
                        if valid && images.len() < 4 {
                            images.push(json!({"type":"image_url","image_url":{"url":format!("data:{mime};base64,{data}")}}));
                            return json!({"text":"Screenshot attached for visual inspection."});
                        }
                        return json!({"text":"Image could not be attached: unsupported format, invalid data, or image size/count limit. Do not claim to have seen it."});
                    }
                }
            }
            Value::Object(
                map.iter()
                    .map(|(k, v)| (k.clone(), walk(v, images)))
                    .collect(),
            )
        } else if let Some(items) = v.as_array() {
            Value::Array(items.iter().map(|v| walk(v, images)).collect())
        } else {
            v.clone()
        }
    }
    let mut images = Vec::new();
    let text = walk(value, &mut images)
        .to_string()
        .chars()
        .take(12000)
        .collect::<String>();
    if images.is_empty() {
        text
    } else {
        json!({"_quixi_tool_media":true,"text":text,"images":images}).to_string()
    }
}

/// Both APIs accept images as user observations; tool replies remain textual.
/// Flush after all tool results so parallel calls retain a valid call/result sequence.
pub(crate) fn provider_messages(messages: &[Value]) -> Vec<Value> {
    let mut out = Vec::new();
    let mut images = Vec::new();
    for m in messages {
        if m["role"] != "tool" && !images.is_empty() {
            out.push(json!({"role":"user","content":std::mem::take(&mut images)}));
        }
        let mut m = m.clone();
        if m["role"] == "tool" {
            if let Some(v) = m["content"]
                .as_str()
                .and_then(|s| serde_json::from_str::<Value>(s).ok())
                .filter(|v| v["_quixi_tool_media"] == true)
            {
                m["content"] = v["text"].clone();
                images.push(json!({"type":"text","text":format!("Images returned by tool call {}:",m["tool_call_id"])}));
                images.extend(v["images"].as_array().into_iter().flatten().cloned());
            }
        }
        out.push(m);
    }
    if !images.is_empty() {
        out.push(json!({"role":"user","content":images}));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn images_survive_tool_history_without_changing_call_order() {
        let data = base64::engine::general_purpose::STANDARD.encode(vec![42; 600]);
        let encoded = encode_result(
            &json!({"content":[{"type":"text","text":"screen"},{"type":"image","mimeType":"image/png","data":data}]}),
        );
        let messages = provider_messages(&[
            json!({"role":"tool","tool_call_id":"a","content":encoded}),
            json!({"role":"tool","tool_call_id":"b","content":"done"}),
        ]);
        assert_eq!(messages[1]["role"], "tool");
        assert_eq!(messages[2]["role"], "user");
        assert!(messages[2]["content"][1]["image_url"]["url"]
            .as_str()
            .unwrap()
            .ends_with(&data));
        assert!(!messages[0]["content"].as_str().unwrap().contains(&data));
    }
    #[test]
    fn invalid_images_are_explained_and_mcp_errors_retained() {
        let encoded = encode_result(
            &json!({"isError":true,"content":[{"type":"image","data":"!".repeat(600)}]}),
        );
        assert!(encoded.contains("Do not claim"));
        assert!(encoded.contains("isError"));
    }
}
