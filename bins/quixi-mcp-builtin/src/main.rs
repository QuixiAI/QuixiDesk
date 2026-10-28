use quixi_mcp::McpHost;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let host = McpHost::new();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from(quixi_proto::paths::default_workspace_str()));

    // Support direct CLI testing: `quixi-mcp-builtin --cli <tool_name> [json_args]`
    if let Some(pos) = args.iter().position(|a| a == "--cli") {
        let tool_name = args.get(pos + 1).map(|s| s.as_str()).unwrap_or("screenshot");
        let tool_args: Value = args
            .get(pos + 2)
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_else(|| json!({}));
        let res = host.call_builtin_tool(tool_name, &tool_args, &cwd)?;
        println!("{}", serde_json::to_string_pretty(&res)?);
        return Ok(());
    }

    let mode = if args.iter().any(|a| a == "--browser-use" || a == "--chrome") {
        "claude-in-chrome"
    } else if args.iter().any(|a| a == "--workspace") {
        "quixi-workspace"
    } else if args.iter().any(|a| a == "--github") {
        "github"
    } else if args.iter().any(|a| a == "--office365") {
        "office365"
    } else {
        "computer-use"
    };

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(req) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };

        let id = req.get("id").cloned();
        let method = req.get("method").and_then(|v| v.as_str()).unwrap_or("");
        let params = req.get("params").cloned().unwrap_or_else(|| json!({}));

        // JSON-RPC notifications do not have an `id` and expect no response
        let Some(req_id) = id else {
            continue;
        };

        let result = match method {
            "initialize" => {
                let proto = params
                    .get("protocolVersion")
                    .and_then(|v| v.as_str())
                    .unwrap_or("2024-11-05");
                json!({
                    "protocolVersion": proto,
                    "capabilities": {
                        "tools": { "listChanged": false },
                        "resources": {},
                        "prompts": {}
                    },
                    "serverInfo": {
                        "name": format!("quixi-{}", mode),
                        "version": "3.6556.2"
                    }
                })
            }
            "ping" => json!({}),
            "tools/list" => {
                let tools = match mode {
                    "claude-in-chrome" => McpHost::claude_browser_and_preview_tools(),
                    "quixi-workspace" => McpHost::workspace_tools(),
                    "github" => McpHost::github_tools(),
                    "office365" => McpHost::office365_tools(),
                    _ => McpHost::computer_use_tools(),
                };
                json!({ "tools": tools })
            }
            "tools/call" => {
                let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                match host.call_builtin_tool(tool_name, &arguments, &cwd) {
                    Ok(res) => res,
                    Err(err) => json!({
                        "content": [{ "type": "text", "text": format!("Tool error: {}", err) }],
                        "isError": true
                    }),
                }
            }
            "resources/list" => json!({ "resources": [] }),
            "resources/templates/list" => json!({ "resourceTemplates": [] }),
            "prompts/list" => json!({ "prompts": [] }),
            _ => json!({}),
        };

        let resp = json!({
            "jsonrpc": "2.0",
            "id": req_id,
            "result": result
        });
        if let Ok(serialized) = serde_json::to_string(&resp) {
            let _ = writeln!(stdout, "{}", serialized);
            let _ = stdout.flush();
        }
    }

    Ok(())
}
