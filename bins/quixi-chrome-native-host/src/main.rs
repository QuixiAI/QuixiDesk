//! `quixi-chrome-native-host` — Native Rust replacement for `Contents/Helpers/chrome-native-host`
//! Bridges Chrome Native Messaging (4-byte native-endian length-prefixed JSON on stdio)
//! to `quixi-mcp` and `quixi-platform::BrowserUse`.

use quixi_mcp::McpHost;
use quixi_platform::BrowserUse;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::Path;

pub fn handle_native_message(host: &McpHost, incoming: &Value) -> Value {
    let msg_type = incoming
        .get("type")
        .or_else(|| incoming.get("action"))
        .and_then(|v| v.as_str())
        .unwrap_or("ping");

    match msg_type {
        "ping" | "get_status" => json!({
            "type": "pong",
            "host": "quixi-chrome-native-host",
            "version": "3.6556.2",
            "connected": true,
            "tabs": BrowserUse::tabs_context(false)["tabs"],
            "echo": incoming,
        }),
        "tabs_context" | "tabs_context_mcp" => json!({
            "type": "tool_response",
            "result": BrowserUse::tabs_context(true),
        }),
        "navigate" => {
            let url = incoming
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("https://example.com");
            let tab_id = incoming.get("tabId").and_then(|v| v.as_u64()).map(|n| n as u32);
            json!({
                "type": "tool_response",
                "result": BrowserUse::navigate(url, tab_id),
            })
        }
        "read_page" => {
            let tab_id = incoming.get("tabId").and_then(|v| v.as_u64()).map(|n| n as u32);
            let filter = incoming.get("filter").and_then(|v| v.as_str());
            json!({
                "type": "tool_response",
                "result": BrowserUse::read_page(tab_id, filter, 50000),
            })
        }
        "execute_tool" | "tool_request" => {
            let tool = incoming
                .get("tool")
                .or_else(|| incoming.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("tabs_context_mcp");
            let args = incoming
                .get("args")
                .or_else(|| incoming.get("arguments"))
                .cloned()
                .unwrap_or_else(|| json!({}));
            let res = host
                .call_tool(
                    "claude-in-chrome",
                    tool,
                    &args,
                    Path::new(quixi_proto::paths::default_workspace_str()),
                )
                .unwrap_or_else(|e| {
                    json!({
                        "content": [{ "type": "text", "text": e.to_string() }],
                        "isError": true
                    })
                });
            json!({
                "type": "tool_response",
                "tool": tool,
                "result": res,
            })
        }
        _ => json!({
            "type": "pong",
            "host": "quixi-chrome-native-host",
            "version": "3.6556.2",
            "echo": incoming,
        }),
    }
}

fn main() -> anyhow::Result<()> {
    let host = McpHost::new();
    let mut stdin = std::io::stdin().lock();
    let mut stdout = std::io::stdout().lock();

    loop {
        let mut len_bytes = [0u8; 4];
        if stdin.read_exact(&mut len_bytes).is_err() {
            break;
        }
        let msg_len = u32::from_ne_bytes(len_bytes) as usize;
        if msg_len == 0 || msg_len > 16 * 1024 * 1024 {
            break;
        }
        let mut payload = vec![0u8; msg_len];
        stdin.read_exact(&mut payload)?;

        let incoming: Value = serde_json::from_slice(&payload).unwrap_or_else(|_| json!({}));
        let reply = handle_native_message(&host, &incoming);
        let reply_bytes = serde_json::to_vec(&reply)?;
        let out_len = (reply_bytes.len() as u32).to_ne_bytes();
        stdout.write_all(&out_len)?;
        stdout.write_all(&reply_bytes)?;
        stdout.flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chrome_native_host_protocol() {
        let host = McpHost::new();
        let ping = handle_native_message(&host, &json!({ "type": "ping" }));
        assert_eq!(ping["type"], "pong");
        assert_eq!(ping["connected"], true);

        let tool_call = handle_native_message(
            &host,
            &json!({
                "type": "execute_tool",
                "tool": "read_page",
                "args": { "tabId": 1, "filter": "all" }
            }),
        );
        assert_eq!(tool_call["type"], "tool_response");
        assert_eq!(tool_call["result"]["isError"], false);
    }
}
