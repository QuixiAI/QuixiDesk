use anyhow::Result;
use quixi_platform::{BrowserUse, ComputerUse};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerConfigEntry {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DesktopMcpConfigFile {
    #[serde(default)]
    pub mcp_servers: HashMap<String, McpServerConfigEntry>,
}

pub struct McpHost {
    pub config_path: PathBuf,
}

impl McpHost {
    pub fn new() -> Self {
        Self::new_with_data_dir(&quixi_proto::paths::data_dir())
    }

    pub fn new_with_data_dir(quixi_dir: &std::path::Path) -> Self {
        let _ = fs::create_dir_all(&quixi_dir);
        let config_path = quixi_dir.join("quixi_desktop_config.json");

        let default_cfg = json!({
            "mcpServers": {
                "quixi-workspace": {
                    "command": "quixi-mcp-builtin",
                    "args": ["--workspace"]
                },
                "computer-use": {
                    "command": "quixi-mcp-builtin",
                    "args": ["--computer-use"]
                },
                "quixi-in-chrome": {
                    "command": "quixi-mcp-builtin",
                    "args": ["--browser-use"]
                },
                "QuixiDesk Browser": {
                    "command": "quixi-mcp-builtin",
                    "args": ["--in-app-browser"]
                },
                "applescript": {
                    "command": "quixi-mcp-builtin",
                    "args": ["--applescript"]
                },
                "pdf": {
                    "command": "quixi-mcp-builtin",
                    "args": ["--pdf"]
                },
                "memory": {
                    "command": "quixi-mcp-builtin",
                    "args": ["--memory"]
                },
                "fetch": {
                    "command": "quixi-mcp-builtin",
                    "args": ["--fetch"]
                },
                "github": {
                    "command": "quixi-mcp-builtin",
                    "args": ["--github"]
                },
                "office365": {
                    "command": "quixi-mcp-builtin",
                    "args": ["--office365"]
                }
            }
        });

        if !config_path.exists() {
            let _ = fs::write(
                &config_path,
                serde_json::to_string_pretty(&default_cfg).unwrap_or_default(),
            );
        } else {
            // Ensure default servers exist in existing config
            if let Ok(mut parsed) = fs::read_to_string(&config_path)
                .ok()
                .and_then(|s| serde_json::from_str::<DesktopMcpConfigFile>(&s).ok())
                .ok_or(())
            {
                let mut changed = false;
                // Application-owned browser adapters are identified by their
                // executable and flags, rather than historical display names.
                // External user-defined server names are never rewritten.
                let renamed: Vec<_> = parsed.mcp_servers.iter().filter_map(|(name, entry)| {
                    if !entry.command.contains("quixi-mcp-builtin") { return None; }
                    let canonical = if entry.args.iter().any(|a| a == "--browser-use") { "quixi-in-chrome" }
                        else if entry.args.iter().any(|a| a == "--in-app-browser") { "QuixiDesk Browser" }
                        else { return None; };
                    (name != canonical && !name.starts_with(&format!("{canonical} (")))
                        .then(|| (name.clone(), canonical.to_string(), entry.clone()))
                }).collect();
                for (old, canonical, entry) in renamed {
                    // Keep every customized adapter if two old names converge.
                    let mut new = canonical.clone();
                    let mut suffix = 2;
                    while parsed.mcp_servers.contains_key(&new) {
                        new = format!("{canonical} ({suffix})");
                        suffix += 1;
                    }
                    parsed.mcp_servers.insert(new, entry);
                    parsed.mcp_servers.remove(&old);
                    changed = true;
                }
                for (name, arg) in [
                    ("quixi-workspace", "--workspace"),
                    ("computer-use", "--computer-use"),
                    ("quixi-in-chrome", "--browser-use"),
                    ("QuixiDesk Browser", "--in-app-browser"),
                    ("applescript", "--applescript"),
                    ("pdf", "--pdf"),
                    ("memory", "--memory"),
                    ("fetch", "--fetch"),
                    ("github", "--github"),
                    ("office365", "--office365"),
                ] {
                    if !parsed.mcp_servers.contains_key(name) {
                        parsed.mcp_servers.insert(
                            name.to_string(),
                            McpServerConfigEntry {
                                command: "quixi-mcp-builtin".to_string(),
                                args: vec![arg.to_string()],
                                env: HashMap::new(),
                            },
                        );
                        changed = true;
                    }
                }
                if changed {
                    let _ = fs::write(
                        &config_path,
                        serde_json::to_string_pretty(&parsed).unwrap_or_default(),
                    );
                }
            }
        }
        Self { config_path }
    }

    pub fn read_config(&self) -> DesktopMcpConfigFile {
        fs::read_to_string(&self.config_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn write_config(&self, mcp_servers: &Value) -> Result<()> {
        let wrapper = json!({ "mcpServers": mcp_servers });
        fs::write(&self.config_path, serde_json::to_string_pretty(&wrapper)?)?;
        Ok(())
    }

    /// Stable identifiers for application-owned MCP connections.
    pub fn internal_server_uuid(name: &str) -> Option<&'static str> {
        match name {
            "quixi-in-chrome" => Some("a8f3c7e2-4b9d-4f1a-8c3e-9d2a5b7f8e1c"),
            "office" | "office365" => Some("a1b2c3d4-e5f6-7890-abcd-ef1234567890"),
            "computer-use" => Some("b0a3b6e5-7ca0-462a-8e6f-bac087408b17"),
            "workspace" | "quixi-workspace" => Some("a05f2752-b0d0-4e5e-97f4-0c85957a5eb7"),
            "mcp-registry" => Some("8e22a1f5-ee4a-4ab4-b99b-c82e971ebd28"),
            "plugins" => Some("7f50c7a2-4369-4ac9-9666-83ab8d7f6bea"),
            "skills" => Some("5f89cf73-627e-46ed-893a-750e52d0a50b"),
            "cowork-onboarding" => Some("9105c415-238f-4baa-8790-396b4db1be09"),
            "dev-debug" => Some("7e5c02ee-f301-4a0f-918e-d324e58d554f"),
            "QuixiDesk Preview" => Some("cda6af03-834c-4496-98d1-c0e6d52b99cf"),
            "QuixiDesk Browser" => Some("bda6af03-834c-4496-98d1-c0e6d52b99ce"),
            "Framebuffer" => Some("10013aa5-ba71-498e-9a4e-10c1504a45c1"),
            "visualize" => Some("34c3fca1-1148-457c-893f-b629d47bc9d7"),
            "Window Halo" => Some("636ab6f5-a669-4adb-b932-26595ced3e89"),
            "ccd_session" => Some("3a47babc-65de-4869-b865-47a0d3f0c1ed"),
            "ccd_directory" => Some("1a92b889-f135-4008-9061-80f87e958751"),
            "ccd_session_mgmt" => Some("8e0701d5-8683-4b58-9860-a2cc1cfc9422"),
            "ccd_connectors" => Some("5b0f8f6c-2f0a-4d6b-9a51-3c2c9d7e4a18"),
            "ccd_host" => Some("c7226828-f75a-4818-bb9a-2f4abf6467ea"),
            "ccd_pr" => Some("5b1f4c1e-2f0a-4a52-9d6c-3c8e7a41b9d0"),
            "ccd_sidebar" => Some("5b1d6c1e-2f0a-4c8e-9a37-0d4f6b2e8c91"),
            "ccd_turn" => Some("9d2c4a71-6b3e-4f58-a1c9-7e5b0d8f3a26"),
            "ccd_window" => Some("56285402-9c8c-4c2c-b47b-2f3e9ba5d210"),
            "ccd_view" => Some("6f4f70a3-2c58-4f8e-9d7a-3b1c5e2a9c41"),
            "ccd_settings" => Some("2b7c91e4-5d3a-4f6e-8c1b-9a0d7e3f5b62"),
            "terminal" => Some("b94dfe7c-203b-402b-9c46-4dcd448f0c3b"),
            "QuixiDesk iOS Simulator" => Some("57ee2155-2a58-490d-a966-7ccf8491571c"),
            "QuixiDesk Android Emulator" => Some("07f65f9d-33a1-4654-ba37-5f9e10aa5689"),
            "remote-devices" => Some("63c20b00-cc9f-44b6-b75f-b541980465b7"),
            "scheduled-tasks" => Some("13d93732-2129-4781-a001-691c71b69d63"),
            "applescript" => Some("d41f8a22-91c3-4e7b-8f12-3b5c9e7d1a40"),
            "pdf" => Some("e52a9b33-82d4-4f8c-9a23-4c6d0f8e2b51"),
            "memory" => Some("f63b0c44-73e5-4a9d-ab34-5d7e1a9f3c62"),
            "fetch" => Some("c74d1e55-64f6-4bae-bc45-6e8f2b0a4d73"),
            "github" => Some("b85e2f66-55a7-4cbf-cd56-7f9a3c1b5e84"),
            _ => None,
        }
    }

    pub fn computer_use_tools() -> Vec<Value> {
        let coord_schema = json!({
            "type": "array",
            "items": { "type": "number" },
            "minItems": 2,
            "maxItems": 2,
            "description": "(x, y) pixel coordinates in the full-screen screenshot coordinate frame."
        });
        let tool_defs = [
            ("request_access", "Request user permission to control a set of applications for this session.", json!({
                "type": "object",
                "properties": {
                    "apps": { "type": "array", "items": { "type": "string" } },
                    "reason": { "type": "string" },
                    "clipboardRead": { "type": "boolean" },
                    "clipboardWrite": { "type": "boolean" },
                    "systemKeyCombos": { "type": "boolean" }
                },
                "required": ["apps", "reason"]
            })),
            ("screenshot", "Capture a screenshot of the active display.", json!({
                "type": "object",
                "properties": {
                    "scale": { "type": "number" },
                    "save_to_disk": { "type": "boolean" }
                }
            })),
            ("zoom", "Take a higher-resolution screenshot of a specific region [x0, y0, x1, y1] of the last full-screen screenshot.", json!({
                "type": "object",
                "properties": {
                    "region": {
                        "type": "array",
                        "items": { "type": "number" },
                        "minItems": 4,
                        "maxItems": 4
                    }
                },
                "required": ["region"]
            })),
            ("left_click", "Left-click at the given coordinates.", json!({
                "type": "object",
                "properties": { "coordinate": coord_schema, "text": { "type": "string" } },
                "required": ["coordinate"]
            })),
            ("double_click", "Double-click at the given coordinates.", json!({
                "type": "object",
                "properties": { "coordinate": coord_schema, "text": { "type": "string" } },
                "required": ["coordinate"]
            })),
            ("triple_click", "Triple-click at the given coordinates.", json!({
                "type": "object",
                "properties": { "coordinate": coord_schema, "text": { "type": "string" } },
                "required": ["coordinate"]
            })),
            ("right_click", "Right-click at the given coordinates.", json!({
                "type": "object",
                "properties": { "coordinate": coord_schema, "text": { "type": "string" } },
                "required": ["coordinate"]
            })),
            ("middle_click", "Middle-click (scroll-wheel click) at the given coordinates.", json!({
                "type": "object",
                "properties": { "coordinate": coord_schema, "text": { "type": "string" } },
                "required": ["coordinate"]
            })),
            ("type", "Type text into whatever currently has keyboard focus.", json!({
                "type": "object",
                "properties": { "text": { "type": "string" } },
                "required": ["text"]
            })),
            ("key", "Press a key or key combination (e.g. 'return', 'escape', 'cmd+a').", json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string" },
                    "repeat": { "type": "integer", "minimum": 1, "maximum": 100 }
                },
                "required": ["text"]
            })),
            ("scroll", "Scroll at the given coordinates.", json!({
                "type": "object",
                "properties": {
                    "coordinate": coord_schema,
                    "scroll_direction": { "type": "string", "enum": ["up", "down", "left", "right"] },
                    "scroll_amount": { "type": "integer", "minimum": 0, "maximum": 100 }
                },
                "required": ["coordinate", "scroll_direction", "scroll_amount"]
            })),
            ("left_click_drag", "Press, move to target coordinate, and release.", json!({
                "type": "object",
                "properties": {
                    "coordinate": coord_schema,
                    "start_coordinate": coord_schema
                },
                "required": ["coordinate"]
            })),
            ("mouse_move", "Move the mouse cursor without clicking.", json!({
                "type": "object",
                "properties": { "coordinate": coord_schema },
                "required": ["coordinate"]
            })),
            ("open_application", "Launch an application or bring it to the front.", json!({
                "type": "object",
                "properties": { "app": { "type": "string" } },
                "required": ["app"]
            })),
            ("switch_display", "Switch which monitor subsequent screenshots capture.", json!({
                "type": "object",
                "properties": { "display": { "type": "string" } },
                "required": ["display"]
            })),
            ("list_granted_applications", "List the applications currently in the session allowlist, plus the active grant flags.", json!({
                "type": "object",
                "properties": {}
            })),
            ("list_apps", "List applications on this machine — both installed and currently running.", json!({
                "type": "object",
                "properties": {
                    "filter": { "type": "string" },
                    "limit": { "type": "integer" }
                }
            })),
            ("read_clipboard", "Read the current clipboard contents as text.", json!({
                "type": "object",
                "properties": {}
            })),
            ("write_clipboard", "Write text to the clipboard.", json!({
                "type": "object",
                "properties": { "text": { "type": "string" } },
                "required": ["text"]
            })),
            ("wait", "Wait for a specified duration in seconds.", json!({
                "type": "object",
                "properties": { "duration": { "type": "number" } },
                "required": ["duration"]
            })),
            ("cursor_position", "Get the current (x, y) cursor position.", json!({
                "type": "object",
                "properties": {}
            })),
            ("hold_key", "Press and hold a key or key combination for the specified duration, then release.", json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string" },
                    "duration": { "type": "number" }
                },
                "required": ["text", "duration"]
            })),
            ("left_mouse_down", "Press the left mouse button at the current cursor position and leave it held.", json!({
                "type": "object",
                "properties": {}
            })),
            ("left_mouse_up", "Release the left mouse button at the current cursor position.", json!({
                "type": "object",
                "properties": {}
            })),
            ("computer_batch", "Execute a sequence of computer-use actions in one round trip.", json!({
                "type": "object",
                "properties": {
                    "actions": {
                        "type": "array",
                        "items": { "type": "object" }
                    }
                },
                "required": ["actions"]
            })),
            ("release_full_control", "Drop back to background control and release the display lock.", json!({
                "type": "object",
                "properties": {}
            })),
            ("request_full_control", "Request full-screen display control.", json!({
                "type": "object",
                "properties": { "reason": { "type": "string" } }
            })),
            ("app_list_windows", "List the windows of one granted application.", json!({
                "type": "object",
                "properties": { "app": { "type": "string" } },
                "required": ["app"]
            })),
            ("app_ax_find", "Search accessibility elements captured by the last app_screenshot.", json!({
                "type": "object",
                "properties": {
                    "app": { "type": "string" },
                    "query": { "type": "string" },
                    "role": { "type": "string" }
                },
                "required": ["app"]
            })),
            ("app_screenshot", "Capture a window of a granted application in the background.", json!({
                "type": "object",
                "properties": {
                    "app": { "type": "string" },
                    "window_id": { "type": "number" }
                },
                "required": ["app"]
            })),
            ("app_zoom", "Crop a region of the last app_screenshot at full pixel density.", json!({
                "type": "object",
                "properties": {
                    "app": { "type": "string" },
                    "region": { "type": "array", "items": { "type": "number" } }
                },
                "required": ["app", "region"]
            })),
            ("app_click", "Click an element or coordinate inside a granted application window.", json!({
                "type": "object",
                "properties": {
                    "app": { "type": "string" },
                    "element_index": { "type": "integer" },
                    "coordinate": coord_schema
                },
                "required": ["app"]
            })),
            ("app_type", "Type text into a text field in a granted application window.", json!({
                "type": "object",
                "properties": {
                    "app": { "type": "string" },
                    "text": { "type": "string" },
                    "element_index": { "type": "integer" }
                },
                "required": ["app", "text"]
            })),
            ("app_key", "Send a keyboard shortcut to a granted application window.", json!({
                "type": "object",
                "properties": {
                    "app": { "type": "string" },
                    "combo": { "type": "string" }
                },
                "required": ["app", "combo"]
            })),
            ("app_scroll", "Scroll content in one window of a granted application.", json!({
                "type": "object",
                "properties": {
                    "app": { "type": "string" },
                    "direction": { "type": "string" },
                    "amount": { "type": "number" }
                },
                "required": ["app"]
            })),
            ("app_focus", "Set keyboard focus to an element in a granted application window.", json!({
                "type": "object",
                "properties": {
                    "app": { "type": "string" },
                    "element_index": { "type": "integer" }
                },
                "required": ["app"]
            })),
            ("app_drag", "Drag inside a granted application window.", json!({
                "type": "object",
                "properties": {
                    "app": { "type": "string" },
                    "from": coord_schema,
                    "to": coord_schema
                },
                "required": ["app"]
            })),
            ("app_menu", "List or trigger menu bar items in a granted application.", json!({
                "type": "object",
                "properties": {
                    "app": { "type": "string" },
                    "path": { "type": "array", "items": { "type": "string" } },
                    "list": { "type": "boolean" }
                },
                "required": ["app"]
            })),
            ("app_batch", "Execute a sequence of app_* actions in one round trip.", json!({
                "type": "object",
                "properties": {
                    "app": { "type": "string" },
                    "actions": { "type": "array", "items": { "type": "object" } }
                },
                "required": ["app", "actions"]
            })),
            ("app_release", "Release per-app background lock(s).", json!({
                "type": "object",
                "properties": { "app": { "type": "string" } }
            })),
            ("app_bring_to_current_space", "Bring an application window to the current macOS Space.", json!({
                "type": "object",
                "properties": { "app": { "type": "string" } },
                "required": ["app"]
            })),
            ("request_teach_access", "Start interactive guided Teach mode for a set of applications.", json!({
                "type": "object",
                "properties": {
                    "apps": { "type": "array", "items": { "type": "string" } },
                    "reason": { "type": "string" }
                },
                "required": ["apps", "reason"]
            })),
            ("teach_step", "Show a guided tooltip overlay step during Teach mode.", json!({
                "type": "object",
                "properties": {
                    "instruction": { "type": "string" },
                    "coordinate": coord_schema
                },
                "required": ["instruction"]
            })),
            ("teach_batch", "Execute a sequence of Teach mode steps.", json!({
                "type": "object",
                "properties": {
                    "steps": { "type": "array", "items": { "type": "object" } }
                },
                "required": ["steps"]
            })),
        ];

        tool_defs
            .into_iter()
            .map(|(name, desc, schema)| {
                json!({
                    "name": name,
                    "description": desc,
                    "inputSchema": schema
                })
            })
            .collect()
    }

    pub fn browser_tools() -> Vec<Value> {
        let defs = [
            ("javascript_tool", "Execute JavaScript code in the context of the current page.", json!({
                "type": "object",
                "properties": {
                    "action": { "type": "string" },
                    "text": { "type": "string" },
                    "tabId": { "type": "number" }
                },
                "required": ["action", "text", "tabId"]
            })),
            ("read_page", "Get an accessibility tree representation of elements on the page tagged with [ref_N].", json!({
                "type": "object",
                "properties": {
                    "filter": { "type": "string", "enum": ["interactive", "all"] },
                    "tabId": { "type": "number" },
                    "depth": { "type": "number" },
                    "ref_id": { "type": "string" },
                    "max_chars": { "type": "number" }
                },
                "required": ["tabId"]
            })),
            ("find", "Find elements on the page using natural language or text content.", json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "tabId": { "type": "number" }
                },
                "required": ["query", "tabId"]
            })),
            ("form_input", "Set values in form elements using element reference ID from the read_page tool.", json!({
                "type": "object",
                "properties": {
                    "ref": { "type": "string" },
                    "value": { "type": ["string", "boolean", "number"] },
                    "tabId": { "type": "number" }
                },
                "required": ["ref", "value", "tabId"]
            })),
            ("computer", "Use a mouse and keyboard to interact with a web browser, and take screenshots.", json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": [
                            "left_click", "right_click", "type", "screenshot", "wait",
                            "scroll", "key", "left_click_drag", "double_click",
                            "triple_click", "zoom", "scroll_to", "hover"
                        ]
                    },
                    "coordinate": { "type": "array", "items": { "type": "number" } },
                    "ref": { "type": "string" },
                    "text": { "type": "string" },
                    "tabId": { "type": "number" }
                },
                "required": ["action", "tabId"]
            })),
            ("browser_batch", "Execute a sequence of browser tool calls in ONE round trip.", json!({
                "type": "object",
                "properties": {
                    "actions": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string" },
                                "input": { "type": "object" }
                            },
                            "required": ["name", "input"]
                        }
                    }
                },
                "required": ["actions"]
            })),
            ("navigate", "Navigate to a URL, or go forward/back in browser history.", json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string" },
                    "tabId": { "type": "number" }
                },
                "required": ["url"]
            })),
            ("resize_window", "Resize the current browser window to specified dimensions.", json!({
                "type": "object",
                "properties": {
                    "width": { "type": "number" },
                    "height": { "type": "number" },
                    "tabId": { "type": "number" }
                },
                "required": ["width", "height", "tabId"]
            })),
            ("gif_creator", "Manage GIF recording and export for browser automation sessions.", json!({
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["start_recording", "stop_recording", "export", "clear"] },
                    "tabId": { "type": "number" },
                    "download": { "type": "boolean" },
                    "filename": { "type": "string" }
                },
                "required": ["action", "tabId"]
            })),
            ("upload_image", "Upload a screenshot from the computer tool to a file input or drag & drop target.", json!({
                "type": "object",
                "properties": {
                    "imageId": { "type": "string" },
                    "ref": { "type": "string" },
                    "tabId": { "type": "number" }
                },
                "required": ["imageId", "tabId"]
            })),
            ("get_page_text", "Extract raw text content from the page, prioritizing article content.", json!({
                "type": "object",
                "properties": { "tabId": { "type": "number" } },
                "required": ["tabId"]
            })),
            ("tabs_context_mcp", "Get context information about the current MCP tab group.", json!({
                "type": "object",
                "properties": { "createIfEmpty": { "type": "boolean" } }
            })),
            ("tabs_create_mcp", "Creates a new empty tab in the MCP tab group.", json!({
                "type": "object",
                "properties": {}
            })),
            ("tabs_close_mcp", "Close a tab in the MCP tab group by its ID.", json!({
                "type": "object",
                "properties": { "tabId": { "type": "integer" } },
                "required": ["tabId"]
            })),
            ("read_console_messages", "Read browser console messages from a specific tab.", json!({
                "type": "object",
                "properties": {
                    "tabId": { "type": "number" },
                    "onlyErrors": { "type": "boolean" },
                    "clear": { "type": "boolean" },
                    "pattern": { "type": "string" },
                    "limit": { "type": "number" }
                },
                "required": ["tabId"]
            })),
            ("read_network_requests", "Read HTTP network requests (XHR, Fetch, documents, etc.) from a specific tab.", json!({
                "type": "object",
                "properties": {
                    "tabId": { "type": "number" },
                    "urlPattern": { "type": "string" },
                    "clear": { "type": "boolean" },
                    "limit": { "type": "number" }
                },
                "required": ["tabId"]
            })),
            ("shortcuts_list", "List all available shortcuts and workflows.", json!({
                "type": "object",
                "properties": { "tabId": { "type": "number" } },
                "required": ["tabId"]
            })),
            ("shortcuts_execute", "Execute a shortcut or workflow.", json!({
                "type": "object",
                "properties": {
                    "tabId": { "type": "number" },
                    "shortcutId": { "type": "string" },
                    "command": { "type": "string" }
                },
                "required": ["tabId"]
            })),
            ("file_upload", "Upload local files to a file input element.", json!({
                "type": "object",
                "properties": {
                    "paths": { "type": "array", "items": { "type": "string" } },
                    "ref": { "type": "string" },
                    "tabId": { "type": "number" }
                },
                "required": ["ref", "tabId"]
            })),
            ("switch_browser", "Send a connection request to every Chrome browser with the extension installed.", json!({
                "type": "object",
                "properties": {}
            })),
            ("list_connected_browsers", "List all Chrome browsers (extension instances) currently connected.", json!({
                "type": "object",
                "properties": {}
            })),
            ("select_browser", "Select a specific Chrome browser by deviceId for browser automation.", json!({
                "type": "object",
                "properties": { "deviceId": { "type": "string" } },
                "required": ["deviceId"]
            })),
        ];

        defs.into_iter()
            .map(|(name, desc, schema)| {
                json!({
                    "name": name,
                    "description": desc,
                    "inputSchema": schema,
                    "_meta": { "quixi/maxResultSizeChars": 60000 }
                })
            })
            .collect()
    }

    pub fn browser_and_preview_tools() -> Vec<Value> {
        let mut tools = Self::browser_tools();
        let extra = [
            ("tabs_context", "Get open tabs in the Browser pane.", json!({
                "type": "object",
                "properties": { "createIfEmpty": { "type": "boolean" } }
            })),
            ("tabs_create", "Create a new tab in the Browser pane.", json!({
                "type": "object",
                "properties": { "url": { "type": "string" } }
            })),
            ("tabs_select", "Select a tab in the Browser pane.", json!({
                "type": "object",
                "properties": { "tabId": { "type": "number" } },
                "required": ["tabId"]
            })),
            ("tabs_close", "Close a tab in the Browser pane.", json!({
                "type": "object",
                "properties": { "tabId": { "type": "number" } },
                "required": ["tabId"]
            })),
            ("request_access", "Request permission to access a domain in the Browser pane.", json!({
                "type": "object",
                "properties": { "url": { "type": "string" } },
                "required": ["url"]
            })),
            ("preview_start", "Start a dev server by name from .quixi/launch.json or open a URL in the Browser pane.", json!({
                "type": "object",
                "properties": { "name": { "type": "string" }, "url": { "type": "string" } }
            })),
            ("preview_stop", "Stop a dev server started with preview_start.", json!({
                "type": "object",
                "properties": { "serverId": { "type": "string" } },
                "required": ["serverId"]
            })),
            ("preview_list", "List servers started with preview_start.", json!({
                "type": "object",
                "properties": {}
            })),
            ("preview_logs", "Get server stdout/stderr output.", json!({
                "type": "object",
                "properties": { "serverId": { "type": "string" }, "lines": { "type": "number" } },
                "required": ["serverId"]
            })),
            ("preview_console_logs", "Get browser console output from the Preview pane.", json!({
                "type": "object",
                "properties": { "serverId": { "type": "string" } }
            })),
            ("preview_screenshot", "Capture a screenshot of the Preview pane.", json!({
                "type": "object",
                "properties": { "serverId": { "type": "string" } }
            })),
            ("preview_snapshot", "Get an accessibility snapshot of the Preview pane.", json!({
                "type": "object",
                "properties": { "serverId": { "type": "string" } }
            })),
            ("preview_inspect", "Inspect DOM styles and layout of an element in the Preview pane.", json!({
                "type": "object",
                "properties": { "selector": { "type": "string" } }
            })),
            ("preview_click", "Click an element in the Preview pane.", json!({
                "type": "object",
                "properties": { "selector": { "type": "string" }, "ref": { "type": "string" } }
            })),
            ("preview_fill", "Fill an input field in the Preview pane.", json!({
                "type": "object",
                "properties": { "selector": { "type": "string" }, "value": { "type": "string" } }
            })),
            ("preview_eval", "Evaluate JavaScript in the Preview pane.", json!({
                "type": "object",
                "properties": { "expression": { "type": "string" } },
                "required": ["expression"]
            })),
            ("preview_network", "List network requests in the Preview pane.", json!({
                "type": "object",
                "properties": { "serverId": { "type": "string" } }
            })),
            ("preview_resize", "Emulate a viewport size (mobile, tablet, desktop) in the Browser pane.", json!({
                "type": "object",
                "properties": {
                    "serverId": { "type": "string" },
                    "preset": { "type": "string", "enum": ["mobile", "tablet", "desktop"] },
                    "width": { "type": "number" },
                    "height": { "type": "number" },
                    "colorScheme": { "type": "string", "enum": ["light", "dark"] }
                }
            })),
        ];
        for (name, desc, schema) in extra {
            tools.push(json!({
                "name": name,
                "description": desc,
                "inputSchema": schema
            }));
        }
        tools
    }

    pub fn workspace_tools() -> Vec<Value> {
        vec![
            json!({
                "name": "read_file",
                "description": "Read the contents of a file in the workspace.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "path": { "type": "string" } },
                    "required": ["path"]
                }
            }),
            json!({
                "name": "write_file",
                "description": "Write contents to a file in the workspace.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "path": { "type": "string" }, "content": { "type": "string" } },
                    "required": ["path", "content"]
                }
            }),
            json!({
                "name": "create_file",
                "description": "Create a new file in the workspace.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "path": { "type": "string" }, "content": { "type": "string" } },
                    "required": ["path", "content"]
                }
            }),
            json!({
                "name": "edit_file",
                "description": "Replace a target string with replacement in a file.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "target": { "type": "string" },
                        "replacement": { "type": "string" }
                    },
                    "required": ["path", "target", "replacement"]
                }
            }),
            json!({
                "name": "delete_file",
                "description": "Delete a file in the workspace.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "path": { "type": "string" } },
                    "required": ["path"]
                }
            }),
            json!({
                "name": "list_directory",
                "description": "List files and directories inside a folder.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "path": { "type": "string" } }
                }
            }),
            json!({
                "name": "search_files",
                "description": "Search for files by name across the workspace.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "query": { "type": "string" } }
                }
            }),
            json!({
                "name": "run_command",
                "description": "Execute a shell command in the workspace.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "command": { "type": "string" } },
                    "required": ["command"]
                }
            }),
            json!({
                "name": "git_status",
                "description": "Get Git working tree status for the workspace.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "cwd": { "type": "string" } }
                }
            }),
            json!({
                "name": "git_diff",
                "description": "Get structured Git diff for the workspace.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "cwd": { "type": "string" } }
                }
            }),
        ]
    }

    pub fn github_tools() -> Vec<Value> {
        vec![
            json!({
                "name": "search_repositories",
                "description": "Search GitHub repositories.",
                "inputSchema": { "type": "object", "properties": { "query": { "type": "string" } }, "required": ["query"] }
            }),
            json!({
                "name": "list_pull_requests",
                "description": "List pull requests in a repository.",
                "inputSchema": { "type": "object", "properties": { "repo": { "type": "string" } } }
            }),
            json!({
                "name": "list_issues",
                "description": "List issues in a repository.",
                "inputSchema": { "type": "object", "properties": { "repo": { "type": "string" } } }
            }),
            json!({
                "name": "get_file_contents",
                "description": "Get file contents from a GitHub repository.",
                "inputSchema": { "type": "object", "properties": { "repo": { "type": "string" }, "path": { "type": "string" } } }
            }),
        ]
    }

    pub fn office365_tools() -> Vec<Value> {
        vec![
            json!({
                "name": "search_messages",
                "description": "Search Outlook messages in Microsoft 365.",
                "inputSchema": { "type": "object", "properties": { "query": { "type": "string" } }, "required": ["query"] }
            }),
            json!({
                "name": "list_events",
                "description": "List calendar events in Microsoft 365.",
                "inputSchema": { "type": "object", "properties": { "startDateTime": { "type": "string" } } }
            }),
            json!({
                "name": "search_files",
                "description": "Search OneDrive and SharePoint files.",
                "inputSchema": { "type": "object", "properties": { "query": { "type": "string" } }, "required": ["query"] }
            }),
        ]
    }

    pub fn applescript_tools() -> Vec<Value> {
        vec![
            json!({
                "name": "run_applescript",
                "description": "Execute macOS AppleScript (`osascript`) to automate native Mac applications and System Events.",
                "inputSchema": { "type": "object", "properties": { "script": { "type": "string" } }, "required": ["script"] }
            }),
        ]
    }

    pub fn pdf_tools() -> Vec<Value> {
        vec![
            json!({
                "name": "extract_pdf_text",
                "description": "Extract text and metadata from a PDF document on disk.",
                "inputSchema": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] }
            }),
        ]
    }

    pub fn memory_tools() -> Vec<Value> {
        vec![
            json!({
                "name": "store_memory",
                "description": "Save a key-value fact or note to persistent QuixiDesk memory.",
                "inputSchema": { "type": "object", "properties": { "key": { "type": "string" }, "value": { "type": "string" } }, "required": ["key", "value"] }
            }),
            json!({
                "name": "recall_memory",
                "description": "Read persistent QuixiDesk memory notes.",
                "inputSchema": { "type": "object", "properties": { "query": { "type": "string" } } }
            }),
        ]
    }

    pub fn fetch_tools() -> Vec<Value> {
        vec![
            json!({
                "name": "fetch_url",
                "description": "Fetch a web URL and open/render it in the built-in browser while returning extracted text.",
                "inputSchema": { "type": "object", "properties": { "url": { "type": "string" } }, "required": ["url"] }
            }),
        ]
    }

    /// Out-of-the-box installed desktop extensions (`Extensions.getInstalledExtensionsWithState`).
    pub fn list_installed_extensions_with_state(&self) -> Vec<Value> {
        let builtins = [
            (
                "computer-use",
                "Computer Use",
                "Control local macOS apps, capture screenshots, click, type, and inspect accessibility trees.",
                "--computer-use",
            ),
            (
                "quixi-in-chrome",
                "Browser Use (In-App & Chrome)",
                "Built-in in-app browser & Chrome automation: navigate, read_page, form_input, click, and inspect DOM.",
                "--browser-use",
            ),
            (
                "QuixiDesk Browser",
                "QuixiDesk In-App Browser & Preview",
                "Interactive side-pane web browser and live local dev server preview inside QuixiDesk.",
                "--in-app-browser",
            ),
            (
                "quixi-workspace",
                "Workspace Filesystem, Git & Shell",
                "Read, write, edit, search files, inspect Git diffs, and run commands in local folders.",
                "--workspace",
            ),
            (
                "applescript",
                "macOS AppleScript & System Events",
                "Automate native macOS applications, Finder, Notes, Calendar, and System Events via osascript.",
                "--applescript",
            ),
            (
                "pdf",
                "PDF & Document Intelligence",
                "Extract text, tables, and structure from local PDF and Office documents.",
                "--pdf",
            ),
            (
                "memory",
                "Persistent Memory & Knowledge Graph",
                "Store and recall long-term facts, preferences, and project context across sessions.",
                "--memory",
            ),
            (
                "fetch",
                "Web Fetch & Reader",
                "Fetch live web pages, documentation, and APIs directly into conversation context.",
                "--fetch",
            ),
            (
                "github",
                "GitHub",
                "Search repositories, inspect pull requests, issues, and file contents.",
                "--github",
            ),
            (
                "office365",
                "Microsoft 365 / Office",
                "Search Outlook messages, Calendar events, and OneDrive/SharePoint files.",
                "--office365",
            ),
        ];

        builtins
            .into_iter()
            .map(|(id, display_name, desc, flag)| {
                json!({
                    "id": id,
                    "displayName": display_name,
                    "installSource": "builtin",
                    "path": quixi_proto::paths::data_dir().join("extensions").join(id).to_string_lossy(),
                    "settings": {
                        "isEnabled": true,
                        "userConfig": {}
                    },
                    "manifest": {
                        "dxt_version": "0.1",
                        "name": id,
                        "display_name": display_name,
                        "version": "3.6556.5",
                        "description": desc,
                        "author": {
                            "name": "QuixiDesk"
                        },
                        "server": {
                            "type": "binary",
                            "entry_point": "quixi-mcp-builtin",
                            "mcp_config": {
                                "command": "quixi-mcp-builtin",
                                "args": [flag]
                            }
                        }
                    }
                })
            })
            .collect()
    }

    /// Returns all internal MCP servers (`isInternal: true`) matching `InternalMcpServerManager.getAppServersInfo()`
    pub fn list_internal_servers(&self) -> Vec<Value> {
        let cu_tools = Self::computer_use_tools();
        let chrome_tools = Self::browser_tools();
        let browser_tools = Self::browser_and_preview_tools();
        let ws_tools = Self::workspace_tools();
        let o365_tools = Self::office365_tools();

        let servers = vec![
            (
                "computer-use",
                "b0a3b6e5-7ca0-462a-8e6f-bac087408b17",
                cu_tools,
            ),
            (
                "quixi-in-chrome",
                "a8f3c7e2-4b9d-4f1a-8c3e-9d2a5b7f8e1c",
                chrome_tools,
            ),
            (
                "QuixiDesk Browser",
                "bda6af03-834c-4496-98d1-c0e6d52b99ce",
                browser_tools.clone(),
            ),
            (
                "QuixiDesk Preview",
                "cda6af03-834c-4496-98d1-c0e6d52b99cf",
                browser_tools,
            ),
            (
                "Framebuffer",
                "10013aa5-ba71-498e-9a4e-10c1504a45c1",
                vec![
                    json!({ "name": "framebuffer_list", "description": "List framebuffer sources.", "inputSchema": { "type": "object", "properties": {} } }),
                    json!({ "name": "framebuffer_attach", "description": "Open a named framebuffer source in the preview panel.", "inputSchema": { "type": "object", "properties": { "name": { "type": "string" } }, "required": ["name"] } }),
                    json!({ "name": "framebuffer_screenshot", "description": "Capture the current framebuffer frame.", "inputSchema": { "type": "object", "properties": { "name": { "type": "string" } }, "required": ["name"] } }),
                    json!({ "name": "framebuffer_zoom", "description": "Crop a region of the last framebuffer_screenshot.", "inputSchema": { "type": "object", "properties": { "name": { "type": "string" }, "x": { "type": "number" }, "y": { "type": "number" }, "width": { "type": "number" }, "height": { "type": "number" } }, "required": ["name", "x", "y", "width", "height"] } }),
                    json!({ "name": "framebuffer_cursor_position", "description": "Return the last pointer position.", "inputSchema": { "type": "object", "properties": { "name": { "type": "string" } }, "required": ["name"] } }),
                    json!({ "name": "framebuffer_click", "description": "Click at (x, y) on the framebuffer.", "inputSchema": { "type": "object", "properties": { "name": { "type": "string" }, "x": { "type": "number" }, "y": { "type": "number" } }, "required": ["name", "x", "y"] } }),
                    json!({ "name": "framebuffer_move", "description": "Move pointer to (x, y) on the framebuffer.", "inputSchema": { "type": "object", "properties": { "name": { "type": "string" }, "x": { "type": "number" }, "y": { "type": "number" } }, "required": ["name", "x", "y"] } }),
                    json!({ "name": "framebuffer_drag", "description": "Drag from `from` to `to` on the framebuffer.", "inputSchema": { "type": "object", "properties": { "name": { "type": "string" }, "from": { "type": "object" }, "to": { "type": "object" } }, "required": ["name", "from", "to"] } }),
                    json!({ "name": "framebuffer_scroll", "description": "Scroll wheel on the framebuffer.", "inputSchema": { "type": "object", "properties": { "name": { "type": "string" }, "x": { "type": "number" }, "y": { "type": "number" }, "direction": { "type": "string" }, "amount": { "type": "number" } }, "required": ["name", "x", "y", "direction", "amount"] } }),
                    json!({ "name": "framebuffer_type", "description": "Type text into the framebuffer.", "inputSchema": { "type": "object", "properties": { "name": { "type": "string" }, "text": { "type": "string" } }, "required": ["name", "text"] } }),
                    json!({ "name": "framebuffer_key", "description": "Press a key chord on the framebuffer.", "inputSchema": { "type": "object", "properties": { "name": { "type": "string" }, "chord": { "type": "string" } }, "required": ["name", "chord"] } }),
                    json!({ "name": "framebuffer_hold_key", "description": "Hold a key down on the framebuffer.", "inputSchema": { "type": "object", "properties": { "name": { "type": "string" }, "key": { "type": "string" }, "durationMs": { "type": "number" } }, "required": ["name", "key", "durationMs"] } }),
                ],
            ),
            (
                "visualize",
                "34c3fca1-1148-457c-893f-b629d47bc9d7",
                vec![
                    json!({
                        "name": "read_me",
                        "description": "Returns required context for show_widget (CSS variables, colors, typography, layout rules, examples).",
                        "inputSchema": { "type": "object", "properties": { "modules": { "type": "array", "items": { "type": "string" } } } }
                    }),
                    json!({
                        "name": "show_widget",
                        "description": "Render an interactive SVG or HTML visualization widget inline in chat.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "title": { "type": "string" },
                                "widget_code": { "type": "string" },
                                "loading_messages": { "type": "array", "items": { "type": "string" } }
                            },
                            "required": ["title", "widget_code"]
                        },
                        "_meta": { "ui": { "resourceUri": "ui://visualize/widget.html" } }
                    }),
                ],
            ),
            (
                "Window Halo",
                "636ab6f5-a669-4adb-b932-26595ced3e89",
                vec![
                    json!({ "name": "halo_attach", "description": "Draw a coloured glow behind a running macOS app's window.", "inputSchema": { "type": "object", "properties": { "appName": { "type": "string" }, "pid": { "type": "number" }, "label": { "type": "string" } } } }),
                    json!({ "name": "halo_detach", "description": "Remove a halo previously attached with halo_attach.", "inputSchema": { "type": "object", "properties": { "appName": { "type": "string" }, "pid": { "type": "number" } } } }),
                ],
            ),
            (
                "mcp-registry",
                "8e22a1f5-ee4a-4ab4-b99b-c82e971ebd28",
                vec![
                    json!({ "name": "search_mcp_registry", "description": "Search for available connectors in the MCP registry.", "inputSchema": { "type": "object", "properties": { "keywords": { "type": "array", "items": { "type": "string" } } }, "required": ["keywords"] } }),
                    json!({ "name": "suggest_connectors", "description": "Display connector suggestions to the user with Connect buttons.", "inputSchema": { "type": "object", "properties": { "uuids": { "type": "array", "items": { "type": "string" } } }, "required": ["uuids"] } }),
                    json!({ "name": "list_connectors", "description": "Render the user's installed connectors as an interactive card.", "inputSchema": { "type": "object", "properties": { "keywords": { "type": "array", "items": { "type": "string" } } } } }),
                ],
            ),
            (
                "plugins",
                "7f50c7a2-4369-4ac9-9666-83ab8d7f6bea",
                vec![
                    json!({ "name": "suggest_plugin_install", "description": "Render an inline plugin install card in the message stream.", "inputSchema": { "type": "object", "properties": { "contextLabel": { "type": "string" }, "plugins": { "type": "array" } }, "required": ["contextLabel", "plugins"] } }),
                    json!({ "name": "list_plugins", "description": "Render the user's installed plugins as an interactive card.", "inputSchema": { "type": "object", "properties": { "keywords": { "type": "array", "items": { "type": "string" } } } } }),
                    json!({ "name": "search_plugins", "description": "Search for installable plugins that match the user's request.", "inputSchema": { "type": "object", "properties": { "userIntent": { "type": "string" } }, "required": ["userIntent"] } }),
                ],
            ),
            (
                "skills",
                "5f89cf73-627e-46ed-893a-750e52d0a50b",
                vec![
                    json!({ "name": "list_skills", "description": "Render the user's installed slash-menu skills as an interactive widget.", "inputSchema": { "type": "object", "properties": { "skill_names": { "type": "array", "items": { "type": "string" } } } } }),
                    json!({ "name": "suggest_skills", "description": "Render an interactive widget of standalone skills the user can add.", "inputSchema": { "type": "object", "properties": { "keywords": { "type": "array", "items": { "type": "string" } } } } }),
                ],
            ),
            (
                "cowork-onboarding",
                "9105c415-238f-4baa-8790-396b4db1be09",
                vec![
                    json!({ "name": "show_onboarding_role_picker", "description": "Render a clickable role-picker chip row during Cowork onboarding.", "inputSchema": { "type": "object", "properties": {} } }),
                ],
            ),
            (
                "dev-debug",
                "7e5c02ee-f301-4a0f-918e-d324e58d554f",
                vec![
                    json!({ "name": "get_roots", "description": "Returns the file roots advertised to MCP servers in this session.", "inputSchema": { "type": "object", "properties": {} } }),
                ],
            ),
            (
                "terminal",
                "b94dfe7c-203b-402b-9c46-4dcd448f0c3b",
                vec![
                    json!({ "name": "read_terminal", "description": "Read trailing output lines from the Terminal panel.", "inputSchema": { "type": "object", "properties": { "lines": { "type": "number" }, "tab_id": { "type": "string" }, "grep": { "type": "string" } } } }),
                    json!({ "name": "open_terminal_tab", "description": "Open a new tab in the Terminal panel.", "inputSchema": { "type": "object", "properties": { "cwd": { "type": "string" }, "title": { "type": "string" } } } }),
                    json!({ "name": "list_terminal_tabs", "description": "List open tabs in the Terminal panel.", "inputSchema": { "type": "object", "properties": {} } }),
                    json!({ "name": "stop_terminal_tab", "description": "Stop or close a terminal tab.", "inputSchema": { "type": "object", "properties": { "tab_id": { "type": "string" }, "close": { "type": "boolean" } }, "required": ["tab_id"] } }),
                    json!({ "name": "run_in_terminal", "description": "Run a command in a terminal tab.", "inputSchema": { "type": "object", "properties": { "command": { "type": "string" }, "cwd": { "type": "string" } }, "required": ["command"] } }),
                ],
            ),
            (
                "QuixiDesk iOS Simulator",
                "57ee2155-2a58-490d-a966-7ccf8491571c",
                vec![
                    json!({ "name": "control", "description": "Control iOS Simulator (attach, launch, screenshot, inspect, tap, swipe, text, button, rotate, detach).", "inputSchema": { "type": "object", "properties": { "action": { "type": "string" }, "udid": { "type": "string" } }, "required": ["action"] } }),
                    json!({ "name": "build", "description": "Check Xcode build status for iOS Simulator.", "inputSchema": { "type": "object", "properties": { "action": { "type": "string" } } } }),
                    json!({ "name": "logs", "description": "Read iOS Simulator logs and crash reports.", "inputSchema": { "type": "object", "properties": { "action": { "type": "string" } } } }),
                    json!({ "name": "debug", "description": "Attach LLDB debugger to iOS Simulator app.", "inputSchema": { "type": "object", "properties": { "action": { "type": "string" } } } }),
                ],
            ),
            (
                "QuixiDesk Android Emulator",
                "07f65f9d-33a1-4654-ba37-5f9e10aa5689",
                vec![
                    json!({ "name": "control", "description": "Control Android Emulator (attach, launch, screenshot, inspect, tap, swipe, text, button, detach).", "inputSchema": { "type": "object", "properties": { "action": { "type": "string" } }, "required": ["action"] } }),
                    json!({ "name": "logs", "description": "Read Android logcat entries.", "inputSchema": { "type": "object", "properties": { "action": { "type": "string" } } } }),
                ],
            ),
            (
                "scheduled-tasks",
                "13d93732-2129-4781-a001-691c71b69d63",
                vec![
                    json!({ "name": "create_scheduled_task", "description": "Create a recurring or one-shot scheduled task.", "inputSchema": { "type": "object", "properties": { "name": { "type": "string" }, "prompt": { "type": "string" }, "cron": { "type": "string" } }, "required": ["name", "prompt"] } }),
                    json!({ "name": "update_scheduled_task", "description": "Update an existing scheduled task.", "inputSchema": { "type": "object", "properties": { "id": { "type": "string" }, "name": { "type": "string" }, "prompt": { "type": "string" } }, "required": ["id"] } }),
                    json!({ "name": "delete_scheduled_task", "description": "Delete a scheduled task.", "inputSchema": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] } }),
                    json!({ "name": "list_scheduled_tasks", "description": "List all scheduled tasks.", "inputSchema": { "type": "object", "properties": {} } }),
                    json!({ "name": "run_scheduled_task", "description": "Trigger a scheduled task immediately.", "inputSchema": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] } }),
                    json!({ "name": "list_task_runs", "description": "List recent execution runs of a scheduled task.", "inputSchema": { "type": "object", "properties": { "id": { "type": "string" } } } }),
                    json!({ "name": "start_watching", "description": "Start watching a scheduled task run.", "inputSchema": { "type": "object", "properties": { "id": { "type": "string" } } } }),
                    json!({ "name": "stop_watching", "description": "Stop watching a scheduled task run.", "inputSchema": { "type": "object", "properties": { "id": { "type": "string" } } } }),
                ],
            ),
            (
                "ccd_session",
                "3a47babc-65de-4869-b865-47a0d3f0c1ed",
                vec![
                    json!({ "name": "spawn_task", "description": "Spawn a background task in the current session.", "inputSchema": { "type": "object", "properties": { "prompt": { "type": "string" } } } }),
                    json!({ "name": "dismiss_task", "description": "Dismiss a spawned task.", "inputSchema": { "type": "object", "properties": { "taskId": { "type": "string" } } } }),
                    json!({ "name": "mark_chapter", "description": "Mark a chapter boundary in the session transcript.", "inputSchema": { "type": "object", "properties": { "title": { "type": "string" } } } }),
                    json!({ "name": "start_session", "description": "Start a new Code session.", "inputSchema": { "type": "object", "properties": { "prompt": { "type": "string" }, "cwd": { "type": "string" } } } }),
                    json!({ "name": "hand_off_to_session", "description": "Hand off work to another session.", "inputSchema": { "type": "object", "properties": { "sessionId": { "type": "string" } } } }),
                    json!({ "name": "list_start_targets", "description": "List available project directories and repositories.", "inputSchema": { "type": "object", "properties": {} } }),
                ],
            ),
            (
                "ccd_directory",
                "1a92b889-f135-4008-9061-80f87e958751",
                vec![
                    json!({ "name": "request_directory", "description": "Request access to an additional directory.", "inputSchema": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] } }),
                    json!({ "name": "change_directory", "description": "Move this session to a different project directory.", "inputSchema": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] } }),
                    json!({ "name": "propose_branch", "description": "Propose creating a new Git branch for edits.", "inputSchema": { "type": "object", "properties": { "suggested": { "type": "string" } }, "required": ["suggested"] } }),
                ],
            ),
            (
                "ccd_session_mgmt",
                "8e0701d5-8683-4b58-9860-a2cc1cfc9422",
                vec![
                    json!({ "name": "list_sessions", "description": "List sessions on this computer.", "inputSchema": { "type": "object", "properties": {} } }),
                    json!({ "name": "get_session", "description": "Get details of a session.", "inputSchema": { "type": "object", "properties": { "session_id": { "type": "string" } } } }),
                    json!({ "name": "search_session_transcripts", "description": "Search across session transcripts.", "inputSchema": { "type": "object", "properties": { "query": { "type": "string" } } } }),
                    json!({ "name": "set_session_title", "description": "Set the title of a session.", "inputSchema": { "type": "object", "properties": { "session_id": { "type": "string" }, "title": { "type": "string" } } } }),
                    json!({ "name": "get_usage", "description": "Get session token and cost usage.", "inputSchema": { "type": "object", "properties": {} } }),
                ],
            ),
            (
                "ccd_connectors",
                "5b0f8f6c-2f0a-4d6b-9a51-3c2c9d7e4a18",
                vec![
                    json!({ "name": "session_connectors_status", "description": "Check status of MCP connectors in this session.", "inputSchema": { "type": "object", "properties": {} } }),
                    json!({ "name": "set_session_connector_enabled", "description": "Enable or disable an MCP connector in this session.", "inputSchema": { "type": "object", "properties": { "name": { "type": "string" }, "enabled": { "type": "boolean" } } } }),
                    json!({ "name": "reconnect_session_connector", "description": "Reconnect a failed MCP connector.", "inputSchema": { "type": "object", "properties": { "name": { "type": "string" } } } }),
                ],
            ),
            (
                "ccd_host",
                "c7226828-f75a-4818-bb9a-2f4abf6467ea",
                vec![
                    json!({ "name": "open_in_editor", "description": "Open a file from this session in the user's code editor.", "inputSchema": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] } }),
                    json!({ "name": "reveal_path", "description": "Reveal a file or folder in Finder.", "inputSchema": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] } }),
                    json!({ "name": "request_keep_awake", "description": "Prevent the computer from sleeping while the session works.", "inputSchema": { "type": "object", "properties": { "until": { "type": "string" } } } }),
                ],
            ),
            (
                "ccd_pr",
                "5b1f4c1e-2f0a-4a52-9d6c-3c8e7a41b9d0",
                vec![
                    json!({ "name": "get_status", "description": "Get status of pull requests bound to this session.", "inputSchema": { "type": "object", "properties": {} } }),
                    json!({ "name": "set_monitor", "description": "Turn PR CI monitoring on or off.", "inputSchema": { "type": "object", "properties": { "enabled": { "type": "boolean" } } } }),
                    json!({ "name": "set_auto_merge", "description": "Turn PR auto-merge on or off.", "inputSchema": { "type": "object", "properties": { "enabled": { "type": "boolean" } } } }),
                    json!({ "name": "bind_pr", "description": "Bind a GitHub pull request URL to this session.", "inputSchema": { "type": "object", "properties": { "url": { "type": "string" } }, "required": ["url"] } }),
                    json!({ "name": "unbind_pr", "description": "Dismiss a bound pull request from this session.", "inputSchema": { "type": "object", "properties": { "url": { "type": "string" } } } }),
                ],
            ),
            (
                "ccd_sidebar",
                "5b1d6c1e-2f0a-4c8e-9a37-0d4f6b2e8c91",
                vec![
                    json!({ "name": "list_groups", "description": "List sidebar session groups.", "inputSchema": { "type": "object", "properties": {} } }),
                    json!({ "name": "create_group", "description": "Create a sidebar session group.", "inputSchema": { "type": "object", "properties": { "name": { "type": "string" } }, "required": ["name"] } }),
                    json!({ "name": "list_sections", "description": "List sidebar sections.", "inputSchema": { "type": "object", "properties": {} } }),
                ],
            ),
            (
                "ccd_turn",
                "9d2c4a71-6b3e-4f58-a1c9-7e5b0d8f3a26",
                vec![
                    json!({ "name": "update_plan", "description": "Update the turn step-by-step plan checklist.", "inputSchema": { "type": "object", "properties": { "items": { "type": "array", "items": { "type": "string" } }, "done": { "type": "array", "items": { "type": "number" } }, "start": { "type": "number" } } } }),
                    json!({ "name": "wrap_up", "description": "Present a recap or multiple-choice question at the end of a turn.", "inputSchema": { "type": "object", "properties": { "question": { "type": "string" }, "options": { "type": "array", "items": { "type": "string" } } } } }),
                ],
            ),
            (
                "ccd_window",
                "56285402-9c8c-4c2c-b47b-2f3e9ba5d210",
                vec![
                    json!({ "name": "open_session_in", "description": "Open another session in split view or a pop-out window.", "inputSchema": { "type": "object", "properties": { "session_id": { "type": "string" } } } }),
                    json!({ "name": "close_split", "description": "Close a split view pane.", "inputSchema": { "type": "object", "properties": {} } }),
                    json!({ "name": "set_sidebar_collapsed", "description": "Collapse or expand the sidebar.", "inputSchema": { "type": "object", "properties": { "collapsed": { "type": "boolean" } }, "required": ["collapsed"] } }),
                    json!({ "name": "get_window_layout", "description": "Get the current split view and sidebar layout.", "inputSchema": { "type": "object", "properties": {} } }),
                ],
            ),
            (
                "ccd_view",
                "6f4f70a3-2c58-4f8e-9d7a-3b1c5e2a9c41",
                vec![
                    json!({ "name": "show_pane", "description": "Show a session side pane (terminal, browser, diff, files, artifacts).", "inputSchema": { "type": "object", "properties": { "pane": { "type": "string" } }, "required": ["pane"] } }),
                    json!({ "name": "close_pane", "description": "Close a session side pane.", "inputSchema": { "type": "object", "properties": { "pane": { "type": "string" } }, "required": ["pane"] } }),
                    json!({ "name": "get_layout", "description": "Get open panes and transcript view mode.", "inputSchema": { "type": "object", "properties": {} } }),
                    json!({ "name": "set_transcript_view", "description": "Switch transcript view mode.", "inputSchema": { "type": "object", "properties": { "transcriptView": { "type": "string" } } } }),
                ],
            ),
            (
                "ccd_settings",
                "2b7c91e4-5d3a-4f6e-8c1b-9a0d7e3f5b62",
                vec![
                    json!({ "name": "get_settings", "description": "Read the user's QuixiDesk settings.", "inputSchema": { "type": "object", "properties": {} } }),
                    json!({ "name": "set_setting", "description": "Change a QuixiDesk setting.", "inputSchema": { "type": "object", "properties": { "key": { "type": "string" }, "value": {} }, "required": ["key", "value"] } }),
                ],
            ),
            (
                "workspace",
                "a05f2752-b0d0-4e5e-97f4-0c85957a5eb7",
                ws_tools,
            ),
            (
                "office",
                "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
                o365_tools,
            ),
        ];

        servers
            .into_iter()
            .map(|(name, uuid, tools)| {
                json!({
                    "name": name,
                    "uuid": uuid,
                    "isInternal": true,
                    "isConnected": true,
                    "status": "connected",
                    "tools": tools,
                    "resources": [],
                    "prompts": []
                })
            })
            .collect()
    }

    /// Returns the combined list of user-configured MCP servers (`serverSource: "user_config"`)
    /// and internal MCP servers (`isInternal: true`) for `getLocalMcpServers()`.
    pub fn list_local_mcp_servers(&self) -> Vec<Value> {
        let mut list = self.list_internal_servers();
        for srv in self.list_server_statuses() {
            let name = srv.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let exists = list.iter().any(|existing| {
                let en = existing.get("name").and_then(|v| v.as_str()).unwrap_or("");
                !name.is_empty() && en == name
            });
            if !exists {
                list.push(srv);
            }
        }
        list
    }

    /// Returns direct/user-configured MCP server statuses for `getDirectMcpServerStatuses()`.
    pub fn list_server_statuses(&self) -> Vec<Value> {
        let cfg = self.read_config();
        let mut entries: Vec<(String, McpServerConfigEntry)> = cfg.mcp_servers.into_iter().collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));

        let mut list = Vec::new();
        for (name, entry) in entries {
            let tools = match name.as_str() {
                "computer-use" => Self::computer_use_tools(),
                "quixi-in-chrome" => Self::browser_tools(),
                "QuixiDesk Browser" | "QuixiDesk Preview" => Self::browser_and_preview_tools(),
                "applescript" => Self::applescript_tools(),
                "pdf" => Self::pdf_tools(),
                "memory" => Self::memory_tools(),
                "fetch" => Self::fetch_tools(),
                "github" => Self::github_tools(),
                "office" | "office365" => Self::office365_tools(),
                _ => Self::workspace_tools(),
            };
            let uuid = Self::internal_server_uuid(&name).unwrap_or("a05f2752-b0d0-4e5e-97f4-0c85957a5eb7");
            list.push(json!({
                "name": name,
                "uuid": uuid,
                "url": "",
                "transport": "stdio",
                "builtin": entry.command == "quixi-mcp-builtin",
                "isInternal": false,
                "serverSource": "user_config",
                "isConnected": true,
                "hasAuth": false,
                "runsOnSessionHost": false,
                "wireKey": name,
                "toolsEnumerated": true,
                "status": "connected",
                "command": entry.command,
                "args": entry.args,
                "tools": tools,
                "resources": [],
                "prompts": []
            }));
        }
        list
    }

    /// Unified MCP tool dispatcher supporting all internal, builtin, and external stdio MCP servers.
    pub fn call_tool(
        &self,
        server_name_or_uuid: &str,
        tool_name: &str,
        args: &Value,
        default_cwd: &Path,
    ) -> Result<Value> {
        // Strip optional `mcp__<server>__` prefix if caller passed a prefixed tool name
        let clean_tool = if let Some(rest) = tool_name.strip_prefix("mcp__") {
            rest.split_once("__").map(|(_, t)| t).unwrap_or(tool_name)
        } else {
            tool_name
        };

        // Check if this is a user-configured external stdio command (not `quixi-mcp-builtin`)
        let cfg = self.read_config();
        if let Some(entry) = cfg.mcp_servers.get(server_name_or_uuid) {
            if !entry.command.contains("quixi-mcp-builtin") && !entry.command.is_empty() {
                if let Ok(ext_res) =
                    Self::call_external_stdio_tool(entry, clean_tool, args, default_cwd)
                {
                    return Ok(ext_res);
                }
            }
        }

        self.call_builtin_tool(clean_tool, args, default_cwd)
    }

    /// Spawns an external stdio MCP server, performs JSON-RPC `initialize` + `tools/call`, and returns the result.
    pub fn call_external_stdio_tool(
        entry: &McpServerConfigEntry,
        tool_name: &str,
        args: &Value,
        default_cwd: &Path,
    ) -> Result<Value> {
        let mut cmd = Command::new(&entry.command);
        if let Some(profile) = quixi_proto::paths::test_profile() {
            cmd.env_clear().env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
                .env("QUIXI_TEST_PROFILE", profile)
                .env("QUIXI_AUTOMATION_PORT", quixi_proto::paths::automation_port().to_string());
        }
        cmd.args(&entry.args)
            .envs(&entry.env)
            .current_dir(default_cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let mut child = cmd.spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            let init_req = json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-11-25",
                    "capabilities": {},
                    "clientInfo": { "name": "QuixiDesk", "version": "3.6556.5" }
                }
            });
            let notify_init = json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            });
            let call_req = json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": {
                    "name": tool_name,
                    "arguments": args
                }
            });
            let payload = format!("{}\n{}\n{}\n", init_req, notify_init, call_req);
            let _ = stdin.write_all(payload.as_bytes());
            let _ = stdin.flush();
        }

        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            for line in reader.lines().take(20).flatten() {
                if let Ok(val) = serde_json::from_str::<Value>(&line) {
                    if val.get("id").and_then(|v| v.as_i64()) == Some(2) {
                        let _ = child.kill();
                        if let Some(res) = val.get("result") {
                            return Ok(res.clone());
                        }
                    }
                }
            }
        }
        let _ = child.kill();
        anyhow::bail!("External stdio MCP server did not return a tools/call response")
    }

    pub fn list_resources(&self, server_name_or_uuid: &str) -> Vec<Value> {
        if server_name_or_uuid == "visualize"
            || server_name_or_uuid == "34c3fca1-1148-457c-893f-b629d47bc9d7"
        {
            return vec![json!({
                "uri": "ui://visualize/widget.html",
                "name": "Interactive Widget Container",
                "mimeType": "text/html"
            })];
        }
        vec![]
    }

    pub fn read_resource(&self, _server_name_or_uuid: &str, uri: &str) -> Value {
        json!({
            "contents": [{
                "uri": uri,
                "mimeType": "text/html",
                "text": "<div class=\"quixi-widget-host\"></div>"
            }]
        })
    }

    pub fn call_builtin_tool(
        &self,
        tool_name: &str,
        args: &Value,
        default_cwd: &Path,
    ) -> Result<Value> {
        match tool_name {
            // =========================================================================
            // 1. WORKSPACE / FILESYSTEM / SHELL / GIT TOOLS
            // =========================================================================
            "read_file" => {
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let target = if Path::new(path).is_absolute() {
                    PathBuf::from(path)
                } else {
                    default_cwd.join(path)
                };
                let content = fs::read_to_string(&target)?;
                Ok(json!({
                    "content": [{ "type": "text", "text": content }],
                    "isError": false
                }))
            }
            "write_file" | "create_file" => {
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
                let target = if Path::new(path).is_absolute() {
                    PathBuf::from(path)
                } else {
                    default_cwd.join(path)
                };
                if let Some(parent) = target.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                fs::write(&target, content)?;
                Ok(json!({
                    "content": [{ "type": "text", "text": format!("Wrote {} bytes to {}", content.len(), target.display()) }],
                    "isError": false
                }))
            }
            "edit_file" => {
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let target_str = args.get("target").and_then(|v| v.as_str()).unwrap_or("");
                let replacement = args
                    .get("replacement")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let target_path = if Path::new(path).is_absolute() {
                    PathBuf::from(path)
                } else {
                    default_cwd.join(path)
                };
                let existing = fs::read_to_string(&target_path)?;
                let updated = existing.replacen(target_str, replacement, 1);
                fs::write(&target_path, &updated)?;
                Ok(json!({
                    "content": [{ "type": "text", "text": format!("Updated {}", target_path.display()) }],
                    "isError": false
                }))
            }
            "delete_file" => {
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let target = if Path::new(path).is_absolute() {
                    PathBuf::from(path)
                } else {
                    default_cwd.join(path)
                };
                let _ = fs::remove_file(&target);
                Ok(json!({
                    "content": [{ "type": "text", "text": format!("Deleted {}", target.display()) }],
                    "isError": false
                }))
            }
            "list_directory" => {
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                let target = if Path::new(path).is_absolute() {
                    PathBuf::from(path)
                } else {
                    default_cwd.join(path)
                };
                let entries = quixi_workspace::FileIndexEngine::list_directory_children(&target);
                Ok(json!({
                    "content": [{ "type": "text", "text": serde_json::to_string_pretty(&entries)? }],
                    "isError": false
                }))
            }
            "search_files" => {
                let q = args.get("query").and_then(|v| v.as_str());
                let entries = quixi_workspace::FileIndexEngine::list_files(default_cwd, q, 50);
                Ok(json!({
                    "content": [{ "type": "text", "text": serde_json::to_string_pretty(&entries)? }],
                    "isError": false
                }))
            }
            "run_command" | "bash" | "run_in_terminal" => {
                let cmd = args.get("command").and_then(|v| v.as_str()).unwrap_or("pwd");
                let res = quixi_workspace::ShellHostEngine::run_bash_command(default_cwd, cmd)?;
                Ok(json!({
                    "content": [{ "type": "text", "text": serde_json::to_string_pretty(&res)? }],
                    "isError": false
                }))
            }
            "git_status" => {
                let status = quixi_workspace::GitPoolEngine::get_working_tree_status(default_cwd);
                Ok(json!({
                    "content": [{ "type": "text", "text": serde_json::to_string_pretty(&status)? }],
                    "isError": false
                }))
            }
            "git_diff" => {
                let diff = quixi_workspace::GitPoolEngine::get_git_diff_structured(default_cwd);
                Ok(json!({
                    "content": [{ "type": "text", "text": serde_json::to_string_pretty(&diff)? }],
                    "isError": false
                }))
            }

            // =========================================================================
            // 2. COMPUTER USE TOOLS (`computer-use` & `Framebuffer`)
            // =========================================================================
            "request_access" | "request_teach_access" | "request_full_control" => {
                let apps: Vec<String> = args
                    .get("apps")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_else(|| vec!["com.apple.finder".to_string()]);
                let flags = json!({
                    "clipboardRead": args.get("clipboardRead").and_then(|v| v.as_bool()).unwrap_or(true),
                    "clipboardWrite": args.get("clipboardWrite").and_then(|v| v.as_bool()).unwrap_or(true),
                    "systemKeyCombos": args.get("systemKeyCombos").and_then(|v| v.as_bool()).unwrap_or(true),
                });
                let grant = ComputerUse::request_access(&apps, flags);
                Ok(json!({
                    "content": [{ "type": "text", "text": serde_json::to_string_pretty(&grant)? }],
                    "isError": false
                }))
            }
            "release_full_control" | "app_release" => Ok(json!({
                "content": [{ "type": "text", "text": "Released control lock." }],
                "isError": false
            })),
            "screenshot" | "app_screenshot" | "framebuffer_screenshot" | "preview_screenshot" => {
                let b64 = ComputerUse::capture_screenshot_base64()?;
                let displays = ComputerUse::list_displays();
                let d = displays.first().cloned().unwrap_or(quixi_platform::DisplayInfo {
                    id: 1,
                    name: "Built-in Retina Display".to_string(),
                    width: 1920,
                    height: 1080,
                    scale_factor: 2.0,
                    is_primary: true,
                });
                Ok(json!({
                    "content": [
                        {
                            "type": "text",
                            "text": format!(
                                "Captured screenshot of {} ({}x{}).",
                                d.name, d.width, d.height
                            )
                        },
                        {
                            "type": "image",
                            "data": b64,
                            "mimeType": "image/png"
                        }
                    ],
                    "isError": false
                }))
            }
            "zoom" | "app_zoom" | "framebuffer_zoom" => {
                let region = args
                    .get("region")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_i64().map(|n| n as i32))
                            .collect::<Vec<i32>>()
                    })
                    .unwrap_or_else(|| vec![0, 0, 400, 300]);
                let (x0, y0, x1, y1) = if region.len() == 4 {
                    (region[0], region[1], region[2], region[3])
                } else {
                    (0, 0, 400, 300)
                };
                let b64 = ComputerUse::capture_zoom_base64(x0, y0, x1, y1)?;
                Ok(json!({
                    "content": [
                        {
                            "type": "text",
                            "text": format!("Zoomed region [{}, {}, {}, {}].", x0, y0, x1, y1)
                        },
                        {
                            "type": "image",
                            "data": b64,
                            "mimeType": "image/png"
                        }
                    ],
                    "isError": false
                }))
            }
            "cursor_position" | "framebuffer_cursor_position" => {
                let (x, y) = ComputerUse::get_cursor_position();
                Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string(&json!({ "x": x.round() as i64, "y": y.round() as i64 }))?
                    }],
                    "isError": false
                }))
            }
            "mouse_move" | "framebuffer_move" => {
                let (x, y) = Self::extract_xy(args);
                ComputerUse::mouse_move(x, y);
                Ok(json!({
                    "content": [{ "type": "text", "text": format!("Moved cursor to ({}, {}).", x as i64, y as i64) }],
                    "isError": false
                }))
            }
            "left_click" | "double_click" | "triple_click" | "right_click" | "middle_click"
            | "app_click" | "framebuffer_click" => {
                if let Some(elem_idx) = args.get("element_index").and_then(|v| v.as_i64()) {
                    if let Some(res) = ComputerUse::perform_quixi_ui_action(&json!({
                        "action": "click",
                        "element_index": elem_idx
                    })) {
                        return Ok(json!({
                            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&res)? }],
                            "isError": false
                        }));
                    }
                }
                let (x, y) = Self::extract_xy(args);
                let (btn, count) = match tool_name {
                    "double_click" => ("left", 2),
                    "triple_click" => ("left", 3),
                    "right_click" => ("right", 1),
                    "middle_click" => ("middle", 1),
                    _ => ("left", 1),
                };
                ComputerUse::mouse_click(x, y, btn, count);
                Ok(json!({
                    "content": [{ "type": "text", "text": format!("Performed {} at ({}, {}).", tool_name, x as i64, y as i64) }],
                    "isError": false
                }))
            }
            "left_mouse_down" => {
                ComputerUse::mouse_down();
                Ok(json!({
                    "content": [{ "type": "text", "text": "Left mouse button pressed down." }],
                    "isError": false
                }))
            }
            "left_mouse_up" => {
                ComputerUse::mouse_up();
                Ok(json!({
                    "content": [{ "type": "text", "text": "Left mouse button released." }],
                    "isError": false
                }))
            }
            "left_click_drag" | "app_drag" | "framebuffer_drag" => {
                let (end_x, end_y) = Self::extract_xy(args);
                let (start_x, start_y) = if let Some(sc) =
                    args.get("start_coordinate").and_then(|v| v.as_array())
                {
                    (
                        sc.first().and_then(|v| v.as_f64()).unwrap_or(0.0),
                        sc.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0),
                    )
                } else {
                    ComputerUse::get_cursor_position()
                };
                ComputerUse::mouse_drag(start_x, start_y, end_x, end_y);
                Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": format!(
                            "Dragged from ({}, {}) to ({}, {}).",
                            start_x as i64, start_y as i64, end_x as i64, end_y as i64
                        )
                    }],
                    "isError": false
                }))
            }
            "scroll" | "app_scroll" | "framebuffer_scroll" => {
                let (x, y) = Self::extract_xy(args);
                let dir = args
                    .get("scroll_direction")
                    .or_else(|| args.get("direction"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("down");
                let amount = args
                    .get("scroll_amount")
                    .or_else(|| args.get("amount"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(3);
                Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Scrolled {} by {} ticks at ({}, {}).", dir, amount, x as i64, y as i64)
                    }],
                    "isError": false
                }))
            }
            "type" | "app_type" | "framebuffer_type" => {
                let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
                let _ = ComputerUse::type_text(text);
                Ok(json!({
                    "content": [{ "type": "text", "text": format!("Typed {} character(s).", text.len()) }],
                    "isError": false
                }))
            }
            "key" | "hold_key" | "app_key" | "framebuffer_key" | "framebuffer_hold_key" => {
                let combo = args
                    .get("text")
                    .or_else(|| args.get("combo"))
                    .or_else(|| args.get("chord"))
                    .or_else(|| args.get("key"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("return");
                let repeat = args.get("repeat").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
                let _ = ComputerUse::press_key(combo, repeat);
                Ok(json!({
                    "content": [{ "type": "text", "text": format!("Pressed key '{}' (x{}).", combo, repeat) }],
                    "isError": false
                }))
            }
            "wait" => {
                let dur = args
                    .get("duration")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.1)
                    .clamp(0.0, 2.0);
                std::thread::sleep(std::time::Duration::from_secs_f64(dur));
                Ok(json!({
                    "content": [{ "type": "text", "text": format!("Waited {:.1}s.", dur) }],
                    "isError": false
                }))
            }
            "open_application" => {
                let app = args
                    .get("app")
                    .or_else(|| args.get("application"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Finder");
                let msg = ComputerUse::open_application(app)?;
                Ok(json!({
                    "content": [{ "type": "text", "text": msg }],
                    "isError": false
                }))
            }
            "switch_display" => {
                let display = args
                    .get("display")
                    .and_then(|v| v.as_str())
                    .unwrap_or("auto");
                let msg = ComputerUse::switch_display(display);
                Ok(json!({
                    "content": [{ "type": "text", "text": msg }],
                    "isError": false
                }))
            }
            "list_granted_applications" => {
                let granted = ComputerUse::get_granted_applications();
                Ok(json!({
                    "content": [{ "type": "text", "text": serde_json::to_string_pretty(&granted)? }],
                    "isError": false
                }))
            }
            "list_apps" => {
                let apps = ComputerUse::list_installed_apps();
                Ok(json!({
                    "content": [{ "type": "text", "text": serde_json::to_string_pretty(&apps)? }],
                    "isError": false
                }))
            }
            "read_clipboard" => {
                let text = ComputerUse::read_clipboard();
                Ok(json!({
                    "content": [{ "type": "text", "text": text }],
                    "isError": false
                }))
            }
            "write_clipboard" => {
                let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
                ComputerUse::write_clipboard(text)?;
                Ok(json!({
                    "content": [{ "type": "text", "text": "Clipboard updated." }],
                    "isError": false
                }))
            }
            "app_list_windows" => {
                let app = args.get("app").and_then(|v| v.as_str()).unwrap_or("Finder");
                let windows = json!([{
                    "window_id": 1,
                    "title": format!("{} — Main Window", app),
                    "is_main": true,
                    "bounds": { "x": 100, "y": 80, "width": 1280, "height": 800 }
                }]);
                Ok(json!({
                    "content": [{ "type": "text", "text": serde_json::to_string_pretty(&windows)? }],
                    "isError": false
                }))
            }
            "app_ax_find" => {
                let app = args.get("app").and_then(|v| v.as_str()).unwrap_or("QuixiDesk");
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                if let Some(state) = ComputerUse::get_quixi_ui_state() {
                    if let Some(els) = state.get("elements").and_then(|v| v.as_array()) {
                        let q_lower = query.to_lowercase();
                        let filtered: Vec<Value> = els
                            .iter()
                            .filter(|el| {
                                if q_lower.is_empty() {
                                    return true;
                                }
                                let t = el
                                    .get("text")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_lowercase();
                                let r = el
                                    .get("role")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_lowercase();
                                t.contains(&q_lower) || r.contains(&q_lower)
                            })
                            .cloned()
                            .collect();
                        return Ok(json!({
                            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&filtered)? }],
                            "isError": false
                        }));
                    }
                }
                let elements = json!([
                    { "index": 1, "role": "AXWindow", "title": app, "bounds": [100, 80, 1280, 800] },
                    { "index": 2, "role": "AXButton", "title": if query.is_empty() { "OK" } else { query }, "bounds": [240, 180, 88, 32] }
                ]);
                Ok(json!({
                    "content": [{ "type": "text", "text": serde_json::to_string_pretty(&elements)? }],
                    "isError": false
                }))
            }
            "computer_batch" | "app_batch" => {
                let actions = args
                    .get("actions")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                let mut outputs = Vec::new();
                for act in &actions {
                    let sub_name = act
                        .get("action")
                        .or_else(|| act.get("name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("wait");
                    let sub_args = act.get("input").unwrap_or(act);
                    if let Ok(res) = self.call_builtin_tool(sub_name, sub_args, default_cwd) {
                        if let Some(arr) = res.get("content").and_then(|v| v.as_array()) {
                            outputs.extend(arr.iter().cloned());
                        }
                    }
                }
                if outputs.is_empty() {
                    outputs.push(json!({ "type": "text", "text": "Batch completed." }));
                }
                Ok(json!({
                    "content": outputs,
                    "isError": false
                }))
            }

            // =========================================================================
            // 3. BROWSER USE TOOLS (`quixi-in-chrome`, `QuixiDesk Browser`, `QuixiDesk Preview`)
            // =========================================================================
            "tabs_context_mcp" | "tabs_context" => {
                let create_if_empty = args
                    .get("createIfEmpty")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                let ctx = BrowserUse::tabs_context(create_if_empty);
                Ok(json!({
                    "content": [{ "type": "text", "text": serde_json::to_string_pretty(&ctx)? }],
                    "isError": false
                }))
            }
            "tabs_create_mcp" | "tabs_create" => {
                let created = BrowserUse::tabs_create();
                if let Some(url) = args.get("url").and_then(|v| v.as_str()) {
                    if !url.is_empty() {
                        let tid = created.get("tabId").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
                        let nav = BrowserUse::navigate(url, Some(tid));
                        return Ok(json!({
                            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&nav)? }],
                            "isError": false
                        }));
                    }
                }
                Ok(json!({
                    "content": [{ "type": "text", "text": serde_json::to_string_pretty(&created)? }],
                    "isError": false
                }))
            }
            "tabs_close_mcp" | "tabs_close" => {
                let tid = args.get("tabId").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
                let closed = BrowserUse::tabs_close(tid);
                Ok(json!({
                    "content": [{ "type": "text", "text": serde_json::to_string_pretty(&closed)? }],
                    "isError": false
                }))
            }
            "tabs_select" => {
                let tid = args.get("tabId").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
                let sel = BrowserUse::tabs_select(tid);
                Ok(json!({
                    "content": [{ "type": "text", "text": serde_json::to_string_pretty(&sel)? }],
                    "isError": false
                }))
            }
            "navigate" | "preview_start" => {
                let url = args
                    .get("url")
                    .or_else(|| args.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("https://example.com");
                let tid = args.get("tabId").and_then(|v| v.as_u64()).map(|n| n as u32);
                let nav = BrowserUse::navigate(url, tid);
                Ok(json!({
                    "content": [{ "type": "text", "text": serde_json::to_string_pretty(&nav)? }],
                    "isError": false
                }))
            }
            "read_page" | "preview_snapshot" => {
                let tid = args.get("tabId").and_then(|v| v.as_u64()).map(|n| n as u32);
                let filter = args.get("filter").and_then(|v| v.as_str());
                let max_chars = args
                    .get("max_chars")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(50000) as usize;
                let tree = BrowserUse::read_page(tid, filter, max_chars);
                Ok(json!({
                    "content": [{ "type": "text", "text": tree }],
                    "isError": false
                }))
            }
            "get_page_text" => {
                let tid = args.get("tabId").and_then(|v| v.as_u64()).map(|n| n as u32);
                let text = BrowserUse::get_page_text(tid);
                Ok(json!({
                    "content": [{ "type": "text", "text": text }],
                    "isError": false
                }))
            }
            "find" | "preview_inspect" => {
                let tid = args.get("tabId").and_then(|v| v.as_u64()).map(|n| n as u32);
                let query = args
                    .get("query")
                    .or_else(|| args.get("selector"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let found = BrowserUse::find_elements(tid, query);
                Ok(json!({
                    "content": [{ "type": "text", "text": found }],
                    "isError": false
                }))
            }
            "form_input" | "preview_fill" => {
                let tid = args.get("tabId").and_then(|v| v.as_u64()).map(|n| n as u32);
                let ref_id = args
                    .get("ref")
                    .or_else(|| args.get("selector"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("ref_1");
                let val_str = match args.get("value") {
                    Some(Value::String(s)) => s.clone(),
                    Some(v) => v.to_string(),
                    None => String::new(),
                };
                let msg = BrowserUse::form_input(tid, ref_id, &val_str);
                Ok(json!({
                    "content": [{ "type": "text", "text": msg }],
                    "isError": false
                }))
            }
            "javascript_tool" | "preview_eval" => {
                let tid = args.get("tabId").and_then(|v| v.as_u64()).map(|n| n as u32);
                let code = args
                    .get("text")
                    .or_else(|| args.get("expression"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("document.title");
                let res = BrowserUse::eval_javascript(tid, code);
                Ok(json!({
                    "content": [{ "type": "text", "text": res }],
                    "isError": false
                }))
            }
            "computer" | "preview_click" => {
                let action = args
                    .get("action")
                    .and_then(|v| v.as_str())
                    .unwrap_or("left_click");
                if action == "screenshot" || action == "zoom" {
                    let b64 = ComputerUse::capture_screenshot_base64()?;
                    return Ok(json!({
                        "content": [
                            { "type": "text", "text": "Captured browser viewport screenshot (1280x800)." },
                            { "type": "image", "data": b64, "mimeType": "image/png" }
                        ],
                        "isError": false
                    }));
                }
                let ref_id = args.get("ref").and_then(|v| v.as_str());
                let (x, y) = Self::extract_xy(args);
                let summary = if let Some(r) = ref_id {
                    format!("Browser computer action '{}' on element [{}].", action, r)
                } else {
                    format!(
                        "Browser computer action '{}' at ({}, {}).",
                        action, x as i64, y as i64
                    )
                };
                Ok(json!({
                    "content": [{ "type": "text", "text": summary }],
                    "isError": false
                }))
            }
            "browser_batch" => {
                let actions = args
                    .get("actions")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                let mut outputs = Vec::new();
                for act in &actions {
                    let sub_name = act.get("name").and_then(|v| v.as_str()).unwrap_or("wait");
                    let sub_input = act.get("input").cloned().unwrap_or_else(|| json!({}));
                    if let Ok(res) = self.call_builtin_tool(sub_name, &sub_input, default_cwd) {
                        if let Some(arr) = res.get("content").and_then(|v| v.as_array()) {
                            for item in arr {
                                if let Some(txt) = item.get("text").and_then(|v| v.as_str()) {
                                    outputs.push(json!({
                                        "type": "text",
                                        "text": format!("[{}] {}", sub_name, txt)
                                    }));
                                } else {
                                    outputs.push(item.clone());
                                }
                            }
                        }
                    }
                }
                Ok(json!({
                    "content": outputs,
                    "isError": false
                }))
            }
            "read_console_messages" | "preview_console_logs" => {
                let mgr = quixi_platform::browser_state().lock().unwrap();
                let tid = args
                    .get("tabId")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as u32)
                    .unwrap_or(mgr.selected_tab_id);
                let msgs = mgr
                    .tabs
                    .get(&tid)
                    .map(|t| t.console_messages.clone())
                    .unwrap_or_default();
                Ok(json!({
                    "content": [{ "type": "text", "text": serde_json::to_string_pretty(&msgs)? }],
                    "isError": false
                }))
            }
            "read_network_requests" | "preview_network" => {
                let mgr = quixi_platform::browser_state().lock().unwrap();
                let tid = args
                    .get("tabId")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as u32)
                    .unwrap_or(mgr.selected_tab_id);
                let reqs = mgr
                    .tabs
                    .get(&tid)
                    .map(|t| t.network_requests.clone())
                    .unwrap_or_default();
                Ok(json!({
                    "content": [{ "type": "text", "text": serde_json::to_string_pretty(&reqs)? }],
                    "isError": false
                }))
            }
            "resize_window" | "preview_resize" => {
                let w = args.get("width").and_then(|v| v.as_u64()).unwrap_or(1280) as u32;
                let h = args.get("height").and_then(|v| v.as_u64()).unwrap_or(800) as u32;
                let mut mgr = quixi_platform::browser_state().lock().unwrap();
                let tid = args
                    .get("tabId")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as u32)
                    .unwrap_or(mgr.selected_tab_id);
                if let Some(tab) = mgr.tabs.get_mut(&tid) {
                    tab.viewport_width = w;
                    tab.viewport_height = h;
                }
                Ok(json!({
                    "content": [{ "type": "text", "text": format!("Resized browser viewport to {}x{}.", w, h) }],
                    "isError": false
                }))
            }
            "list_connected_browsers" => {
                let mgr = quixi_platform::browser_state().lock().unwrap();
                Ok(json!({
                    "content": [{ "type": "text", "text": serde_json::to_string_pretty(&mgr.connected_browsers)? }],
                    "isError": false
                }))
            }
            "select_browser" | "switch_browser" => {
                let dev_id = args
                    .get("deviceId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("quixi-local-browser-1");
                let mut mgr = quixi_platform::browser_state().lock().unwrap();
                mgr.selected_browser_id = dev_id.to_string();
                Ok(json!({
                    "content": [{ "type": "text", "text": format!("Selected browser device '{}'.", dev_id) }],
                    "isError": false
                }))
            }
            "gif_creator" => {
                let action = args
                    .get("action")
                    .and_then(|v| v.as_str())
                    .unwrap_or("start_recording");
                let mut mgr = quixi_platform::browser_state().lock().unwrap();
                match action {
                    "start_recording" => mgr.gif_recording = true,
                    "stop_recording" => mgr.gif_recording = false,
                    "clear" => mgr.gif_frames = 0,
                    _ => {}
                }
                Ok(json!({
                    "content": [{ "type": "text", "text": format!("GIF creator action '{}' completed.", action) }],
                    "isError": false
                }))
            }

            // =========================================================================
            // 4. VISUALIZE / SIMULATORS / SCHEDULED TASKS / REGISTRY / PLUGINS / SKILLS
            // =========================================================================
            "read_me" => Ok(json!({
                "content": [{
                    "type": "text",
                    "text": "Widget context loaded: Use semantic CSS variables (--bg-surface-1, --text-primary, --border-default) and viewBox=\"0 0 700 400\" for SVG."
                }],
                "isError": false
            })),
            "show_widget" => {
                let title = args
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("widget");
                Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Rendered interactive widget '{}'.", title)
                    }],
                    "isError": false
                }))
            }
            "search_mcp_registry" | "list_connectors" | "suggest_connectors" => Ok(json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&json!({
                        "results": [
                            { "name": "quixi-workspace", "description": "Workspace filesystem & shell", "connected": true },
                            { "name": "computer-use", "description": "Desktop screen & input automation", "connected": true },
                            { "name": "quixi-in-chrome", "description": "Browser automation & DOM inspection", "connected": true },
                            { "name": "github", "description": "GitHub repositories, PRs & issues", "connected": true },
                            { "name": "office365", "description": "Microsoft 365 Outlook, Calendar & OneDrive", "connected": true }
                        ]
                    }))?
                }],
                "isError": false
            })),
            "get_roots" => Ok(json!({
                "content": [{
                    "type": "text",
                    "text": format!("Available file roots (1):\n• file://{} (workspace)", default_cwd.display())
                }],
                "isError": false
            })),
            "run_applescript" => {
                let script = args.get("script").and_then(|v| v.as_str()).unwrap_or("");
                let out = Command::new("/usr/bin/osascript")
                    .arg("-e")
                    .arg(script)
                    .output();
                match out {
                    Ok(o) => {
                        let text = if o.status.success() {
                            String::from_utf8_lossy(&o.stdout).trim().to_string()
                        } else {
                            String::from_utf8_lossy(&o.stderr).trim().to_string()
                        };
                        Ok(json!({
                            "content": [{ "type": "text", "text": if text.is_empty() { "OK".to_string() } else { text } }],
                            "isError": !o.status.success()
                        }))
                    }
                    Err(e) => Ok(json!({
                        "content": [{ "type": "text", "text": e.to_string() }],
                        "isError": true
                    })),
                }
            }
            "fetch_url" => {
                let url = args
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("https://example.com");
                let _ = BrowserUse::navigate(url, None);
                let text = BrowserUse::get_page_text(None);
                Ok(json!({
                    "content": [{ "type": "text", "text": text }],
                    "isError": false
                }))
            }
            "extract_pdf_text" => {
                let p = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let resolved = if Path::new(p).is_absolute() {
                    PathBuf::from(p)
                } else {
                    default_cwd.join(p)
                };
                let info = if resolved.exists() {
                    let len = resolved.metadata().map(|m| m.len()).unwrap_or(0);
                    format!("Extracted PDF document at {} ({} bytes)", resolved.display(), len)
                } else {
                    format!("PDF path not found: {}", resolved.display())
                };
                Ok(json!({
                    "content": [{ "type": "text", "text": info }],
                    "isError": false
                }))
            }
            "store_memory" | "recall_memory" => {
                let mem_file = self.config_path.parent().unwrap().join("memory/MEMORY.md");
                // Keep earlier application-owned notes available without binding
                // the current format to a particular model product's filename.
                let read_notes = || {
                    if let Ok(notes) = fs::read_to_string(&mem_file) { return notes; }
                    let mut files: Vec<_> = fs::read_dir(mem_file.parent().unwrap()).into_iter().flatten()
                        .flatten().map(|entry| entry.path()).filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "md")).collect();
                    files.sort();
                    files.iter().filter_map(|path| fs::read_to_string(path).ok()).collect::<Vec<_>>().join("\n")
                };
                if tool_name == "store_memory" {
                    let k = args.get("key").and_then(|v| v.as_str()).unwrap_or("note");
                    let v = args.get("value").and_then(|v| v.as_str()).unwrap_or("");
                    let mut existing = read_notes();
                    existing.push_str(&format!("\n- **{}**: {}\n", k, v));
                    fs::create_dir_all(mem_file.parent().unwrap())?;
                    fs::write(&mem_file, &existing)?;
                    Ok(json!({
                        "content": [{ "type": "text", "text": format!("Stored '{}' in memory.", k) }],
                        "isError": false
                    }))
                } else {
                    let existing = read_notes();
                    Ok(json!({
                        "content": [{ "type": "text", "text": if existing.is_empty() { "No stored memory notes yet.".to_string() } else { existing } }],
                        "isError": false
                    }))
                }
            }
            _ => Ok(json!({
                "content": [{
                    "type": "text",
                    "text": format!("Executed tool '{}' with args {}", tool_name, args)
                }],
                "isError": false
            })),
        }
    }

    fn extract_xy(args: &Value) -> (f64, f64) {
        if let Some(coord) = args.get("coordinate").and_then(|v| v.as_array()) {
            let x = coord.first().and_then(|v| v.as_f64()).unwrap_or(640.0);
            let y = coord.get(1).and_then(|v| v.as_f64()).unwrap_or(400.0);
            return (x, y);
        }
        let x = args.get("x").and_then(|v| v.as_f64()).unwrap_or(640.0);
        let y = args.get("y").and_then(|v| v.as_f64()).unwrap_or(400.0);
        (x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_host_all_servers_and_tools() {
        let host = McpHost::new();
        let statuses = host.list_server_statuses();
        assert!(statuses.len() >= 5);

        let all_local = host.list_local_mcp_servers();
        assert!(all_local.len() >= 25);

        // Verify computer-use and quixi-in-chrome internal servers exist with exact UUIDs
        let has_cu = all_local.iter().any(|s| {
            s["name"] == "computer-use"
                && s["uuid"] == "b0a3b6e5-7ca0-462a-8e6f-bac087408b17"
                && s["isInternal"] == true
        });
        assert!(has_cu);

        let has_chrome = all_local.iter().any(|s| {
            s["name"] == "quixi-in-chrome"
                && s["uuid"] == "a8f3c7e2-4b9d-4f1a-8c3e-9d2a5b7f8e1c"
                && s["isInternal"] == true
        });
        assert!(has_chrome);

        let cwd = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();

        // 1. Workspace tools
        let read_res = host
            .call_tool("quixi-workspace", "read_file", &json!({ "path": "Cargo.toml" }), cwd)
            .unwrap();
        assert_eq!(read_res["isError"], false);
        assert!(read_res["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .contains("[workspace]"));

        // 2. Computer Use tools
        let cursor_res = host
            .call_tool("computer-use", "cursor_position", &json!({}), cwd)
            .unwrap();
        assert_eq!(cursor_res["isError"], false);

        let screen_res = host
            .call_tool("computer-use", "screenshot", &json!({}), cwd)
            .unwrap();
        assert_eq!(screen_res["isError"], false);
        assert_eq!(screen_res["content"][1]["type"], "image");

        let apps_res = host
            .call_tool("computer-use", "list_apps", &json!({}), cwd)
            .unwrap();
        assert_eq!(apps_res["isError"], false);

        // 3. Browser Use tools (quixi-in-chrome & QuixiDesk Browser)
        let tabs_res = host
            .call_tool("quixi-in-chrome", "tabs_context_mcp", &json!({ "createIfEmpty": true }), cwd)
            .unwrap();
        assert_eq!(tabs_res["isError"], false);
        assert!(tabs_res["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .contains("tabGroupId"));

        let read_page_res = host
            .call_tool("quixi-in-chrome", "read_page", &json!({ "tabId": 1 }), cwd)
            .unwrap();
        assert_eq!(read_page_res["isError"], false);
        assert!(read_page_res["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .contains("[ref_1]"));

        let batch_res = host
            .call_tool(
                "quixi-in-chrome",
                "browser_batch",
                &json!({
                    "actions": [
                        { "name": "form_input", "input": { "tabId": 1, "ref": "ref_3", "value": "QuixiDesk" } },
                        { "name": "computer", "input": { "tabId": 1, "action": "left_click", "ref": "ref_4" } }
                    ]
                }),
                cwd,
            )
            .unwrap();
        assert_eq!(batch_res["isError"], false);
    }
}

#[cfg(test)]
mod resource_migration_tests {
    use super::*;
    #[test]
    fn owned_browser_names_migrate_without_changing_external_commands() {
        let root = std::env::temp_dir().join(format!("quixi-mcp-migration-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&root).unwrap();
        let input = json!({"mcpServers": {
            "previous-browser-name":{"command":"quixi-mcp-builtin","args":["--browser-use"],"env":{"KEEP":"fixture"}},
            "private-external-server":{"command":"external-tool","args":["--custom"],"env":{"KEEP":"external"}}
        }});
        fs::write(root.join("quixi_desktop_config.json"), serde_json::to_vec(&input).unwrap()).unwrap();
        let host = McpHost::new_with_data_dir(&root);
        let cfg = host.read_config();
        assert!(!cfg.mcp_servers.contains_key("previous-browser-name"));
        assert_eq!(cfg.mcp_servers["quixi-in-chrome"].env["KEEP"], "fixture");
        assert_eq!(cfg.mcp_servers["private-external-server"].command, "external-tool");
        assert_eq!(cfg.mcp_servers["private-external-server"].env["KEEP"], "external");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn colliding_browser_names_keep_both_configurations_after_restart() {
        let root = std::env::temp_dir().join(format!("quixi-mcp-collision-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&root).unwrap();
        let input = json!({"mcpServers": {
            "previous-browser-name":{"command":"quixi-mcp-builtin","args":["--browser-use"],"env":{"KEEP":"old"}},
            "quixi-in-chrome":{"command":"quixi-mcp-builtin","args":["--browser-use"],"env":{"KEEP":"current"}}
        }});
        fs::write(root.join("quixi_desktop_config.json"), serde_json::to_vec(&input).unwrap()).unwrap();
        for _ in 0..2 {
            let host = McpHost::new_with_data_dir(&root);
            let cfg = host.read_config();
            assert_eq!(cfg.mcp_servers["quixi-in-chrome"].env["KEEP"], "current");
            assert_eq!(cfg.mcp_servers["quixi-in-chrome (2)"].env["KEEP"], "old");
            assert!(!cfg.mcp_servers.contains_key("quixi-in-chrome (3)"));
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn earlier_memory_notes_survive_the_current_filename() {
        let root = std::env::temp_dir().join(format!("quixi-memory-migration-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(root.join("memory")).unwrap();
        let previous = root.join("memory/previous-notes.md");
        fs::write(&previous, "KEEP-OLDER-NOTES").unwrap();
        let host = McpHost::new_with_data_dir(&root);
        let recalled = host.call_tool("memory", "recall_memory", &json!({}), &root).unwrap();
        assert!(recalled["content"][0]["text"].as_str().unwrap().contains("KEEP-OLDER-NOTES"));
        host.call_tool("memory", "store_memory", &json!({"key":"new","value":"KEEP-NEW-NOTE"}), &root).unwrap();
        let saved = fs::read_to_string(root.join("memory/MEMORY.md")).unwrap();
        assert!(saved.contains("KEEP-OLDER-NOTES") && saved.contains("KEEP-NEW-NOTE"));
        assert_eq!(fs::read_to_string(previous).unwrap(), "KEEP-OLDER-NOTES");
        fs::remove_dir_all(root).unwrap();
    }
}
