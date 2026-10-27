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

/// Executes one tool call. Returns `(text_result, is_error)`.
pub fn execute(
    name: &str,
    input: &Value,
    cwd: &str,
    permission_mode: &str,
    mcp_host: &McpHost,
) -> (String, bool) {
    let auto_ok = matches!(
        permission_mode,
        "auto" | "bypassPermissions" | "acceptEdits"
    );

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
/// Read-only tools — `screenshot`, `get_*`, `list_*`, `read_*`, `keywords` — stay available
/// in every permission mode, so the agent can always look before it asks to touch anything.
fn mcp_tool_needs_confirm(server: &str, tool: &str) -> bool {
    let server_is_gui = matches!(
        server.to_ascii_lowercase().as_str(),
        "computer-use" | "browser-use"
    );
    if !server_is_gui {
        return false;
    }
    let t = tool.to_ascii_lowercase();
    if t.starts_with("get_")
        || t.starts_with("list_")
        || t.starts_with("read_")
        || t == "keywords"
        || t == "screenshot"
    {
        return false;
    }
    matches!(
        t.as_str(),
        "click"
            | "double_click"
            | "right_click"
            | "move"
            | "drag"
            | "scroll"
            | "type"
            | "type_text"
            | "key"
            | "press_key"
            | "hotkey"
            | "navigate"
            | "open_url"
            | "launch"
            | "launch_app"
            | "focus"
            | "activate"
            | "close"
            | "quit"
    )
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
    fn other_servers_are_not_gated_here() {
        // filesystem/git/fetch servers are ordinary tool calls, not GUI actions.
        assert!(!mcp_tool_needs_confirm("filesystem", "write_file"));
        assert!(!mcp_tool_needs_confirm("git", "commit"));
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
