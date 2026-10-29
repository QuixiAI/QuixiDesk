use chrono::Utc;
use quixi_engine::QuixiEngine;
use quixi_proto::{
    CoworkSpace, CoworkSpaceFolder, CoworkSpaceLink, CoworkSpaceProject, EipcInvokePayload,
    EipcMethodKind, EipcReplyPayload, ParsedEipcChannel,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

const EXACT_SCHEMAS_JSON: &str = include_str!("../ipc-exact-schemas.json");

pub struct EipcRouter {
    pub engine: Arc<QuixiEngine>,
    pub exact_methods: HashMap<String, Value>,
    pub exact_stores: HashMap<String, Value>,
}

impl EipcRouter {
    pub fn new(engine: Arc<QuixiEngine>) -> Self {
        let parsed: Value = serde_json::from_str(EXACT_SCHEMAS_JSON).unwrap_or_else(|_| json!({}));
        let mut exact_methods = HashMap::new();
        if let Some(m) = parsed.get("methods").and_then(|v| v.as_object()) {
            for (k, v) in m {
                exact_methods.insert(k.clone(), v.clone());
            }
        }
        let mut exact_stores = HashMap::new();
        if let Some(s) = parsed.get("stores").and_then(|v| v.as_object()) {
            for (k, v) in s {
                exact_stores.insert(k.clone(), v.clone());
            }
        }
        Self {
            engine,
            exact_methods,
            exact_stores,
        }
    }

    pub fn dispatch(&self, req: &EipcInvokePayload) -> EipcReplyPayload {
        let ch = req.channel.as_str();

        // Non-$eipc_message$ legacy channels
        if ch == "artifact-window-open-gate:state" {
            return EipcReplyPayload::ok(json!({ "active": false }));
        }
        if ch == "list-mcp-servers" || ch.contains("ListMcpServers") {
            let names: Vec<String> = self
                .engine
                .mcp_host
                .list_server_statuses()
                .into_iter()
                .filter_map(|s| s.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()))
                .collect();
            return EipcReplyPayload::ok(json!(names));
        }
        if ch == "connect-to-mcp-server" || ch == "request-open-mcp-settings" {
            return EipcReplyPayload::ok(json!({ "ok": true }));
        }

        let Some(parsed) = ParsedEipcChannel::parse(ch) else {
            return EipcReplyPayload::ok(Value::Null);
        };

        // Handle _$store$_getStateSync and _$store$_getState
        if parsed.kind == EipcMethodKind::StoreGetStateSync
            || parsed.kind == EipcMethodKind::StoreGetState
        {
            let sk = parsed.store_key();
            let dot_key = format!("{}.{}", parsed.service, parsed.member);
            let mut val = self
                .engine
                .stores
                .read()
                .get(&sk)
                .cloned()
                .unwrap_or(Value::Null);
            if let Some(schema_val) = self.exact_stores.get(&dot_key) {
                val = Self::merge_with_exact_schema(val, schema_val);
            } else if val.is_null() {
                val = json!({});
            }
            return EipcReplyPayload::ok(val);
        }

        if parsed.namespace == "quixi.web" && parsed.member == "delete" {
            let kind = match parsed.service.as_str() {
                "LocalAgentModeSessions" => Some("sessions"),
                "LocalSessions" => Some("code_sessions"),
                _ => None,
            };
            if let Some(kind) = kind {
                let id = req.args.first().and_then(Value::as_str).unwrap_or_default();
                if !id.is_empty() {
                    if let Err(error) = self.engine.delete_session_storage(kind, id) {
                        return EipcReplyPayload::err(error);
                    }
                }
            }
        }
        let raw_res = self.dispatch_service(
            &parsed.namespace,
            &parsed.service,
            &parsed.member,
            &req.args,
        );
        let dot_key = format!("{}.{}", parsed.service, parsed.member);
        let preserve_null = raw_res.is_null()
            && (!req.args.is_empty()
                || matches!(
                    dot_key.as_str(),
                    "FileSystem.browseFolder"
                        | "FileSystem.browseFolders"
                        | "FileSystem.browseFile"
                        | "FileSystem.browseFiles"
                        | "LocalAgentModeSessions.browseAndAddTrustedFolder"
                ));
        let res = if preserve_null {
            Value::Null
        } else if let Some(schema_val) = self.exact_methods.get(&dot_key) {
            Self::merge_with_exact_schema(raw_res, schema_val)
        } else {
            raw_res
        };

        EipcReplyPayload::ok(res)
    }

    fn percent_decode(s: &str) -> String {
        let bytes = s.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'%' && i + 2 < bytes.len() {
                if let (Some(h), Some(l)) = (
                    (bytes[i + 1] as char).to_digit(16),
                    (bytes[i + 2] as char).to_digit(16),
                ) {
                    out.push(((h << 4) | l) as u8);
                    i += 3;
                    continue;
                }
            }
            out.push(bytes[i]);
            i += 1;
        }
        String::from_utf8(out).unwrap_or_else(|_| s.to_string())
    }

    fn merge_with_exact_schema(actual: Value, schema: &Value) -> Value {
        if actual.is_null() {
            return schema.clone();
        }
        match (actual, schema) {
            (Value::Object(mut act_map), Value::Object(sch_map)) => {
                for (k, sch_v) in sch_map {
                    match act_map.remove(k) {
                        Some(act_v) => {
                            act_map.insert(k.clone(), Self::merge_with_exact_schema(act_v, sch_v));
                        }
                        None => {
                            act_map.insert(k.clone(), sch_v.clone());
                        }
                    }
                }
                Value::Object(act_map)
            }
            (Value::Array(act_arr), Value::Array(_)) => Value::Array(act_arr),
            (Value::Bool(b), Value::Bool(_)) => Value::Bool(b),
            (Value::String(s), Value::String(_)) => Value::String(s),
            (Value::Number(n), Value::Number(_)) => Value::Number(n),
            (_, sch_other) => sch_other.clone(),
        }
    }

    fn dispatch_service(
        &self,
        _ns: &str,
        service: &str,
        member: &str,
        args: &[Value],
    ) -> Value {
        let now_ms = Utc::now().timestamp_millis() as u64;

        match (service, member) {
            // --- AppFeatures & AppPreferences & AppConfig ---
            ("AppFeatures", "getSupportedFeatures") => self.engine.desktop_features.clone(),
            ("AppPreferences", "getPreferences") | ("Preferences", "getPreferences") => {
                self.engine.app_preferences.read().clone()
            }
            ("AppPreferences", "setPreference") | ("Preferences", "setPreference") => {
                if let (Some(key), Some(val)) = (args.first().and_then(|v| v.as_str()), args.get(1))
                {
                    if let Some(obj) = self.engine.app_preferences.write().as_object_mut() {
                        obj.insert(key.to_string(), val.clone());
                    }
                    let prefs = self.engine.app_preferences.read().clone();
                    self.engine.emit_eipc(
                        "quixi.settings",
                        "AppPreferences",
                        "preferencesChanged",
                        prefs,
                    );
                }
                Value::Null
            }
            ("AppConfig", "getAppConfig") => json!({
                "isDxtAutoUpdatesEnabled": true,
                "isUsingBuiltInNodeForMcp": true
            }),
            ("CcdCli", "getCcdCliStatus") => json!({
                "state": "installed",
                "blockedBy": "",
                "appInApplications": true
            }),
            ("CcdCli", "installCcdCli") | ("CcdCli", "uninstallCcdCli") => json!({
                "ok": true,
                "code": "ok",
                "blockedBy": "",
                "manualCommand": "ln -sf /Applications/QuixiDesk.app/Contents/MacOS/quixi-app /usr/local/bin/quixi"
            }),
            ("DesktopInfo", "getDefaultBrowserName") => json!("Safari"),
            ("DesktopInfo", "getSystemInfo") => json!({
                "app_version": "3.6556.7",
                "os_version": "15.0",
                "os_pretty_name": "macOS Sequoia",
                "cpu_model": "Apple Silicon",
                "platform": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
                "total_memory": 17179869184u64,
                "can_elevate_to_admin": true,
                "is_msix": false,
                "installer_variant": "dmg"
            }),
            ("Startup", "isMenuBarEnabled") | ("Startup", "isStartupOnLoginEnabled") => json!(true),
            ("GlobalShortcut", "getGlobalShortcut") => json!("Alt+Space"),
            ("WakeScheduler", "getStatus") => json!({ "enabled": false, "nextWakeAt": null }),
            ("MCP", "getMcpServersConfig") => {
                let cfg = self.engine.mcp_host.read_config();
                json!(cfg.mcp_servers)
            }
            ("MCP", "setMcpServerConfigs") => {
                if let Some(val) = args.first() {
                    let _ = self.engine.mcp_host.write_config(val);
                }
                Value::Null
            }
            ("MCP", "getMcpServersConfigWithStatus") => {
                let cfg = self.engine.mcp_host.read_config();
                let mut map = serde_json::Map::new();
                for (name, entry) in cfg.mcp_servers {
                    map.insert(
                        name,
                        json!({
                            "command": entry.command,
                            "args": entry.args,
                            "env": entry.env,
                            "status": "running"
                        }),
                    );
                }
                Value::Object(map)
            }
            ("MCP", "isLocalDevMcpEnabled") => json!(true),
            ("Extensions", "getInstalledExtensionsWithState") => {
                json!(self.engine.mcp_host.list_installed_extensions_with_state())
            }
            ("Extensions", "getAvailableExtensionRuntimes")
            | ("Extensions", "getExtensionVersions") => json!([]),
            ("Extensions", "isExtensionsEnabled")
            | ("Extensions", "isDirectoryEnabled")
            | ("Extensions", "isLocalExtensionInstallAllowed") => json!(true),
            ("Extensions", "isDesktopExtensionSignatureRequired") => json!(false),
            ("Extensions", "getIsUpdateAvailable") => json!(""),
            ("Extensions", "getExtensionSettings") => json!({ "isEnabled": true }),
            ("HistoryImport", "enumerateSessionSources") => {
                let cli_sessions = quixi_workspace::TranscriptSearchEngine::list_cli_sessions(25);
                if cli_sessions.is_empty() {
                    json!([])
                } else {
                    json!([{
                        "id": "quixi-cli-local",
                        "label": "Workspace CLI (~/.quixi/cli/projects)",
                        "rootPath": quixi_proto::paths::account_home().join(".quixi/cli/projects").to_string_lossy(),
                        "origin": "local",
                        "kind": "cli",
                        "sessionCount": cli_sessions.len()
                    }])
                }
            }
            ("HistoryImport", "getImportHistory")
            | ("HistoryImport", "listInstructionTexts")
            | ("HistoryImport", "listSourceExtras") => json!([]),
            ("HistoryImport", "getAutoImportStatus") => json!({ "state": "idle" }),
            ("HistoryImport", "getImportCapabilities") => json!({
                "remotePull": true,
                "remotePullConfigured": true,
                "localZip": true,
                "localSessions": true,
                "sessionExport": true,
                "hostLoopRequired": false,
                "cliUpdateRequired": false,
                "hasImportableSessions": true,
                "local1PSessionCount": 0,
                "promptBanner": ""
            }),
            ("WindowState", "getAppFocused")
            | ("WindowState", "getOnScreen")
            | ("WindowState", "getVisibility")
            | ("WindowState", "getWindowActive") => json!(true),
            ("WindowState", "getFullscreen") | ("WindowState", "getKeptHidden") => json!(false),
            ("WindowState", "getZoomFactor") => json!(1.0),
            ("DeviceRegistry", "getFreeDiskSpace") | ("DeviceStorage", "getFreeDiskSpace") => {
                json!(107374182400u64)
            }
            ("DeviceStorage", "getUsage") => self.engine.get_device_storage_usage(),
            ("DesktopNotifications", "getAuthorizationStatus") => json!("authorized"),
            ("DesktopNotifications", "supportsNotificationActions") => json!(true),
            ("ForgeState", "getForgeStateSnapshot") => json!([]),
            ("GitState", "getGitStateSnapshot") => json!([]),
            ("LimitAutoResume", "getLimitAutoResumeState") => json!({
                "hostOwns": true,
                "seq": 1,
                "windows": []
            }),
            ("LocalSessionEnvironment", "get") | ("LocalSessionEnvironment", "getScoped") => {
                json!({})
            }
            ("LocalSessionPresets", "load") => json!([]),
            ("PluginBridgeMcp", "listServers") => {
                json!(self.engine.mcp_host.list_server_statuses())
            }
            ("FramebufferPreview", "listSources") => json!([
                {
                    "name": "macos-desktop",
                    "origin": "local",
                    "width": 1920,
                    "height": 1080,
                    "agentInput": true
                }
            ]),
            ("FramebufferPreview", "attach") => json!({
                "sessionId": "fb_1",
                "name": "macos-desktop",
                "origin": "local",
                "width": 1920,
                "height": 1080
            }),
            ("FramebufferPreview", "requestFramePort") => json!(true),
            ("WorkspaceImport", "listImportSpaceTwins") | ("WorkspaceImport", "pickLocalZips") => {
                json!([])
            }
            ("WorkspaceImport", "getAuthState") => json!({ "status": "signed-out" }),
            ("CoworkFilePreview", "isEnabled")
            | ("CoworkFilePreview", "isBuiltInViewerAvailable")
            | ("CoworkFilePreview", "isEpitaxyPreviewEnabled")
            | ("CoworkFilePreview", "isOpenInDefaultAppEnabled")
            | ("CoworkFilePreview", "isVmReady") => json!(true),

            // --- Launch (Dev Servers & Preview Tabs) ---
            ("Launch", "isAvailable") => json!(true),
            ("Launch", "isJitlessForced") | ("Launch", "getAutoVerify") => json!(false),
            ("Launch", "detectServerCandidates") => {
                let cwd = args
                    .first()
                    .and_then(|v| v.as_str())
                    .unwrap_or(quixi_proto::paths::default_workspace_str());
                json!(self.engine.detect_server_candidates(Path::new(cwd)))
            }
            ("Launch", "getPreviewTabs") => {
                let tabs = self.engine.preview_tabs.read().clone();
                let active = self.engine.active_preview_tab_id.read().clone();
                json!({
                    "tabs": tabs,
                    "activeTabId": active
                })
            }
            ("Launch", "openPreviewTab") => {
                let url = args
                    .first()
                    .and_then(|v| v.as_str())
                    .unwrap_or("http://localhost:3000");
                let tab_id = format!("tab_{}", Uuid::new_v4());
                self.engine.preview_tabs.write().push(json!({
                    "id": tab_id,
                    "url": url,
                    "title": url,
                }));
                *self.engine.active_preview_tab_id.write() = tab_id.clone();
                json!(tab_id)
            }
            ("Launch", "closePreviewTab") => {
                let tab_id = args.first().and_then(|v| v.as_str()).unwrap_or("");
                self.engine
                    .preview_tabs
                    .write()
                    .retain(|t| t.get("id").and_then(|v| v.as_str()) != Some(tab_id));
                json!(true)
            }
            ("Launch", "setActivePreviewTab") => {
                if let Some(tab_id) = args.first().and_then(|v| v.as_str()) {
                    *self.engine.active_preview_tab_id.write() = tab_id.to_string();
                }
                json!(true)
            }

            // --- Simulator ---
            ("Simulator", "listDevices") => json!(QuixiEngine::list_simulator_devices()),
            ("Simulator", "listDeviceTypes") => json!({ "runtimes": [] }),

            // --- DesktopIntl & Account ---
            ("DesktopIntl", "getInitialLocale") => {
                json!({ "locale": *self.engine.user_locale.read() })
            }
            ("DesktopIntl", "requestLocaleChange") => {
                if let Some(loc) = args
                    .first()
                    .and_then(|v| v.as_str())
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                {
                    *self.engine.user_locale.write() = loc.to_string();
                    let _ = std::fs::write(self.engine.data_dir.join("locale.txt"), loc);
                }
                Value::Null
            }
            ("Account", "setAccountDetails") => Value::Null,

            // --- AboutWindow, QuickWindow, MainWindowTitleBar ---
            ("AboutWindow", "getAppName") => json!("QuixiDesk"),
            ("AboutWindow", "getBuildProps") => json!({
                "commitHash": "20c019a1e61fa5b687616af57d31f97bb3181723",
                "isNestBuild": false,
                "commitTimestamp": "2026-10-26T00:00:00.000Z",
                "buildType": "prod",
                "appVersion": "3.6556.7"
            }),
            ("AboutWindow", "ancientJournal") => Value::Null,

            // --- WindowControl & WindowManager & WindowLayout ---
            ("WindowControl", "captureScreenshot") => {
                let b64 = quixi_platform::ComputerUse::capture_screenshot_base64()
                    .unwrap_or_default();
                json!(b64)
            }
            ("WindowManager", "GetWindowState") => json!({
                "isMaximized": false,
                "isMinimized": false,
                "isFullScreen": false,
                "isFocused": true
            }),
            ("SessionPopouts", "takeRestorableLayout") => {
                json!({ "popouts": [], "panes": [], "projects": [] })
            }
            ("SessionPopouts", "takeRestorablePopouts") => json!([]),

            // --- LocalAgentModeSessions (Cowork, Skills & 3P Chat) ---
            ("LocalAgentModeSessions", "getAll") => {
                let mut list: Vec<_> = self
                    .engine
                    .agent_sessions
                    .read()
                    .values()
                    .filter(|s| !s.session_id.trim().is_empty())
                    .cloned()
                    .collect();
                list.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
                json!(list)
            }
            ("LocalAgentModeSessions", "getSessionList") => {
                let mut list: Vec<_> = self
                    .engine
                    .agent_sessions
                    .read()
                    .values()
                    .filter(|s| !s.session_id.trim().is_empty())
                    .cloned()
                    .collect();
                list.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
                json!({
                    "sessions": list,
                    "restPending": false
                })
            }
            ("LocalAgentModeSessions", "getSession") => {
                let sid = args.first().and_then(|v| v.as_str()).unwrap_or("");
                if sid.trim().is_empty() {
                    Value::Null
                } else {
                    self.engine
                        .agent_sessions
                        .read()
                        .get(sid)
                        .map(|s| json!(s))
                        .unwrap_or(Value::Null)
                }
            }
            ("LocalAgentModeSessions", "getSessionMissReason") => {
                json!({ "reason": "not_found" })
            }
            ("LocalAgentModeSessions", "getTranscript") => {
                let sid = args.first().and_then(|v| v.as_str()).unwrap_or("");
                self.engine
                    .agent_sessions
                    .read()
                    .get(sid)
                    .map(|s| json!(s.buffered_messages))
                    .unwrap_or_else(|| json!([]))
            }
            ("LocalAgentModeSessions", "getTranscriptRead") => {
                let sid = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let msgs = self
                    .engine
                    .agent_sessions
                    .read()
                    .get(sid)
                    .map(|s| s.buffered_messages.clone())
                    .unwrap_or_default();
                let count = msgs.len();
                json!({
                    "messages": msgs,
                    "diag": {
                        "source": "hit",
                        "mainMs": 1,
                        "messages": count,
                        "handlerStartWallMs": now_ms
                    }
                })
            }
            ("LocalAgentModeSessions", "start") => {
                if args.is_empty() {
                    return Value::Null;
                }
                let opts = args.first().cloned().unwrap_or_else(|| json!({}));
                let session_id = self.engine.start_agent_session(&opts);
                self.engine
                    .agent_sessions
                    .read()
                    .get(&session_id)
                    .map(|s| json!(s))
                    .unwrap_or_else(|| json!({ "sessionId": session_id }))
            }
            ("LocalAgentModeSessions", "sendMessage") => {
                if args.is_empty() {
                    return Value::Null;
                }
                let sid = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let msg = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
                let msg_uuid = args.get(4).and_then(|v| v.as_str());
                self.engine.send_agent_message_with_uuid(sid, msg, msg_uuid);
                json!({ "dispatched": true })
            }
            ("LocalAgentModeSessions", "updateSession") => {
                let sid = args.first().and_then(|v| v.as_str()).unwrap_or("");
                if !sid.is_empty() {
                    if let Some(opts) = args.get(1).and_then(|v| v.as_object()) {
                        let mut found = false;
                        {
                            let mut sessions = self.engine.agent_sessions.write();
                            if let Some(sess) = sessions.get_mut(sid) {
                                found = true;
                                if let Some(t) = opts.get("title").and_then(|v| v.as_str()) {
                                    sess.title = t.to_string();
                                }
                                if let Some(st) = opts.get("isStarred").and_then(|v| v.as_bool()) {
                                    sess.is_starred = st;
                                }
                                if let Some(ar) = opts.get("isArchived").and_then(|v| v.as_bool()) {
                                    sess.is_archived = ar;
                                }
                                if let Some(sp_val) = opts.get("spaceId") {
                                    if sp_val.is_null() {
                                        sess.space_id = None;
                                    } else if let Some(sp) = sp_val.as_str() {
                                        if sp.is_empty() {
                                            sess.space_id = None;
                                        } else {
                                            sess.space_id = Some(sp.to_string());
                                        }
                                    }
                                }
                                if let Some(pm) =
                                    opts.get("permissionMode").and_then(|v| v.as_str())
                                {
                                    sess.permission_mode = pm.to_string();
                                }
                                if let Some(folders) =
                                    opts.get("userSelectedFolders").and_then(|v| v.as_array())
                                {
                                    sess.user_selected_folders = folders
                                        .iter()
                                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                        .collect();
                                }
                                if let Some(projs) = opts
                                    .get("userSelectedProjectUuids")
                                    .and_then(|v| v.as_array())
                                {
                                    sess.user_selected_project_uuids = projs
                                        .iter()
                                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                        .collect();
                                }
                                sess.last_activity_at = now_ms;
                                self.engine.persist_agent_session(sess);
                            }
                        }
                        if found {
                            self.engine.emit_eipc(
                                "quixi.web",
                                "LocalAgentModeSessions",
                                "onEvent",
                                json!({
                                    "type": "session_updated",
                                    "sessionId": sid
                                }),
                            );
                        }
                    }
                }
                Value::Null
            }
            ("LocalAgentModeSessions", "setModel") => {
                let sid = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let requested = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
                let selected = self.engine.provider_manager.select_model(requested);
                let mut found = false;
                if let Some(sess) = self.engine.agent_sessions.write().get_mut(sid) {
                    found = true;
                    sess.model = selected;
                    self.engine.persist_agent_session(sess);
                }
                if found {
                    self.engine.emit_eipc(
                        "quixi.web",
                        "LocalAgentModeSessions",
                        "onEvent",
                        json!({
                            "type": "session_updated",
                            "sessionId": sid
                        }),
                    );
                }
                Value::Null
            }
            ("LocalAgentModeSessions", "setEffort") => {
                let sid = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let effort = args.get(1).and_then(|v| v.as_str()).unwrap_or("medium");
                let mut found = false;
                if let Some(sess) = self.engine.agent_sessions.write().get_mut(sid) {
                    found = true;
                    sess.effort_level = effort.to_string();
                    self.engine.persist_agent_session(sess);
                }
                if found {
                    self.engine.emit_eipc(
                        "quixi.web",
                        "LocalAgentModeSessions",
                        "onEvent",
                        json!({
                            "type": "session_updated",
                            "sessionId": sid
                        }),
                    );
                }
                Value::Null
            }
            ("LocalAgentModeSessions", "getToolPermissions") | ("LocalSessions", "getToolPermissions") => {
                self.engine.pending_tool_approvals(args.first().and_then(Value::as_str).unwrap_or(""))
            }
            ("LocalAgentModeSessions", "resolveToolPermission") | ("LocalSessions", "resolveToolPermission") => {
                self.engine.resolve_tool_approval(args.first().and_then(Value::as_str).unwrap_or(""),
                    args.get(1).and_then(Value::as_str).unwrap_or(""),args.get(2).and_then(Value::as_str).unwrap_or(""))
            }
            ("LocalAgentModeSessions", "setPermissionMode") => {
                let sid = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let mode = args.get(1).and_then(|v| v.as_str()).unwrap_or("default");
                let mut found = false;
                if let Some(sess) = self.engine.agent_sessions.write().get_mut(sid) {
                    found = true;
                    sess.permission_mode = mode.to_string();
                    self.engine.persist_agent_session(sess);
                }
                if found {
                    self.engine.emit_eipc("quixi.web", "LocalAgentModeSessions", "onEvent",
                        json!({"type":"permission_mode_changed","sessionId":sid,"permissionMode":mode}));
                    self.engine.emit_eipc(
                        "quixi.web",
                        "LocalAgentModeSessions",
                        "onEvent",
                        json!({
                            "type": "session_updated",
                            "sessionId": sid
                        }),
                    );
                }
                json!(found)
            }
            ("LocalAgentModeSessions", "stop") | ("LocalAgentModeSessions", "stopTask") => {
                let sid = args.first().and_then(|v| v.as_str()).unwrap_or("");
                self.engine.cancel_turn(sid);
                let mut found = false;
                if let Some(sess) = self.engine.agent_sessions.write().get_mut(sid) {
                    found = true;
                    sess.is_running = false;
                    self.engine.persist_agent_session(sess);
                }
                if found {
                    self.engine.emit_eipc(
                        "quixi.web",
                        "LocalAgentModeSessions",
                        "onEvent",
                        json!({
                            "type": "session_updated",
                            "sessionId": sid
                        }),
                    );
                }
                if found {
                    self.engine.emit_eipc("quixi.web", "LocalAgentModeSessions", "onEvent",
                        json!({"type":"close","sessionId":sid,"code":0}));
                }
                Value::Null
            }
            ("LocalAgentModeSessions", "archive") => {
                let sid = args.first().and_then(|v| v.as_str()).unwrap_or("");
                self.engine.cancel_turn(sid);
                let mut found = false;
                if let Some(sess) = self.engine.agent_sessions.write().get_mut(sid) {
                    found = true;
                    sess.is_archived = true;
                    sess.is_running = false;
                    self.engine.persist_agent_session(sess);
                }
                if found {
                    self.engine.emit_eipc(
                        "quixi.web",
                        "LocalAgentModeSessions",
                        "onEvent",
                        json!({
                            "type": "archived",
                            "sessionId": sid
                        }),
                    );
                }
                Value::Null
            }
            ("LocalAgentModeSessions", "unarchive") => {
                let sid = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let mut found = false;
                if let Some(sess) = self.engine.agent_sessions.write().get_mut(sid) {
                    found = true;
                    sess.is_archived = false;
                    self.engine.persist_agent_session(sess);
                }
                if found {
                    self.engine.emit_eipc(
                        "quixi.web",
                        "LocalAgentModeSessions",
                        "onEvent",
                        json!({
                            "type": "session_updated",
                            "sessionId": sid
                        }),
                    );
                }
                Value::Null
            }
            ("LocalAgentModeSessions", "delete") => {
                let sid = args.first().and_then(|v| v.as_str()).unwrap_or("");
                self.engine.agent_sessions.write().remove(sid);
                self.engine.chat_conversations.write().remove(sid);
                let _ = fs::remove_file(
                    self.engine
                        .data_dir
                        .join("sessions")
                        .join(format!("{}.json", sid)),
                );
                if !sid.is_empty() {
                    self.engine.emit_eipc(
                        "quixi.web",
                        "LocalAgentModeSessions",
                        "onEvent",
                        json!({
                            "type": "deleted",
                            "sessionId": sid
                        }),
                    );
                }
                Value::Null
            }
            ("LocalAgentModeSessions", "getDefaultWorkspaceFolders") => {
                // Trust is an access grant, not an implicit selection of every project.
                json!([quixi_proto::paths::default_workspace_str()])
            }
            ("LocalAgentModeSessions", "isFolderTrusted") => json!(true),
            ("LocalAgentModeSessions", "getLocalMcpServers")
            | ("LocalSessions", "getLocalMcpServers") => {
                json!(self.engine.mcp_host.list_local_mcp_servers())
            }
            ("LocalAgentModeSessions", "getDirectMcpServerStatuses") => json!([]),
            ("LocalSessions", "getSessionCliMcpServers") => json!([]),
            ("LocalAgentModeSessions", "mcpCallTool")
            | ("LocalSessions", "mcpCallTool") => {
                if args.is_empty() {
                    json!({ "content": [], "isError": false })
                } else {
                    let (server, tool, tool_args) = if args.len() >= 3 {
                        if args.len() >= 4 {
                            (
                                args.get(1)
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("quixi-workspace"),
                                args.get(2).and_then(|v| v.as_str()).unwrap_or(""),
                                args.get(3).cloned().unwrap_or_else(|| json!({})),
                            )
                        } else {
                            (
                                args.first()
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("quixi-workspace"),
                                args.get(1).and_then(|v| v.as_str()).unwrap_or(""),
                                args.get(2).cloned().unwrap_or_else(|| json!({})),
                            )
                        }
                    } else if let Some(obj) = args.first().and_then(|v| v.as_object()) {
                        (
                            obj.get("serverName")
                                .or_else(|| obj.get("server"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("quixi-workspace"),
                            obj.get("name")
                                .or_else(|| obj.get("toolName"))
                                .or_else(|| obj.get("tool"))
                                .and_then(|v| v.as_str())
                                .unwrap_or(""),
                            obj.get("arguments")
                                .or_else(|| obj.get("args"))
                                .cloned()
                                .unwrap_or_else(|| json!({})),
                        )
                    } else {
                        ("quixi-workspace", "", json!({}))
                    };
                    if tool.is_empty() {
                        json!({ "content": [], "isError": false })
                    } else {
                        let out = self
                            .engine
                            .mcp_host
                            .call_tool(
                                server,
                                tool,
                                &tool_args,
                                Path::new(quixi_proto::paths::default_workspace_str()),
                            )
                            .unwrap_or_else(|e| {
                                json!({
                                    "content": [{ "type": "text", "text": e.to_string() }],
                                    "isError": true
                                })
                            });
                        self.engine
                            .emit("quixi:browser-updated", quixi_platform::BrowserUse::get_full_state());
                        out
                    }
                }
            }
            ("LocalAgentModeSessions", "directMcpCallTool")
            | ("CoworkArtifactBridge", "callMcpTool") => {
                if args.is_empty() {
                    json!({ "content": [], "isError": false })
                } else {
                    let (server, tool, tool_args) =
                        if let Some(obj) = args.first().and_then(|v| v.as_object()) {
                            (
                                obj.get("serverName")
                                    .or_else(|| obj.get("server"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("quixi-workspace"),
                                obj.get("name")
                                    .or_else(|| obj.get("toolName"))
                                    .or_else(|| obj.get("tool"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or(""),
                                obj.get("arguments")
                                    .or_else(|| obj.get("args"))
                                    .cloned()
                                    .unwrap_or_else(|| json!({})),
                            )
                        } else {
                            (
                                args.first()
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("quixi-workspace"),
                                args.get(1).and_then(|v| v.as_str()).unwrap_or(""),
                                args.get(2).cloned().unwrap_or_else(|| json!({})),
                            )
                        };
                    if tool.is_empty() {
                        json!({ "content": [], "isError": false })
                    } else {
                        let out = self
                            .engine
                            .mcp_host
                            .call_tool(
                                server,
                                tool,
                                &tool_args,
                                Path::new(quixi_proto::paths::default_workspace_str()),
                            )
                            .unwrap_or_else(|e| {
                                json!({
                                    "content": [{ "type": "text", "text": e.to_string() }],
                                    "isError": true
                                })
                            });
                        self.engine
                            .emit("quixi:browser-updated", quixi_platform::BrowserUse::get_full_state());
                        out
                    }
                }
            }
            ("LocalSessions", "sshHostMcpCallTool") => {
                if args.is_empty() {
                    json!({})
                } else {
                    let server = args
                        .get(1)
                        .and_then(|v| v.as_str())
                        .unwrap_or("quixi-workspace");
                    let tool = args.get(2).and_then(|v| v.as_str()).unwrap_or("");
                    let tool_args = args.get(3).cloned().unwrap_or_else(|| json!({}));
                    if tool.is_empty() {
                        json!({})
                    } else {
                        let res = self
                            .engine
                            .mcp_host
                            .call_tool(
                                server,
                                tool,
                                &tool_args,
                                Path::new(quixi_proto::paths::default_workspace_str()),
                            )
                            .unwrap_or_else(|e| {
                                json!({
                                    "content": [{ "type": "text", "text": e.to_string() }],
                                    "isError": true
                                })
                            });
                        json!({ "result": res })
                    }
                }
            }
            ("LocalAgentModeSessions", "mcpListResources")
            | ("LocalSessions", "mcpListResources") => {
                let server = args
                    .get(1)
                    .or_else(|| args.first())
                    .and_then(|v| v.as_str())
                    .unwrap_or("quixi-workspace");
                json!(self.engine.mcp_host.list_resources(server))
            }
            ("LocalAgentModeSessions", "directMcpListResources") => {
                let server = args
                    .first()
                    .and_then(|v| v.as_str())
                    .unwrap_or("quixi-workspace");
                json!(self.engine.mcp_host.list_resources(server))
            }
            ("LocalAgentModeSessions", "mcpReadResource")
            | ("LocalSessions", "mcpReadResource")
            | ("LocalSessions", "sshHostMcpReadResource") => {
                let server = args
                    .get(1)
                    .and_then(|v| v.as_str())
                    .unwrap_or("quixi-workspace");
                let uri = args
                    .get(2)
                    .and_then(|v| v.as_str())
                    .unwrap_or("quixi://workspace/readme");
                self.engine.mcp_host.read_resource(server, uri)
            }
            ("LocalAgentModeSessions", "directMcpReadResource") => {
                let server = args
                    .first()
                    .and_then(|v| v.as_str())
                    .unwrap_or("quixi-workspace");
                let uri = args
                    .get(1)
                    .and_then(|v| v.as_str())
                    .unwrap_or("quixi://workspace/readme");
                self.engine.mcp_host.read_resource(server, uri)
            }
            ("LocalAgentModeSessions", "reconnectMcpServer")
            | ("LocalSessions", "reconnectMcpServer")
            | ("LocalAgentModeSessions", "toggleMcpServer")
            | ("LocalSessions", "toggleMcpServer")
            | ("LocalAgentModeSessions", "authenticateMcpServer")
            | ("LocalSessions", "authenticateMcpServer")
            | ("LocalAgentModeSessions", "clearMcpServerAuth")
            | ("LocalSessions", "clearMcpServerAuth")
            | ("LocalAgentModeSessions", "authorizeDirectMcpServer")
            | ("LocalAgentModeSessions", "authorizeDirectMcpServerWithAccountPicker")
            | ("LocalAgentModeSessions", "disconnectDirectMcpServerV2") => {
                json!({ "ok": true })
            }
            ("LocalAgentModeSessions", "disconnectDirectMcpServer") => json!(true),
            ("LocalAgentModeSessions", "setMcpServers")
            | ("LocalSessions", "setMcpServers")
            | ("LocalAgentModeSessions", "replaceEnabledMcpTools")
            | ("LocalSessions", "replaceEnabledMcpTools")
            | ("LocalAgentModeSessions", "replaceRemoteMcpServers")
            | ("LocalSessions", "replaceRemoteMcpServers") => {
                json!({ "enabledMcpTools": {} })
            }
            ("LocalAgentModeSessions", "listLocalSkills") => {
                json!(self.engine.list_local_skills())
            }
            ("LocalAgentModeSessions", "getLocalSkillFiles") => {
                let skill_id = args.first().and_then(|v| v.as_str()).unwrap_or("docx");
                json!(self.engine.get_local_skill_files(skill_id))
            }
            ("LocalAgentModeSessions", "saveLocalSkill") => {
                if args.is_empty() {
                    return Value::Null;
                }
                let opts = args.first().cloned().unwrap_or_else(|| json!({}));
                self.engine.save_local_skill(&opts)
            }
            ("LocalAgentModeSessions", "deleteLocalSkill") => {
                let skill_id = args.first().and_then(|v| v.as_str()).unwrap_or("");
                self.engine.delete_local_skill(skill_id)
            }
            ("LocalAgentModeSessions", "searchSessions") => {
                let q = args.first().and_then(|v| v.as_str()).unwrap_or("");
                json!(self.engine.search_local_sessions(q, 50))
            }
            ("LocalAgentModeSessions", "getBridgeConsent") => json!(false),
            ("LocalAgentModeSessions", "getSessionsBridgeEnabled") => json!(true),
            ("LocalAgentModeSessions", "browseAndGrantRemoteSessionFolder") => {
                let prompt = args.first().and_then(|v| v.as_str());
                if let Some(folder) = quixi_platform::ComputerUse::pick_folder(prompt) {
                    let mut trusted = self.engine.trusted_folders.write();
                    if !trusted.contains(&folder) {
                        trusted.push(folder.clone());
                    }
                    json!({ "ok": true, "folderPath": folder })
                } else {
                    json!({ "ok": false, "folderPath": null })
                }
            }
            ("LocalAgentModeSessions", "browseAndAddTrustedFolder") => {
                let prompt = args.first().and_then(|v| v.as_str());
                if let Some(folder) = quixi_platform::ComputerUse::pick_folder(prompt) {
                    let mut trusted = self.engine.trusted_folders.write();
                    if !trusted.contains(&folder) {
                        trusted.push(folder.clone());
                    }
                    json!(folder)
                } else {
                    Value::Null
                }
            }
            ("LocalAgentModeSessions", "addTrustedFolder") => {
                if let Some(folder) = args.first().and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                    let mut trusted = self.engine.trusted_folders.write();
                    if !trusted.contains(&folder.to_string()) {
                        trusted.push(folder.to_string());
                    }
                }
                json!(true)
            }
            ("LocalAgentModeSessions", "removeTrustedFolder") => {
                if let Some(folder) = args.first().and_then(|v| v.as_str()) {
                    self.engine.trusted_folders.write().retain(|f| f != folder);
                }
                json!(true)
            }
            ("LocalAgentModeSessions", "setDraftSessionFolders") => {
                if let Some(arr) = args.first().and_then(|v| v.as_array()) {
                    let mut trusted = self.engine.trusted_folders.write();
                    for item in arr {
                        if let Some(p) = item.as_str().filter(|s| !s.is_empty()) {
                            if !trusted.contains(&p.to_string()) {
                                trusted.push(p.to_string());
                            }
                        }
                    }
                }
                json!(true)
            }
            ("LocalAgentModeSessions", "addFolderToSession") => {
                let sid = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let folder = args
                    .get(1)
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| quixi_platform::ComputerUse::pick_folder(Some("Add folder to session")));
                if let Some(folder_path) = folder {
                    {
                        let mut trusted = self.engine.trusted_folders.write();
                        if !trusted.contains(&folder_path) {
                            trusted.push(folder_path.clone());
                        }
                    }
                    if let Some(sess) = self.engine.agent_sessions.write().get_mut(sid) {
                        if !sess.user_selected_folders.contains(&folder_path) {
                            sess.user_selected_folders.push(folder_path.clone());
                        }
                        sess.cwd = folder_path.clone();
                        self.engine.persist_agent_session(sess);
                    }
                    if !sid.is_empty() {
                        self.engine.emit_eipc(
                            "quixi.web",
                            "LocalAgentModeSessions",
                            "onEvent",
                            json!({
                                "type": "session_updated",
                                "sessionId": sid
                            }),
                        );
                    }
                    json!({ "ok": true, "folderPath": folder_path })
                } else {
                    json!({ "ok": false, "folderPath": null })
                }
            }
            ("LocalAgentModeSessions", "grantRemoteSessionFolders") => {
                let folders = args.get(1).cloned().unwrap_or_else(|| json!([]));
                json!({ "granted": folders, "rejected": [] })
            }

            // --- LocalSessions (Code tab / CCD / Git / Terminal PTY / CLI Sessions) ---
            ("LocalSessions", "getAll") => {
                let mut list: Vec<_> = self
                    .engine
                    .code_sessions
                    .read()
                    .values()
                    .filter(|s| !s.session_id.trim().is_empty())
                    .cloned()
                    .collect();
                list.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
                json!(list)
            }
            ("LocalSessions", "getSessionList") => {
                let mut list: Vec<_> = self
                    .engine
                    .code_sessions
                    .read()
                    .values()
                    .filter(|s| !s.session_id.trim().is_empty())
                    .cloned()
                    .collect();
                list.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
                json!({
                    "sessions": list,
                    "loadGeneration": 1
                })
            }
            ("LocalSessions", "loadArchivedSessions") => {
                let mut list: Vec<_> = self
                    .engine
                    .code_sessions
                    .read()
                    .values()
                    .filter(|s| !s.session_id.trim().is_empty() && s.is_archived)
                    .cloned()
                    .collect();
                list.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
                let count = list.len();
                json!({
                    "sessions": list,
                    "totalCount": count
                })
            }
            ("LocalSessions", "getSession") => {
                let sid = args.first().and_then(|v| v.as_str()).unwrap_or("");
                if sid.trim().is_empty() {
                    Value::Null
                } else {
                    self.engine
                        .code_sessions
                        .read()
                        .get(sid)
                        .map(|s| json!(s))
                        .unwrap_or(Value::Null)
                }
            }
            ("LocalSessions", "start") => {
                if args.is_empty() {
                    return Value::Null;
                }
                let opts = args.first().cloned().unwrap_or_else(|| json!({}));
                let session_id = self.engine.start_code_session(&opts);
                self.engine
                    .code_sessions
                    .read()
                    .get(&session_id)
                    .map(|s| json!(s))
                    .unwrap_or_else(|| json!({ "sessionId": session_id }))
            }
            ("LocalSessions", "sendMessage") => {
                if args.is_empty() {
                    return Value::Null;
                }
                let sid = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let msg = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
                let msg_uuid = args.get(7).and_then(|v| v.as_str());
                self.engine.send_code_message_with_uuid(sid, msg, msg_uuid);
                Value::Null
            }
            ("LocalSessions", "updateSession") => {
                let sid = args.first().and_then(|v| v.as_str()).unwrap_or("");
                if !sid.is_empty() {
                    if let Some(opts) = args.get(1).and_then(|v| v.as_object()) {
                        let mut updated_sess = None;
                        if let Some(sess) = self.engine.code_sessions.write().get_mut(sid) {
                            if let Some(t) = opts.get("title").and_then(|v| v.as_str()) {
                                sess.title = t.to_string();
                            }
                            if let Some(st) = opts.get("isStarred").and_then(|v| v.as_bool()) {
                                sess.is_starred = st;
                            }
                            if let Some(ar) = opts.get("isArchived").and_then(|v| v.as_bool()) {
                                sess.is_archived = ar;
                            }
                            if let Some(pm) = opts.get("permissionMode").and_then(|v| v.as_str()) {
                                sess.permission_mode = pm.to_string();
                            }
                            if let Some(cwd) = opts.get("cwd").and_then(|v| v.as_str()) {
                                sess.cwd = cwd.to_string();
                            }
                            if let Some(br) = opts.get("branch").and_then(|v| v.as_str()) {
                                sess.branch = Some(br.to_string());
                            }
                            sess.last_activity_at = now_ms;
                            self.engine.persist_code_session(sess);
                            updated_sess = Some(sess.clone());
                        }
                        if let Some(s) = updated_sess {
                            self.engine.emit_eipc(
                                "quixi.web",
                                "LocalSessions",
                                "onEvent",
                                json!({
                                    "type": "session_updated",
                                    "sessionId": sid,
                                    "session": s
                                }),
                            );
                        }
                    }
                }
                Value::Null
            }
            ("LocalSessions", "setModel") => {
                let sid = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let requested = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
                let selected = self.engine.provider_manager.select_model(requested);
                let mut updated_sess = None;
                if let Some(sess) = self.engine.code_sessions.write().get_mut(sid) {
                    sess.model = selected;
                    self.engine.persist_code_session(sess);
                    updated_sess = Some(sess.clone());
                }
                if let Some(s) = updated_sess {
                    self.engine.emit_eipc(
                        "quixi.web",
                        "LocalSessions",
                        "onEvent",
                        json!({
                            "type": "session_updated",
                            "sessionId": sid,
                            "session": s
                        }),
                    );
                }
                Value::Null
            }
            ("LocalSessions", "setEffort") => {
                let sid = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let effort = args.get(1).and_then(|v| v.as_str()).unwrap_or("medium");
                let mut updated_sess = None;
                if let Some(sess) = self.engine.code_sessions.write().get_mut(sid) {
                    sess.effort = effort.to_string();
                    self.engine.persist_code_session(sess);
                    updated_sess = Some(sess.clone());
                }
                if let Some(s) = updated_sess {
                    self.engine.emit_eipc(
                        "quixi.web",
                        "LocalSessions",
                        "onEvent",
                        json!({
                            "type": "session_updated",
                            "sessionId": sid,
                            "session": s
                        }),
                    );
                }
                Value::Null
            }
            ("LocalSessions", "setPermissionMode") => {
                let sid = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let mode = args.get(1).and_then(|v| v.as_str()).unwrap_or("default");
                let mut updated_sess = None;
                if let Some(sess) = self.engine.code_sessions.write().get_mut(sid) {
                    sess.permission_mode = mode.to_string();
                    self.engine.persist_code_session(sess);
                    updated_sess = Some(sess.clone());
                }
                if let Some(s) = updated_sess {
                    self.engine.emit_eipc(
                        "quixi.web",
                        "LocalSessions",
                        "onEvent",
                        json!({
                            "type": "permission_mode_changed",
                            "sessionId": sid,
                            "permissionMode": mode
                        }),
                    );
                    self.engine.emit_eipc(
                        "quixi.web",
                        "LocalSessions",
                        "onEvent",
                        json!({
                            "type": "session_updated",
                            "sessionId": sid,
                            "session": s
                        }),
                    );
                }
                Value::Null
            }
            ("LocalSessions", "stop")
            | ("LocalSessions", "interrupt")
            | ("LocalSessions", "stopTask") => {
                let sid = args.first().and_then(|v| v.as_str()).unwrap_or("");
                self.engine.cancel_turn(sid);
                let mut updated_sess = None;
                if let Some(sess) = self.engine.code_sessions.write().get_mut(sid) {
                    sess.turn_running = false;
                    sess.is_running = false;
                    self.engine.persist_code_session(sess);
                    updated_sess = Some(sess.clone());
                }
                if let Some(s) = updated_sess {
                    self.engine.emit_eipc(
                        "quixi.web",
                        "LocalSessions",
                        "onEvent",
                        json!({
                            "type": "stopped",
                            "sessionId": sid,
                            "session": s
                        }),
                    );
                    self.engine.emit_eipc(
                        "quixi.web",
                        "LocalSessions",
                        "onEvent",
                        json!({
                            "type": "session_updated",
                            "sessionId": sid,
                            "session": s
                        }),
                    );
                }
                Value::Null
            }
            ("LocalSessions", "clearSession") => {
                let sid = args.first().and_then(|v| v.as_str()).unwrap_or("");
                if let Some(msgs) = self.engine.code_transcripts.write().get_mut(sid) {
                    msgs.clear();
                }
                if !sid.is_empty() {
                    self.engine.emit_eipc(
                        "quixi.web",
                        "LocalSessions",
                        "onEvent",
                        json!({
                            "type": "cleared",
                            "sessionId": sid
                        }),
                    );
                }
                Value::Null
            }
            ("LocalSessions", "archive") => {
                let sid = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let mut updated_sess = None;
                if let Some(sess) = self.engine.code_sessions.write().get_mut(sid) {
                    sess.is_archived = true;
                    sess.turn_running = false;
                    sess.is_running = false;
                    self.engine.persist_code_session(sess);
                    updated_sess = Some(sess.clone());
                }
                if let Some(s) = updated_sess {
                    self.engine.emit_eipc(
                        "quixi.web",
                        "LocalSessions",
                        "onEvent",
                        json!({
                            "type": "archived",
                            "sessionId": sid,
                            "session": s
                        }),
                    );
                }
                Value::Null
            }
            ("LocalSessions", "unarchive") => {
                let sid = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let mut updated_sess = None;
                if let Some(sess) = self.engine.code_sessions.write().get_mut(sid) {
                    sess.is_archived = false;
                    self.engine.persist_code_session(sess);
                    updated_sess = Some(sess.clone());
                }
                if let Some(s) = updated_sess {
                    self.engine.emit_eipc(
                        "quixi.web",
                        "LocalSessions",
                        "onEvent",
                        json!({
                            "type": "unarchived",
                            "sessionId": sid,
                            "session": s
                        }),
                    );
                    self.engine.emit_eipc(
                        "quixi.web",
                        "LocalSessions",
                        "onEvent",
                        json!({
                            "type": "session_updated",
                            "sessionId": sid,
                            "session": s
                        }),
                    );
                }
                Value::Null
            }
            ("LocalSessions", "delete") => {
                let sid = args.first().and_then(|v| v.as_str()).unwrap_or("");
                self.engine.code_sessions.write().remove(sid);
                self.engine.code_transcripts.write().remove(sid);
                let _ = fs::remove_file(
                    self.engine
                        .data_dir
                        .join("code_sessions")
                        .join(format!("{}.json", sid)),
                );
                if !sid.is_empty() {
                    self.engine.emit_eipc(
                        "quixi.web",
                        "LocalSessions",
                        "onEvent",
                        json!({
                            "type": "deleted",
                            "sessionId": sid
                        }),
                    );
                }
                Value::Null
            }
            ("LocalSessions", "getTranscript") => {
                let sid = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let msgs = self
                    .engine
                    .code_transcripts
                    .read()
                    .get(sid)
                    .cloned()
                    .unwrap_or_default();
                json!(msgs)
            }
            ("LocalSessions", "getTranscriptTail") => {
                let sid = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let msgs = self
                    .engine
                    .code_transcripts
                    .read()
                    .get(sid)
                    .cloned()
                    .unwrap_or_default();
                json!({ "messages": msgs, "hasMore": false })
            }
            ("LocalSessions", "getBusyShellPtyKeys") => json!({
                "live": [],
                "probed": true,
                "unknown": [],
                "busy": self.engine.shell_host.get_busy_pty_keys(),
                "tabs": []
            }),
            ("LocalSessions", "getTerminalShells") => {
                quixi_workspace::ShellHostEngine::get_terminal_shells()
            }
            ("LocalSessions", "startShellPty") => {
                let key = args.first().and_then(|v| v.as_str()).unwrap_or("default");
                self.engine
                    .shell_host
                    .start_pty(key, Path::new(quixi_proto::paths::default_workspace_str()))
            }
            ("LocalSessions", "writeShellPty") => {
                let key = args.first().and_then(|v| v.as_str()).unwrap_or("default");
                let data = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
                self.engine.shell_host.write_pty(key, data);
                Value::Null
            }
            ("LocalSessions", "resizeShellPty") => {
                let key = args.first().and_then(|v| v.as_str()).unwrap_or("default");
                let cols = args.get(1).and_then(|v| v.as_u64()).unwrap_or(120) as u16;
                let rows = args.get(2).and_then(|v| v.as_u64()).unwrap_or(32) as u16;
                self.engine.shell_host.resize_pty(key, cols, rows);
                Value::Null
            }
            ("LocalSessions", "stopShellPty") => {
                let key = args.first().and_then(|v| v.as_str()).unwrap_or("default");
                self.engine.shell_host.stop_pty(key);
                Value::Null
            }
            ("LocalSessions", "getSSHHostCommands") => json!({ "entries": [] }),
            ("LocalSessions", "getCodeStats") => json!({
                "totalSessions": self.engine.code_sessions.read().len(),
                "totalMessages": 0,
                "activeDays": 1,
                "firstSessionDate": null,
                "lastSessionDate": null,
                "peakActivityHour": null,
                "streaks": { "currentStreak": 1, "longestStreak": 1 },
                "dailyActivity": [],
                "dailyModelTokens": [],
                "modelUsage": {}
            }),
            ("LocalSessions", "getDetectedProjects") => {
                let mut seen = std::collections::HashSet::new();
                let mut projects = Vec::new();
                for sp in self.engine.cowork_spaces.read().values() {
                    for f in &sp.folders {
                        if seen.insert(f.path.clone()) {
                            projects.push(json!({
                                "path": f.path,
                                "name": f.name,
                                "lastActivity": sp.updated_at,
                                "lastUsedAt": sp.updated_at
                            }));
                        }
                    }
                }
                for tf in self.engine.trusted_folders.read().iter() {
                    if seen.insert(tf.clone()) {
                        let name = Path::new(tf)
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or(tf);
                        projects.push(json!({
                            "path": tf,
                            "name": name,
                            "lastActivity": now_ms,
                            "lastUsedAt": now_ms
                        }));
                    }
                }
                json!(projects)
            }
            ("LocalSessions", "listDirectoryAtCwd")
            | ("LocalSessions", "listSessionDirectory") => {
                let dir_str = args
                    .first()
                    .and_then(|v| v.as_str())
                    .unwrap_or(quixi_proto::paths::default_workspace_str());
                let entries =
                    quixi_workspace::FileIndexEngine::list_directory_children(Path::new(dir_str));
                json!(entries)
            }
            ("LocalSessions", "readFileAtCwd") | ("LocalSessions", "readSessionFile") => {
                let path_str = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let contents = fs::read_to_string(path_str).unwrap_or_default();
                json!({ "contents": contents, "absPath": path_str })
            }
            ("LocalSessions", "getLocalBranches")
            | ("LocalSessions", "getSSHLocalBranches")
            | ("LocalSessions", "getRemoteLocalBranches") => {
                let cwd_str = args
                    .first()
                    .and_then(|v| v.as_str())
                    .unwrap_or(quixi_proto::paths::default_workspace_str());
                json!(quixi_workspace::GitPoolEngine::get_local_branches(
                    Path::new(cwd_str)
                ))
            }
            ("LocalSessions", "getGitInfo")
            | ("LocalSessions", "getSSHGitInfo")
            | ("LocalSessions", "getRemoteGitInfo") => {
                let cwd_str = args
                    .first()
                    .and_then(|v| v.as_str())
                    .unwrap_or(quixi_proto::paths::default_workspace_str());
                let repo = quixi_workspace::GitPoolEngine::inspect_repo(Path::new(cwd_str));
                json!({
                    "repo": repo.root_path.unwrap_or_else(|| cwd_str.to_string()),
                    "branch": repo.branch.unwrap_or_else(|| "main".to_string())
                })
            }
            ("LocalSessions", "getGitDiff")
            | ("LocalSessions", "getCommitDiff")
            | ("LocalSessions", "getSSHGitDiff")
            | ("LocalSessions", "getRemoteGitDiff") => {
                let cwd_str = args
                    .first()
                    .and_then(|v| v.as_str())
                    .unwrap_or(quixi_proto::paths::default_workspace_str());
                quixi_workspace::GitPoolEngine::get_git_diff_structured(Path::new(cwd_str))
            }
            ("LocalSessions", "getGitDiffStats")
            | ("LocalSessions", "getSSHGitDiffStats")
            | ("LocalSessions", "getRemoteGitDiffStats") => {
                let cwd_str = args
                    .first()
                    .and_then(|v| v.as_str())
                    .unwrap_or(quixi_proto::paths::default_workspace_str());
                quixi_workspace::GitPoolEngine::get_git_diff_stats(Path::new(cwd_str))
            }
            ("LocalSessions", "getGitCommits")
            | ("LocalSessions", "getSSHGitCommits")
            | ("LocalSessions", "getRemoteGitCommits") => {
                let cwd_str = args
                    .first()
                    .and_then(|v| v.as_str())
                    .unwrap_or(quixi_proto::paths::default_workspace_str());
                json!(quixi_workspace::GitPoolEngine::get_git_commits(
                    Path::new(cwd_str),
                    25
                ))
            }
            ("LocalSessions", "getWorkingTreeStatus") => {
                let cwd_str = args
                    .first()
                    .and_then(|v| v.as_str())
                    .unwrap_or(quixi_proto::paths::default_workspace_str());
                quixi_workspace::GitPoolEngine::get_working_tree_status(Path::new(cwd_str))
            }
            ("LocalSessions", "getDiffFileContent") => {
                let rel_path = args.get(1).and_then(|v| v.as_str()).unwrap_or("Cargo.toml");
                quixi_workspace::GitPoolEngine::get_diff_file_content(
                    Path::new(quixi_proto::paths::default_workspace_str()),
                    rel_path,
                )
            }
            ("LocalSessions", "runBashCommand") => {
                let cmd = args.first().and_then(|v| v.as_str()).unwrap_or("pwd");
                quixi_workspace::ShellHostEngine::run_bash_command(
                    Path::new(quixi_proto::paths::default_workspace_str()),
                    cmd,
                )
                .unwrap_or_else(|_| json!({ "stdout": "", "stderr": "", "exitCode": 1 }))
            }
            ("LocalSessions", "listCliSessions")
            | ("LocalSessions", "listResumableCliSessions") => json!([]),
            ("LocalSessions", "readCliTranscript") => {
                let sid = args.first().and_then(|v| v.as_str()).unwrap_or("");
                quixi_workspace::TranscriptSearchEngine::read_cli_transcript(sid)
            }
            ("LocalSessions", "searchSessions") => {
                let q = args.first().and_then(|v| v.as_str()).unwrap_or("");
                json!(self.engine.search_local_sessions(q, 50))
            }

            // --- WorkspaceCode ---
            ("WorkspaceCode", "getStatus") => json!({
                "status": "ready",
                "version": "2.1.300"
            }),
            ("WorkspaceCode", "prepare") => json!({ "ready": true }),

            // --- CoworkSpaces ---
            ("CoworkSpaces", "getAllSpaces") => {
                let mut list: Vec<_> = self
                    .engine
                    .cowork_spaces
                    .read()
                    .values()
                    .cloned()
                    .collect();
                list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
                json!(list)
            }
            ("CoworkSpaces", "getSpacesStoreState") => json!({ "state": "loaded" }),
            ("CoworkSpaces", "getSpace") => {
                let id = args.first().and_then(|v| v.as_str()).unwrap_or("");
                self.engine
                    .cowork_spaces
                    .read()
                    .get(id)
                    .map(|s| json!(s))
                    .unwrap_or(Value::Null)
            }
            ("CoworkSpaces", "createSpaceFolder") => {
                if args.is_empty() {
                    return Value::Null;
                }
                let location = args
                    .first()
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or(quixi_proto::paths::default_workspace_str());
                let raw_name = args
                    .get(1)
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or("New Project");
                let safe_name: String = raw_name
                    .chars()
                    .map(|c| if c == '/' || c == '\\' || c == ':' { '-' } else { c })
                    .collect();
                let full_path = Path::new(location).join(safe_name.trim());
                let _ = fs::create_dir_all(&full_path);
                let full_str = full_path.to_string_lossy().to_string();
                {
                    let mut trusted = self.engine.trusted_folders.write();
                    if !trusted.contains(&full_str) {
                        trusted.push(full_str.clone());
                    }
                }
                json!(full_str)
            }
            ("CoworkSpaces", "createSpace") => {
                if args.is_empty() {
                    return Value::Null;
                }
                let opts = args.first().cloned().unwrap_or_else(|| json!({}));
                let id = format!("space_{}", Uuid::new_v4());
                let name = opts
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("New Project")
                    .to_string();
                let description = opts
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let instructions = opts
                    .get("instructions")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let icon = opts
                    .get("icon")
                    .and_then(|v| v.as_str())
                    .unwrap_or("folder")
                    .to_string();
                let color = opts
                    .get("color")
                    .and_then(|v| v.as_str())
                    .unwrap_or("clay")
                    .to_string();
                let sp = CoworkSpace {
                    id: id.clone(),
                    name,
                    description,
                    instructions,
                    icon,
                    color,
                    created_at: now_ms,
                    updated_at: now_ms,
                    folders: vec![],
                    links: vec![],
                    projects: vec![],
                };
                self.engine.persist_cowork_space(&sp);
                self.engine
                    .cowork_spaces
                    .write()
                    .insert(id, sp.clone());
                self.engine.emit_eipc(
                    "quixi.web",
                    "CoworkSpaces",
                    "onSpaceEvent",
                    json!({ "type": "created", "space": sp }),
                );
                json!(sp)
            }
            ("CoworkSpaces", "updateSpace") => {
                let id = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let updates = args.get(1).cloned().unwrap_or_else(|| json!({}));
                let mut updated_sp = None;
                {
                    let mut spaces = self.engine.cowork_spaces.write();
                    if let Some(sp) = spaces.get_mut(id) {
                        if let Some(n) = updates.get("name").and_then(|v| v.as_str()) {
                            sp.name = n.to_string();
                        }
                        if let Some(d) = updates.get("description").and_then(|v| v.as_str()) {
                            sp.description = d.to_string();
                        }
                        if let Some(inst) = updates.get("instructions").and_then(|v| v.as_str()) {
                            sp.instructions = inst.to_string();
                        }
                        if let Some(ic) = updates.get("icon").and_then(|v| v.as_str()) {
                            sp.icon = ic.to_string();
                        }
                        if let Some(col) = updates.get("color").and_then(|v| v.as_str()) {
                            sp.color = col.to_string();
                        }
                        sp.updated_at = now_ms;
                        self.engine.persist_cowork_space(sp);
                        updated_sp = Some(sp.clone());
                    }
                }
                if let Some(sp) = updated_sp {
                    self.engine.emit_eipc(
                        "quixi.web",
                        "CoworkSpaces",
                        "onSpaceEvent",
                        json!({ "type": "updated", "space": sp }),
                    );
                    json!(sp)
                } else {
                    Value::Null
                }
            }
            ("CoworkSpaces", "deleteSpace") => {
                let id = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let removed = self.engine.cowork_spaces.write().remove(id);
                let _ = fs::remove_file(
                    self.engine
                        .data_dir
                        .join("spaces")
                        .join(format!("{}.json", id)),
                );
                if removed.is_some() || !id.is_empty() {
                    self.engine.emit_eipc(
                        "quixi.web",
                        "CoworkSpaces",
                        "onSpaceEvent",
                        json!({ "type": "deleted", "space": { "id": id } }),
                    );
                    json!(true)
                } else {
                    json!(false)
                }
            }
            ("CoworkSpaces", "addFolderToSpace") => {
                let id = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let folder_path = args
                    .get(1)
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or(quixi_proto::paths::default_workspace_str());
                {
                    let mut trusted = self.engine.trusted_folders.write();
                    if !trusted.contains(&folder_path.to_string()) {
                        trusted.push(folder_path.to_string());
                    }
                }
                let mut updated_sp = None;
                {
                    let mut spaces = self.engine.cowork_spaces.write();
                    if let Some(sp) = spaces.get_mut(id) {
                        if !sp.folders.iter().any(|f| f.path == folder_path) {
                            let name = Path::new(folder_path)
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| folder_path.to_string());
                            sp.folders.push(CoworkSpaceFolder {
                                path: folder_path.to_string(),
                                name,
                                added_at: now_ms,
                            });
                        }
                        sp.updated_at = now_ms;
                        self.engine.persist_cowork_space(sp);
                        updated_sp = Some(sp.clone());
                    }
                }
                if let Some(sp) = updated_sp {
                    self.engine.emit_eipc(
                        "quixi.web",
                        "CoworkSpaces",
                        "onSpaceEvent",
                        json!({ "type": "updated", "space": sp }),
                    );
                    return json!(sp);
                }
                Value::Null
            }
            ("CoworkSpaces", "removeFolderFromSpace") => {
                let id = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let folder_path = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
                let mut updated_sp = None;
                {
                    let mut spaces = self.engine.cowork_spaces.write();
                    if let Some(sp) = spaces.get_mut(id) {
                        sp.folders.retain(|f| f.path != folder_path);
                        sp.updated_at = now_ms;
                        self.engine.persist_cowork_space(sp);
                        updated_sp = Some(sp.clone());
                    }
                }
                if let Some(sp) = updated_sp {
                    self.engine.emit_eipc(
                        "quixi.web",
                        "CoworkSpaces",
                        "onSpaceEvent",
                        json!({ "type": "updated", "space": sp }),
                    );
                    return json!(sp);
                }
                Value::Null
            }
            ("CoworkSpaces", "addProjectToSpace") => {
                let id = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let proj_uuid = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
                let proj_name = self
                    .engine
                    .projects
                    .read()
                    .get(proj_uuid)
                    .and_then(|p| p.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Project")
                    .to_string();
                let mut updated_sp = None;
                {
                    let mut spaces = self.engine.cowork_spaces.write();
                    if let Some(sp) = spaces.get_mut(id) {
                        if !sp.projects.iter().any(|p| p.uuid == proj_uuid) {
                            sp.projects.push(CoworkSpaceProject {
                                uuid: proj_uuid.to_string(),
                                name: proj_name,
                            });
                        }
                        sp.updated_at = now_ms;
                        self.engine.persist_cowork_space(sp);
                        updated_sp = Some(sp.clone());
                    }
                }
                if let Some(sp) = updated_sp {
                    self.engine.emit_eipc(
                        "quixi.web",
                        "CoworkSpaces",
                        "onSpaceEvent",
                        json!({ "type": "updated", "space": sp }),
                    );
                    return json!(sp);
                }
                Value::Null
            }
            ("CoworkSpaces", "removeProjectFromSpace") => {
                let id = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let proj_uuid = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
                let mut updated_sp = None;
                {
                    let mut spaces = self.engine.cowork_spaces.write();
                    if let Some(sp) = spaces.get_mut(id) {
                        sp.projects.retain(|p| p.uuid != proj_uuid);
                        sp.updated_at = now_ms;
                        self.engine.persist_cowork_space(sp);
                        updated_sp = Some(sp.clone());
                    }
                }
                if let Some(sp) = updated_sp {
                    self.engine.emit_eipc(
                        "quixi.web",
                        "CoworkSpaces",
                        "onSpaceEvent",
                        json!({ "type": "updated", "space": sp }),
                    );
                    return json!(sp);
                }
                Value::Null
            }
            ("CoworkSpaces", "addLinkToSpace") => {
                let id = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let link_obj = args.get(1).cloned().unwrap_or_else(|| json!({}));
                let url = link_obj
                    .get("url")
                    .and_then(|v| v.as_str())
                    .or_else(|| args.get(1).and_then(|v| v.as_str()))
                    .unwrap_or("")
                    .to_string();
                let title = link_obj
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&url)
                    .to_string();
                let mut updated_sp = None;
                {
                    let mut spaces = self.engine.cowork_spaces.write();
                    if let Some(sp) = spaces.get_mut(id) {
                        sp.links.push(CoworkSpaceLink {
                            id: format!("link_{}", Uuid::new_v4()),
                            url,
                            title,
                        });
                        sp.updated_at = now_ms;
                        self.engine.persist_cowork_space(sp);
                        updated_sp = Some(sp.clone());
                    }
                }
                if let Some(sp) = updated_sp {
                    self.engine.emit_eipc(
                        "quixi.web",
                        "CoworkSpaces",
                        "onSpaceEvent",
                        json!({ "type": "updated", "space": sp }),
                    );
                    return json!(sp);
                }
                Value::Null
            }
            ("CoworkSpaces", "removeLinkFromSpace") => {
                let id = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let link_id = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
                let mut updated_sp = None;
                {
                    let mut spaces = self.engine.cowork_spaces.write();
                    if let Some(sp) = spaces.get_mut(id) {
                        sp.links.retain(|l| l.id != link_id && l.url != link_id);
                        sp.updated_at = now_ms;
                        self.engine.persist_cowork_space(sp);
                        updated_sp = Some(sp.clone());
                    }
                }
                if let Some(sp) = updated_sp {
                    self.engine.emit_eipc(
                        "quixi.web",
                        "CoworkSpaces",
                        "onSpaceEvent",
                        json!({ "type": "updated", "space": sp }),
                    );
                    return json!(sp);
                }
                Value::Null
            }
            ("CoworkSpaces", "copyFilesToSpaceFolder") => {
                let id = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let target_dir = self
                    .engine
                    .cowork_spaces
                    .read()
                    .get(id)
                    .and_then(|sp| sp.folders.first().map(|f| f.path.clone()));
                if let (Some(dest_dir), Some(files)) =
                    (target_dir, args.get(1).and_then(|v| v.as_array()))
                {
                    let _ = fs::create_dir_all(&dest_dir);
                    for f in files {
                        if let Some(src_path) = f.as_str() {
                            if let Some(fname) = Path::new(src_path).file_name() {
                                let dst_path = Path::new(&dest_dir).join(fname);
                                let _ = fs::copy(src_path, dst_path);
                            }
                        }
                    }
                }
                json!(true)
            }
            ("CoworkSpaces", "listFolderContents") => {
                let arg0 = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let arg1 = args.get(1).and_then(|v| v.as_str());
                let resolved_path = if let Some(p) = arg1.filter(|s| !s.is_empty()) {
                    p.to_string()
                } else if arg0.starts_with("space_") {
                    self.engine
                        .cowork_spaces
                        .read()
                        .get(arg0)
                        .and_then(|sp| sp.folders.first().map(|f| f.path.clone()))
                        .unwrap_or_else(|| quixi_proto::paths::default_workspace_str().to_string())
                } else if !arg0.is_empty() {
                    arg0.to_string()
                } else {
                    quixi_proto::paths::default_workspace_str().to_string()
                };
                let entries = quixi_workspace::FileIndexEngine::list_directory_children(Path::new(
                    &resolved_path,
                ));
                json!(entries)
            }
            ("CoworkSpaces", "readFileContents") => {
                let file_path = args
                    .get(1)
                    .and_then(|v| v.as_str())
                    .or_else(|| args.first().and_then(|v| v.as_str()))
                    .unwrap_or("");
                let content = fs::read_to_string(file_path).unwrap_or_default();
                json!({
                    "content": content,
                    "encoding": "utf8",
                    "mimeType": "text/plain"
                })
            }
            ("CoworkSpaces", "openFile") => json!(true),
            ("CoworkSpaces", "getRemoteSessionSpaces") => json!([]),
            ("CoworkSpaces", "setRemoteSessionSpace")
            | ("CoworkSpaces", "appendRemoteSessionSpaceFolders")
            | ("CoworkSpaces", "removeRemoteSessionSpace") => json!(true),
            ("CoworkSpaces", "getSpaceMemoryCapabilities") => json!({
                "memoryStatusProjectV1": true,
                "memoryRetryWindowV1": true
            }),
            ("CoworkSpaces", "getSpaceMemorySummary")
            | ("CoworkSpaces", "readSpaceMemoryIndex") => json!({
                "files": [],
                "summary": ""
            }),

            // --- CoworkStorage, CoworkMemory, CoworkArtifacts, ScheduledTasks ---
            ("CoworkStorage", "getUsage") => self.engine.get_cowork_storage_usage(),
            ("CoworkMemory", "readGlobalMemory") => json!(self.engine.read_global_memory()),
            ("CoworkMemory", "writeGlobalMemory") => {
                let content = args.first().and_then(|v| v.as_str()).unwrap_or("");
                json!(self.engine.write_global_memory(content))
            }
            ("CoworkMemory", "listAccountMemories") => json!(self.engine.list_account_memories()),
            ("CoworkArtifacts", "isSharingEnabled") | ("CoworkArtifacts", "isAutoPublishEnabled") => json!(false),
            ("CoworkArtifacts", "getArtifactHtml") => {
                let id = args.first().and_then(Value::as_str).unwrap_or("");
                self.engine.artifacts.read().get(id).map(|a| a["html"].clone()).unwrap_or(Value::Null)
            }
            ("CoworkArtifacts", "getAllArtifacts") => {
                let list: Vec<Value> = self.engine.artifacts.read().values().cloned().collect();
                json!(list)
            }
            ("CoworkArtifacts", "deleteArtifact") => {
                let id = args.first().and_then(|v| v.as_str()).unwrap_or("");
                json!(self.engine.delete_local_artifact(id))
            }
            ("CoworkArtifacts", "setArtifactName") => {
                let id = args.first().and_then(Value::as_str).unwrap_or("");
                let name = args.get(1).and_then(Value::as_str).unwrap_or("");
                json!(self.engine.update_local_artifact(id, "name", json!(name)))
            }
            ("CoworkArtifacts", "setArtifactStarred") => {
                let id = args.first().and_then(Value::as_str).unwrap_or("");
                let starred = args.get(1).and_then(Value::as_bool).unwrap_or(false);
                json!(self.engine.update_local_artifact(id, "isStarred", json!(starred)))
            }
            ("CoworkScheduledTasks", "runScheduledTask") => {
                self.engine.run_scheduled_task(args.first().and_then(Value::as_str).unwrap_or_default())
            }
            ("CoworkScheduledTasks", "getAllScheduledTasks")
            | ("CCDScheduledTasks", "getAllScheduledTasks") => {
                let list: Vec<Value> = self
                    .engine
                    .scheduled_tasks
                    .read()
                    .values()
                    .cloned()
                    .collect();
                json!(list)
            }
            ("CoworkScheduledTasks", "createScheduledTask")
            | ("CoworkScheduledTasks", "updateScheduledTask")
            | ("CCDScheduledTasks", "createScheduledTask")
            | ("CCDScheduledTasks", "updateScheduledTask") => {
                if args.is_empty() {
                    return Value::Null;
                }
                let opts = args.first().cloned().unwrap_or_else(|| json!({}));
                self.engine.create_or_update_scheduled_task(&opts)
            }
            ("CoworkScheduledTasks", "deleteScheduledTask")
            | ("CCDScheduledTasks", "deleteScheduledTask") => {
                let id = args.first().and_then(|v| v.as_str()).unwrap_or("");
                json!(self.engine.update_scheduled_task_status(id, "deleted"))
            }
            ("CoworkScheduledTasks", "updateScheduledTaskStatus")
            | ("CCDScheduledTasks", "updateScheduledTaskStatus") => {
                let id = args.first().and_then(|v| v.as_str()).unwrap_or("");
                let status = args.get(1).and_then(|v| v.as_str()).unwrap_or("enabled");
                json!(self.engine.update_scheduled_task_status(id, status))
            }
            ("CoworkScheduledTasks", "getScheduledTaskFileContent")
            | ("CCDScheduledTasks", "getScheduledTaskFileContent") => {
                let id = args.first().and_then(|v| v.as_str()).unwrap_or("");
                json!(self.engine.get_scheduled_task_file_content(id))
            }
            ("CoworkScheduledTasks", "getWatcherHistory")
            | ("CCDScheduledTasks", "getWatcherHistory") => json!([]),
            ("CoworkScheduledTasks", "markListenerReady")
            | ("CCDScheduledTasks", "markListenerReady")
            | ("CoworkScheduledTasks", "setEventAckSupported")
            | ("CCDScheduledTasks", "setEventAckSupported")
            | ("CoworkScheduledTasks", "ackEvent")
            | ("CCDScheduledTasks", "ackEvent")
            | ("CoworkScheduledTasks", "releaseDispatchClaim")
            | ("CCDScheduledTasks", "releaseDispatchClaim") => json!(true),

            // --- CoworkUserFiles ---
            ("CoworkUserFiles", "getInfo") => {
                let root = self.engine.data_dir.to_string_lossy().to_string();
                json!({
                    "currentRoot": root,
                    "managedRoot": Value::Null,
                    "homeRoot": root,
                    "isLegacyDocumentsRoot": false
                })
            }
            ("CoworkUserFiles", "reveal") => {
                let _ = std::process::Command::new("open")
                    .arg(&self.engine.data_dir)
                    .spawn();
                json!(true)
            }

            // --- DocumentFunnel ---
            ("DocumentFunnel", "ensureScratchRoot") => json!({
                "path": self.engine.data_dir.join("scratchpad").to_string_lossy()
            }),

            // --- Custom3pSetup & Custom3pApiKeySignIn ---
            ("Custom3pSetup", "getConfigHealth") | ("Custom3pSetup", "recheckConfigHealth") => {
                self.engine.netstack.get_custom3p_config_health()
            }
            ("Custom3pSetup", "getLoginDesktop3pStatus") => json!({
                "enabled": true,
                "source": "local",
                "provider": "anthropic",
                "thirdPartyConfigured": true,
                "thirdPartyCanRun": true,
                "needsInteractiveAuth": false,
                "hybrid": false,
                "apiSignInAllowed": true
            }),
            ("Custom3pSetup", "listConfigs") | ("Custom3pSetup", "deleteConfig") => json!({
                "appliedId": "default",
                "entries": [
                    {
                        "id": "default",
                        "name": "QuixiDesk Default",
                        "provider": "anthropic"
                    }
                ],
                "isManaged": false,
                "platform": if cfg!(target_os = "macos") { "darwin" } else if cfg!(target_os = "windows") { "win32" } else { "linux" }
            }),
            ("Custom3pSetup", "readConfig") => json!({
                "ok": true,
                "config": {
                    "deploymentMode": "3p",
                    "deploymentDisplayName": "QuixiDesk"
                }
            }),
            ("Custom3pSetup", "signInWithAnthropicApi") | ("Custom3pApiKeySignIn", "signIn") => {
                if let Some(key) = args.first().and_then(|v| v.as_str()) {
                    self.engine.netstack.save_api_key(key);
                } else if let Some(key) = args
                    .first()
                    .and_then(|v| v.get("apiKey"))
                    .and_then(|v| v.as_str())
                {
                    self.engine.netstack.save_api_key(key);
                }
                json!({ "ok": true })
            }

            // --- QuixiVM ---
            ("QuixiVM", "getRunningStatus") => json!("ready"),
            ("QuixiVM", "getDownloadStatus") => json!("ready"),
            ("QuixiVM", "isSupported") => json!({ "status": "supported" }),
            ("QuixiVM", "isHostLoopModeEnabled")
            | ("QuixiVM", "isHostLoopDevOverrideActive") => json!(false),

            // --- ComputerUseTcc & ChromeExtension ---
            ("ComputerUseTcc", "getState") => {
                json!(quixi_platform::ComputerUse::get_tcc_permissions())
            }
            ("ComputerUseTcc", "listInstalledApps") => {
                json!(quixi_platform::ComputerUse::list_installed_apps())
            }
            ("ComputerUseTcc", "requestAccessibility")
            | ("ComputerUseTcc", "requestScreenRecording")
            | ("ComputerUseTcc", "requestMicrophone") => json!("granted"),
            ("ComputerUseTcc", _) => json!(quixi_platform::ComputerUse::get_tcc_permissions()),
            ("ChromeExtension", "isInstalled")
            | ("ChromeExtension", "launchBrowser")
            | ("ChromeExtension", "restartChrome") => json!(true),
            ("ChromeExtension", "installExtension") => json!({ "status": "succeeded" }),
            ("ChromeExtension", "getConnectedExtensions") => {
                json!(
                    quixi_platform::browser_state()
                        .lock()
                        .unwrap()
                        .connected_browsers
                )
            }
            ("ChromeExtension", "getBrowserActivity") => {
                quixi_platform::BrowserUse::tabs_context(false)["tabs"].clone()
            }
            ("ChromeExtension", _) => json!({ "installed": true, "connected": true }),

            // --- LocalPlugins & CustomPlugins ---
            ("LocalPlugins", "getPlugins") => json!(*self.engine.plugins.read()),
            ("LocalPlugins", "getDownloadedRemotePlugins") => json!([]),
            ("CustomPlugins", "listMarketplaces") => json!(*self.engine.marketplaces.read()),
            ("CustomPlugins", "listAvailablePlugins") => json!(*self.engine.plugins.read()),

            // --- FilePickers & FileSystem & Resources ---
            ("FilePickers", "getDirectoryPath") => {
                let multi = args.first().and_then(|v| v.as_bool()).unwrap_or(false);
                if multi {
                    json!(quixi_platform::ComputerUse::pick_folders(Some(
                        "Choose folder(s)"
                    )))
                } else if let Some(p) =
                    quixi_platform::ComputerUse::pick_folder(Some("Choose a folder"))
                {
                    let mut trusted = self.engine.trusted_folders.write();
                    if !trusted.contains(&p) {
                        trusted.push(p.clone());
                    }
                    json!([p])
                } else {
                    json!([])
                }
            }
            ("FilePickers", "getFilePath") => {
                let multi = args.first().and_then(|v| v.as_bool()).unwrap_or(false);
                if multi {
                    json!(quixi_platform::ComputerUse::pick_files(Some(
                        "Choose file(s)"
                    )))
                } else if let Some(p) =
                    quixi_platform::ComputerUse::pick_file(Some("Choose a file"))
                {
                    json!([p])
                } else {
                    json!([])
                }
            }
            ("FileSystem", "browseFolder") => {
                let prompt = args.first().and_then(|v| v.as_str());
                if let Some(p) = quixi_platform::ComputerUse::pick_folder(prompt) {
                    let mut trusted = self.engine.trusted_folders.write();
                    if !trusted.contains(&p) {
                        trusted.push(p.clone());
                    }
                    json!(p)
                } else {
                    Value::Null
                }
            }
            ("FileSystem", "browseFolders") => {
                let prompt = args.first().and_then(|v| v.as_str());
                let folders = quixi_platform::ComputerUse::pick_folders(prompt);
                if folders.is_empty() {
                    Value::Null
                } else {
                    let mut trusted = self.engine.trusted_folders.write();
                    for f in &folders {
                        if !trusted.contains(f) {
                            trusted.push(f.clone());
                        }
                    }
                    json!(folders)
                }
            }
            ("FileSystem", "browseFile") => {
                let prompt = args.first().and_then(|v| v.as_str());
                if let Some(p) = quixi_platform::ComputerUse::pick_file(prompt) {
                    json!(p)
                } else {
                    Value::Null
                }
            }
            ("FileSystem", "browseFiles") => {
                let prompt = args.first().and_then(|v| v.as_str());
                let files = quixi_platform::ComputerUse::pick_files(prompt);
                if files.is_empty() {
                    Value::Null
                } else {
                    json!(files)
                }
            }
            ("FileSystem", "getSystemPath") => {
                let kind = args
                    .first()
                    .and_then(|v| v.as_str())
                    .unwrap_or("documents")
                    .to_lowercase();
                let home = Some(quixi_proto::paths::account_home()).unwrap_or_else(quixi_proto::paths::home_dir);
                let resolved = match kind.as_str() {
                    "documents" => if quixi_proto::paths::test_profile().is_some() { home.join("Documents") } else { dirs::document_dir().unwrap_or_else(|| home.join("Documents")) },
                    "downloads" => if quixi_proto::paths::test_profile().is_some() { home.join("Downloads") } else { dirs::download_dir().unwrap_or_else(|| home.join("Downloads")) },
                    "desktop" => if quixi_proto::paths::test_profile().is_some() { home.join("Desktop") } else { dirs::desktop_dir().unwrap_or_else(|| home.join("Desktop")) },
                    "home" => home,
                    _ => if quixi_proto::paths::test_profile().is_some() { home.join("Documents") } else { dirs::document_dir().unwrap_or_else(|| home.join("Documents")) },
                };
                json!(resolved.to_string_lossy().to_string())
            }
            ("FileSystem", "listDirectory") => {
                let raw_dir = args
                    .get(1)
                    .and_then(|v| v.as_str())
                    .or_else(|| args.first().and_then(|v| v.as_str()))
                    .unwrap_or(quixi_proto::paths::default_workspace_str());
                let dir_str = Self::percent_decode(raw_dir);
                let entries =
                    quixi_workspace::FileIndexEngine::list_directory_children(Path::new(&dir_str));
                let dirs: Vec<String> = entries
                    .iter()
                    .filter(|e| e.is_dir)
                    .map(|e| e.name.clone())
                    .collect();
                json!({ "directories": dirs, "entries": entries })
            }
            ("FileSystem", "listFilesInFolder") => {
                let raw_dir = args
                    .get(1)
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .or_else(|| {
                        args.first()
                            .and_then(|v| v.as_str())
                            .filter(|s| !s.starts_with("local_"))
                    })
                    .unwrap_or(quixi_proto::paths::default_workspace_str());
                let dir_str = Self::percent_decode(raw_dir);
                let entries =
                    quixi_workspace::FileIndexEngine::list_directory_children(Path::new(&dir_str));
                json!(entries)
            }
            ("FileSystem", "openLocalFile") | ("FileSystem", "showInFolder") => json!(true),
            ("FileSystem", "readLocalFile") => {
                let raw_path = args
                    .get(1)
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .or_else(|| args.first().and_then(|v| v.as_str()))
                    .unwrap_or("");
                let decoded = Self::percent_decode(raw_path);
                let path_buf = if Path::new(&decoded).is_absolute() {
                    std::path::PathBuf::from(&decoded)
                } else {
                    let sid = args.first().and_then(|v| v.as_str()).unwrap_or("");
                    let base = self
                        .engine
                        .agent_sessions
                        .read()
                        .get(sid)
                        .map(|s| s.cwd.clone())
                        .unwrap_or_else(|| quixi_proto::paths::default_workspace_str().to_string());
                    Path::new(&base).join(&decoded)
                };
                let content = fs::read_to_string(&path_buf).unwrap_or_default();
                let name = path_buf
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("file.txt");
                json!({
                    "content": content,
                    "encoding": "utf8",
                    "mimeType": "text/plain",
                    "fileName": name
                })
            }
            ("FileSystem", "writeLocalFile") => {
                let (raw_path, content) = if args.len() >= 3 {
                    (
                        args.get(1).and_then(|v| v.as_str()).unwrap_or(""),
                        args.get(2).and_then(|v| v.as_str()).unwrap_or(""),
                    )
                } else {
                    (
                        args.first().and_then(|v| v.as_str()).unwrap_or(""),
                        args.get(1).and_then(|v| v.as_str()).unwrap_or(""),
                    )
                };
                let decoded = Self::percent_decode(raw_path);
                if !decoded.trim().is_empty() {
                    let p = Path::new(&decoded);
                    if let Some(parent) = p.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    let _ = fs::write(p, content);
                }
                json!(true)
            }
            ("Resources", "findFiles") => {
                let q = args.first().and_then(|v| v.as_str());
                let entries = quixi_workspace::FileIndexEngine::list_files(
                    Path::new(quixi_proto::paths::default_workspace_str()),
                    q,
                    100,
                );
                json!(entries)
            }

            _ => Value::Null,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quixi_proto::EIPC_UUID_MAIN;
    use std::path::PathBuf;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn make_router() -> (Arc<QuixiEngine>, EipcRouter) {
        let res = workspace_root().join("resources");
        let engine = QuixiEngine::new_with_data_dir(res, std::env::temp_dir().join(format!("quixi-ipc-test-{}", uuid::Uuid::new_v4())));
        *engine.user_locale.write() = "en-US".to_string();
        {
            let mut st = engine.provider_manager.state.write();
            for p in &mut st.providers {
                p.configured = false;
            }
        }
        let router = EipcRouter::new(engine.clone());
        (engine, router)
    }

    #[test]
    fn test_eipc_services_and_stores() {
        let (_engine, router) = make_router();

        // 1. DesktopIntl.getInitialLocale
        let loc = router
            .dispatch(&EipcInvokePayload {
                channel: format!(
                    "$eipc_message$_{}_$_quixi.hybrid_$_DesktopIntl_$_getInitialLocale",
                    EIPC_UUID_MAIN
                ),
                args: vec![],
            })
            .result
            .unwrap();
        assert_eq!(loc["locale"], "en-US");

        // 2. AboutWindow.getAppName
        let app_name = router
            .dispatch(&EipcInvokePayload {
                channel: format!(
                    "$eipc_message$_{}_$_quixi.internal.ui_$_AboutWindow_$_getAppName",
                    EIPC_UUID_MAIN
                ),
                args: vec![],
            })
            .result
            .unwrap();
        assert_eq!(app_name, json!("QuixiDesk"));

        // 3. CoworkSpaces.getAllSpaces
        let spaces = router
            .dispatch(&EipcInvokePayload {
                channel: format!(
                    "$eipc_message$_{}_$_quixi.web_$_CoworkSpaces_$_getAllSpaces",
                    EIPC_UUID_MAIN
                ),
                args: vec![],
            })
            .result
            .unwrap();
        assert!(spaces.is_array());
        assert_eq!(spaces[0]["name"], "QuixiDesk Workspace");

        // 4. Bundled skills via LocalAgentModeSessions.listLocalSkills
        let skills = router
            .dispatch(&EipcInvokePayload {
                channel: format!(
                    "$eipc_message$_{}_$_quixi.web_$_LocalAgentModeSessions_$_listLocalSkills",
                    EIPC_UUID_MAIN
                ),
                args: vec![],
            })
            .result
            .unwrap();
        assert!(skills.as_array().unwrap().len() >= 6);

        // 5. Store getStateSync
        let managed = router
            .dispatch(&EipcInvokePayload {
                channel: format!(
                    "$eipc_message$_{}_$_quixi.web_$_ManagedConfig_$_managedRendererConfig_$store$_getStateSync",
                    EIPC_UUID_MAIN
                ),
                args: vec![],
            })
            .result
            .unwrap();
        assert_eq!(managed["deploymentDisplayName"], "QuixiDesk");
    }

    #[test]
    fn test_api_bootstrap_and_chat_sse() {
        let (engine, _router) = make_router();

        // 1. /edge-api/bootstrap
        let (status, ct, body) = engine.handle_api_request(
            "GET",
            "/edge-api/bootstrap?statsig_hashing_algorithm=djb2",
            b"",
        );
        assert_eq!(status, 200);
        assert!(ct.contains("application/json"));
        let boot_json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(boot_json["account"]["display_name"], "QuixiDesk");

        // 2. /api/organizations/*/chat_conversations/*/completion SSE
        let req_body = serde_json::to_vec(&json!({
            "prompt": "Give me a grocery list. Do not use tools or inspect files."
        }))
        .unwrap();
        let (status2, ct2, sse_bytes) = engine.handle_api_request(
            "POST",
            "/api/organizations/00000000-0000-4000-8000-000000000001/chat_conversations/test-conv-1/completion",
            &req_body,
        );
        assert_eq!(status2, 200);
        assert!(ct2.contains("text/event-stream"));
        let sse_str = String::from_utf8(sse_bytes).unwrap();
        assert!(sse_str.contains("event: message_start"));
        assert!(sse_str.contains("event: content_block_delta"));
        // An unavailable provider reports the failure; a word such as "list" must not
        // silently inspect the workspace or substitute a fabricated status report.
        assert!(sse_str.contains("No connected provider returned a response"));
        assert!(!sse_str.contains("Contents of"));
        assert!(!sse_str.contains("QuixiDesk Workspace Status"));
        assert!(sse_str.contains("event: message_stop"));
    }

    #[test]
    fn test_owned_session_lifecycles() {
        let (_engine, router) = make_router();

        // 2. Test full LocalAgentModeSessions (Cowork / 3P Chat) lifecycle
        let start_agent = router
            .dispatch(&EipcInvokePayload {
                channel: format!(
                    "$eipc_message$_{}_$_quixi.web_$_LocalAgentModeSessions_$_start",
                    EIPC_UUID_MAIN
                ),
                args: vec![json!({
                    "message": "Check git status",
                    "sessionType": "cowork",
                    "userSelectedFolders": [workspace_root()]
                })],
            })
            .result
            .unwrap();
        let agent_sid = start_agent["sessionId"].as_str().unwrap().to_string();
        assert!(agent_sid.starts_with("local_"));

        for _ in 0..100 {
            if _engine.agent_sessions.read().get(&agent_sid).is_some_and(|session| !session.is_running) { break; }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        let transcript = router
            .dispatch(&EipcInvokePayload {
                channel: format!(
                    "$eipc_message$_{}_$_quixi.web_$_LocalAgentModeSessions_$_getTranscript",
                    EIPC_UUID_MAIN
                ),
                args: vec![json!(agent_sid)],
            })
            .result
            .unwrap();
        assert!(transcript.as_array().unwrap().len() >= 2);

        // 3. Test full LocalSessions (Code / CCD) lifecycle
        let start_code = router
            .dispatch(&EipcInvokePayload {
                channel: format!(
                    "$eipc_message$_{}_$_quixi.web_$_LocalSessions_$_start",
                    EIPC_UUID_MAIN
                ),
                args: vec![json!({
                    "prompt": "List files in workspace",
                    "cwd": workspace_root()
                })],
            })
            .result
            .unwrap();
        let code_sid = start_code["sessionId"].as_str().unwrap().to_string();
        assert!(code_sid.starts_with("local_"));
        assert!(!code_sid.starts_with("local_cli_"));

        for _ in 0..100 {
            if _engine.code_sessions.read().get(&code_sid).is_some_and(|session| !session.turn_running) { break; }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        let code_transcript = router
            .dispatch(&EipcInvokePayload {
                channel: format!(
                    "$eipc_message$_{}_$_quixi.web_$_LocalSessions_$_getTranscript",
                    EIPC_UUID_MAIN
                ),
                args: vec![json!(code_sid)],
            })
            .result
            .unwrap();
        assert!(code_transcript.as_array().unwrap().len() >= 2);

        // Clean up persisted test session files and verify delete -> updateSession -> getSession
        // never resurrects a deleted session or creates an empty-sessionId "Untitled" task
        let _ = router.dispatch(&EipcInvokePayload {
            channel: format!(
                "$eipc_message$_{}_$_quixi.web_$_LocalAgentModeSessions_$_delete",
                EIPC_UUID_MAIN
            ),
            args: vec![json!(agent_sid)],
        });
        let _ = router.dispatch(&EipcInvokePayload {
            channel: format!(
                "$eipc_message$_{}_$_quixi.web_$_LocalAgentModeSessions_$_updateSession",
                EIPC_UUID_MAIN
            ),
            args: vec![json!(agent_sid), json!({ "isStarred": false })],
        });
        let get_deleted_agent = router
            .dispatch(&EipcInvokePayload {
                channel: format!(
                    "$eipc_message$_{}_$_quixi.web_$_LocalAgentModeSessions_$_getSession",
                    EIPC_UUID_MAIN
                ),
                args: vec![json!(agent_sid), json!({ "skipReplay": true, "metadataOnly": true })],
            })
            .result
            .unwrap();
        assert!(
            get_deleted_agent.is_null(),
            "getSession for deleted session must return Null, got: {:?}",
            get_deleted_agent
        );

        let _ = router.dispatch(&EipcInvokePayload {
            channel: format!(
                "$eipc_message$_{}_$_quixi.web_$_LocalSessions_$_delete",
                EIPC_UUID_MAIN
            ),
            args: vec![json!(code_sid)],
        });
        let get_deleted_code = router
            .dispatch(&EipcInvokePayload {
                channel: format!(
                    "$eipc_message$_{}_$_quixi.web_$_LocalSessions_$_getSession",
                    EIPC_UUID_MAIN
                ),
                args: vec![json!(code_sid)],
            })
            .result
            .unwrap();
        assert!(get_deleted_code.is_null());
    }

    #[test]
    fn test_mcp_computer_use_and_browser_use_e2e() {
        let (_engine, router) = make_router();

        // 1. Verify LocalAgentModeSessions.getLocalMcpServers returns all 34 MCP servers (5 user + 29 internal)
        let mcp_servers = router
            .dispatch(&EipcInvokePayload {
                channel: format!(
                    "$eipc_message$_{}_$_quixi.web_$_LocalAgentModeSessions_$_getLocalMcpServers",
                    EIPC_UUID_MAIN
                ),
                args: vec![],
            })
            .result
            .unwrap();
        let servers_arr = mcp_servers.as_array().unwrap();
        assert!(servers_arr.len() >= 28);
        assert!(servers_arr
            .iter()
            .any(|s| s["uuid"] == "b0a3b6e5-7ca0-462a-8e6f-bac087408b17"
                && s["name"] == "computer-use"
                && s["tools"].as_array().unwrap().len() >= 44));
        assert!(servers_arr
            .iter()
            .any(|s| s["uuid"] == "a8f3c7e2-4b9d-4f1a-8c3e-9d2a5b7f8e1c"
                && s["name"] == "quixi-in-chrome"
                && s["tools"].as_array().unwrap().len() >= 22));
        assert!(servers_arr
            .iter()
            .any(|s| s["uuid"] == "bda6af03-834c-4496-98d1-c0e6d52b99ce"
                && (s["name"] == "QuixiDesk Browser" || s["name"] == "QuixiDesk Preview")
                && s["tools"].as_array().unwrap().len() >= 29));

        // 2. Verify ComputerUse tool execution via directMcpCallTool
        let cu_pos = router
            .dispatch(&EipcInvokePayload {
                channel: format!(
                    "$eipc_message$_{}_$_quixi.web_$_LocalAgentModeSessions_$_directMcpCallTool",
                    EIPC_UUID_MAIN
                ),
                args: vec![
                    json!("b0a3b6e5-7ca0-462a-8e6f-bac087408b17"),
                    json!("cursor_position"),
                    json!({}),
                ],
            })
            .result
            .unwrap();
        assert_eq!(cu_pos["isError"], false);
        assert!(cu_pos["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("\"x\":"));

        let cu_apps = router
            .dispatch(&EipcInvokePayload {
                channel: format!(
                    "$eipc_message$_{}_$_quixi.web_$_LocalAgentModeSessions_$_directMcpCallTool",
                    EIPC_UUID_MAIN
                ),
                args: vec![
                    json!("computer-use"),
                    json!("list_apps"),
                    json!({}),
                ],
            })
            .result
            .unwrap();
        assert_eq!(cu_apps["isError"], false);
        assert!(!cu_apps["content"][0]["text"].as_str().unwrap().is_empty());

        // 3. Verify BrowserUse tool execution via mcpCallTool
        let tabs_ctx = router
            .dispatch(&EipcInvokePayload {
                channel: format!(
                    "$eipc_message$_{}_$_quixi.web_$_LocalAgentModeSessions_$_mcpCallTool",
                    EIPC_UUID_MAIN
                ),
                args: vec![
                    json!("session_1"),
                    json!("a8f3c7e2-4b9d-4f1a-8c3e-9d2a5b7f8e1c"),
                    json!("tabs_context_mcp"),
                    json!({ "createIfEmpty": true }),
                ],
            })
            .result
            .unwrap();
        assert_eq!(tabs_ctx["isError"], false);
        assert!(tabs_ctx["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("tabGroupId"));

        let read_page = router
            .dispatch(&EipcInvokePayload {
                channel: format!(
                    "$eipc_message$_{}_$_quixi.web_$_LocalAgentModeSessions_$_directMcpCallTool",
                    EIPC_UUID_MAIN
                ),
                args: vec![
                    json!("quixi-in-chrome"),
                    json!("read_page"),
                    json!({ "tabId": 1, "filter": "all" }),
                ],
            })
            .result
            .unwrap();
        assert_eq!(read_page["isError"], false);
        assert!(read_page["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("[ref_1]"));

        // 4. Verify ComputerUseTcc and ChromeExtension IPC services
        let tcc_apps = router
            .dispatch(&EipcInvokePayload {
                channel: format!(
                    "$eipc_message$_{}_$_quixi.web_$_ComputerUseTcc_$_listInstalledApps",
                    EIPC_UUID_MAIN
                ),
                args: vec![],
            })
            .result
            .unwrap();
        assert!(tcc_apps.is_array());
        assert!(!tcc_apps.as_array().unwrap().is_empty());

        let chrome_exts = router
            .dispatch(&EipcInvokePayload {
                channel: format!(
                    "$eipc_message$_{}_$_quixi.web_$_ChromeExtension_$_getConnectedExtensions",
                    EIPC_UUID_MAIN
                ),
                args: vec![],
            })
            .result
            .unwrap();
        assert!(!chrome_exts.as_array().unwrap().is_empty());
    }

    #[test]
    fn test_connected_provider_model_selector_and_stream_events() {
        let (engine, router) = make_router();

        // 1. Configure only the OpenAI provider in memory.
        {
            let mut st = engine.provider_manager.state.write();
            st.active_provider = "openai".to_string();
            st.active_model = "gpt-6.1-sol".to_string();
            for p in &mut st.providers {
                p.oauth_access_token.clear();
                p.api_key_raw.clear();
                if p.id == "openai" {
                    p.configured = true;
                    p.auth_mode = "oauth".to_string();
                    p.selected_model = "gpt-6.1-sol".to_string();
                } else {
                    p.configured = false;
                }
            }
        }

        // 2. Verify bootstrap exposes only models from connected providers.
        let (status, _ct, body) = engine.handle_api_request("GET", "/edge-api/bootstrap", b"");
        assert_eq!(status, 200);
        let boot: Value = serde_json::from_slice(&body).unwrap();
        let cowork_cfg = boot["model_selector_config"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == "cowork")
            .unwrap();
        let model_ids: Vec<&str> = cowork_cfg["models"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|m| m["id"].as_str())
            .collect();
        assert!(model_ids.contains(&"gpt-6.1-sol"));
        assert!(model_ids.contains(&"gpt-6-astra"));
        assert!(!model_ids.contains(&"claude-sonnet-4-6"));
        assert!(!model_ids.contains(&"claude-opus-4-7"));
        assert!(!model_ids.contains(&"gpt-5.4")); // Platform API model excluded when auth_mode == oauth

        let cowork_state = boot["model_selector_state"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["id"] == "cowork")
            .unwrap();
        assert_eq!(cowork_state["model"], "gpt-6.1-sol");

        // 3. Also connect Grok (xAI) in memory and verify multi-provider dropdown includes both OpenAI + xAI
        {
            let mut st = engine.provider_manager.state.write();
            for p in &mut st.providers {
                if p.id == "xai" {
                    p.configured = true;
                    p.selected_model = "grok-4".to_string();
                }
            }
        }
        let (_st2, _ct2, body2) = engine.handle_api_request("GET", "/edge-api/bootstrap", b"");
        let boot2: Value = serde_json::from_slice(&body2).unwrap();
        let multi_ids: Vec<&str> = boot2["model_selector_config"][0]["models"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|m| m["id"].as_str())
            .collect();
        assert!(multi_ids.contains(&"gpt-6.1-sol"));
        assert!(multi_ids.contains(&"grok-4"));
        assert!(!multi_ids.contains(&"claude-sonnet-4-6"));

        // 4. Verify `stream_event` emission sequence and model resolution during LocalAgentModeSessions
        let events = Arc::new(std::sync::Mutex::new(Vec::<Value>::new()));
        let events_clone = Arc::clone(&events);
        engine.set_emitter(Arc::new(move |ch: &str, payload: Value| {
            if ch.contains("LocalAgentModeSessions_$_onEvent") {
                events_clone.lock().unwrap().push(payload);
            }
        }));

        let start_res = router
            .dispatch(&EipcInvokePayload {
                channel: format!(
                    "$eipc_message$_{}_$_quixi.web_$_LocalAgentModeSessions_$_start",
                    EIPC_UUID_MAIN
                ),
                args: vec![json!({
                    "message": "Check git status",
                    "model": "claude-sonnet-4-6", // Unconnected model requested by stale caller
                    "sessionType": "cowork",
                    "userSelectedFolders": [workspace_root()]
                })],
            })
            .result
            .unwrap();
        let sid = start_res["sessionId"].as_str().unwrap().to_string();
        // Even though caller requested claude-sonnet-4-6, session model resolved to connected gpt-6.1-sol!
        assert_eq!(start_res["model"], "gpt-6.1-sol");

        for _ in 0..50 {
            std::thread::sleep(std::time::Duration::from_millis(40));
            let has_stop = events.lock().unwrap().iter().any(|ev| {
                ev.get("sessionId").and_then(|v| v.as_str()) == Some(&sid)
                    && ev.pointer("/message/event/type").and_then(|v| v.as_str())
                        == Some("message_stop")
            });
            if has_stop {
                break;
            }
        }

        let recorded = events.lock().unwrap().clone();
        let mut saw_message_start = false;
        let mut saw_block_start = false;
        let mut saw_block_delta = false;
        let mut saw_block_stop = false;
        let mut saw_message_stop = false;
        let mut stream_msg_id = String::new();
        let mut assistant_msg_id = String::new();
        let mut assistant_model = String::new();

        for ev in &recorded {
            if ev.get("sessionId").and_then(|v| v.as_str()) != Some(&sid) {
                continue;
            }
            if let Some(msg) = ev.get("message") {
                match msg.get("type").and_then(|v| v.as_str()) {
                    Some("stream_event") => {
                        let se_type = msg
                            .pointer("/event/type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        match se_type {
                            "message_start" => {
                                saw_message_start = true;
                                stream_msg_id = msg
                                    .pointer("/event/message/id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                assert_eq!(
                                    msg.pointer("/event/message/model").and_then(|v| v.as_str()),
                                    Some("gpt-6.1-sol")
                                );
                            }
                            "content_block_start" => saw_block_start = true,
                            "content_block_delta" => saw_block_delta = true,
                            "content_block_stop" => saw_block_stop = true,
                            "message_stop" => saw_message_stop = true,
                            _ => {}
                        }
                    }
                    Some("assistant") => {
                        assistant_msg_id = msg
                            .pointer("/message/id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        assistant_model = msg
                            .pointer("/message/model")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                    }
                    _ => {}
                }
            }
        }

        assert!(saw_message_start, "Expected stream_event message_start");
        assert!(saw_block_start, "Expected stream_event content_block_start");
        assert!(saw_block_delta, "Expected stream_event content_block_delta");
        assert!(saw_block_stop, "Expected stream_event content_block_stop");
        assert!(saw_message_stop, "Expected stream_event message_stop");
        assert!(!stream_msg_id.is_empty());
        assert_eq!(stream_msg_id, assistant_msg_id);
        assert_eq!(assistant_model, "gpt-6.1-sol");

        // Clean up test session file
        let _ = router.dispatch(&EipcInvokePayload {
            channel: format!(
                "$eipc_message$_{}_$_quixi.web_$_LocalAgentModeSessions_$_delete",
                EIPC_UUID_MAIN
            ),
            args: vec![json!(sid)],
        });
    }
}

#[cfg(test)]
mod session_control_regression_tests {
    use super::*;
    #[test]
    fn permission_changes_and_stop_publish_renderer_events() {
        let engine = QuixiEngine::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"));
        let router = EipcRouter::new(engine.clone());
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let recorded = events.clone();
        engine.set_emitter(Arc::new(move |_, event| recorded.lock().unwrap().push(event)));
        let sid = engine.start_agent_session(&json!({"permissionMode":"auto"}));
        let call = |method: &str, args: Vec<Value>| router.dispatch(&EipcInvokePayload {
            channel: format!("$eipc_message$_{}_$_quixi.web_$_LocalAgentModeSessions_$_{}", quixi_proto::EIPC_UUID_MAIN, method), args,
        });
        assert_eq!(call("setPermissionMode",vec![json!(sid),json!("default")]).result,Some(json!(true)));
        call("stop",vec![json!(sid)]);
        let events = events.lock().unwrap();
        assert!(events.iter().any(|event| event["type"] == "permission_mode_changed" && event["permissionMode"] == "default"));
        assert!(events.iter().any(|event| event["type"] == "close" && event["code"] == 0));
        drop(events);
        call("delete",vec![json!(sid)]);
    }
}
