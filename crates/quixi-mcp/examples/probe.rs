//! Read-only discovery smoke test: cargo run -p quixi-mcp --example probe -- npx -y rea-agents@6.7.0 mcp
use anyhow::{Context, Result};
use quixi_mcp::McpHost;
use serde_json::json;
use std::sync::atomic::AtomicBool;
fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let command = args
        .next()
        .context("Pass an MCP stdio command and arguments")?;
    let data = tempfile::tempdir()?;
    let host = McpHost::new_with_data_dir(data.path());
    host.write_config(&json!({"probe":{"command":command,"args":args.collect::<Vec<_>>(),"connectTimeoutMs":120000}}))?;
    let cwd = std::env::current_dir()?;
    let stop = AtomicBool::new(false);
    let mut cursor = 0;
    let mut names = Vec::new();
    loop {
        let page = host.discover_tools(Some("probe"), None, cursor, &cwd, &stop)?;
        for tool in page["tools"].as_array().unwrap() {
            names.push(tool["name"].as_str().unwrap().to_owned());
        }
        if let Some(n) = page["nextCursor"].as_u64() {
            cursor = n as usize;
        } else {
            break;
        }
    }
    println!(
        "{}",
        json!({"toolCount":names.len(),"tools":names,"status":host.list_server_statuses()[0]["status"]})
    );
    if names.iter().any(|n| n == "binary_session") {
        let result =
            host.call_tool_cancellable("probe", "binary_session", &json!({}), &cwd, &stop)?;
        println!(
            "{}",
            json!({"binarySessionIsError":result["isError"],"contentBlocks":result["content"].as_array().map(|v|v.len()),"resultBytes":result.to_string().len()})
        );
    }
    host.disconnect_server("probe")?;
    Ok(())
}
