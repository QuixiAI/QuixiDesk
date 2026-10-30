use quixi_mcp::McpHost;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let host = McpHost::new();
    let cwd = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from(quixi_proto::paths::default_workspace_str()));

    // Support direct CLI testing: `quixi-mcp-builtin --cli <tool_name> [json_args]`
    if let Some(pos) = args.iter().position(|a| a == "--cli") {
        let tool_name = args
            .get(pos + 1)
            .map(|s| s.as_str())
            .unwrap_or("screenshot");
        let tool_args: Value = args
            .get(pos + 2)
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_else(|| json!({}));
        let res = host.call_builtin_tool(tool_name, &tool_args, &cwd)?;
        println!("{}", serde_json::to_string_pretty(&res)?);
        return Ok(());
    }

    let mode = if args.iter().any(|a| a == "--browser-use" || a == "--chrome") {
        "quixi-in-chrome"
    } else if args.iter().any(|a| a == "--workspace") {
        "quixi-workspace"
    } else if args.iter().any(|a| a == "--in-app-browser") {
        "QuixiDesk Browser"
    } else if args.iter().any(|a| a == "--github") {
        "github"
    } else if args.iter().any(|a| a == "--office365") {
        "office365"
    } else if args.iter().any(|a| a == "--applescript") {
        "applescript"
    } else if args.iter().any(|a| a == "--memory") {
        "memory"
    } else if args.iter().any(|a| a == "--fetch") {
        "fetch"
    } else if args.iter().any(|a| a == "--pdf") {
        "pdf"
    } else {
        "computer-use"
    };

    let catalog = McpHost::builtin_catalog(mode);
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    // A successful initialize response is followed by the client's initialized
    // notification. Tools remain unavailable until that ordering is complete.
    let mut phase = 0u8;
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(&line) {
            Ok(request) => handle(&host, mode, &catalog, &cwd, &mut phase, request),
            Err(_) => Some(error(Value::Null, -32700, "Invalid JSON")),
        };
        if let Some(response) = response {
            writeln!(stdout, "{}", response)?;
            stdout.flush()?;
        }
    }
    Ok(())
}

fn error(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message.into()}})
}

fn handle(
    host: &McpHost,
    mode: &str,
    catalog: &[Value],
    cwd: &std::path::Path,
    phase: &mut u8,
    request: Value,
) -> Option<Value> {
    let id = request.get("id").cloned();
    if request["jsonrpc"] != "2.0"
        || !request["method"].is_string()
        || id
            .as_ref()
            .is_some_and(|id| !(id.is_null() || id.is_number() || id.is_string()))
    {
        return Some(error(Value::Null, -32600, "Invalid JSON-RPC request"));
    }
    let method = request["method"].as_str().unwrap();
    let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
    if !params.is_object() {
        return id.map(|id| error(id, -32602, "Parameters must be an object"));
    }
    let Some(id) = id else {
        if method == "notifications/initialized" && *phase == 1 {
            *phase = 2;
        }
        return None;
    };
    let result: Result<Value, (i64, String)> = (|| match method {
        "initialize" => {
            if *phase != 0 {
                return Err((-32600, "This connection is already initialized".into()));
            }
            let requested = params["protocolVersion"]
                .as_str()
                .ok_or((-32602, "protocolVersion is required".into()))?;
            if !params["capabilities"].is_object()
                || !params["clientInfo"]["name"].is_string()
                || !params["clientInfo"]["version"].is_string()
            {
                return Err((
                    -32602,
                    "Initialize requires capabilities and clientInfo name/version".into(),
                ));
            }
            let version =
                if ["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"].contains(&requested) {
                    requested
                } else {
                    "2025-11-25"
                };
            *phase = 1;
            Ok(
                json!({"protocolVersion":version,"capabilities":{"tools":{"listChanged":false}},
                    "serverInfo":{"name":format!("quixi-{mode}"),"version":env!("CARGO_PKG_VERSION")}}),
            )
        }
        "ping" => Ok(json!({})),
        "tools/list" | "tools/call" if *phase != 2 => Err((
            -32002,
            "Complete initialize and notifications/initialized before using tools".into(),
        )),
        "tools/list" => {
            if params.get("cursor").is_some() {
                return Err((-32602, "This catalog has no additional pages".into()));
            }
            Ok(json!({"tools":catalog}))
        }
        "tools/call" => {
            let name = params["name"]
                .as_str()
                .ok_or((-32602, "Tool name is required".into()))?;
            if !catalog.iter().any(|tool| tool["name"] == name) {
                return Err((
                    -32602,
                    format!("Unknown or unavailable tool '{name}' for server '{mode}'"),
                ));
            }
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            if !arguments.is_object() {
                return Err((-32602, "Tool arguments must be an object".into()));
            }
            Ok(
                match host.call_builtin_server_tool(mode, name, &arguments, cwd) {
                    Ok(result) => result,
                    Err(error) => {
                        json!({"content":[{"type":"text","text":format!("Tool error: {error}")}],"isError":true})
                    }
                },
            )
        }
        _ => Err((-32601, format!("Unknown JSON-RPC method '{method}'"))),
    })();
    Some(match result {
        Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
        Err((code, message)) => error(id, code, message),
    })
}
