//! Executable tools for the desktop agent loop.
//!
//! Until now QuixiDesk could only *talk*: every turn was prompt + workspace context
//! -> text. This module gives the model the ability to actually act - run a shell
//! command, read/write/edit files, fetch a page, and call any MCP tool (including the
//! built-in Computer Use and Browser Use servers), with a permission gate.

use quixi_mcp::McpHost;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Canonical tool names. They match the names the renderer already knows how to label
/// (`Read` shows "Read <file>", `Bash` shows the command, `mcp__*` shows the server tool).
pub const TOOL_BASH: &str = "Bash";
pub const TOOL_READ: &str = "Read";
pub const TOOL_WRITE: &str = "Write";
pub const TOOL_EDIT: &str = "Edit";
pub const TOOL_WEB_FETCH: &str = "WebFetch";
pub const TOOL_MCP_CALL: &str = "McpCall";

/// OpenAI-style function schemas handed to the provider.
pub fn tool_schemas() -> Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": TOOL_BASH,
                "description": "Run a shell command (zsh) on this Mac in the workspace folder and return stdout, stderr and the exit code. Use it for git, builds, tests, `open -a App`, `osascript` and anything else a terminal can do.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "command": { "type": "string", "description": "The command to run." },
                        "description": { "type": "string", "description": "Short description of what the command does." }
                    },
                    "required": ["command"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": TOOL_READ,
                "description": "Read a text file from disk. Relative paths resolve against the workspace folder.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "file_path": { "type": "string" },
                        "offset": { "type": "integer", "description": "1-based line to start at." },
                        "limit": { "type": "integer", "description": "Maximum number of lines (default 400)." }
                    },
                    "required": ["file_path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": TOOL_WRITE,
                "description": "Create or overwrite a file with the given content. Parent folders are created as needed.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "file_path": { "type": "string" },
                        "content": { "type": "string" }
                    },
                    "required": ["file_path", "content"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": TOOL_EDIT,
                "description": "Replace one exact, unique occurrence of old_string with new_string inside a file.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "file_path": { "type": "string" },
                        "old_string": { "type": "string" },
                        "new_string": { "type": "string" }
                    },
                    "required": ["file_path", "old_string", "new_string"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": TOOL_WEB_FETCH,
                "description": "Open a URL in the in-app browser pane and return the page title, text and interactive DOM refs.",
                "parameters": {
                    "type": "object",
                    "properties": { "url": { "type": "string" } },
                    "required": ["url"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": TOOL_MCP_CALL,
                "description": "Call a tool on a connected MCP server. Use server 'computer-use' for screenshots/clicks/typing and 'browser-use' for browser tabs and DOM inspection.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "server": { "type": "string", "description": "Server name or UUID, e.g. computer-use." },
                        "tool": { "type": "string", "description": "Tool name on that server, e.g. screenshot." },
                        "arguments": { "type": "object", "description": "Tool arguments object." }
                    },
                    "required": ["server", "tool"]
                }
            }
        },
        {
            "type":"function",
            "function":{
                "name":"Artifact",
                "description":"Create a private local HTML artifact and open its preview. Use self-contained HTML with inline CSS; no external resources. Does not publish, upload, or modify project files. Publishing is unavailable.",
                "parameters":{"type":"object","properties":{
                    "title":{"type":"string"},"html":{"type":"string"}
                },"required":["title","html"],"additionalProperties":false}
            }
        }
    ])
}

fn resolve_path(raw: &str, cwd: &str) -> PathBuf {
    let p = Path::new(raw);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        Path::new(cwd).join(p)
    }
}

/// Commands that are never run unless the session is in an auto/bypass permission mode.
fn looks_destructive(command: &str) -> bool {
    let c = command.to_lowercase();
    let patterns: [&str; 19] = [
        "rm\u{20}-rf",
        "sudo ",
        "mkfs",
        "dd if=",
        "diskutil erase",
        "shutdown",
        "reboot",
        "chmod -r 777 /",
        "> /dev/",
        "git push --force",
        "git reset --hard",
        "drop\u{20}table",
        "truncate ",
        // closing or discarding the user's windows/apps is destructive in practice:
        "saving no",
        "close window",
        "close every window",
        "keystroke \"w\"",
        "quit application",
        "tell application \"mail\" to quit",
    ];
    patterns.iter().any(|p| c.contains(p)) || (c.contains("curl") && c.contains("| sh"))
}

/// Validate required arguments before any side effect, including truncated model output.
pub fn validate_input(name: &str, input: &Value) -> Result<(), String> {
    let schemas = tool_schemas();
    let schema = schemas.as_array().unwrap().iter()
        .find(|s| s["function"]["name"] == name)
        .ok_or_else(|| format!("Unknown tool `{name}`."))?;
    let object = input.as_object().ok_or("Tool arguments must be an object.")?;
    let params = &schema["function"]["parameters"];
    for key in params["required"].as_array().unwrap() {
        let key = key.as_str().unwrap();
        if !object.contains_key(key) {
            return Err(format!("Missing required argument `{key}`."));
        }
    }
    for (key, value) in object {
        let valid = match params["properties"][key]["type"].as_str() {
            Some("string") => value.is_string(),
            Some("object") => value.is_object(),
            Some("integer") => value.as_u64().is_some(),
            _ => true,
        };
        if !valid { return Err(format!("Invalid argument `{key}`.")); }
    }
    Ok(())
}

/// Cache results so repeated calls (including calls in the same batch) cannot act twice.
#[derive(Default)]
pub struct ToolResults(std::collections::HashMap<String, (String, bool)>);
impl ToolResults {
    fn cacheable(name: &str) -> bool { !matches!(name, TOOL_READ | TOOL_WEB_FETCH) }
    pub fn contains(&self, name: &str, input: &Value) -> bool { Self::cacheable(name) && self.0.contains_key(&format!("{name}|{input}")) }
    pub fn run(&mut self, name: &str, input: &Value, action: impl FnOnce() -> (String, bool)) -> (String, bool) {
        // Reads are observations, not effects: an intervening mutation may change them.
        if !Self::cacheable(name) { return action(); }
        let key = format!("{name}|{input}");
        self.0.entry(key).or_insert_with(action).clone()
    }
}

/// Executes one tool call. Returns `(text_result, is_error)`.
pub fn execute(
    name: &str,
    input: &Value,
    cwd: &str,
    permission_mode: &str,
    mcp_host: &McpHost,
) -> (String, bool) {
    if let Err(error) = validate_input(name, input) {
        return (error, true);
    }
    let auto_ok = matches!(
        permission_mode,
        "auto" | "bypassPermissions"
    );

    if !auto_ok && !(permission_mode == "acceptEdits" && matches!(name, TOOL_WRITE | TOOL_EDIT)) && matches!(name, TOOL_BASH | TOOL_WRITE | TOOL_EDIT | TOOL_WEB_FETCH) {
        return (format!("Approval required for {name}. Switch this session to Auto to allow actions."), true);
    }
    match name {
        TOOL_BASH => {
            let command = input.get("command").and_then(|v| v.as_str()).unwrap_or("");
            if command.trim().is_empty() {
                return ("No command provided.".to_string(), true);
            }
            if looks_destructive(command) && !auto_ok {
                return (
                    format!(
                        "Blocked by QuixiDesk permission mode '{permission_mode}': this command looks destructive. \
                         Switch the session pill from Manual to Auto if that is really intended."
                    ),
                    true,
                );
            }
            match quixi_workspace::ShellHostEngine::run_bash_command(Path::new(cwd), command) {
                Ok(val) => {
                    let stdout = val.get("stdout").and_then(|v| v.as_str()).unwrap_or("");
                    let stderr = val.get("stderr").and_then(|v| v.as_str()).unwrap_or("");
                    let code = val.get("exitCode").and_then(|v| v.as_i64()).unwrap_or(-1);
                    let mut out = String::new();
                    if !stdout.trim().is_empty() {
                        out.push_str(stdout.trim_end());
                    }
                    if !stderr.trim().is_empty() {
                        out.push_str("\n[stderr]\n");
                        out.push_str(stderr.trim_end());
                    }
                    out.push_str(&format!("\n[exit code {code}]"));
                    (out.chars().take(12_000).collect(), code != 0)
                }
                Err(err) => (format!("Failed to run command: {err}"), true),
            }
        }
        TOOL_READ => {
            let raw = input
                .get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let target = resolve_path(raw, cwd);
            let offset = input
                .get("offset")
                .and_then(|v| v.as_u64())
                .unwrap_or(1)
                .max(1) as usize;
            let limit = input
                .get("limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(400)
                .max(1) as usize;
            match std::fs::read_to_string(&target) {
                Ok(raw_text) => {
                    let body: String = raw_text
                        .lines()
                        .skip(offset - 1)
                        .take(limit)
                        .collect::<Vec<_>>()
                        .join("\n");
                    (
                        format!("{}\n\n[{} lines total]", body, raw_text.lines().count()),
                        false,
                    )
                }
                Err(err) => (format!("Cannot read {}: {err}", target.display()), true),
            }
        }
        TOOL_WRITE => {
            let raw = input
                .get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let content = input.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let target = resolve_path(raw, cwd);
            if let Some(parent) = target.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match std::fs::write(&target, content) {
                Ok(()) => (
                    format!("Wrote {} bytes to {}", content.len(), target.display()),
                    false,
                ),
                Err(err) => (format!("Cannot write {}: {err}", target.display()), true),
            }
        }
        TOOL_EDIT => {
            let raw = input
                .get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let old = input
                .get("old_string")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let new = input
                .get("new_string")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let target = resolve_path(raw, cwd);
            match std::fs::read_to_string(&target) {
                Ok(body) => {
                    let hits = body.matches(old).count();
                    if hits != 1 {
                        return (
                            format!(
                                "old_string occurs {hits} times in {} - it must occur exactly once.",
                                target.display()
                            ),
                            true,
                        );
                    }
                    let updated = body.replacen(old, new, 1);
                    match std::fs::write(&target, &updated) {
                        Ok(()) => (format!("Edited {}", target.display()), false),
                        Err(err) => (format!("Cannot write {}: {err}", target.display()), true),
                    }
                }
                Err(err) => (format!("Cannot read {}: {err}", target.display()), true),
            }
        }
        TOOL_WEB_FETCH => {
            let url = input.get("url").and_then(|v| v.as_str()).unwrap_or("");
            if url.trim().is_empty() {
                return ("No url provided.".to_string(), true);
            }
            let nav = quixi_platform::BrowserUse::navigate(url, None);
            let title = nav
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Untitled");
            let final_url = nav.get("url").and_then(|v| v.as_str()).unwrap_or(url);
            let text = quixi_platform::BrowserUse::get_page_text(None);
            let refs = quixi_platform::BrowserUse::read_page(None, Some("interactive"), 4);
            (
                format!(
                    "URL: {final_url}\nTitle: {title}\n\n{}\n\nInteractive refs:\n{}",
                    text.chars().take(6000).collect::<String>(),
                    refs.chars().take(3000).collect::<String>()
                ),
                false,
            )
        }
        TOOL_MCP_CALL => {
            let server = input.get("server").and_then(|v| v.as_str()).unwrap_or("");
            let tool = input.get("tool").and_then(|v| v.as_str()).unwrap_or("");
            let args = input.get("arguments").cloned().unwrap_or_else(|| json!({}));
            if server.is_empty() || tool.is_empty() {
                return ("Both `server` and `tool` are required.".to_string(), true);
            }
            if mcp_tool_needs_confirm(server, tool) && !auto_ok {
                return (
                    format!(
                        "Blocked: `{server}/{tool}` moves the mouse, types, or changes app state. \
                         Switch the permission pill to Auto to allow it."
                    ),
                    true,
                );
            }
            match mcp_host.call_tool(server, tool, &args, Path::new(cwd)) {
                Ok(res) => (
                    summarize_mcp_result(&res).chars().take(12_000).collect(),
                    false,
                ),
                Err(err) => (format!("MCP call {server}/{tool} failed: {err}"), true),
            }
        }
        other => (format!("Unknown tool `{other}`."), true),
    }
}

/// Tools that act on the machine (mouse, keyboard, app/window lifecycle, tab control).
/// Known observational tools stay available in every permission mode. GUI server
/// aliases and UUIDs share the same gate; unknown tools require confirmation.
fn mcp_tool_needs_confirm(server: &str, tool: &str) -> bool {
    let server = server.to_ascii_lowercase();
    let gui_names = ["computer-use", "browser-use", "claude-in-chrome",
        "Claude Browser", "Claude Preview", "Framebuffer", "Window Halo"];
    let server_is_gui = gui_names.iter().any(|name| {
        server == name.to_ascii_lowercase()
            || McpHost::internal_server_uuid(name)
                .is_some_and(|uuid| server == uuid.to_ascii_lowercase())
    });
    if !server_is_gui {
        return !matches!(tool.to_ascii_lowercase().as_str(),
            "read_file" | "list_files" | "list_directory" | "git_status" | "git_diff" | "git_log" | "recall_memory");
    }
    // Only known observational tools are exempt. Unknown actions require consent.
    !matches!(tool.to_ascii_lowercase().as_str(),
        "screenshot" | "zoom" | "get_window_layout" | "get_page_text"
        | "read_page" | "list_connected_browsers" | "keywords")

}

/// Screenshots come back as base64 PNGs. They are useless to the model and would blow up
/// the context (and cost), so they are replaced by a short marker.
fn summarize_mcp_result(value: &Value) -> String {
    fn strip(v: &Value) -> Value {
        match v {
            Value::Object(map) => {
                let mut out = serde_json::Map::new();
                for (k, val) in map {
                    if matches!(k.as_str(), "data" | "base64" | "image" | "image_data") {
                        if let Some(s) = val.as_str() {
                            if s.len() > 512 {
                                out.insert(
                                    k.clone(),
                                    json!(format!("[image omitted: {} base64 chars]", s.len())),
                                );
                                continue;
                            }
                        }
                    }
                    out.insert(k.clone(), strip(val));
                }
                Value::Object(out)
            }
            Value::Array(items) => Value::Array(items.iter().map(strip).collect()),
            other => other.clone(),
        }
    }
    strip(value).to_string()
}

/// Re-exported so tests can exercise the guard without duplicating patterns.
pub fn is_destructive_for_test(command: &str) -> bool {
    looks_destructive(command)
}

#[cfg(test)]
mod gate_tests {
    use super::*;

    #[test]
    fn read_only_mcp_tools_never_need_confirmation() {
        assert!(!mcp_tool_needs_confirm("computer-use", "screenshot"));
        assert!(!mcp_tool_needs_confirm("computer-use", "get_window_layout"));
        assert!(!mcp_tool_needs_confirm(
            "browser-use",
            "list_connected_browsers"
        ));
        assert!(!mcp_tool_needs_confirm("browser-use", "get_page_text"));
    }

    #[test]
    fn gui_action_tools_need_confirmation() {
        assert!(mcp_tool_needs_confirm("computer-use", "click"));
        assert!(mcp_tool_needs_confirm("computer-use", "type"));
        assert!(mcp_tool_needs_confirm("computer-use", "key"));
        assert!(mcp_tool_needs_confirm("browser-use", "navigate"));
        assert!(mcp_tool_needs_confirm("browser-use", "click"));
    }

    #[test]
    fn mutations_on_other_servers_also_require_confirmation() {
        // Manual mode also protects mutations through non-GUI servers.
        assert!(mcp_tool_needs_confirm("filesystem", "write_file"));
        assert!(mcp_tool_needs_confirm("git", "commit"));
        assert!(!mcp_tool_needs_confirm("filesystem", "read_file"));
    }

    #[test]
    fn destructive_shell_stays_blocked_without_auto_mode() {
        assert!(looks_destructive("rm -rf build"));
        assert!(looks_destructive("sudo systemsetup -setremotelogin on"));
        assert!(looks_destructive("git push --force origin main"));
        assert!(looks_destructive(
            "osascript -e 'tell application \"Mail\" to close window 1'"
        ));
        assert!(!looks_destructive(
            "osascript -e 'tell application \"Mail\" to activate'"
        ));
        assert!(!looks_destructive("git status"));
    }
}

#[cfg(test)]
mod regression_tests {
    use super::*;
    #[test]
    fn gui_actions_and_aliases_require_permission() {
        for server in ["computer-use", "COMPUTER-USE", "b0a3b6e5-7ca0-462a-8e6f-bac087408b17", "Claude Browser", "Framebuffer"] {
            for tool in ["left_click", "triple_click", "middle_click", "left_click_drag", "new_action"] {
                assert!(mcp_tool_needs_confirm(server, tool), "{server}/{tool}");
            }
            assert!(!mcp_tool_needs_confirm(server, "screenshot"));
        }
    }
    #[test]
    fn repeated_calls_reuse_result_without_running_again() {
        let mut results = ToolResults::default();
        let input = json!({"command": "create-draft"});
        assert_eq!(results.run("Bash", &input, || ("created".into(), false)), ("created".into(), false));
        assert_eq!(results.run("Bash", &input, || panic!("must not run twice")), ("created".into(), false));
        assert_eq!(results.run("Bash", &json!({"command":"different"}), || ("second".into(), false)).0, "second");
    }
    #[test]
    fn reads_observe_changes_while_mutations_stay_deduplicated() {
        let mut results = ToolResults::default();
        let root = std::env::temp_dir().join(format!("quixi-read-cache-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("chain.txt");
        let host = McpHost::new();
        let cwd = root.to_str().unwrap();
        let read = json!({"file_path":file});
        let write = json!({"file_path":file,"content":"ALPHA"});
        results.run("Write", &write, || execute("Write", &write, cwd, "auto", &host));
        assert!(results.run("Read", &read, || execute("Read", &read, cwd, "auto", &host)).0.contains("ALPHA"));
        let edit = json!({"file_path":file,"old_string":"ALPHA","new_string":"BETA"});
        assert!(!results.run("Edit", &edit, || execute("Edit", &edit, cwd, "auto", &host)).1);
        assert!(!results.contains("Read", &read));
        assert!(results.run("Read", &read, || execute("Read", &read, cwd, "auto", &host)).0.contains("BETA"));
        results.run("Write", &write, || panic!("do not repeat mutation"));
        assert_eq!(std::fs::read_to_string(file).unwrap(), "BETA");
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn invalid_arguments_are_rejected_before_execution() {
        assert!(validate_input("Write", &json!({"file_path":"a"})).is_err());
        assert!(validate_input("Write", &json!({"file_path":"a", "content":null})).is_err());
        assert!(validate_input("Bash", &json!([])).is_err());
        assert!(validate_input("Write", &json!({"file_path":"a", "content":""})).is_ok());
    }
}

#[cfg(test)]
mod permission_regression_tests {
    use super::*;
    #[test]
    fn manual_blocks_shell_and_file_changes() {
        let host = McpHost::new();
        for (name, input) in [(TOOL_BASH,json!({"command":"printf SHOULD_NOT_RUN"})),(TOOL_WRITE,json!({"file_path":"/tmp/quixi-must-not-write","content":"x"})),(TOOL_EDIT,json!({"file_path":"a","old_string":"x","new_string":"y"}))] {
            let (message, error) = execute(name,&input,"/tmp","default",&host);
            assert!(error); assert!(message.contains("Approval required"));
        }
    }
}
