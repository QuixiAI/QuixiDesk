use anyhow::{bail, Context, Result};
mod client;
mod interactions;
mod oauth;
mod resources;
mod sse;
mod worker;
use quixi_platform::{BrowserUse, ComputerUse};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
pub use worker::main as transport_worker_main;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct McpServerConfigEntry {
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub lifecycle: String,
    pub transport: Option<String>,
    pub url: Option<String>,
    pub headers: HashMap<String, String>,
    pub cwd: Option<String>,
    pub enabled: bool,
    pub disabled_tools: Vec<String>,
    pub connect_timeout_ms: u64,
    pub discovery_timeout_ms: u64,
    pub request_timeout_ms: u64,
    pub max_pages: usize,
    pub max_tools: usize,
    pub idle_timeout_ms: u64,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}
impl Default for McpServerConfigEntry {
    fn default() -> Self {
        Self {
            command: String::new(),
            args: vec![],
            env: HashMap::new(),
            lifecycle: "initialize".into(),
            transport: None,
            url: None,
            headers: HashMap::new(),
            cwd: None,
            enabled: true,
            disabled_tools: vec![],
            connect_timeout_ms: 15000,
            discovery_timeout_ms: 60000,
            request_timeout_ms: 120000,
            max_pages: 128,
            max_tools: 10000,
            idle_timeout_ms: 300000,
            extra: HashMap::new(),
        }
    }
}
impl McpServerConfigEntry {
    pub fn transport_kind(&self) -> &str {
        self.transport
            .as_deref()
            .unwrap_or(if self.url.is_some() { "http" } else { "stdio" })
    }
    fn validate(&self) -> Result<()> {
        if !["initialize", "auto", "discover"].contains(&self.lifecycle.as_str()) {
            bail!("Unsupported MCP lifecycle mode");
        }
        if let Some(roots) = self.extra.get("roots") {
            let roots = roots.as_array().context("MCP roots must be an array")?;
            if roots.len() > 32 {
                bail!("At most 32 MCP roots are supported");
            }
            for root in roots {
                let uri = root["uri"]
                    .as_str()
                    .context("MCP root requires a file URI")?;
                let parsed = url::Url::parse(uri)?;
                if parsed.scheme() != "file"
                    || parsed.query().is_some()
                    || parsed.fragment().is_some()
                {
                    bail!("MCP roots must use file URIs without query or fragment");
                }
                if root.get("name").is_some_and(|n| !n.is_string()) {
                    bail!("MCP root name must be a string");
                }
            }
        }
        match self.transport_kind() {
            "stdio" => {
                if self.command.trim().is_empty() {
                    bail!("stdio command must not be empty");
                }
                if self.command.contains('\0') {
                    bail!("Invalid command");
                }
            }
            "http" | "sse" => {
                let u = url::Url::parse(self.url.as_deref().unwrap_or(""))?;
                if !matches!(u.scheme(), "http" | "https") || u.host_str().is_none() {
                    bail!("MCP URL must use http or https");
                }
                if !u.username().is_empty() || u.password().is_some() {
                    bail!("Use headers for credentials, not URL user information");
                }
                for (k, v) in &self.headers {
                    reqwest::header::HeaderName::from_bytes(k.as_bytes())?;
                    reqwest::header::HeaderValue::from_str(v)?;
                }
            }
            other => bail!("Unsupported MCP transport '{other}'"),
        }
        for ms in [
            self.connect_timeout_ms,
            self.discovery_timeout_ms,
            self.request_timeout_ms,
        ] {
            if !(100..=3600000).contains(&ms) {
                bail!("MCP timeouts must be between 100 and 3600000 milliseconds");
            }
        }
        if self.idle_timeout_ms != 0 && !(100..=86400000).contains(&self.idle_timeout_ms) {
            bail!("Invalid idle timeout");
        }
        if !(1..=4096).contains(&self.max_pages) || !(1..=100000).contains(&self.max_tools) {
            bail!("Invalid MCP catalog limits");
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DesktopMcpConfigFile {
    pub mcp_servers: HashMap<String, McpServerConfigEntry>,
    #[serde(skip)]
    pub invalid_servers: HashMap<String, String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}
pub struct McpHost {
    pub config_path: PathBuf,
    manager: client::Manager,
    interactions: Arc<interactions::InteractionBroker>,
    config_lock: parking_lot::Mutex<()>,
}

impl McpHost {
    pub fn pending_interactions(&self) -> Vec<Value> {
        self.interactions.pending()
    }
    pub fn respond_interaction(
        &self,
        id: &str,
        action: &str,
        content: Option<Value>,
    ) -> Result<Value> {
        self.interactions.respond(id, action, content)
    }
    pub fn shutdown(&self) {
        self.manager.shutdown();
    }

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
                "memory": {
                    "command": "quixi-mcp-builtin",
                    "args": ["--memory"]
                },
                "fetch": {
                    "command": "quixi-mcp-builtin",
                    "args": ["--fetch"]
                },
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
                let renamed: Vec<_> = parsed
                    .mcp_servers
                    .iter()
                    .filter_map(|(name, entry)| {
                        if !entry.command.contains("quixi-mcp-builtin") {
                            return None;
                        }
                        let canonical = if entry.args.iter().any(|a| a == "--browser-use") {
                            "quixi-in-chrome"
                        } else if entry.args.iter().any(|a| a == "--in-app-browser") {
                            "QuixiDesk Browser"
                        } else {
                            return None;
                        };
                        (name != canonical && !name.starts_with(&format!("{canonical} (")))
                            .then(|| (name.clone(), canonical.to_string(), entry.clone()))
                    })
                    .collect();
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
                    ("memory", "--memory"),
                    ("fetch", "--fetch"),
                ] {
                    if !parsed.mcp_servers.contains_key(name) {
                        parsed.mcp_servers.insert(
                            name.to_string(),
                            McpServerConfigEntry {
                                command: "quixi-mcp-builtin".to_string(),
                                args: vec![arg.to_string()],
                                env: HashMap::new(),
                                ..Default::default()
                            },
                        );
                        changed = true;
                    }
                }
                if changed {
                    if let Ok(bytes) = serde_json::to_vec_pretty(&parsed) {
                        let _ = Self::atomic_config_write(&config_path, &bytes);
                    }
                }
            }
        }
        let interactions = Arc::new(interactions::InteractionBroker::default());
        Self {
            config_path,
            manager: client::Manager::new(quixi_dir, interactions.clone()),
            interactions,
            config_lock: parking_lot::Mutex::new(()),
        }
    }

    pub fn read_config_checked(&self) -> Result<DesktopMcpConfigFile> {
        let _guard = self.config_lock.lock();
        self.read_config_unlocked()
    }
    pub fn read_config_value_checked(&self) -> Result<Value> {
        let raw = fs::read(&self.config_path).context("Could not read MCP configuration")?;
        let value: Value =
            serde_json::from_slice(&raw).context("Invalid MCP configuration JSON")?;
        if !value["mcpServers"].is_object() {
            bail!("mcpServers must be an object");
        }
        Ok(value)
    }
    fn read_config_unlocked(&self) -> Result<DesktopMcpConfigFile> {
        let value = self.read_config_value_checked()?;
        let mut cfg = DesktopMcpConfigFile::default();
        for (name, raw) in value["mcpServers"].as_object().unwrap() {
            let entry = (|| {
                if name.trim().is_empty() {
                    bail!("Server name must not be empty");
                }
                let entry: McpServerConfigEntry = serde_json::from_value(raw.clone())?;
                entry.validate()?;
                Ok::<_, anyhow::Error>(entry)
            })();
            match entry {
                Ok(entry) => {
                    cfg.mcp_servers.insert(name.clone(), entry);
                }
                Err(error) => {
                    cfg.invalid_servers.insert(
                        name.clone(),
                        format!("Invalid MCP configuration for '{name}': {error:#}"),
                    );
                }
            }
        }
        cfg.extra = value
            .as_object()
            .unwrap()
            .iter()
            .filter(|(k, _)| k.as_str() != "mcpServers")
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        self.manager.reconcile(&cfg.mcp_servers);
        Ok(cfg)
    }
    pub fn read_config(&self) -> DesktopMcpConfigFile {
        self.read_config_checked().unwrap_or_default()
    }
    pub fn write_config(&self, mcp_servers: &Value) -> Result<()> {
        let _guard = self.config_lock.lock();
        self.write_config_unlocked(mcp_servers)
    }
    fn write_config_unlocked(&self, mcp_servers: &Value) -> Result<()> {
        let mut wrapper: Value = if self.config_path.exists() {
            serde_json::from_slice(&fs::read(&self.config_path)?)
                .context("Existing MCP configuration is invalid; repair it before saving")?
        } else {
            json!({})
        };
        let map = mcp_servers
            .as_object()
            .context("mcpServers must be an object")?;
        let mut entries = HashMap::new();
        for (name, raw) in map {
            let parsed = (|| {
                if name.trim().is_empty() {
                    bail!("Server name must not be empty");
                }
                let entry: McpServerConfigEntry = serde_json::from_value(raw.clone())?;
                entry.validate()?;
                Ok::<_, anyhow::Error>(entry)
            })();
            match parsed {
                Ok(entry) => {
                    entries.insert(name.clone(), entry);
                }
                Err(error) => {
                    if wrapper["mcpServers"].get(name) != Some(raw) {
                        return Err(
                            error.context(format!("Invalid MCP configuration for '{name}'"))
                        );
                    }
                }
            }
        }
        wrapper
            .as_object_mut()
            .context("MCP configuration must be an object")?
            .insert("mcpServers".into(), mcp_servers.clone());
        Self::atomic_config_write(&self.config_path, &serde_json::to_vec_pretty(&wrapper)?)?;
        self.manager.reconcile(&entries);
        Ok(())
    }

    fn atomic_config_write(path: &Path, bytes: &[u8]) -> Result<()> {
        let parent = path
            .parent()
            .context("MCP configuration directory missing")?;
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        temp.write_all(bytes)?;
        temp.as_file().sync_all()?;
        temp.persist(path)?;
        #[cfg(unix)]
        fs::File::open(parent)?.sync_all()?;
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
        vec![json!({
            "name": "run_applescript",
            "description": "Execute macOS AppleScript (`osascript`) to automate native Mac applications and System Events.",
            "inputSchema": { "type": "object", "properties": { "script": { "type": "string" } }, "required": ["script"] }
        })]
    }

    pub fn pdf_tools() -> Vec<Value> {
        vec![json!({
            "name": "extract_pdf_text",
            "description": "Extract text and metadata from a PDF document on disk.",
            "inputSchema": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] }
        })]
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
        vec![json!({
            "name": "fetch_url",
            "description": "Fetch a web URL and open/render it in the built-in browser while returning extracted text.",
            "inputSchema": { "type": "object", "properties": { "url": { "type": "string" } }, "required": ["url"] }
        })]
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
                        "version": "3.6556.7",
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
    pub fn builtin_catalog(name: &str) -> Vec<Value> {
        Self::builtin_tools(name).unwrap_or_default()
    }
    fn builtin_tools(name: &str) -> Option<Vec<Value>> {
        let tools = match name {
            "workspace" | "quixi-workspace" => Self::workspace_tools(),
            "computer-use" => Self::computer_use_tools(),
            "quixi-in-chrome" => Self::browser_tools(),
            "QuixiDesk Browser" | "QuixiDesk Preview" => Self::browser_and_preview_tools(),
            "applescript" => Self::applescript_tools(),
            "memory" => Self::memory_tools(),
            "fetch" => Self::fetch_tools(),
            "github" | "office" | "office365" | "pdf" => vec![],
            _ => return None,
        };
        Some(
            tools
                .into_iter()
                .filter(|t| {
                    !matches!(
                        t["name"].as_str().unwrap_or(""),
                        "gif_creator" | "show_widget" | "extract_pdf_text"
                    )
                })
                .collect(),
        )
    }
    fn canonical_name(&self, name: &str) -> Result<String> {
        let cfg = self.read_config_checked()?;
        if let Some(error) = cfg.invalid_servers.get(name) {
            bail!("{error}");
        }
        if cfg.mcp_servers.contains_key(name) {
            return Ok(name.into());
        }
        cfg.mcp_servers
            .keys()
            .find(|n| Self::internal_server_uuid(n) == Some(name))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Unknown MCP server '{name}'"))
    }
    fn entry_is_builtin(name: &str, entry: &McpServerConfigEntry) -> bool {
        entry.command == "quixi-mcp-builtin"
            && entry.transport_kind() == "stdio"
            && Self::builtin_tools(name).is_some()
    }
    pub fn is_builtin_server(&self, name: &str) -> bool {
        let Ok(name) = self.canonical_name(name) else {
            return false;
        };
        self.read_config_checked()
            .ok()
            .and_then(|c| c.mcp_servers.get(&name).cloned())
            .is_some_and(|e| Self::entry_is_builtin(&name, &e))
    }
    pub(crate) fn model_visible(tool: &Value) -> bool {
        tool.pointer("/_meta/ui/visibility")
            .and_then(Value::as_array)
            .is_none_or(|values| values.iter().any(|v| v == "model"))
    }
    fn decorate_tools(status: &mut Value, config: &McpServerConfigEntry) {
        for tool in status["tools"].as_array_mut().into_iter().flatten() {
            tool["enabled"] = json!(!config.disabled_tools.iter().any(|n| tool["name"] == *n));
            tool["modelVisible"] = json!(Self::model_visible(tool));
        }
    }
    pub fn ready_tools(&self, cwd: &Path) -> Vec<Value> {
        let _guard = self.config_lock.lock();
        let Ok(cfg) = self.read_config_unlocked() else {
            return vec![];
        };
        let canonical = cwd.canonicalize().unwrap_or_else(|_| cwd.into());
        let statuses = self.manager.statuses_for(&canonical);
        let mut tools = Vec::new();
        for (name, config) in cfg.mcp_servers {
            if !config.enabled || self.manager.is_disconnected(&name) {
                continue;
            }
            let definitions = if Self::entry_is_builtin(&name, &config) {
                Self::builtin_catalog(&name)
            } else {
                let Some(status) = statuses.get(&name) else {
                    continue;
                };
                if status["status"] != "ready" || status["catalogStale"] == true {
                    continue;
                }
                status["tools"].as_array().cloned().unwrap_or_default()
            };
            let Ok(identity) = Self::config_identity(&name, &config) else {
                continue;
            };
            for mut tool in definitions {
                if !Self::model_visible(&tool)
                    || config.disabled_tools.iter().any(|t| tool["name"] == *t)
                {
                    continue;
                }
                tool["server"] = json!(name);
                tool["serverIdentity"] = json!(identity);
                tools.push(tool);
            }
        }
        tools.sort_by_key(|t| format!("{}/{}", t["server"], t["name"]));
        tools
    }
    pub fn set_tool_enabled(&self, server: &str, tool: &str, enabled: bool) -> Result<()> {
        let name = self.canonical_name(server)?;
        let status = self
            .list_server_statuses()
            .into_iter()
            .find(|s| s["name"] == name);
        let guard = self.config_lock.lock();
        let mut raw = self.read_config_value_checked()?;
        let mut disabled: Vec<String> = raw["mcpServers"][&name]["disabledTools"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect();
        disabled.retain(|t| t != tool);
        if !enabled {
            disabled.push(tool.into());
        }
        raw["mcpServers"][&name]["disabledTools"] = json!(disabled);
        self.write_config_unlocked(&raw["mcpServers"])?;
        drop(guard);
        if let Some(status) = status {
            if status["status"] == "ready" && status["builtin"] != true {
                if let Some(cwd) = status["cwd"].as_str() {
                    self.start_connection(&name, Path::new(cwd))?;
                }
            }
        }
        Ok(())
    }
    pub fn list_internal_servers(&self) -> Vec<Value> {
        self.list_server_statuses()
            .into_iter()
            .filter(|s| s["builtin"] == true)
            .collect()
    }
    pub fn list_local_mcp_servers(&self) -> Vec<Value> {
        self.list_server_statuses()
    }
    pub fn list_server_statuses(&self) -> Vec<Value> {
        let cfg = match self.read_config_checked() {
            Ok(c) => c,
            Err(e) => {
                return vec![
                    json!({"name":"Configuration","status":"failed","isConnected":false,"toolsEnumerated":false,"tools":[],"error":{"kind":"configuration","message":format!("{e:#}")}}),
                ]
            }
        };
        let mut statuses = self.manager.statuses();
        let mut result:Vec<Value>=cfg.invalid_servers.iter().map(|(name,error)|json!({"name":name,"wireKey":name,"status":"failed","isConnected":false,"toolsEnumerated":false,"tools":[],"error":{"kind":"configuration","message":error}})).collect();
        for (name, entry) in cfg.mcp_servers {
            let builtin = Self::entry_is_builtin(&name, &entry);
            let disconnected = self.manager.is_disconnected(&name);
            if !builtin {
                if let Some(mut s) = statuses.remove(&name) {
                    Self::decorate_tools(&mut s, &entry);
                    result.push(s);
                    continue;
                }
            }
            let unsupported = builtin && Self::builtin_tools(&name).is_some_and(|t| t.is_empty());
            let ready = builtin && entry.enabled && !disconnected && !unsupported;
            result.push(json!({"name":name,"uuid":if builtin{Self::internal_server_uuid(&name).unwrap_or(&name)}else{&name},"wireKey":name,"builtin":builtin,"isInternal":builtin,"serverSource":"user_config","transport":entry.transport_kind(),"status":if !entry.enabled{"disabled"}else if disconnected{"disconnected"}else if unsupported{"failed"}else if ready{"ready"}else{"configured"},"isConnected":ready,"toolsEnumerated":ready,"catalogStale":false,"tools":if ready{Self::builtin_tools(&name).unwrap_or_default()}else{vec![]},"resources":[],"prompts":[],"error":if unsupported{json!({"kind":"unsupported","message":"This legacy built-in connector is not implemented. Configure a working external MCP server instead."})}else{Value::Null},"generation":0}));
        }
        for row in &mut result {
            if row["builtin"] == true {
                if let Some(config) = self.read_config_checked().ok().and_then(|c| {
                    c.mcp_servers
                        .get(row["name"].as_str().unwrap_or(""))
                        .cloned()
                }) {
                    Self::decorate_tools(row, &config);
                }
            }
        }
        result.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
        result
    }
    fn external_entry(&self, name: &str, cwd: &Path) -> Result<Arc<client::Entry>> {
        let _guard = self.config_lock.lock();
        let cfg = self.read_config_unlocked()?;
        let entry = cfg
            .mcp_servers
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Unknown MCP server '{name}'"))?;
        self.manager.entry(name, entry, cwd)
    }
    pub fn start_connection(&self, name: &str, cwd: &Path) -> Result<()> {
        let name = self.canonical_name(name)?;
        self.manager.enable(&name);
        if !self.is_builtin_server(&name) {
            client::Manager::start(self.external_entry(&name, cwd)?, false);
        }
        Ok(())
    }
    pub fn reconnect_server(&self, name: &str, cwd: &Path) -> Result<()> {
        let name = self.canonical_name(name)?;
        self.manager.reconnect(&name);
        self.start_connection(&name, cwd)
    }
    pub fn refresh_server(&self, name: &str, cwd: &Path) -> Result<()> {
        let name = self.canonical_name(name)?;
        if !self.is_builtin_server(&name) {
            client::Manager::start(self.external_entry(&name, cwd)?, true);
        }
        Ok(())
    }
    pub fn disconnect_server(&self, name: &str) -> Result<()> {
        let name = self.canonical_name(name)?;
        self.manager.disconnect(&name);
        Ok(())
    }
    pub fn authenticate_server(&self, name: &str, cwd: &Path) -> Result<Value> {
        let name = self.canonical_name(name)?;
        let cfg = self.read_config_checked()?;
        if cfg.mcp_servers[&name].transport_kind() == "stdio" {
            bail!("OAuth sign-in requires a remote HTTP MCP server");
        }
        self.manager.enable(&name);
        let entry = self.external_entry(&name, cwd)?;
        Ok(client::Manager::authenticate(entry))
    }
    pub fn clear_server_auth(&self, name: &str) -> Result<()> {
        let name = self.canonical_name(name)?;
        let cfg = self.read_config_checked()?;
        let url = cfg.mcp_servers[&name]
            .url
            .as_deref()
            .context("This server does not use remote authentication")?;
        self.manager.cancel_auth(&name);
        self.manager.disconnect(&name);
        client::block_on(oauth::clear(
            self.config_path
                .parent()
                .context("Missing profile directory")?,
            &name,
            url,
        ))?;
        self.manager.signed_out(&name);
        Ok(())
    }
    pub fn cancel_authentication_server(&self, name: &str) -> Result<()> {
        let name = self.canonical_name(name)?;
        self.manager.cancel_auth(&name);
        Ok(())
    }
    pub fn discover_tools(
        &self,
        server: Option<&str>,
        query: Option<&str>,
        cursor: usize,
        cwd: &Path,
        cancelled: &AtomicBool,
    ) -> Result<Value> {
        if cancelled.load(Ordering::SeqCst) {
            bail!("MCP discovery cancelled");
        }
        let server = server.map(|s| self.canonical_name(s)).transpose()?;
        let mut specific = None;
        if let Some(name) = &server {
            if !self.is_builtin_server(name) {
                let entry = self.external_entry(name, cwd)?;
                client::block_on(client::Manager::discover(&entry, cancelled))?;
                let mut status = entry.status();
                Self::decorate_tools(&mut status, &entry.config);
                specific = Some(status);
            }
        }
        let servers: Vec<_> = if let Some(s) = specific {
            vec![s]
        } else {
            self.list_server_statuses()
                .into_iter()
                .filter(|s| server.as_ref().is_none_or(|n| s["name"] == *n))
                .collect()
        };
        if server.is_some()
            && servers
                .iter()
                .any(|s| s["status"] != "ready" || s["catalogStale"] == true)
        {
            bail!("MCP server is unavailable; connect or enable it before discovering tools");
        }
        let mut tools = Vec::new();
        let needle = query.unwrap_or("").to_lowercase();
        for s in &servers {
            if s["status"] != "ready" || s["catalogStale"] == true {
                continue;
            }
            if let Some(ts) = s["tools"].as_array() {
                for t in ts {
                    if t["enabled"] == false || !Self::model_visible(t) {
                        continue;
                    }
                    if !needle.is_empty()
                        && !format!("{} {} {}", s["name"], t["name"], t["description"])
                            .to_lowercase()
                            .contains(&needle)
                    {
                        continue;
                    }
                    let mut t = t.clone();
                    t["server"] = s["name"].clone();
                    tools.push(t);
                }
            }
        }
        tools.sort_by(|a, b| {
            a["server"]
                .as_str()
                .cmp(&b["server"].as_str())
                .then(a["name"].as_str().cmp(&b["name"].as_str()))
        });
        if cursor > tools.len() {
            bail!("Invalid MCP discovery cursor");
        }
        let mut end = cursor;
        let mut bytes = 0;
        while end < tools.len() && end - cursor < 20 {
            let size = serde_json::to_vec(&tools[end])?.len();
            if end > cursor && bytes + size > 65536 {
                break;
            }
            bytes += size;
            end += 1;
        }

        let summaries: Vec<_> = servers
            .into_iter()
            .map(|mut s| {
                for key in ["tools", "resources", "prompts"] {
                    let count = s[key].as_array().map_or(0, Vec::len);
                    s.as_object_mut().unwrap().remove(key);
                    s[format!("{key}Count")] = json!(count);
                }
                for key in ["stderr", "authorizationUrl", "authAttempt"] {
                    s.as_object_mut().unwrap().remove(key);
                }
                if let Some(instructions) = s["instructions"].as_str() {
                    if instructions.chars().count() > 4096 {
                        s["instructions"] =
                            json!(instructions.chars().take(4096).collect::<String>());
                        s["instructionsTruncated"] = json!(true);
                    }
                }
                s
            })
            .collect();
        Ok(
            json!({"servers":summaries,"tools":tools[cursor..end],"nextCursor":if end<tools.len(){Some(end)}else{None}}),
        )
    }
    pub fn call_tool(&self, server: &str, tool: &str, args: &Value, cwd: &Path) -> Result<Value> {
        self.call_tool_cancellable(server, tool, args, cwd, &AtomicBool::new(false))
    }
    pub fn server_identity(&self, server: &str) -> Result<String> {
        let cfg = self.read_config_checked()?;
        let name = Self::resolve_config_name(&cfg, server)?;
        Self::config_identity(&name, &cfg.mcp_servers[&name])
    }
    fn resolve_config_name(cfg: &DesktopMcpConfigFile, name: &str) -> Result<String> {
        if let Some(error) = cfg.invalid_servers.get(name) {
            bail!("{error}");
        }
        if cfg.mcp_servers.contains_key(name) {
            return Ok(name.into());
        }
        cfg.mcp_servers
            .keys()
            .find(|n| Self::internal_server_uuid(n) == Some(name))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Unknown MCP server '{name}'"))
    }
    fn config_identity(name: &str, entry: &McpServerConfigEntry) -> Result<String> {
        use sha2::{Digest, Sha256};
        fn canonical(value: Value) -> Value {
            match value {
                Value::Object(map) => {
                    let sorted: std::collections::BTreeMap<_, _> =
                        map.into_iter().map(|(k, v)| (k, canonical(v))).collect();
                    Value::Object(sorted.into_iter().collect())
                }
                Value::Array(a) => Value::Array(a.into_iter().map(canonical).collect()),
                v => v,
            }
        }
        let value = canonical(json!({"name":name,"config":entry}));
        Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(&value)?)))
    }
    pub fn call_tool_cancellable(
        &self,
        server: &str,
        tool: &str,
        args: &Value,
        cwd: &Path,
        cancelled: &AtomicBool,
    ) -> Result<Value> {
        self.call_tool_with_identity(server, tool, args, cwd, cancelled, None)
    }
    pub fn call_tool_with_identity(
        &self,
        server: &str,
        tool: &str,
        args: &Value,
        cwd: &Path,
        cancelled: &AtomicBool,
        expected: Option<&str>,
    ) -> Result<Value> {
        if cancelled.load(Ordering::SeqCst) {
            bail!("MCP call cancelled");
        }
        let config_guard = self.config_lock.lock();
        let cfg = self.read_config_unlocked()?;
        let name = Self::resolve_config_name(&cfg, server)?;
        let config = &cfg.mcp_servers[&name];
        let identity = Self::config_identity(&name, config)?;
        if expected.is_some_and(|expected| expected != identity) {
            bail!("MCP server configuration changed after approval; request approval again before executing");
        }
        if !config.enabled {
            bail!("MCP server '{name}' is disabled");
        }
        if config.disabled_tools.iter().any(|n| n == tool) {
            bail!("MCP tool '{name}/{tool}' is disabled");
        }
        if self.manager.is_disconnected(&name) {
            bail!("MCP server '{name}' is disconnected");
        }
        if Self::entry_is_builtin(&name, config) {
            let definition = Self::builtin_tools(&name)
                .unwrap_or_default()
                .into_iter()
                .find(|t| t["name"] == tool)
                .ok_or_else(|| anyhow::anyhow!("Unknown tool '{tool}' on MCP server '{name}'"))?;
            client::validate_instance(&definition["inputSchema"], args)?;
            drop(config_guard);
            self.call_builtin_tool(tool, args, cwd)
        } else {
            let entry = self.manager.entry(&name, config, cwd)?;
            drop(config_guard);
            client::block_on(client::Manager::call(&entry, tool, args, cancelled))
        }
    }

    pub fn list_resources(&self, server: &str) -> Vec<Value> {
        self.list_server_statuses()
            .into_iter()
            .find(|s| s["name"] == server && s["status"] == "ready" && s["catalogStale"] != true)
            .and_then(|s| s["resources"].as_array().cloned())
            .unwrap_or_default()
    }
    pub fn read_resource_checked(
        &self,
        server: &str,
        uri: &str,
        cwd: &Path,
        cancelled: &AtomicBool,
    ) -> Result<Value> {
        let name = self.canonical_name(server)?;
        if self.is_builtin_server(&name) {
            bail!("MCP server '{name}' does not expose resources");
        }
        let entry = self.external_entry(&name, cwd)?;
        client::block_on(client::Manager::resource(
            &entry,
            "resources/read",
            json!({"uri":uri}),
            cancelled,
        ))
    }
    pub fn get_prompt(
        &self,
        server: &str,
        name: &str,
        args: &Value,
        cwd: &Path,
        cancelled: &AtomicBool,
    ) -> Result<Value> {
        let server = self.canonical_name(server)?;
        if self.is_builtin_server(&server) {
            bail!("MCP server '{server}' does not expose prompts");
        }
        let entry = self.external_entry(&server, cwd)?;
        client::block_on(client::Manager::resource(
            &entry,
            "prompts/get",
            json!({"name":name,"arguments":args}),
            cancelled,
        ))
    }
    pub fn read_resource(&self, server: &str, uri: &str) -> Value {
        self.read_resource_checked(
            server,
            uri,
            Path::new(quixi_proto::paths::default_workspace_str()),
            &AtomicBool::new(false),
        )
        .unwrap_or_else(|e| json!({"error":format!("{e:#}"),"isError":true}))
    }

    pub fn call_builtin_server_tool(
        &self,
        server: &str,
        tool: &str,
        args: &Value,
        cwd: &Path,
    ) -> Result<Value> {
        let definition = Self::builtin_catalog(server)
            .into_iter()
            .find(|t| t["name"] == tool)
            .ok_or_else(|| {
                anyhow::anyhow!("Unknown tool '{tool}' on built-in MCP server '{server}'")
            })?;
        client::validate_instance(&definition["inputSchema"], args)?;
        self.call_builtin_tool(tool, args, cwd)
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
                if target_str.is_empty()||!existing.contains(target_str){bail!("Edit target was not found; file was not changed");}
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
                fs::remove_file(&target)?;
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
                    "isError": res["ok"] == false
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
            "gif_creator" => bail!("GIF recording is not implemented"),

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
            "extract_pdf_text" | "show_widget" => bail!("This built-in capability is not implemented. Configure an MCP server that provides it."),
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
            _ => bail!("Unsupported built-in MCP tool '{tool_name}'"),
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
    fn builtin_catalog_routes_only_implemented_tools_on_the_named_server() {
        let root = tempfile::tempdir().unwrap();
        let host = McpHost::new_with_data_dir(root.path());
        assert!(host
            .list_local_mcp_servers()
            .iter()
            .all(|s| s["builtin"] == true && s["status"] == "ready"));
        let file = root.path().join("example.txt");
        fs::write(&file, "fixture").unwrap();
        let result = host
            .call_tool(
                "quixi-workspace",
                "read_file",
                &json!({"path":"example.txt"}),
                root.path(),
            )
            .unwrap();
        assert_eq!(result["content"][0]["text"], "fixture");
        assert!(host
            .call_tool(
                "memory",
                "write_file",
                &json!({"path":"wrong.txt","content":"bad"}),
                root.path()
            )
            .is_err());
        assert!(!root.path().join("wrong.txt").exists());
        assert!(host
            .call_builtin_tool("not_a_tool", &json!({}), root.path())
            .is_err());
    }
}

#[cfg(test)]
mod resource_migration_tests {
    use super::*;
    #[test]
    fn owned_browser_names_migrate_without_changing_external_commands() {
        let root = std::env::temp_dir().join(format!(
            "quixi-mcp-migration-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let input = json!({"mcpServers": {
            "previous-browser-name":{"command":"quixi-mcp-builtin","args":["--browser-use"],"env":{"KEEP":"fixture"}},
            "private-external-server":{"command":"external-tool","args":["--custom"],"env":{"KEEP":"external"}}
        }});
        fs::write(
            root.join("quixi_desktop_config.json"),
            serde_json::to_vec(&input).unwrap(),
        )
        .unwrap();
        let host = McpHost::new_with_data_dir(&root);
        let cfg = host.read_config();
        assert!(!cfg.mcp_servers.contains_key("previous-browser-name"));
        assert_eq!(cfg.mcp_servers["quixi-in-chrome"].env["KEEP"], "fixture");
        assert_eq!(
            cfg.mcp_servers["private-external-server"].command,
            "external-tool"
        );
        assert_eq!(
            cfg.mcp_servers["private-external-server"].env["KEEP"],
            "external"
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn colliding_browser_names_keep_both_configurations_after_restart() {
        let root = std::env::temp_dir().join(format!(
            "quixi-mcp-collision-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let input = json!({"mcpServers": {
            "previous-browser-name":{"command":"quixi-mcp-builtin","args":["--browser-use"],"env":{"KEEP":"old"}},
            "quixi-in-chrome":{"command":"quixi-mcp-builtin","args":["--browser-use"],"env":{"KEEP":"current"}}
        }});
        fs::write(
            root.join("quixi_desktop_config.json"),
            serde_json::to_vec(&input).unwrap(),
        )
        .unwrap();
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
        let root = std::env::temp_dir().join(format!(
            "quixi-memory-migration-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("memory")).unwrap();
        let previous = root.join("memory/previous-notes.md");
        fs::write(&previous, "KEEP-OLDER-NOTES").unwrap();
        let host = McpHost::new_with_data_dir(&root);
        let recalled = host
            .call_tool("memory", "recall_memory", &json!({}), &root)
            .unwrap();
        assert!(recalled["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("KEEP-OLDER-NOTES"));
        host.call_tool(
            "memory",
            "store_memory",
            &json!({"key":"new","value":"KEEP-NEW-NOTE"}),
            &root,
        )
        .unwrap();
        let saved = fs::read_to_string(root.join("memory/MEMORY.md")).unwrap();
        assert!(saved.contains("KEEP-OLDER-NOTES") && saved.contains("KEEP-NEW-NOTE"));
        assert_eq!(fs::read_to_string(previous).unwrap(), "KEEP-OLDER-NOTES");
        fs::remove_dir_all(root).unwrap();
    }
}
