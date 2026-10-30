//! Read-only discovery smoke test: cargo run -p quixi-mcp --example probe -- npx -y rea-agents@6.7.0 mcp
//! Public HTTP example: cargo run -p quixi-mcp --example probe -- --url https://docs.x.ai/api/mcp
use anyhow::{Context, Result};
use quixi_mcp::McpHost;
use serde_json::json;
use std::sync::atomic::AtomicBool;
fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let command = args
        .next()
        .context("Pass an MCP stdio command and arguments, or --url URL")?;
    let data = tempfile::tempdir()?;
    let host = McpHost::new_with_data_dir(data.path());
    let mut config = if command == "--url" {
        let url = args.next().context("--url requires an MCP endpoint")?;
        anyhow::ensure!(args.next().is_none(), "Unexpected arguments after URL");
        json!({"transport":"http","url":url,"connectTimeoutMs":120000})
    } else {
        json!({"command":command,"args":args.collect::<Vec<_>>(),"connectTimeoutMs":120000})
    };
    // Remote stdio processes need explicit remote roots; local workspace paths
    // are not translated across SSH. This read-only probe supports verifying that override.
    if let Ok(uri) = std::env::var("QUIXI_MCP_PROBE_ROOT_URI") {
        config["roots"] = json!([{"uri":uri,"name":"Remote workspace"}]);
    }
    host.write_config(&json!({"probe":config}))?;
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
    if names.iter().any(|n| n == "list_doc_pages") {
        let result =
            host.call_tool_cancellable("probe", "list_doc_pages", &json!({}), &cwd, &stop)?;
        println!(
            "{}",
            json!({"listDocPagesIsError":result["isError"],"contentBlocks":result["content"].as_array().map(|v|v.len()),"resultBytes":result.to_string().len()})
        );
    }
    host.disconnect_server("probe")?;
    Ok(())
}
