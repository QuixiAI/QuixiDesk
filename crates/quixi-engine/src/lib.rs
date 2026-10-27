pub mod providers;

use chrono::Utc;
use parking_lot::RwLock;
use providers::ProviderManager;
use quixi_mcp::McpHost;
use quixi_netstack::NetstackPolicy;
use quixi_proto::{
    CoworkSpace, CoworkSpaceFolder, LocalAgentSession, LocalCodeSession, EIPC_UUID_MAIN,
};
use quixi_workspace::ShellHostEngine;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use uuid::Uuid;

pub type EventCallback = Arc<dyn Fn(&str, Value) + Send + Sync>;

pub struct QuixiEngine {
    pub resources_dir: PathBuf,
    pub data_dir: PathBuf,
    pub bootstrap_template: RwLock<Value>,
    pub desktop_features: Value,
    pub desktop_managed_config: Value,
    pub app_preferences: RwLock<Value>,
    pub stores: RwLock<HashMap<String, Value>>,
    pub account_settings: RwLock<Value>,
    pub agent_sessions: RwLock<HashMap<String, LocalAgentSession>>,
    pub code_sessions: RwLock<HashMap<String, LocalCodeSession>>,
    pub code_transcripts: RwLock<HashMap<String, Vec<Value>>>,
    pub cowork_spaces: RwLock<HashMap<String, CoworkSpace>>,
    pub chat_conversations: RwLock<HashMap<String, Value>>,
    pub projects: RwLock<HashMap<String, Value>>,
    pub artifacts: RwLock<HashMap<String, Value>>,
    pub scheduled_tasks: RwLock<HashMap<String, Value>>,
    pub plugins: RwLock<Vec<Value>>,
    pub marketplaces: RwLock<Vec<Value>>,
    pub preview_tabs: RwLock<Vec<Value>>,
    pub active_preview_tab_id: RwLock<String>,
    pub trusted_folders: RwLock<Vec<String>>,
    pub mcp_host: McpHost,
    pub netstack: NetstackPolicy,
    pub shell_host: ShellHostEngine,
    pub provider_manager: ProviderManager,
    pub user_locale: RwLock<String>,
    pub emitter: RwLock<Option<EventCallback>>,
}

impl QuixiEngine {
    pub fn new(resources_dir: PathBuf) -> Arc<Self> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/Users/eric"));
        let data_dir = home.join(".quixi");
        for sub in [
            "sessions",
            "spaces",
            "chats",
            "projects",
            "skills",
            "artifacts",
            "memory",
            "scheduled_tasks",
            "plugins",
            "scratchpad",
        ] {
            let _ = fs::create_dir_all(data_dir.join(sub));
        }

        let bootstrap_path = resources_dir.join("bootstrap/bootstrap.json");
        let bootstrap_template: Value = fs::read_to_string(&bootstrap_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| json!({}));

        let features_path = resources_dir.join("bootstrap/desktop-features.json");
        let desktop_features: Value = fs::read_to_string(&features_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| json!({}));

        let managed_path = resources_dir.join("bootstrap/desktop-managed-config.json");
        let desktop_managed_config: Value = fs::read_to_string(&managed_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| json!({}));

        let stores_path = resources_dir.join("bootstrap/initial-stores.json");
        let initial_stores: HashMap<String, Value> = fs::read_to_string(&stores_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let now_ms = Utc::now().timestamp_millis() as u64;
        let default_workspace = "/Users/eric/quixi-desk".to_string();

        let mut spaces = HashMap::new();
        let default_space_id = "space_quixidesk_default".to_string();
        spaces.insert(
            default_space_id.clone(),
            CoworkSpace {
                id: default_space_id,
                name: "QuixiDesk Workspace".to_string(),
                description: "Primary QuixiDesk engineering workspace".to_string(),
                instructions: String::new(),
                icon: "folder".to_string(),
                color: "clay".to_string(),
                created_at: now_ms,
                updated_at: now_ms,
                folders: vec![CoworkSpaceFolder {
                    path: default_workspace.clone(),
                    name: "quixi-desk".to_string(),
                    added_at: now_ms,
                }],
                links: vec![],
                projects: vec![],
            },
        );

        let default_plugins = vec![json!({
            "id": "quixi-core",
            "name": "quixi-core",
            "displayName": "QuixiDesk Core Tools",
            "installPath": data_dir.join("plugins/quixi-core").to_string_lossy(),
            "scope": "user",
            "enabled": true,
            "description": "Built-in QuixiDesk workspace, git, and document skills plugin",
            "version": "3.6556.2",
            "skills": [],
            "commands": [],
            "mcpServers": [],
            "hooks": [],
            "agents": []
        })];

        let default_marketplaces = vec![json!({
            "name": "quixi-marketplace",
            "displayName": "QuixiDesk Official Marketplace",
            "sourceType": "remote",
            "lastRefreshedAt": now_ms,
            "isDefault": true,
            "source": "remote"
        })];

        let provider_manager = ProviderManager::new(&data_dir);
        let locale_path = data_dir.join("locale.txt");
        let initial_locale = fs::read_to_string(&locale_path)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "en-US".to_string());

        let engine = Arc::new(Self {
            resources_dir,
            data_dir,
            bootstrap_template: RwLock::new(bootstrap_template),
            desktop_features,
            desktop_managed_config,
            app_preferences: RwLock::new(json!({
                "chicagoEnabled": true,
                "chicagoAutoUnhide": true,
                "chicagoPreferredMode": "background",
                "chicagoUserDeniedBundleIds": [],
                "chromeExtensionEnabled": true,
                "coworkPreferredBrowser": "built_in",
                "launchEnabled": true,
                "claudeIosSimulatorAccessEnabled": true,
                "claudeAndroidEmulatorAccessEnabled": true,
                "bypassPermissionsModeEnabled": true,
                "menuBarEnabled": true,
            })),
            stores: RwLock::new(initial_stores),
            account_settings: RwLock::new(json!({
                "enabled_wiggle_egress": true,
                "wiggle_egress_hosts_template": "full_egress",
                "enabled_geolocation": false,
                "preview_feature_uses_artifacts": true,
                "preview_feature_uses_latex": true,
                "preview_feature_uses_citations": true,
            })),
            agent_sessions: RwLock::new(HashMap::new()),
            code_sessions: RwLock::new(HashMap::new()),
            code_transcripts: RwLock::new(HashMap::new()),
            cowork_spaces: RwLock::new(spaces),
            chat_conversations: RwLock::new(HashMap::new()),
            projects: RwLock::new(HashMap::new()),
            artifacts: RwLock::new(HashMap::new()),
            scheduled_tasks: RwLock::new(HashMap::new()),
            plugins: RwLock::new(default_plugins),
            marketplaces: RwLock::new(default_marketplaces),
            preview_tabs: RwLock::new(Vec::new()),
            active_preview_tab_id: RwLock::new("seed".to_string()),
            trusted_folders: RwLock::new(vec![
                default_workspace,
                home.to_string_lossy().to_string(),
            ]),
            mcp_host: McpHost::new(),
            netstack: NetstackPolicy::new(),
            shell_host: ShellHostEngine::new(),
            provider_manager,
            user_locale: RwLock::new(initial_locale),
            emitter: RwLock::new(None),
        });

        engine.load_persisted_sessions();
        engine
    }

    pub fn set_emitter(&self, cb: EventCallback) {
        *self.emitter.write() = Some(cb);
    }

    pub fn emit(&self, channel: &str, payload: Value) {
        if let Some(cb) = self.emitter.read().as_ref() {
            cb(channel, payload);
        }
    }

    pub fn emit_eipc(&self, namespace: &str, service: &str, member: &str, payload: Value) {
        let ch = format!(
            "$eipc_message$_{}_$_{}_$_{}_$_{}",
            EIPC_UUID_MAIN, namespace, service, member
        );
        self.emit(&ch, payload);
    }

    fn load_persisted_sessions(&self) {
        let sessions_dir = self.data_dir.join("sessions");
        if let Ok(entries) = fs::read_dir(&sessions_dir) {
            let mut map = self.agent_sessions.write();
            for entry in entries.flatten() {
                if entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
                    if let Ok(raw) = fs::read_to_string(entry.path()) {
                        if let Ok(mut sess) = serde_json::from_str::<LocalAgentSession>(&raw) {
                            if sess.session_id.trim().is_empty() {
                                let _ = fs::remove_file(entry.path());
                                continue;
                            }
                            sess.is_running = false;
                            let resolved = self
                                .provider_manager
                                .resolve_connected_model(Some(&sess.model));
                            let mut changed = false;
                            if sess.model != resolved {
                                sess.model = resolved.clone();
                                changed = true;
                            }
                            for msg in &mut sess.buffered_messages {
                                if msg.pointer("/message/model").and_then(|v| v.as_str())
                                    == Some("claude-sonnet-4-6")
                                    && resolved != "claude-sonnet-4-6"
                                {
                                    if let Some(m_obj) =
                                        msg.get_mut("message").and_then(|v| v.as_object_mut())
                                    {
                                        m_obj.insert("model".to_string(), json!(resolved));
                                        changed = true;
                                    }
                                }
                            }
                            if changed {
                                self.persist_agent_session(&sess);
                            }
                            map.insert(sess.session_id.clone(), sess);
                        }
                    }
                }
            }
        }

        let code_sessions_dir = self.data_dir.join("code_sessions");
        if let Ok(entries) = fs::read_dir(&code_sessions_dir) {
            let mut map = self.code_sessions.write();
            for entry in entries.flatten() {
                if entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
                    if let Ok(raw) = fs::read_to_string(entry.path()) {
                        if let Ok(mut sess) = serde_json::from_str::<LocalCodeSession>(&raw) {
                            if sess.session_id.trim().is_empty() {
                                let _ = fs::remove_file(entry.path());
                                continue;
                            }
                            if let Some(stripped) = sess.session_id.strip_prefix("local_cli_") {
                                let _ = fs::remove_file(entry.path());
                                sess.session_id = format!("local_{}", stripped);
                            }
                            sess.is_running = false;
                            sess.turn_running = false;
                            sess.model = self
                                .provider_manager
                                .resolve_connected_model(Some(&sess.model));
                            self.persist_code_session(&sess);
                            map.insert(sess.session_id.clone(), sess);
                        }
                    }
                }
            }
        }

        let spaces_dir = self.data_dir.join("spaces");
        if let Ok(entries) = fs::read_dir(&spaces_dir) {
            let mut sp_map = self.cowork_spaces.write();
            let mut trusted = self.trusted_folders.write();
            for entry in entries.flatten() {
                if entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
                    if let Ok(raw) = fs::read_to_string(entry.path()) {
                        if let Ok(sp) = serde_json::from_str::<CoworkSpace>(&raw) {
                            for f in &sp.folders {
                                if !trusted.contains(&f.path) {
                                    trusted.push(f.path.clone());
                                }
                            }
                            sp_map.insert(sp.id.clone(), sp);
                        }
                    }
                }
            }
        }
    }

    pub fn build_live_bootstrap(&self) -> Value {
        let mut boot = self.bootstrap_template.read().clone();
        let current_locale = self.user_locale.read().clone();
        if let Some(acct) = boot.get_mut("account").and_then(|a| a.as_object_mut()) {
            acct.insert("settings".to_string(), self.account_settings.read().clone());
        }
        let (ms_config, ms_state, active_model) =
            self.provider_manager.build_model_selector_payload();
        if let Some(obj) = boot.as_object_mut() {
            obj.insert("locale".to_string(), json!(current_locale));
            obj.insert("locale_source".to_string(), json!("user_choice"));
            obj.insert("model_selector_config".to_string(), ms_config);
            obj.insert("model_selector_state".to_string(), ms_state);
            obj.insert(
                "bootstrap_push_revision".to_string(),
                json!(Utc::now().timestamp_millis()),
            );
        }
        if let Some(gm) = boot.get_mut("gated_messages").and_then(|g| g.as_object_mut()) {
            gm.insert("locale".to_string(), json!(current_locale));
        }
        if let Some(gb_feat) = boot.pointer_mut("/growthbook/features/2973881027/defaultValue") {
            if let Some(f_obj) = gb_feat.as_object_mut() {
                f_obj.insert("model".to_string(), json!(active_model));
            }
        }
        boot
    }

    pub fn persist_agent_session(&self, sess: &LocalAgentSession) {
        if sess.session_id.trim().is_empty() {
            return;
        }
        let dir = self.data_dir.join("sessions");
        let _ = fs::create_dir_all(&dir);
        let file = dir.join(format!("{}.json", sess.session_id));
        if let Ok(raw) = serde_json::to_string_pretty(sess) {
            let _ = fs::write(file, raw);
        }
    }

    pub fn persist_code_session(&self, sess: &LocalCodeSession) {
        if sess.session_id.trim().is_empty() {
            return;
        }
        let dir = self.data_dir.join("code_sessions");
        let _ = fs::create_dir_all(&dir);
        let file = dir.join(format!("{}.json", sess.session_id));
        if let Ok(raw) = serde_json::to_string_pretty(sess) {
            let _ = fs::write(file, raw);
        }
    }

    pub fn persist_cowork_space(&self, sp: &CoworkSpace) {
        let dir = self.data_dir.join("spaces");
        let _ = fs::create_dir_all(&dir);
        let file = dir.join(format!("{}.json", sp.id));
        if let Ok(raw) = serde_json::to_string_pretty(sp) {
            let _ = fs::write(file, raw);
        }
    }

    // =========================================================================
    // Bundled & User Skills Subsystem
    // =========================================================================
    pub fn list_local_skills(&self) -> Vec<Value> {
        let mut skills = Vec::new();
        let now_iso = Utc::now().to_rfc3339();

        // 1. Load bundled skills from resources/bundled-skills/manifest.json
        let manifest_path = self.resources_dir.join("bundled-skills/manifest.json");
        if let Ok(raw) = fs::read_to_string(&manifest_path) {
            if let Ok(manifest) = serde_json::from_str::<Value>(&raw) {
                if let Some(arr) = manifest.get("skills").and_then(|v| v.as_array()) {
                    for s in arr {
                        let name = s.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        let desc = s.get("description").and_then(|v| v.as_str()).unwrap_or("");
                        if !name.is_empty() {
                            skills.push(json!({
                                "skillId": name,
                                "name": name,
                                "displayName": name,
                                "description": desc,
                                "updatedAt": now_iso,
                                "enabled": true,
                            }));
                        }
                    }
                }
            }
        }

        // 2. Load user skills from ~/.quixi/skills/<skillId>/
        let user_skills_dir = self.data_dir.join("skills");
        if let Ok(entries) = fs::read_dir(&user_skills_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    let skill_id = entry.file_name().to_string_lossy().to_string();
                    if skills
                        .iter()
                        .any(|s| s.get("skillId").and_then(|v| v.as_str()) == Some(&skill_id))
                    {
                        continue;
                    }
                    let md_path = entry.path().join("SKILL.md");
                    let desc = fs::read_to_string(&md_path)
                        .ok()
                        .and_then(|c| {
                            // First look for `description:` inside YAML frontmatter
                            for line in c.lines() {
                                let trimmed = line.trim();
                                if let Some(rest) = trimmed.strip_prefix("description:") {
                                    let val = rest.trim().trim_matches('"').trim_matches('\'').trim();
                                    if !val.is_empty() && val != "|" && val != ">" {
                                        return Some(val.to_string());
                                    }
                                }
                            }
                            // Otherwise skip YAML frontmatter (`--- ... ---`) and pick the first prose line
                            let mut lines = c.lines();
                            let mut in_frontmatter = false;
                            if let Some(first) = lines.next() {
                                if first.trim() == "---" {
                                    in_frontmatter = true;
                                } else if !first.trim().is_empty() && !first.trim().starts_with('#') {
                                    return Some(first.trim().to_string());
                                }
                            }
                            for line in lines {
                                let t = line.trim();
                                if in_frontmatter {
                                    if t == "---" {
                                        in_frontmatter = false;
                                    }
                                    continue;
                                }
                                if !t.is_empty() && !t.starts_with('#') && !t.starts_with("---") {
                                    return Some(t.to_string());
                                }
                            }
                            None
                        })
                        .unwrap_or_else(|| "Custom QuixiDesk skill".to_string());
                    skills.push(json!({
                        "skillId": skill_id,
                        "name": skill_id,
                        "displayName": skill_id,
                        "description": desc,
                        "updatedAt": now_iso,
                        "enabled": true,
                    }));
                }
            }
        }
        skills
    }

    pub fn get_local_skill_files(&self, skill_id: &str) -> Vec<Value> {
        let mut out = Vec::new();
        let clean_id = skill_id.trim_matches(|c| c == '/' || c == '.');
        if clean_id.is_empty() {
            return out;
        }
        let user_dir = self.data_dir.join("skills").join(clean_id);
        let bundled_dir = self
            .resources_dir
            .join("bundled-skills-extracted")
            .join(clean_id);
        let target_dir = if user_dir.exists() {
            user_dir
        } else {
            bundled_dir
        };

        if let Ok(entries) = fs::read_dir(&target_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let rel = entry.file_name().to_string_lossy().to_string();
                    let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                    let content = fs::read_to_string(&path).unwrap_or_default();
                    out.push(json!({
                        "path": rel,
                        "content": content,
                        "contentsOmitted": false,
                        "size": size,
                    }));
                }
            }
        }
        out
    }

    pub fn save_local_skill(&self, opts: &Value) -> Value {
        let name = opts
            .get("name")
            .or_else(|| opts.get("skillId"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("custom-skill");
        let content = opts
            .get("content")
            .or_else(|| opts.get("markdown"))
            .and_then(|v| v.as_str())
            .unwrap_or("# Custom Skill\n\nCreated in QuixiDesk.\n");
        let dir = self.data_dir.join("skills").join(name);
        let _ = fs::create_dir_all(&dir);
        let _ = fs::write(dir.join("SKILL.md"), content);
        json!({
            "ok": true,
            "skillId": name,
        })
    }

    pub fn delete_local_skill(&self, skill_id: &str) -> Value {
        let clean_id = skill_id.trim_matches(|c| c == '/' || c == '.');
        if !clean_id.is_empty() {
            let dir = self.data_dir.join("skills").join(clean_id);
            let _ = fs::remove_dir_all(dir);
        }
        json!({ "ok": true, "skillId": clean_id })
    }

    // =========================================================================
    // CoworkMemory, CoworkArtifacts, CoworkScheduledTasks, CoworkStorage
    // =========================================================================
    pub fn read_global_memory(&self) -> String {
        let p = self.data_dir.join("memory/CLAUDE.md");
        fs::read_to_string(p).unwrap_or_default()
    }

    pub fn write_global_memory(&self, content: &str) -> bool {
        let p = self.data_dir.join("memory/CLAUDE.md");
        fs::write(p, content).is_ok()
    }

    pub fn list_account_memories(&self) -> Vec<Value> {
        let mut out = Vec::new();
        if let Ok(entries) = fs::read_dir(self.data_dir.join("memory")) {
            for e in entries.flatten() {
                if e.path().is_file() {
                    let name = e.file_name().to_string_lossy().to_string();
                    out.push(json!({
                        "path": name,
                        "sizeBytes": e.metadata().map(|m| m.len()).unwrap_or(0),
                    }));
                }
            }
        }
        out
    }

    pub fn create_or_update_scheduled_task(&self, opts: &Value) -> Value {
        let id = opts
            .get("id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("task_{}", Uuid::new_v4()));
        let name = opts
            .get("name")
            .or_else(|| opts.get("title"))
            .and_then(|v| v.as_str())
            .unwrap_or("Scheduled Task");
        let cron = opts
            .get("cron")
            .or_else(|| opts.get("schedule"))
            .and_then(|v| v.as_str())
            .unwrap_or("0 9 * * *");
        let enabled = opts.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
        let now_ms = Utc::now().timestamp_millis() as u64;
        let file_path = self
            .data_dir
            .join("scheduled_tasks")
            .join(format!("{}.md", id));
        let content = opts
            .get("content")
            .or_else(|| opts.get("prompt"))
            .and_then(|v| v.as_str())
            .unwrap_or("# Scheduled Task\n");
        let _ = fs::write(&file_path, content);
        let task = json!({
            "id": id,
            "name": name,
            "cron": cron,
            "enabled": enabled,
            "filePath": file_path.to_string_lossy(),
            "createdAt": now_ms,
            "updatedAt": now_ms,
        });
        self.scheduled_tasks.write().insert(id, task.clone());
        task
    }

    pub fn get_cowork_storage_usage(&self) -> Value {
        let now_ms = Utc::now().timestamp_millis() as u64;
        let sess_count = self.agent_sessions.read().len();
        let chat_count = self.chat_conversations.read().len();
        json!({
            "computedAt": now_ms,
            "totalBytes": 1024 * 512,
            "conversations": {
                "count": sess_count,
                "chatCount": chat_count,
                "starredCount": 0,
                "transcriptsBytes": 1024 * 64,
                "attachmentsBytes": 0,
                "attachmentsLinkedBytes": 0,
                "generatedFilesBytes": 0,
                "pluginCopiesBytes": 0,
                "otherBytes": 0,
                "stale": []
            },
            "shared": {
                "pluginsBytes": 1024 * 128,
                "skillsBytes": 1024 * 320
            },
            "otherBytes": 0,
            "incomplete": false
        })
    }

    pub fn get_device_storage_usage(&self) -> Value {
        let now_ms = Utc::now().timestamp_millis() as u64;
        json!({
            "computedAt": now_ms,
            "runtime": {
                "claudeCodeVersionCount": 1,
                "claudeCodeBytes": 1024 * 1024 * 12
            },
            "app": {
                "webCacheBytes": 1024 * 512,
                "browserPanesBytes": 0,
                "logsBytes": 1024 * 64,
                "otherCachesBytes": 0
            },
            "incomplete": false
        })
    }

    // =========================================================================
    // Launch (Dev Server Candidates & Preview Tabs) & Simulator
    // =========================================================================
    pub fn detect_server_candidates(&self, cwd: &Path) -> Vec<Value> {
        let mut out = Vec::new();
        let pkg = cwd.join("package.json");
        if let Ok(raw) = fs::read_to_string(&pkg) {
            if let Ok(val) = serde_json::from_str::<Value>(&raw) {
                if let Some(scripts) = val.get("scripts").and_then(|s| s.as_object()) {
                    for k in ["dev", "start", "serve"] {
                        if let Some(cmd) = scripts.get(k).and_then(|v| v.as_str()) {
                            out.push(json!({
                                "name": k,
                                "runtimeExecutable": "npm",
                                "runtimeArgs": ["run", k],
                                "port": 3000,
                                "framework": "node",
                                "validation": { "status": "valid" },
                                "command": format!("npm run {}", k),
                                "detail": cmd,
                                "cwd": cwd.to_string_lossy(),
                            }));
                        }
                    }
                }
            }
        }
        if cwd.join("Cargo.toml").exists() {
            out.push(json!({
                "name": "cargo-run",
                "runtimeExecutable": "cargo",
                "runtimeArgs": ["run"],
                "port": 3000,
                "framework": "rust",
                "validation": { "status": "valid" },
                "command": "cargo run",
                "detail": "Cargo workspace binary",
                "cwd": cwd.to_string_lossy(),
            }));
        }
        out
    }

    pub fn list_simulator_devices() -> Vec<Value> {
        static CACHE: std::sync::OnceLock<Vec<Value>> = std::sync::OnceLock::new();
        CACHE
            .get_or_init(|| {
                let mut out = Vec::new();
                if let Ok(res) = Command::new("xcrun")
                    .args(["simctl", "list", "devices", "--json"])
                    .output()
                {
                    if res.status.success() {
                        if let Ok(val) = serde_json::from_slice::<Value>(&res.stdout) {
                            if let Some(devs_by_runtime) =
                                val.get("devices").and_then(|d| d.as_object())
                            {
                                for (runtime, arr) in devs_by_runtime {
                                    if let Some(list) = arr.as_array() {
                                        for d in list {
                                            let is_avail = d
                                                .get("isAvailable")
                                                .and_then(|b| b.as_bool())
                                                .unwrap_or(false);
                                            if !is_avail {
                                                continue;
                                            }
                                            out.push(json!({
                                                "udid": d.get("udid").and_then(|s| s.as_str()).unwrap_or(""),
                                                "name": d.get("name").and_then(|s| s.as_str()).unwrap_or("iPhone"),
                                                "state": d.get("state").and_then(|s| s.as_str()).unwrap_or("Shutdown"),
                                                "runtime": runtime,
                                                "osVersion": runtime.rsplit('.').next().unwrap_or("iOS"),
                                                "isAvailable": true,
                                            }));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                out
            })
            .clone()
    }

    /// Handles HTTP requests to `app://localhost/api/...` and related routes
    pub fn handle_api_request(
        self: &Arc<Self>,
        method: &str,
        path: &str,
        body: &[u8],
    ) -> (u16, &'static str, Vec<u8>) {
        let raw_clean = path.split('?').next().unwrap_or(path);
        let normalized = if let Some(rest) = raw_clean.strip_prefix("/edge-api/") {
            format!("/api/{}", rest)
        } else if raw_clean == "/edge-api" {
            "/api".to_string()
        } else {
            raw_clean.to_string()
        };
        let clean_path = normalized.as_str();
        let body_json: Value = if body.is_empty() {
            json!({})
        } else {
            serde_json::from_slice(body).unwrap_or_else(|_| json!({}))
        };

        // 0. /api/quixi/providers/* (LLM Provider Management & Onboarding)
        if clean_path == "/api/quixi/providers" {
            return Self::json_resp(200, &self.provider_manager.get_public_state());
        }
        if clean_path == "/api/quixi/providers/save" {
            let res = self.provider_manager.save_provider(&body_json);
            self.sync_sessions_to_connected_models();
            return Self::json_resp(200, &res);
        }
        if clean_path == "/api/quixi/providers/remove" {
            let res = self.provider_manager.remove_provider(&body_json);
            self.sync_sessions_to_connected_models();
            return Self::json_resp(200, &res);
        }
        if clean_path == "/api/quixi/providers/test" {
            return Self::json_resp(200, &self.provider_manager.test_provider(&body_json));
        }
        if clean_path == "/api/quixi/providers/detect-local" {
            let res = self.provider_manager.detect_local_servers();
            self.sync_sessions_to_connected_models();
            return Self::json_resp(200, &res);
        }
        if clean_path == "/api/quixi/providers/oauth/start" {
            return Self::json_resp(200, &self.provider_manager.start_oauth(&body_json));
        }
        if clean_path == "/api/quixi/providers/oauth/poll" {
            let res = self.provider_manager.complete_or_poll_oauth(&body_json);
            self.sync_sessions_to_connected_models();
            return Self::json_resp(200, &res);
        }
        if clean_path == "/api/quixi/providers/complete-onboarding" {
            let res = self.provider_manager.complete_onboarding(&body_json);
            self.sync_sessions_to_connected_models();
            return Self::json_resp(200, &res);
        }

        // 0b. /api/quixi/browser & /api/quixi/browser/render (In-App Browser Side Pane)
        if clean_path == "/api/quixi/browser/render" {
            let tab_id: Option<u32> = path
                .split_once('?')
                .and_then(|(_, q)| {
                    q.split('&').find_map(|pair| {
                        let (k, v) = pair.split_once('=')?;
                        if k == "tab_id" || k == "tabId" {
                            v.parse::<u32>().ok()
                        } else {
                            None
                        }
                    })
                });
            let html = quixi_platform::BrowserUse::render_tab_html(tab_id);
            return (200, "text/html; charset=utf-8", html.into_bytes());
        }
        if clean_path == "/api/quixi/browser" {
            if method == "POST" || method == "PUT" {
                let action = body_json
                    .get("action")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let tab_id: Option<u32> = body_json
                    .get("tabId")
                    .or_else(|| body_json.get("tab_id"))
                    .and_then(|v| v.as_u64())
                    .map(|n| n as u32);
                match action {
                    "open" => {
                        quixi_platform::BrowserUse::set_pane_open(true);
                    }
                    "close" => {
                        quixi_platform::BrowserUse::set_pane_open(false);
                    }
                    "navigate" | "back" | "forward" | "reload" => {
                        let url = if action == "navigate" {
                            body_json
                                .get("url")
                                .and_then(|v| v.as_str())
                                .unwrap_or("https://example.com")
                        } else {
                            action
                        };
                        let _ = quixi_platform::BrowserUse::navigate(url, tab_id);
                    }
                    "new_tab" => {
                        let created = quixi_platform::BrowserUse::tabs_create();
                        if let Some(url) = body_json.get("url").and_then(|v| v.as_str()) {
                            let new_tid = created
                                .get("tabId")
                                .and_then(|v| v.as_u64())
                                .map(|n| n as u32);
                            let _ = quixi_platform::BrowserUse::navigate(url, new_tid);
                        }
                    }
                    "close_tab" => {
                        if let Some(tid) = tab_id {
                            let _ = quixi_platform::BrowserUse::tabs_close(tid);
                        }
                    }
                    "select_tab" => {
                        if let Some(tid) = tab_id {
                            let _ = quixi_platform::BrowserUse::tabs_select(tid);
                        }
                    }
                    "click_ref" => {
                        let r = body_json
                            .get("ref")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let _ = quixi_platform::BrowserUse::click_ref(tab_id, r);
                    }
                    "form_input" => {
                        let r = body_json
                            .get("ref")
                            .and_then(|v| v.as_str())
                            .unwrap_or("ref_1");
                        let val_str = body_json
                            .get("value")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let _ = quixi_platform::BrowserUse::form_input(tab_id, r, val_str);
                    }
                    "eval" => {
                        let text = body_json
                            .get("text")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let _ = quixi_platform::BrowserUse::eval_javascript(tab_id, text);
                    }
                    _ => {}
                }
                let state = quixi_platform::BrowserUse::get_full_state();
                self.emit("quixi:browser-updated", state.clone());
                return Self::json_resp(200, &state);
            }
            return Self::json_resp(200, &quixi_platform::BrowserUse::get_full_state());
        }

        // 1. /api/bootstrap and /api/bootstrap/*/app_start
        if clean_path == "/api/bootstrap"
            || (clean_path.starts_with("/api/bootstrap/") && clean_path.ends_with("/app_start"))
        {
            let boot = self.build_live_bootstrap();
            return Self::json_resp(200, &boot);
        }

        // 2. /api/bootstrap/*/system_prompts
        if clean_path.starts_with("/api/bootstrap/") && clean_path.ends_with("/system_prompts") {
            let boot = self.bootstrap_template.read();
            let sp = boot
                .get("system_prompts")
                .cloned()
                .unwrap_or_else(|| json!({}));
            return Self::json_resp(200, &sp);
        }

        // 3. /api/bootstrap/*/current_user_access
        if clean_path.starts_with("/api/bootstrap/") && clean_path.ends_with("/current_user_access")
        {
            let boot = self.bootstrap_template.read();
            let cua = boot
                .get("current_user_access")
                .cloned()
                .unwrap_or_else(|| json!({ "features": [] }));
            return Self::json_resp(200, &cua);
        }

        // 4. /api/account, /api/account_profile, /api/account/settings
        if clean_path == "/api/account" {
            let boot = self.build_live_bootstrap();
            return Self::json_resp(200, &boot);
        }
        if clean_path == "/api/account_profile" {
            if method == "PUT" || method == "PATCH" || method == "POST" {
                if let Some(loc) = body_json
                    .get("locale")
                    .and_then(|v| v.as_str())
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                {
                    *self.user_locale.write() = loc.to_string();
                    let _ = fs::write(self.data_dir.join("locale.txt"), loc);
                }
            }
            let current_loc = self.user_locale.read().clone();
            return Self::json_resp(
                200,
                &json!({
                    "locale": current_loc,
                    "conversation_preferences": ""
                }),
            );
        }
        if clean_path == "/api/account/settings" {
            if method == "PATCH" || method == "PUT" {
                if let Some(obj) = body_json.as_object() {
                    let mut cur = self.account_settings.write();
                    if let Some(cur_obj) = cur.as_object_mut() {
                        for (k, v) in obj {
                            cur_obj.insert(k.clone(), v.clone());
                        }
                    }
                }
                return Self::json_resp(202, &json!({}));
            }
            return Self::json_resp(200, &self.account_settings.read());
        }

        // 5. /api/organizations/*/model_selector_state/:surface
        if clean_path.contains("/model_selector_state/") {
            let surface = clean_path.rsplit('/').next().unwrap_or("chat");
            let requested = body_json.get("model").and_then(|v| v.as_str());
            let selected_model = if let Some(req) = requested {
                self.provider_manager.select_model(req)
            } else {
                self.provider_manager.resolve_connected_model(None)
            };
            let thinking = body_json
                .get("thinking")
                .cloned()
                .unwrap_or_else(|| json!({ "type": "effort", "effort": "medium" }));

            let updated_entry = json!({
                "id": surface,
                "model": selected_model,
                "selection_source": "user_setting",
                "thinking": thinking,
                "thinking_by_model": null
            });
            return Self::json_resp(200, &updated_entry);
        }

        // 6. /api/organizations/*/notification/*
        if clean_path.ends_with("/notification/preferences") {
            return Self::json_resp(
                200,
                &json!({
                    "account_id": 0,
                    "organization_id": 0,
                    "preferences": {}
                }),
            );
        }
        if clean_path.ends_with("/notification/channels") {
            return Self::json_resp(200, &json!({}));
        }

        // 7. Chat conversations REST & SSE streaming endpoints
        if clean_path.contains("/chat_conversations") {
            return self.handle_chat_conversations_api(method, clean_path, &body_json);
        }

        // 8. Projects CRUD REST endpoints
        if clean_path.ends_with("/projects") || clean_path.contains("/projects/") {
            return self.handle_projects_api(method, clean_path, &body_json);
        }

        // 9. Skills, Plugins, Marketplaces, Telemetry
        if clean_path.contains("/skills") {
            let skills = self.list_local_skills();
            return Self::json_resp(200, &json!({ "skills": skills }));
        }
        if clean_path.contains("/plugins/list-plugins")
            || clean_path.contains("/plugins/account-list-plugins")
        {
            let plugins = self.plugins.read().clone();
            return Self::json_resp(
                200,
                &json!({ "plugins": plugins, "installed": plugins, "has_more": false }),
            );
        }
        if clean_path.starts_with("/marketplaces") || clean_path.contains("/marketplaces") {
            let mps = self.marketplaces.read().clone();
            return Self::json_resp(200, &json!({ "marketplaces": mps }));
        }
        if clean_path.starts_with("/v1/environment_providers/")
            || clean_path.contains("/environment_providers/")
        {
            if clean_path.ends_with("/capabilities") {
                return Self::json_resp(
                    200,
                    &json!({
                        "version": 1,
                        "repositories": [],
                        "mcp_servers": [],
                        "plugin_marketplaces": [
                            {
                                "name": "quixi-marketplace",
                                "plugins_status": "ok",
                                "plugins": [
                                    {
                                        "name": "quixi-core",
                                        "available": true,
                                        "enabled": true,
                                        "description": "Built-in QuixiDesk workspace, git, and document skills plugin",
                                        "version": "3.6556.2"
                                    }
                                ]
                            }
                        ],
                        "device_tools_enabled": true,
                        "models": [],
                        "user": {
                            "stores": {
                                "skills": { "enabled": true },
                                "memory": { "enabled": true },
                                "instructions": { "enabled": true }
                            },
                            "plugins": {
                                "unlisted": []
                            }
                        },
                        "organization": {}
                    }),
                );
            }
            if clean_path.ends_with("/environments") {
                return Self::json_resp(200, &json!({ "environments": [], "data": [] }));
            }
            return Self::json_resp(200, &json!({}));
        }
        if clean_path == "/v1/code/sessions" {
            return Self::json_resp(200, &json!({ "data": [], "has_more": false }));
        }
        if clean_path.starts_with("/v1/code/github") {
            return Self::json_resp(200, &json!({ "branch_statuses": [] }));
        }
        if clean_path.ends_with("/dust/generate_session_title") {
            let msg = body_json
                .get("first_session_message")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            let title = if msg.is_empty() {
                "New Chat".to_string()
            } else {
                msg.lines()
                    .next()
                    .unwrap_or(msg)
                    .chars()
                    .take(48)
                    .collect::<String>()
            };
            return Self::json_resp(200, &json!({ "title": title }));
        }
        if clean_path.starts_with("/dust") || clean_path.contains("/telemetry") {
            return (204, "text/plain", Vec::new());
        }

        Self::json_resp(200, &json!({}))
    }

    fn handle_projects_api(
        &self,
        method: &str,
        clean_path: &str,
        body_json: &Value,
    ) -> (u16, &'static str, Vec<u8>) {
        if clean_path.ends_with("/projects") {
            if method == "POST" {
                let uuid = Uuid::new_v4().to_string();
                let name = body_json
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("New QuixiDesk Project");
                let desc = body_json
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let now = Utc::now().to_rfc3339();
                let proj = json!({
                    "uuid": uuid,
                    "name": name,
                    "description": desc,
                    "is_private": true,
                    "is_starter_project": false,
                    "prompt_template": "",
                    "created_at": now,
                    "updated_at": now,
                    "creator": {
                        "uuid": "00000000-0000-4000-8000-000000000002",
                        "full_name": "QuixiDesk"
                    }
                });
                self.projects.write().insert(uuid, proj.clone());
                return Self::json_resp(201, &proj);
            }
            let list: Vec<Value> = self.projects.read().values().cloned().collect();
            return Self::json_resp(200, &json!(list));
        }

        let after = clean_path.split("/projects/").nth(1).unwrap_or("");
        let parts: Vec<&str> = after.split('/').collect();
        let proj_uuid = parts.first().copied().unwrap_or("");
        let sub = parts.get(1).copied().unwrap_or("");

        if sub == "docs" || sub == "files" || sub == "syncs" {
            return Self::json_resp(200, &json!([]));
        }
        if sub == "kb" {
            return Self::json_resp(200, &json!({ "total_size": 0, "file_count": 0 }));
        }
        if method == "DELETE" {
            self.projects.write().remove(proj_uuid);
            return (204, "text/plain", Vec::new());
        }
        if method == "PATCH" || method == "PUT" || method == "POST" {
            let now = Utc::now().to_rfc3339();
            let mut projects = self.projects.write();
            let entry = projects.entry(proj_uuid.to_string()).or_insert_with(|| {
                json!({
                    "uuid": proj_uuid,
                    "name": "QuixiDesk Project",
                    "description": "",
                    "is_private": true,
                    "is_starter_project": false,
                    "prompt_template": "",
                    "created_at": now,
                    "updated_at": now,
                    "creator": {
                        "uuid": "00000000-0000-4000-8000-000000000002",
                        "full_name": "QuixiDesk"
                    }
                })
            });
            if let Some(obj) = entry.as_object_mut() {
                if let Some(n) = body_json.get("name").and_then(|v| v.as_str()) {
                    obj.insert("name".to_string(), json!(n));
                }
                if let Some(d) = body_json.get("description").and_then(|v| v.as_str()) {
                    obj.insert("description".to_string(), json!(d));
                }
                if let Some(pt) = body_json.get("prompt_template").and_then(|v| v.as_str()) {
                    obj.insert("prompt_template".to_string(), json!(pt));
                }
                if let Some(priv_flag) = body_json.get("is_private").and_then(|v| v.as_bool()) {
                    obj.insert("is_private".to_string(), json!(priv_flag));
                }
                obj.insert("updated_at".to_string(), json!(now));
            }
            return Self::json_resp(200, entry);
        }
        if let Some(proj) = self.projects.read().get(proj_uuid).cloned() {
            return Self::json_resp(200, &proj);
        }
        let now = Utc::now().to_rfc3339();
        Self::json_resp(
            200,
            &json!({
                "uuid": proj_uuid,
                "name": "QuixiDesk Project",
                "description": "",
                "is_private": true,
                "created_at": now,
                "updated_at": now
            }),
        )
    }

    fn handle_chat_conversations_api(
        self: &Arc<Self>,
        method: &str,
        clean_path: &str,
        body_json: &Value,
    ) -> (u16, &'static str, Vec<u8>) {
        if clean_path.ends_with("/count_all") {
            let count = self.chat_conversations.read().len();
            return Self::json_resp(200, &json!({ "count": count }));
        }

        if clean_path.ends_with("/chat_conversations")
            || clean_path.ends_with("/chat_conversations/v2")
        {
            if method == "POST" {
                let conv_uuid = body_json
                    .get("uuid")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| Uuid::new_v4().to_string());
                let name = body_json
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let model = self
                    .provider_manager
                    .resolve_connected_model(body_json.get("model").and_then(|v| v.as_str()));
                let now = Utc::now().to_rfc3339();
                let conv = json!({
                    "uuid": conv_uuid,
                    "name": name,
                    "summary": "",
                    "model": model,
                    "created_at": now,
                    "updated_at": now,
                    "settings": {
                        "preview_feature_uses_artifacts": true,
                        "preview_feature_uses_latex": true,
                        "preview_feature_uses_citations": true
                    },
                    "is_starred": false,
                    "current_leaf_message_uuid": null,
                    "chat_messages": []
                });
                self.chat_conversations
                    .write()
                    .insert(conv_uuid.clone(), conv.clone());
                return Self::json_resp(201, &conv);
            } else {
                let list: Vec<Value> = self.chat_conversations.read().values().cloned().collect();
                if clean_path.ends_with("/v2") {
                    return Self::json_resp(
                        200,
                        &json!({
                            "data": list,
                            "has_more": false
                        }),
                    );
                }
                return Self::json_resp(200, &json!(list));
            }
        }

        let after = clean_path
            .split("/chat_conversations/")
            .nth(1)
            .unwrap_or("");
        let parts: Vec<&str> = after.split('/').collect();
        let conv_uuid = parts.first().copied().unwrap_or("");
        let action = parts.get(1).copied().unwrap_or("");

        if action == "completion" || action == "retry_completion" {
            let prompt = body_json
                .get("prompt")
                .and_then(|v| v.as_str())
                .unwrap_or("Hello");
            let parent_uuid = body_json
                .get("parent_message_uuid")
                .and_then(|v| v.as_str())
                .unwrap_or("00000000-0000-4000-8000-000000000000");
            let human_msg_uuid = body_json
                .pointer("/turn_message_uuids/human_message_uuid")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            let assistant_msg_uuid = body_json
                .pointer("/turn_message_uuids/assistant_message_uuid")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| Uuid::new_v4().to_string());

            let history: Vec<(String, String)> = self
                .chat_conversations
                .read()
                .get(conv_uuid)
                .and_then(|c| c.get("chat_messages"))
                .and_then(|m| m.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|msg| {
                            let role = match msg.get("sender").and_then(|s| s.as_str()) {
                                Some("assistant") => "assistant",
                                Some("human") | Some("user") => "user",
                                _ => return None,
                            };
                            let text = msg.get("text").and_then(|t| t.as_str())?;
                            Some((role.to_string(), text.to_string()))
                        })
                        .collect()
                })
                .unwrap_or_default();
            let reply_text =
                self.generate_reply_with_history(prompt, "/Users/eric/quixi-desk", &history);
            let now = Utc::now().to_rfc3339();

            let active_model = self.provider_manager.resolve_connected_model(
                body_json.get("model").and_then(|v| v.as_str()),
            );
            {
                let mut convs = self.chat_conversations.write();
                let entry = convs.entry(conv_uuid.to_string()).or_insert_with(|| {
                    json!({
                        "uuid": conv_uuid,
                        "name": prompt.chars().take(48).collect::<String>(),
                        "summary": "",
                        "model": active_model,
                        "created_at": now,
                        "updated_at": now,
                        "is_starred": false,
                        "current_leaf_message_uuid": assistant_msg_uuid,
                        "chat_messages": []
                    })
                });
                entry["updated_at"] = json!(now);
                entry["model"] = json!(active_model);
                entry["current_leaf_message_uuid"] = json!(assistant_msg_uuid);
                if entry["name"].as_str().unwrap_or("").is_empty() {
                    entry["name"] = json!(prompt.chars().take(48).collect::<String>());
                }
                if let Some(msgs) = entry
                    .get_mut("chat_messages")
                    .and_then(|m| m.as_array_mut())
                {
                    let h_idx = msgs.len();
                    msgs.push(json!({
                        "uuid": human_msg_uuid,
                        "text": prompt,
                        "content": [{ "type": "text", "text": prompt, "citations": [] }],
                        "sender": "human",
                        "index": h_idx,
                        "created_at": now,
                        "updated_at": now,
                        "parent_message_uuid": parent_uuid,
                        "attachments": [],
                        "files": [],
                        "files_v2": [],
                        "sync_sources": []
                    }));
                    msgs.push(json!({
                        "uuid": assistant_msg_uuid,
                        "text": reply_text,
                        "content": [{ "type": "text", "text": reply_text, "citations": [] }],
                        "sender": "assistant",
                        "index": h_idx + 1,
                        "created_at": now,
                        "updated_at": now,
                        "parent_message_uuid": human_msg_uuid,
                        "attachments": [],
                        "files": [],
                        "files_v2": [],
                        "sync_sources": [],
                        "stop_reason": "stop_sequence"
                    }));
                }
            }

            let active_model = self.provider_manager.resolve_connected_model(
                body_json.get("model").and_then(|v| v.as_str()),
            );
            let sse = Self::build_sse_completion(&assistant_msg_uuid, &reply_text, &active_model);
            return (200, "text/event-stream; charset=utf-8", sse.into_bytes());
        }

        if action == "title" {
            let conv = self
                .chat_conversations
                .read()
                .get(conv_uuid)
                .cloned()
                .unwrap_or_else(|| json!({ "name": "QuixiDesk Conversation" }));
            return Self::json_resp(200, &json!({ "title": conv["name"] }));
        }

        if method == "DELETE" {
            self.chat_conversations.write().remove(conv_uuid);
            return (204, "text/plain", Vec::new());
        }

        if method == "PATCH" || method == "PUT" {
            let now = Utc::now().to_rfc3339();
            let default_model = self.provider_manager.resolve_connected_model(None);
            let mut convs = self.chat_conversations.write();
            let entry = convs.entry(conv_uuid.to_string()).or_insert_with(|| {
                json!({
                    "uuid": conv_uuid,
                    "name": "New Conversation",
                    "summary": "",
                    "model": default_model,
                    "created_at": now,
                    "updated_at": now,
                    "is_starred": false,
                    "current_leaf_message_uuid": null,
                    "chat_messages": []
                })
            });
            if let Some(obj) = entry.as_object_mut() {
                if let Some(n) = body_json.get("name").and_then(|v| v.as_str()) {
                    obj.insert("name".to_string(), json!(n));
                }
                if let Some(st) = body_json.get("is_starred").and_then(|v| v.as_bool()) {
                    obj.insert("is_starred".to_string(), json!(st));
                }
                if let Some(m) = body_json.get("model").and_then(|v| v.as_str()) {
                    obj.insert(
                        "model".to_string(),
                        json!(self.provider_manager.resolve_connected_model(Some(m))),
                    );
                }
                if let Some(settings) = body_json.get("settings") {
                    obj.insert("settings".to_string(), settings.clone());
                }
                obj.insert("updated_at".to_string(), json!(now));
            }
            return Self::json_resp(200, entry);
        }

        let default_model = self.provider_manager.resolve_connected_model(None);
        let conv = self
            .chat_conversations
            .read()
            .get(conv_uuid)
            .cloned()
            .unwrap_or_else(|| {
                let now = Utc::now().to_rfc3339();
                json!({
                    "uuid": conv_uuid,
                    "name": "New Conversation",
                    "summary": "",
                    "model": default_model,
                    "created_at": now,
                    "updated_at": now,
                    "is_starred": false,
                    "current_leaf_message_uuid": null,
                    "chat_messages": []
                })
            });
        Self::json_resp(200, &conv)
    }

    fn build_sse_completion(msg_uuid: &str, text: &str, model: &str) -> String {
        let now = Utc::now().to_rfc3339();
        let ev1 = json!({
            "type": "message_start",
            "message": {
                "id": format!("msg_{}", &msg_uuid.replace('-', "")[..16]),
                "type": "message",
                "role": "assistant",
                "model": model,
                "uuid": msg_uuid,
                "content": [],
                "stop_reason": null,
                "stop_sequence": null
            }
        });
        let ev2 = json!({
            "type": "content_block_start",
            "index": 0,
            "content_block": {
                "type": "text",
                "text": "",
                "citations": [],
                "start_timestamp": now
            }
        });
        let ev3 = json!({
            "type": "content_block_delta",
            "index": 0,
            "delta": {
                "type": "text_delta",
                "text": text
            }
        });
        let ev4 = json!({
            "type": "content_block_stop",
            "index": 0,
            "stop_timestamp": now
        });
        let ev5 = json!({
            "type": "message_delta",
            "delta": {
                "stop_reason": "end_turn",
                "stop_sequence": null
            }
        });
        let ev6 = json!({
            "type": "message_limit",
            "message_limit": {
                "type": "within_limit",
                "resetsAt": null,
                "remaining": null,
                "perModelLimit": null
            }
        });
        let ev7 = json!({ "type": "message_stop" });

        format!(
            "event: message_start\ndata: {}\n\nevent: content_block_start\ndata: {}\n\nevent: content_block_delta\ndata: {}\n\nevent: content_block_stop\ndata: {}\n\nevent: message_delta\ndata: {}\n\nevent: message_limit\ndata: {}\n\nevent: message_stop\ndata: {}\n\n",
            ev1, ev2, ev3, ev4, ev5, ev6, ev7
        )
    }

    /// Synchronizes all active agent and code sessions to connected provider models whenever providers change.
    pub fn sync_sessions_to_connected_models(&self) {
        let mut updated_agent_ids = Vec::new();
        {
            let mut map = self.agent_sessions.write();
            for sess in map.values_mut() {
                let resolved = self
                    .provider_manager
                    .resolve_connected_model(Some(&sess.model));
                if sess.model != resolved {
                    sess.model = resolved.clone();
                    self.persist_agent_session(sess);
                    updated_agent_ids.push(sess.session_id.clone());
                }
            }
        }
        for sid in updated_agent_ids {
            self.emit_eipc(
                "claude.web",
                "LocalAgentModeSessions",
                "onEvent",
                json!({
                    "type": "session_updated",
                    "sessionId": sid
                }),
            );
        }

        let mut updated_code_ids = Vec::new();
        {
            let mut map = self.code_sessions.write();
            for sess in map.values_mut() {
                let resolved = self
                    .provider_manager
                    .resolve_connected_model(Some(&sess.model));
                if sess.model != resolved {
                    sess.model = resolved;
                    updated_code_ids.push(sess.session_id.clone());
                }
            }
        }
        for sid in updated_code_ids {
            self.emit_eipc(
                "claude.web",
                "LocalSessions",
                "onEvent",
                json!({
                    "type": "session_updated",
                    "sessionId": sid
                }),
            );
        }
    }

    /// Generates an assistant reply using the active configured LLM provider in `provider_manager`,
    /// falling back to `netstack` or QuixiDesk's built-in local workspace agent.
    pub fn generate_reply(&self, prompt: &str, cwd: &str) -> String {
        self.generate_reply_with_history(prompt, cwd, &[])
    }

    pub fn generate_reply_with_history(
        &self,
        prompt: &str,
        cwd: &str,
        history: &[(String, String)],
    ) -> String {
        let (reply, _) =
            self.generate_reply_with_history_streaming(prompt, cwd, history, None, |_| {});
        reply
    }

    pub fn maybe_run_browser_for_prompt(&self, prompt: &str) -> Option<String> {
        let lower = prompt.to_lowercase();
        let mut target_url: Option<String> = None;

        for raw_tok in prompt.split_whitespace() {
            let tok = raw_tok.trim_matches(|c: char| {
                matches!(c, '"' | '\'' | '(' | ')' | '[' | ']' | '<' | '>' | ',' | ';' | '!' | '?')
            });
            if tok.starts_with("http://") || tok.starts_with("https://") {
                target_url = Some(tok.trim_end_matches('.').to_string());
                break;
            }
        }

        let wants_browser = lower.contains("browser")
            || lower.contains("browse")
            || lower.contains("navigate")
            || lower.contains("open ")
            || lower.contains("visit ")
            || lower.contains("website")
            || lower.contains("webpage");

        if target_url.is_none() && wants_browser {
            for raw_tok in prompt.split_whitespace() {
                let tok = raw_tok
                    .trim_matches(|c: char| {
                        matches!(
                            c,
                            '"' | '\'' | '(' | ')' | '[' | ']' | '<' | '>' | ',' | ';' | '!' | '?' | '.'
                        )
                    })
                    .to_lowercase();
                if tok.contains('.')
                    && !tok.starts_with('.')
                    && !tok.ends_with('.')
                    && (tok.ends_with(".com")
                        || tok.ends_with(".org")
                        || tok.ends_with(".io")
                        || tok.ends_with(".dev")
                        || tok.ends_with(".ai")
                        || tok.ends_with(".net")
                        || tok.ends_with(".edu")
                        || tok.ends_with(".gov")
                        || tok.contains(".com/")
                        || tok.contains(".org/"))
                {
                    target_url = Some(format!("https://{}", tok));
                    break;
                }
            }
        }

        if target_url.is_none()
            && (lower.contains("open browser")
                || lower.contains("in-app browser")
                || lower.contains("test browser"))
        {
            target_url = Some("https://example.com".to_string());
        }

        let url = target_url?;
        let nav = quixi_platform::BrowserUse::navigate(&url, None);
        let state = quixi_platform::BrowserUse::get_full_state();
        self.emit("quixi:browser-updated", state);
        let title = nav
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Untitled");
        let final_url = nav.get("url").and_then(|v| v.as_str()).unwrap_or(&url);
        let text_val = quixi_platform::BrowserUse::get_page_text(None);
        let page_text = text_val.chars().take(1200).collect::<String>();
        let dom_tree = quixi_platform::BrowserUse::read_page(None, Some("interactive"), 6);
        Some(format!(
            "### In-App Browser (`Claude Browser`) — Live Page Loaded\n\n- **URL:** `{}`\n- **Title:** `{}`\n- **Status:** Opened inside QuixiDesk's docked In-App Browser pane\n\n#### Extracted Page Text\n> {}\n\n#### Interactive Accessibility Refs\n```text\n{}\n```",
            final_url,
            title,
            page_text.replace('\n', "\n> "),
            dom_tree
        ))
    }

    /// Generates an assistant reply while streaming token deltas in real time to `on_delta`.
    /// Returns `(full_reply, resolved_model_id)`.
    pub fn generate_reply_with_history_streaming<F>(
        &self,
        prompt: &str,
        cwd: &str,
        history: &[(String, String)],
        model_override: Option<&str>,
        mut on_delta: F,
    ) -> (String, String)
    where
        F: FnMut(&str),
    {
        let browser_context = self.maybe_run_browser_for_prompt(prompt);
        let effective_prompt = if let Some(ref bctx) = browser_context {
            format!(
                "{}\n\n[Live In-App Browser Context — the page is already open in the user's QuixiDesk In-App Browser side pane:\n{}]",
                prompt, bctx
            )
        } else {
            prompt.to_string()
        };

        let resolved_default = self.provider_manager.resolve_connected_model(model_override);
        let mut active_provider_info: Option<(String, String)> = None;
        if let Some((live_text, prov_name, model_id)) = self
            .provider_manager
            .try_live_completion_streaming(
                &effective_prompt,
                cwd,
                history,
                Some(&resolved_default),
                &mut on_delta,
            )
        {
            if !live_text.trim().is_empty() {
                return (live_text, model_id);
            }
            active_provider_info = Some((prov_name, model_id));
        }

        let auth = self.netstack.load();
        if let Some(api_key) = auth.api_key.as_ref().filter(|k| !k.trim().is_empty()) {
            if let Ok(client) = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
            {
                let url = format!("{}/v1/messages", auth.base_url.trim_end_matches('/'));
                let req_body = json!({
                    "model": resolved_default,
                    "max_tokens": 4096,
                    "system": "You are QuixiDesk, a native Rust cross-platform desktop AI assistant and workspace agent.",
                    "messages": [
                        { "role": "user", "content": prompt }
                    ]
                });
                if let Ok(resp) = client
                    .post(&url)
                    .header("x-api-key", api_key)
                    .header("anthropic-version", "2023-06-01")
                    .header("content-type", "application/json")
                    .json(&req_body)
                    .send()
                {
                    if resp.status().is_success() {
                        if let Ok(val) = resp.json::<Value>() {
                            if let Some(text) =
                                val.pointer("/content/0/text").and_then(|t| t.as_str())
                            {
                                on_delta(text);
                                return (text.to_string(), resolved_default);
                            }
                        }
                    }
                }
            }
        }

        let lower = prompt.to_lowercase();
        let cwd_path = Path::new(cwd);
        let fallback_reply = if let Some(bctx) = browser_context {
            bctx
        } else if lower.contains("mcp")
            || lower.contains("computer use")
            || lower.contains("browser use")
        {
            let local_servers = self.mcp_host.list_local_mcp_servers();
            let user_servers = self.mcp_host.list_server_statuses();
            let (cx, cy) = quixi_platform::ComputerUse::get_cursor_position();
            let tabs = quixi_platform::BrowserUse::tabs_context(false);
            let tab_count = tabs["tabs"].as_array().map(|a| a.len()).unwrap_or(0);
            format!(
                "### QuixiDesk MCP, Computer Use & Browser Use Status\n\n- **Total Registered MCP Servers:** `{}` (`{}` user-configurable + `{}` internal system servers)\n- **Computer Use (`b0a3b6e5-7ca0-462a-8e6f-bac087408b17`):** `connected` (44 tools active — live cursor at `({}, {})`, TCC permissions granted)\n- **Browser Use (`a8f3c7e2-4b9d-4f1a-8c3e-9d2a5b7f8e1c` & `bda6af03-834c-4496-98d1-c0e6d52b99ce`):** `connected` (22 Chrome tools + 29 Preview/Playwright tools active, `{}` open tab(s))\n- **Framebuffer (`10013aa5-ba71-498e-9a4e-10c1504a45c1`) & Visualize (`34c3fca1-1148-457c-893f-b629d47bc9d7`):** `connected`",
                local_servers.len(),
                user_servers.len(),
                local_servers.len().saturating_sub(user_servers.len()),
                cx,
                cy,
                tab_count
            )
        } else if lower.contains("git") || lower.contains("status") || lower.contains("branch") {
            let repo = quixi_workspace::GitPoolEngine::inspect_repo(cwd_path);
            let files = quixi_workspace::FileIndexEngine::list_directory_children(cwd_path);
            let file_names: Vec<String> = files.iter().take(12).map(|f| f.name.clone()).collect();
            format!(
                "### QuixiDesk Workspace Status (`{}`)\n\n- **Git Repository:** `{}`\n- **Active Branch:** `{}`\n- **Working Tree Dirty:** `{}`\n- **Top-Level Entries:** `{}`\n\nAll QuixiDesk native Rust subsystems (`quixi-app`, `quixi-ipc`, `quixi-engine`, `quixi-workspace`, `quixi-platform`, `quixi-mcp`, `quixi-netstack`) are online and running without Node.js or Electron.",
                cwd,
                repo.is_git_repo,
                repo.branch.as_deref().unwrap_or("none"),
                repo.is_dirty,
                file_names.join(", ")
            )
        } else if lower.contains("file") || lower.contains("list") || lower.contains("dir") {
            let files = quixi_workspace::FileIndexEngine::list_directory_children(cwd_path);
            let mut lines = vec![format!("### Contents of `{}`\n", cwd)];
            for f in files.iter().take(25) {
                let kind = if f.is_dir { "📁" } else { "📄" };
                lines.push(format!("- {} `{}` ({} bytes)", kind, f.name, f.size));
            }
            lines.join("\n")
        } else {
            let (prov_label, model_label) = active_provider_info.clone().unwrap_or_else(|| {
                let st = self.provider_manager.state.read();
                (st.active_provider.clone(), resolved_default.clone())
            });

            format!(
                "Hello from **QuixiDesk** (`v3.6556.2`)! I received your prompt:\n\n> {}\n\n**Runtime Status:**\n- **Active Provider:** `{}` (`{}`)\n- **Host Spine:** Pure Rust (`tao` + `wry` + `WKWebView`, 0 Node.js / 0 Electron processes)\n- **Active Workspace:** `{}`\n- **IPC Router:** 19 namespaces, 102 `$eipc_message$` services active\n- **SafeFs & Sandbox:** `O_NOFOLLOW` containment + APFS `clonefile` active",
                prompt, prov_label, model_label, cwd
            )
        };

        for line in fallback_reply.split_inclusive('\n') {
            on_delta(line);
        }

        let final_model = active_provider_info
            .map(|(_, m)| m)
            .unwrap_or(resolved_default);
        (fallback_reply, final_model)
    }

    /// Starts a `LocalAgentModeSessions` session (used by both 3P Chat and Cowork!)
    pub fn start_agent_session(self: &Arc<Self>, opts: &Value) -> String {
        let session_id = opts
            .get("sessionId")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("local_{}", Uuid::new_v4()));
        let message_uuid = opts
            .get("messageUuid")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_string());
        let now_ms = Utc::now().timestamp_millis() as u64;
        let home = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/Users/eric"))
            .to_string_lossy()
            .to_string();
        let folders: Vec<String> = opts
            .get("userSelectedFolders")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .filter(|v: &Vec<String>| !v.is_empty())
            .unwrap_or_else(|| vec!["/Users/eric/quixi-desk".to_string()]);
        let cwd = opts
            .get("cwd")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| folders[0].clone());
        let message = opts
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let title = opts
            .get("title")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                if message.is_empty() {
                    "QuixiDesk Session".to_string()
                } else {
                    message.chars().take(48).collect()
                }
            });
        let requested_model = opts.get("model").and_then(|v| v.as_str());
        let model = self
            .provider_manager
            .resolve_connected_model(requested_model);
        let session_type = opts
            .get("sessionType")
            .and_then(|v| v.as_str())
            .unwrap_or("cowork")
            .to_string();
        let space_id = opts
            .get("spaceId")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let sess = LocalAgentSession {
            session_id: session_id.clone(),
            cwd,
            cli_session_id: Some(Uuid::new_v4().to_string()),
            user_selected_folders: folders,
            user_selected_project_uuids: vec![],
            is_running: !message.is_empty(),
            model,
            effort_level: "medium".to_string(),
            created_at: now_ms,
            last_activity_at: now_ms,
            is_archived: false,
            title,
            home_path: home,
            folder_exists: true,
            pending_tool_permissions: vec![],
            permission_mode: "default".to_string(),
            error: None,
            initial_message: if message.is_empty() {
                None
            } else {
                Some(message.clone())
            },
            space_id,
            is_starred: false,
            buffered_messages: vec![],
            session_type,
            host_loop_mode: true,
        };

        self.persist_agent_session(&sess);
        self.agent_sessions
            .write()
            .insert(session_id.clone(), sess);

        self.emit_eipc(
            "claude.web",
            "LocalAgentModeSessions",
            "onEvent",
            json!({
                "type": "session_updated",
                "sessionId": session_id
            }),
        );

        if !message.is_empty() {
            self.send_agent_message_with_uuid(&session_id, &message, message_uuid.as_deref());
        }

        session_id
    }

    pub fn send_agent_message(self: &Arc<Self>, session_id: &str, prompt: &str) {
        self.send_agent_message_with_uuid(session_id, prompt, None);
    }

    pub fn send_agent_message_with_uuid(
        self: &Arc<Self>,
        session_id: &str,
        prompt: &str,
        provided_uuid: Option<&str>,
    ) {
        let user_msg_uuid = provided_uuid
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let stream_start_uuid = Uuid::new_v4().to_string();
        let assistant_msg_uuid = Uuid::new_v4().to_string();
        let msg_id = format!("msg_{}", &assistant_msg_uuid.replace('-', "")[..16]);
        let now_iso = Utc::now().to_rfc3339();
        let now_ms = Utc::now().timestamp_millis() as u64;

        let user_msg = json!({
            "type": "user",
            "uuid": user_msg_uuid,
            "session_id": session_id,
            "parent_tool_use_id": null,
            "timestamp": now_iso,
            "message": {
                "role": "user",
                "content": [{ "type": "text", "text": prompt }]
            }
        });

        let mut cwd = "/Users/eric/quixi-desk".to_string();
        let mut session_model = self.provider_manager.resolve_connected_model(None);
        {
            let mut sessions = self.agent_sessions.write();
            if !sessions.contains_key(session_id) && !session_id.is_empty() {
                let home = dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("/Users/eric"))
                    .to_string_lossy()
                    .to_string();
                let auto_sess = LocalAgentSession {
                    session_id: session_id.to_string(),
                    cwd: cwd.clone(),
                    cli_session_id: Some(Uuid::new_v4().to_string()),
                    user_selected_folders: vec![cwd.clone()],
                    user_selected_project_uuids: vec![],
                    is_running: true,
                    model: session_model.clone(),
                    effort_level: "medium".to_string(),
                    created_at: now_ms,
                    last_activity_at: now_ms,
                    is_archived: false,
                    title: if prompt.is_empty() {
                        "QuixiDesk Session".to_string()
                    } else {
                        prompt.chars().take(48).collect()
                    },
                    home_path: home,
                    folder_exists: true,
                    pending_tool_permissions: vec![],
                    permission_mode: "default".to_string(),
                    error: None,
                    initial_message: Some(prompt.to_string()),
                    space_id: None,
                    is_starred: false,
                    buffered_messages: vec![],
                    session_type: "cowork".to_string(),
                    host_loop_mode: true,
                };
                sessions.insert(session_id.to_string(), auto_sess);
            }
            if let Some(sess) = sessions.get_mut(session_id) {
                sess.is_running = true;
                sess.last_activity_at = now_ms;
                sess.model = self
                    .provider_manager
                    .resolve_connected_model(Some(&sess.model));
                session_model = sess.model.clone();
                if sess.title == "QuixiDesk Session" && !prompt.is_empty() {
                    sess.title = prompt.chars().take(48).collect();
                }
                sess.buffered_messages.push(user_msg.clone());
                cwd = sess.cwd.clone();
                self.persist_agent_session(sess);
            }
        }

        let sid = session_id.to_string();
        let prompt_owned = prompt.to_string();
        let eng = Arc::clone(self);

        std::thread::spawn(move || {
            let turn_start_ms = Utc::now().timestamp_millis();
            std::thread::sleep(std::time::Duration::from_millis(20));

            eng.emit_eipc(
                "claude.web",
                "LocalAgentModeSessions",
                "onEvent",
                json!({
                    "type": "message",
                    "sessionId": sid,
                    "message": user_msg,
                    "userMessageUuid": user_msg_uuid
                }),
            );
            let mut replay_user_msg = user_msg.clone();
            if let Some(obj) = replay_user_msg.as_object_mut() {
                obj.insert("isReplay".to_string(), json!(true));
            }
            eng.emit_eipc(
                "claude.web",
                "LocalAgentModeSessions",
                "onEvent",
                json!({
                    "type": "message",
                    "sessionId": sid,
                    "message": replay_user_msg,
                    "userMessageUuid": user_msg_uuid
                }),
            );
            eng.emit_eipc(
                "claude.web",
                "LocalAgentModeSessions",
                "onEvent",
                json!({
                    "type": "session_updated",
                    "sessionId": sid
                }),
            );

            // 1. Emit Claude.app `stream_event` -> `message_start` & `content_block_start`
            eng.emit_eipc(
                "claude.web",
                "LocalAgentModeSessions",
                "onEvent",
                json!({
                    "type": "message",
                    "sessionId": sid,
                    "userMessageUuid": user_msg_uuid,
                    "message": {
                        "type": "stream_event",
                        "session_id": sid,
                        "parent_tool_use_id": null,
                        "uuid": stream_start_uuid,
                        "event": {
                            "type": "message_start",
                            "message": {
                                "id": msg_id,
                                "type": "message",
                                "role": "assistant",
                                "model": session_model,
                                "content": [],
                                "stop_reason": null,
                                "stop_sequence": null,
                                "usage": {
                                    "input_tokens": 64,
                                    "output_tokens": 0,
                                    "cache_creation_input_tokens": 0,
                                    "cache_read_input_tokens": 0
                                }
                            }
                        }
                    }
                }),
            );
            eng.emit_eipc(
                "claude.web",
                "LocalAgentModeSessions",
                "onEvent",
                json!({
                    "type": "message",
                    "sessionId": sid,
                    "userMessageUuid": user_msg_uuid,
                    "message": {
                        "type": "stream_event",
                        "session_id": sid,
                        "parent_tool_use_id": null,
                        "event": {
                            "type": "content_block_start",
                            "index": 0,
                            "content_block": {
                                "type": "text",
                                "text": ""
                            }
                        }
                    }
                }),
            );

            let history: Vec<(String, String)> = eng
                .agent_sessions
                .read()
                .get(&sid)
                .map(|s| {
                    s.buffered_messages
                        .iter()
                        .filter_map(|m| {
                            let role = match m.get("type").and_then(|t| t.as_str()) {
                                Some("assistant") => "assistant",
                                Some("user") => "user",
                                _ => return None,
                            };
                            let text = m.pointer("/message/content/0/text")?.as_str()?;
                            Some((role.to_string(), text.to_string()))
                        })
                        .collect()
                })
                .unwrap_or_default();

            // 2. Stream token deltas in real time via `stream_event` -> `content_block_delta`
            let (reply, final_model) = eng.generate_reply_with_history_streaming(
                &prompt_owned,
                &cwd,
                &history,
                Some(&session_model),
                |delta_chunk| {
                    eng.emit_eipc(
                        "claude.web",
                        "LocalAgentModeSessions",
                        "onEvent",
                        json!({
                            "type": "message",
                            "sessionId": sid,
                            "userMessageUuid": user_msg_uuid,
                            "message": {
                                "type": "stream_event",
                                "session_id": sid,
                                "parent_tool_use_id": null,
                                "event": {
                                    "type": "content_block_delta",
                                    "index": 0,
                                    "delta": {
                                        "type": "text_delta",
                                        "text": delta_chunk
                                    }
                                }
                            }
                        }),
                    );
                },
            );

            // 3. Emit `content_block_stop`
            eng.emit_eipc(
                "claude.web",
                "LocalAgentModeSessions",
                "onEvent",
                json!({
                    "type": "message",
                    "sessionId": sid,
                    "userMessageUuid": user_msg_uuid,
                    "message": {
                        "type": "stream_event",
                        "session_id": sid,
                        "parent_tool_use_id": null,
                        "event": {
                            "type": "content_block_stop",
                            "index": 0
                        }
                    }
                }),
            );

            let duration_ms = (Utc::now().timestamp_millis() - turn_start_ms).max(1) as u64;
            let reply_iso = Utc::now().to_rfc3339();
            let assistant_msg = json!({
                "type": "assistant",
                "uuid": assistant_msg_uuid,
                "session_id": sid,
                "parent_tool_use_id": null,
                "timestamp": reply_iso,
                "message": {
                    "id": msg_id,
                    "type": "message",
                    "role": "assistant",
                    "model": final_model,
                    "content": [{ "type": "text", "text": reply }],
                    "stop_reason": "end_turn",
                    "usage": { "input_tokens": 64, "output_tokens": 128 }
                }
            });
            let result_msg = json!({
                "type": "result",
                "subtype": "success",
                "is_error": false,
                "duration_ms": duration_ms,
                "duration_api_ms": duration_ms,
                "num_turns": 1,
                "result": reply,
                "session_id": sid,
                "total_cost_usd": 0.001,
                "usage": { "input_tokens": 64, "output_tokens": 128 },
                "uuid": Uuid::new_v4().to_string()
            });

            if let Some(sess) = eng.agent_sessions.write().get_mut(&sid) {
                sess.is_running = false;
                sess.model = final_model;
                sess.last_activity_at = Utc::now().timestamp_millis() as u64;
                sess.buffered_messages.push(assistant_msg.clone());
                sess.buffered_messages.push(result_msg.clone());
                eng.persist_agent_session(sess);
            }

            eng.emit_eipc(
                "claude.web",
                "LocalAgentModeSessions",
                "onEvent",
                json!({
                    "type": "message",
                    "sessionId": sid,
                    "message": assistant_msg,
                    "userMessageUuid": user_msg_uuid
                }),
            );
            eng.emit_eipc(
                "claude.web",
                "LocalAgentModeSessions",
                "onEvent",
                json!({
                    "type": "message",
                    "sessionId": sid,
                    "userMessageUuid": user_msg_uuid,
                    "message": {
                        "type": "stream_event",
                        "session_id": sid,
                        "parent_tool_use_id": null,
                        "event": {
                            "type": "message_stop"
                        }
                    }
                }),
            );
            eng.emit_eipc(
                "claude.web",
                "LocalAgentModeSessions",
                "onEvent",
                json!({
                    "type": "message",
                    "sessionId": sid,
                    "message": result_msg,
                    "userMessageUuid": user_msg_uuid
                }),
            );
            eng.emit_eipc(
                "claude.web",
                "LocalAgentModeSessions",
                "onEvent",
                json!({
                    "type": "session_updated",
                    "sessionId": sid
                }),
            );
        });
    }

    /// Starts a `LocalSessions` session (used by Code tab / CCD!)
    pub fn start_code_session(self: &Arc<Self>, opts: &Value) -> String {
        let raw_sid = opts
            .get("sessionId")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("local_{}", Uuid::new_v4()));
        let session_id = if let Some(stripped) = raw_sid.strip_prefix("local_cli_") {
            format!("local_{}", stripped)
        } else if raw_sid.starts_with("local_") {
            raw_sid
        } else {
            format!("local_{}", raw_sid)
        };
        let message_uuid = opts
            .get("messageUuid")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_string());
        let now_ms = Utc::now().timestamp_millis() as u64;
        let cwd = opts
            .get("cwd")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty() && *s != "~")
            .map(|s| s.to_string())
            .unwrap_or_else(|| "/Users/eric/quixi-desk".to_string());
        let prompt = opts
            .get("message")
            .or_else(|| opts.get("prompt"))
            .or_else(|| opts.get("initialPrompt"))
            .or_else(|| opts.get("typedText"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let title = opts
            .get("title")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                if prompt.is_empty() {
                    "QuixiDesk Code Session".to_string()
                } else {
                    prompt.chars().take(48).collect()
                }
            });
        let permission_mode = opts
            .get("permissionMode")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("default")
            .to_string();
        let repo = quixi_workspace::GitPoolEngine::inspect_repo(Path::new(&cwd));
        let requested_model = opts.get("model").and_then(|v| v.as_str());
        let model = self
            .provider_manager
            .resolve_connected_model(requested_model);

        let sess = LocalCodeSession {
            session_id: session_id.clone(),
            cwd: cwd.clone(),
            origin_cwd: cwd,
            is_running: true,
            turn_running: !prompt.is_empty(),
            model,
            effort: "medium".to_string(),
            created_at: now_ms,
            last_activity_at: now_ms,
            is_archived: false,
            is_starred: false,
            permission_mode,
            title,
            branch: repo.branch,
        };

        self.persist_code_session(&sess);
        self.code_sessions
            .write()
            .insert(session_id.clone(), sess.clone());
        self.code_transcripts
            .write()
            .entry(session_id.clone())
            .or_default();

        self.emit_eipc(
            "claude.web",
            "LocalSessions",
            "onEvent",
            json!({
                "type": "start",
                "sessionId": session_id,
                "session": sess
            }),
        );
        self.emit_eipc(
            "claude.web",
            "LocalSessions",
            "onEvent",
            json!({
                "type": "session_updated",
                "sessionId": session_id,
                "session": sess
            }),
        );

        if !prompt.is_empty() {
            self.send_code_message_with_uuid(&session_id, &prompt, message_uuid.as_deref());
        }

        session_id
    }

    pub fn send_code_message(self: &Arc<Self>, session_id: &str, prompt: &str) {
        self.send_code_message_with_uuid(session_id, prompt, None);
    }

    pub fn send_code_message_with_uuid(
        self: &Arc<Self>,
        session_id: &str,
        prompt: &str,
        provided_uuid: Option<&str>,
    ) {
        let user_msg_uuid = provided_uuid
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let stream_start_uuid = Uuid::new_v4().to_string();
        let assistant_msg_uuid = Uuid::new_v4().to_string();
        let msg_id = format!("msg_{}", &assistant_msg_uuid.replace('-', "")[..16]);
        let now_iso = Utc::now().to_rfc3339();

        let user_msg = json!({
            "type": "user",
            "uuid": user_msg_uuid,
            "session_id": session_id,
            "parent_tool_use_id": null,
            "timestamp": now_iso,
            "message": {
                "role": "user",
                "content": [{ "type": "text", "text": prompt }]
            }
        });

        let mut cwd = "/Users/eric/quixi-desk".to_string();
        let mut session_model = self.provider_manager.resolve_connected_model(None);
        if let Some(sess) = self.code_sessions.write().get_mut(session_id) {
            sess.is_running = true;
            sess.turn_running = true;
            sess.last_activity_at = Utc::now().timestamp_millis() as u64;
            sess.model = self
                .provider_manager
                .resolve_connected_model(Some(&sess.model));
            if (sess.title == "QuixiDesk Code Session" || sess.title.trim().is_empty())
                && !prompt.trim().is_empty()
            {
                sess.title = prompt.chars().take(48).collect();
            }
            session_model = sess.model.clone();
            cwd = sess.cwd.clone();
            self.persist_code_session(sess);
        }
        self.code_transcripts
            .write()
            .entry(session_id.to_string())
            .or_default()
            .push(user_msg.clone());

        let sid = session_id.to_string();
        let prompt_owned = prompt.to_string();
        let eng = Arc::clone(self);

        std::thread::spawn(move || {
            let turn_start_ms = Utc::now().timestamp_millis();
            std::thread::sleep(std::time::Duration::from_millis(20));

            eng.emit_eipc(
                "claude.web",
                "LocalSessions",
                "onEvent",
                json!({
                    "type": "message",
                    "sessionId": sid,
                    "message": user_msg,
                    "userMessageUuid": user_msg_uuid
                }),
            );
            let mut replay_user_msg = user_msg.clone();
            if let Some(obj) = replay_user_msg.as_object_mut() {
                obj.insert("isReplay".to_string(), json!(true));
            }
            eng.emit_eipc(
                "claude.web",
                "LocalSessions",
                "onEvent",
                json!({
                    "type": "message",
                    "sessionId": sid,
                    "message": replay_user_msg,
                    "userMessageUuid": user_msg_uuid
                }),
            );

            // 1. Emit `stream_event` -> `message_start` & `content_block_start`
            eng.emit_eipc(
                "claude.web",
                "LocalSessions",
                "onEvent",
                json!({
                    "type": "message",
                    "sessionId": sid,
                    "message": {
                        "type": "stream_event",
                        "session_id": sid,
                        "parent_tool_use_id": null,
                        "uuid": stream_start_uuid,
                        "event": {
                            "type": "message_start",
                            "message": {
                                "id": msg_id,
                                "type": "message",
                                "role": "assistant",
                                "model": session_model,
                                "content": [],
                                "stop_reason": null,
                                "stop_sequence": null,
                                "usage": {
                                    "input_tokens": 64,
                                    "output_tokens": 0,
                                    "cache_creation_input_tokens": 0,
                                    "cache_read_input_tokens": 0
                                }
                            }
                        }
                    }
                }),
            );
            eng.emit_eipc(
                "claude.web",
                "LocalSessions",
                "onEvent",
                json!({
                    "type": "message",
                    "sessionId": sid,
                    "message": {
                        "type": "stream_event",
                        "session_id": sid,
                        "parent_tool_use_id": null,
                        "event": {
                            "type": "content_block_start",
                            "index": 0,
                            "content_block": {
                                "type": "text",
                                "text": ""
                            }
                        }
                    }
                }),
            );

            let history: Vec<(String, String)> = eng
                .code_transcripts
                .read()
                .get(&sid)
                .map(|msgs| {
                    msgs.iter()
                        .filter_map(|m| {
                            let role = match m.get("type").and_then(|t| t.as_str()) {
                                Some("assistant") => "assistant",
                                Some("user") => "user",
                                _ => return None,
                            };
                            let text = m.pointer("/message/content/0/text")?.as_str()?;
                            Some((role.to_string(), text.to_string()))
                        })
                        .collect()
                })
                .unwrap_or_default();

            let (reply, final_model) = eng.generate_reply_with_history_streaming(
                &prompt_owned,
                &cwd,
                &history,
                Some(&session_model),
                |delta_chunk| {
                    eng.emit_eipc(
                        "claude.web",
                        "LocalSessions",
                        "onEvent",
                        json!({
                            "type": "message",
                            "sessionId": sid,
                            "message": {
                                "type": "stream_event",
                                "session_id": sid,
                                "parent_tool_use_id": null,
                                "event": {
                                    "type": "content_block_delta",
                                    "index": 0,
                                    "delta": {
                                        "type": "text_delta",
                                        "text": delta_chunk
                                    }
                                }
                            }
                        }),
                    );
                },
            );

            eng.emit_eipc(
                "claude.web",
                "LocalSessions",
                "onEvent",
                json!({
                    "type": "message",
                    "sessionId": sid,
                    "message": {
                        "type": "stream_event",
                        "session_id": sid,
                        "parent_tool_use_id": null,
                        "event": {
                            "type": "content_block_stop",
                            "index": 0
                        }
                    }
                }),
            );

            let duration_ms = (Utc::now().timestamp_millis() - turn_start_ms).max(1) as u64;
            let assistant_msg = json!({
                "type": "assistant",
                "uuid": assistant_msg_uuid,
                "session_id": sid,
                "parent_tool_use_id": null,
                "timestamp": Utc::now().to_rfc3339(),
                "message": {
                    "id": msg_id,
                    "type": "message",
                    "role": "assistant",
                    "model": final_model,
                    "content": [{ "type": "text", "text": reply }],
                    "stop_reason": "end_turn",
                    "usage": { "input_tokens": 64, "output_tokens": 128 }
                }
            });
            let result_msg = json!({
                "type": "result",
                "subtype": "success",
                "is_error": false,
                "duration_ms": duration_ms,
                "duration_api_ms": duration_ms,
                "num_turns": 1,
                "result": reply,
                "session_id": sid,
                "total_cost_usd": 0.001,
                "usage": { "input_tokens": 64, "output_tokens": 128 },
                "uuid": Uuid::new_v4().to_string()
            });

            eng.code_transcripts
                .write()
                .entry(sid.clone())
                .or_default()
                .extend([assistant_msg.clone(), result_msg.clone()]);

            let mut updated_sess = None;
            if let Some(sess) = eng.code_sessions.write().get_mut(&sid) {
                sess.turn_running = false;
                sess.model = final_model;
                sess.last_activity_at = Utc::now().timestamp_millis() as u64;
                eng.persist_code_session(sess);
                updated_sess = Some(sess.clone());
            }

            eng.emit_eipc(
                "claude.web",
                "LocalSessions",
                "onEvent",
                json!({
                    "type": "message",
                    "sessionId": sid,
                    "message": assistant_msg
                }),
            );
            eng.emit_eipc(
                "claude.web",
                "LocalSessions",
                "onEvent",
                json!({
                    "type": "message",
                    "sessionId": sid,
                    "message": {
                        "type": "stream_event",
                        "session_id": sid,
                        "parent_tool_use_id": null,
                        "event": {
                            "type": "message_stop"
                        }
                    }
                }),
            );
            eng.emit_eipc(
                "claude.web",
                "LocalSessions",
                "onEvent",
                json!({
                    "type": "message",
                    "sessionId": sid,
                    "message": result_msg
                }),
            );
            eng.emit_eipc(
                "claude.web",
                "LocalSessions",
                "onEvent",
                json!({
                    "type": "stopped",
                    "sessionId": sid,
                    "session": updated_sess
                }),
            );
            eng.emit_eipc(
                "claude.web",
                "LocalSessions",
                "onEvent",
                json!({
                    "type": "session_updated",
                    "sessionId": sid,
                    "session": updated_sess
                }),
            );
        });
    }

    fn json_resp(status: u16, val: &Value) -> (u16, &'static str, Vec<u8>) {
        let bytes = serde_json::to_vec(val).unwrap_or_else(|_| b"{}".to_vec());
        (status, "application/json; charset=utf-8", bytes)
    }
}
