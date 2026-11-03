pub mod changes;
pub mod pty;

use anyhow::Result;
use ignore::WalkBuilder;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

/// 1. FileIndexEngine (replaces `fileIndexWorker.js`)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexedFileEntry {
    pub path: String,
    pub relative_path: String,
    pub name: String,
    pub is_dir: bool,
    pub is_directory: bool,
    pub size: u64,
}

pub struct FileIndexEngine;

impl FileIndexEngine {
    /// Lists or searches files in `root` respecting `.gitignore`.
    pub fn list_files(root: &Path, query: Option<&str>, limit: usize) -> Vec<IndexedFileEntry> {
        let q = query.unwrap_or("").to_lowercase();
        let mut results = Vec::new();
        let walker = WalkBuilder::new(root)
            .hidden(false)
            .git_ignore(true)
            .max_depth(Some(8))
            .build();

        for entry in walker.flatten() {
            if entry.path() == root {
                continue;
            }
            let path = entry.path();
            if path.components().any(|c| {
                c.as_os_str() == ".git"
                    || c.as_os_str() == "node_modules"
                    || c.as_os_str() == "target"
            }) {
                continue;
            }
            let rel = path
                .strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();
            if !q.is_empty() && !rel.to_lowercase().contains(&q) {
                continue;
            }
            let meta = entry.metadata().ok();
            let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
            let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            results.push(IndexedFileEntry {
                path: path.to_string_lossy().to_string(),
                relative_path: rel,
                name,
                is_dir,
                is_directory: is_dir,
                size,
            });
            if results.len() >= limit {
                break;
            }
        }
        results
    }

    /// Lists immediate children of a directory (for `LocalSessions.listDirectoryAtCwd`, `CoworkSpaces.listFolderContents`).
    pub fn list_directory_children(dir: &Path) -> Vec<IndexedFileEntry> {
        let mut out = Vec::new();
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let meta = entry.metadata().ok();
                let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
                let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                let name = entry.file_name().to_string_lossy().to_string();
                out.push(IndexedFileEntry {
                    path: path.to_string_lossy().to_string(),
                    relative_path: name.clone(),
                    name,
                    is_dir,
                    is_directory: is_dir,
                    size,
                });
            }
        }
        out.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name)));
        out
    }
}

/// 2. GitPoolEngine (replaces `heavyWorkWorker.js` git & worktree tasks)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRepoState {
    pub is_git_repo: bool,
    pub root_path: Option<String>,
    pub branch: Option<String>,
    pub head_sha: Option<String>,
    pub is_dirty: bool,
    pub modified_files: Vec<String>,
    pub untracked_files: Vec<String>,
}

pub struct GitPoolEngine;

impl GitPoolEngine {
    pub fn inspect_repo(cwd: &Path) -> GitRepoState {
        let root_out = Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .current_dir(cwd)
            .output();

        let root_path = match root_out {
            Ok(o) if o.status.success() => {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            }
            _ => {
                return GitRepoState {
                    is_git_repo: false,
                    root_path: None,
                    branch: None,
                    head_sha: None,
                    is_dirty: false,
                    modified_files: vec![],
                    untracked_files: vec![],
                }
            }
        };

        let branch = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(cwd)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

        let head_sha = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(cwd)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

        let mut modified_files = Vec::new();
        let mut untracked_files = Vec::new();
        if let Ok(status_out) = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(cwd)
            .output()
        {
            if status_out.status.success() {
                for line in String::from_utf8_lossy(&status_out.stdout).lines() {
                    if line.len() > 3 {
                        let code = &line[..2];
                        let file = line[3..].trim().to_string();
                        if code == "??" {
                            untracked_files.push(file);
                        } else {
                            modified_files.push(file);
                        }
                    }
                }
            }
        }

        let is_dirty = !modified_files.is_empty() || !untracked_files.is_empty();
        GitRepoState {
            is_git_repo: true,
            root_path,
            branch,
            head_sha,
            is_dirty,
            modified_files,
            untracked_files,
        }
    }

    pub fn git_diff(cwd: &Path) -> String {
        Command::new("git")
            .args(["diff", "HEAD"])
            .current_dir(cwd)
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default()
    }

    pub fn get_local_branches(cwd: &Path) -> Vec<String> {
        let mut branches = Vec::new();
        if let Ok(out) = Command::new("git")
            .args(["branch", "--format=%(refname:short)"])
            .current_dir(cwd)
            .output()
        {
            if out.status.success() {
                for line in String::from_utf8_lossy(&out.stdout).lines() {
                    let b = line.trim();
                    if !b.is_empty() {
                        branches.push(b.to_string());
                    }
                }
            }
        }
        if branches.is_empty() {
            branches.push("main".to_string());
        }
        branches
    }

    pub fn get_git_commits(cwd: &Path, limit: usize) -> Vec<Value> {
        let mut commits = Vec::new();
        if let Ok(out) = Command::new("git")
            .args([
                "log",
                &format!("-n{}", limit.max(1)),
                "--pretty=format:%H|%h|%an|%ae|%at|%s",
            ])
            .current_dir(cwd)
            .output()
        {
            if out.status.success() {
                for line in String::from_utf8_lossy(&out.stdout).lines() {
                    let parts: Vec<&str> = line.splitn(6, '|').collect();
                    if parts.len() == 6 {
                        commits.push(json!({
                            "sha": parts[0],
                            "shortSha": parts[1],
                            "authorName": parts[2],
                            "authorEmail": parts[3],
                            "timestamp": parts[4].parse::<u64>().unwrap_or(0) * 1000,
                            "subject": parts[5],
                            "message": parts[5],
                        }));
                    }
                }
            }
        }
        commits
    }

    fn numstat_map(cwd: &Path) -> HashMap<String, (usize, usize, bool)> {
        let mut stats = HashMap::new();
        if let Ok(out) = Command::new("git")
            .args(["diff", "--numstat", "HEAD"])
            .current_dir(cwd)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .output()
        {
            if out.status.success() {
                for line in String::from_utf8_lossy(&out.stdout).lines() {
                    let parts: Vec<&str> = line.splitn(3, '\t').collect();
                    if parts.len() == 3 {
                        let binary = parts[0] == "-" || parts[1] == "-";
                        let add = parts[0].parse::<usize>().unwrap_or(0);
                        let del = parts[1].parse::<usize>().unwrap_or(0);
                        stats.insert(parts[2].trim().to_string(), (add, del, binary));
                    }
                }
            }
        }
        stats
    }

    fn ahead_behind(cwd: &Path) -> (usize, usize) {
        Command::new("git")
            .args(["rev-list", "--left-right", "--count", "@{upstream}...HEAD"])
            .current_dir(cwd)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| {
                let s = String::from_utf8_lossy(&o.stdout);
                let mut it = s.split_whitespace();
                let behind = it.next()?.parse::<usize>().ok()?;
                let ahead = it.next()?.parse::<usize>().ok()?;
                Some((ahead, behind))
            })
            .unwrap_or((0, 0))
    }

    fn untracked_line_count(cwd: &Path, rel: &str) -> usize {
        let p = cwd.join(rel);
        if let Ok(meta) = fs::symlink_metadata(&p) {
            if meta.is_file() && meta.len() <= 1_048_576 {
                if let Ok(text) = fs::read_to_string(&p) {
                    return text.lines().count().max(usize::from(!text.is_empty()));
                }
            }
        }
        0
    }

    pub fn get_git_diff_structured(cwd: &Path) -> Value {
        let state = Self::inspect_repo(cwd);
        let root = state
            .root_path
            .as_deref()
            .map(Path::new)
            .unwrap_or(cwd);
        let numstats = Self::numstat_map(root);
        let (ahead_by, behind_by) = Self::ahead_behind(root);
        let branch = state.branch.unwrap_or_else(|| "main".to_string());
        let head = state.head_sha.unwrap_or_default();
        let mut files = Vec::new();
        for f in &state.modified_files {
            let (add, del, binary) = numstats.get(f).copied().unwrap_or((0, 0, false));
            files.push(json!({
                "path": f,
                "status": "modified",
                "additions": add,
                "deletions": del,
                "binary": binary,
            }));
        }
        for f in &state.untracked_files {
            let add = Self::untracked_line_count(root, f);
            files.push(json!({
                "path": f,
                "status": "untracked",
                "additions": add,
                "deletions": 0,
            }));
        }
        json!({
            "base_ref": "HEAD",
            "head_ref": branch,
            "merge_base": head,
            "files": files,
            "ahead_by": ahead_by,
            "behind_by": behind_by,
            "total_commits": 1,
        })
    }

    pub fn get_git_diff_stats(cwd: &Path) -> Value {
        let wt = Self::get_working_tree_status(cwd);
        let (ahead_by, behind_by) = Self::ahead_behind(cwd);
        let count = wt["files"].as_array().map_or(0, Vec::len);
        json!({
            "additions": wt["additions"].as_u64().unwrap_or(0),
            "deletions": wt["deletions"].as_u64().unwrap_or(0),
            "fileCount": count,
            "ahead_by": ahead_by,
            "behind_by": behind_by,
        })
    }

    pub fn get_working_tree_status(cwd: &Path) -> Value {
        let state = Self::inspect_repo(cwd);
        let root = state
            .root_path
            .as_deref()
            .map(Path::new)
            .unwrap_or(cwd);
        let numstats = Self::numstat_map(root);
        let mut files = Vec::new();
        let mut total_add = 0usize;
        let mut total_del = 0usize;
        for f in &state.modified_files {
            let (add, del, binary) = numstats.get(f).copied().unwrap_or((0, 0, false));
            total_add += add;
            total_del += del;
            files.push(json!({
                "path": f,
                "status": "modified",
                "additions": add,
                "deletions": del,
                "binary": binary,
            }));
        }
        for f in &state.untracked_files {
            let add = Self::untracked_line_count(root, f);
            total_add += add;
            files.push(json!({
                "path": f,
                "status": "untracked",
                "additions": add,
                "deletions": 0,
            }));
        }
        json!({
            "files": files,
            "additions": total_add,
            "deletions": total_del,
        })
    }

    pub fn get_diff_file_content(cwd: &Path, rel_path: &str) -> Value {
        let abs = cwd.join(rel_path);
        let new_text = fs::read_to_string(&abs).ok();
        let old_text = Command::new("git")
            .args(["show", &format!("HEAD:{}", rel_path)])
            .current_dir(cwd)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string());
        json!({
            "oldText": old_text,
            "newText": new_text,
        })
    }

    pub fn stash_working_tree(cwd: &Path) -> bool {
        Command::new("git")
            .args(["stash", "push", "-u", "-m", "QuixiDesk stash"])
            .current_dir(cwd)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

/// 3. Shell & PTY Host (replaces `shellPathWorker.js` & `ptyHostWorker.js`)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PtySessionEntry {
    pub key: String,
    pub cwd: String,
    pub shell: String,
    pub cols: u16,
    pub rows: u16,
    pub buffer: String,
    pub busy: bool,
}

#[derive(Clone, Default)]
pub struct ShellHostEngine {
    pub ptys: Arc<Mutex<HashMap<String, PtySessionEntry>>>,
    pub terminals: pty::TerminalSessionManager,
}

impl ShellHostEngine {
    pub fn default_shell() -> String {
        #[cfg(target_os = "windows")]
        { std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".into()) }
        #[cfg(not(target_os = "windows"))]
        { std::env::var("SHELL").unwrap_or_else(|_| {
            if cfg!(target_os = "macos") { "/bin/zsh" }
            else if Path::new("/bin/bash").is_file() { "/bin/bash" }
            else { "/bin/sh" }.into()
        }) }
    }

    pub fn new() -> Self {
        Self {
            ptys: Arc::new(Mutex::new(HashMap::new())),
            terminals: pty::TerminalSessionManager::new(),
        }
    }

    #[cfg(unix)]
    fn hydrated_login_env() -> &'static HashMap<String, String> {
        static LOGIN_ENV: std::sync::OnceLock<HashMap<String, String>> = std::sync::OnceLock::new();
        LOGIN_ENV.get_or_init(|| {
            use std::os::unix::process::CommandExt;
            use std::process::Stdio;
            use std::time::{Duration, Instant};

            let candidates = [Self::default_shell(), "/bin/zsh".into(), "/bin/bash".into(), "/bin/sh".into()];
            let begin_marker = "__QUIXI_ENV_BEGIN__";
            let end_marker = "__QUIXI_ENV_END__";
            let script = format!("printf '%s\\0' '{begin_marker}'; env -0; printf '%s\\0' '{end_marker}'");

            for shell in candidates {
                if !Path::new(&shell).is_file() {
                    continue;
                }
                let mut cmd = Command::new(&shell);
                cmd.args(["-ilc", &script])
                    .env("DISABLE_AUTO_UPDATE", "true")
                    .env("ZSH_TMUX_AUTOSTARTED", "true")
                    .env("ZSH_TMUX_AUTOSTART", "false")
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .process_group(0);
                let Ok(mut child) = cmd.spawn() else {
                    continue;
                };
                let pid = child.id() as i32;
                let mut stdout = child.stdout.take().unwrap();
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    use std::io::Read;
                    let mut bytes = Vec::new();
                    let _ = stdout.read_to_end(&mut bytes);
                    let _ = tx.send(bytes);
                });
                let start = Instant::now();
                let mut finished = false;
                while start.elapsed() < Duration::from_secs(4) {
                    if let Ok(Some(_)) = child.try_wait() {
                        finished = true;
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                if !finished {
                    unsafe {
                        libc::kill(-pid, libc::SIGKILL);
                    }
                    let _ = child.kill();
                    let _ = child.wait();
                    continue;
                }
                let Ok(raw) = rx.recv_timeout(Duration::from_millis(500)) else {
                    continue;
                };
                let fields: Vec<&[u8]> = raw.split(|&b| b == 0).collect();
                let Some(b_idx) = fields.iter().position(|f| *f == begin_marker.as_bytes()) else {
                    continue;
                };
                let Some(e_rel) = fields[b_idx + 1..].iter().position(|f| *f == end_marker.as_bytes()) else {
                    continue;
                };
                let mut map = HashMap::new();
                for entry in &fields[b_idx + 1..b_idx + 1 + e_rel] {
                    if let Ok(s) = std::str::from_utf8(entry) {
                        if let Some((k, v)) = s.split_once('=') {
                            if !matches!(
                                k,
                                "PWD" | "OLDPWD" | "SHLVL" | "_"
                                    | "DISABLE_AUTO_UPDATE"
                                    | "ZSH_TMUX_AUTOSTARTED"
                                    | "ZSH_TMUX_AUTOSTART"
                            ) && !k.starts_with("QUIXI_")
                            {
                                map.insert(k.to_string(), v.to_string());
                            }
                        }
                    }
                }
                if !map.is_empty() {
                    return map;
                }
            }
            HashMap::new()
        })
    }

    pub fn resolve_user_shell_env() -> HashMap<String, String> {
        if let Some(profile) = quixi_proto::paths::test_profile() {
            #[cfg(target_os = "windows")]
            {
                let mut env = HashMap::new();
                // cmd and native tools need these OS settings even in an isolated
                // account fixture. Do not forward credentials or other user env.
                for name in ["SystemRoot", "ComSpec", "PATH", "PATHEXT"] {
                    if let Some(value) = std::env::var_os(name) {
                        env.insert(name.to_string(), value.to_string_lossy().into_owned());
                    }
                }
                let tmp = profile.join("tmp");
                let _ = fs::create_dir_all(&tmp);
                env.insert("TEMP".into(), tmp.to_string_lossy().into_owned());
                env.insert("TMP".into(), tmp.to_string_lossy().into_owned());
                return env;
            }
            #[cfg(not(target_os = "windows"))]
            return HashMap::from([
                ("PATH".into(), "/usr/bin:/bin:/usr/sbin:/sbin".into()),
                ("LANG".into(), "en_US.UTF-8".into()),
                ("ZDOTDIR".into(), profile.join("home").to_string_lossy().into_owned()),
                ("TMPDIR".into(), profile.join("tmp").to_string_lossy().into_owned()),
            ]);
        }
        let mut map: HashMap<String, String> = std::env::vars().collect();
        #[cfg(unix)]
        {
            // Hydrate login-shell variables when launched from a minimal GUI environment (e.g. Finder/Dock).
            if map.get("PATH").is_none_or(|p| !p.contains("homebrew") && !p.contains(".cargo")) {
                for (k, v) in Self::hydrated_login_env() {
                    map.entry(k.clone()).or_insert_with(|| v.clone());
                }
                if let Some(login_path) = Self::hydrated_login_env().get("PATH") {
                    map.insert("PATH".to_string(), login_path.clone());
                }
            }
        }
        // Scrub internal automation tokens and provider API secrets from agent subprocesses.
        map.retain(|k, _| {
            let up = k.to_ascii_uppercase();
            up != "QUIXI_AUTOMATION_TOKEN"
                && !up.ends_with("_API_KEY")
                && !up.ends_with("_SECRET_KEY")
        });
        #[cfg(target_os = "windows")]
        return map;
        #[cfg(not(target_os = "windows"))]
        {
            let current_path = map.get("PATH").cloned().unwrap_or_default();
            let mut extra_paths = vec![
                "/opt/homebrew/bin".to_string(),
                "/usr/local/bin".to_string(),
            ];
            {
                let home = quixi_proto::paths::account_home();
                extra_paths.push(home.join(".cargo/bin").to_string_lossy().to_string());
                extra_paths.push(home.join(".local/bin").to_string_lossy().to_string());
            }
            if !current_path.is_empty() {
                extra_paths.push(current_path);
            }
            map.insert("PATH".to_string(), extra_paths.join(":"));
            map
        }
    }

    pub fn get_terminal_shells() -> Value {
        let default_shell = Self::default_shell();
        #[cfg(target_os = "windows")]
        return json!({"shells":[{"id":"cmd","name":"Command Prompt","path":default_shell,"isDefault":true}],"selected":default_shell});
        #[cfg(not(target_os = "windows"))]
        {
        let mut shells = Vec::new();
        for (id, name, bin) in [
            ("zsh", "zsh", "/bin/zsh"),
            ("bash", "bash", "/bin/bash"),
            ("sh", "sh", "/bin/sh"),
        ] {
            if Path::new(bin).exists() {
                shells.push(json!({
                    "id": id,
                    "name": name,
                    "path": bin,
                    "isDefault": bin == default_shell,
                }));
            }
        }
        json!({
            "shells": shells,
            "selected": default_shell,
        })
        }
    }

    pub fn start_pty(&self, key: &str, cwd: &Path) -> Value {
        let shell = Self::default_shell();
        let env = Self::resolve_user_shell_env();
        let motd = match self.terminals.open("", Some(key), Some(key), cwd, &shell, &env) {
            Ok((_, initial)) => initial,
            Err(_) => format!("QuixiDesk Terminal ({})\r\n$ ", cwd.display()),
        };
        let entry = PtySessionEntry {
            key: key.to_string(),
            cwd: cwd.to_string_lossy().to_string(),
            shell,
            cols: 120,
            rows: 32,
            buffer: motd.clone(),
            busy: false,
        };
        if let Ok(mut map) = self.ptys.lock() {
            map.insert(key.to_string(), entry);
        }
        json!({ "ok": true, "id": key, "motd": motd })
    }

    pub fn write_pty(&self, key: &str, data: &str) {
        if self.terminals.write_raw("", key, data).is_ok() {
            if let Ok(delta) = self.terminals.read_delta("", key, 0) {
                if let Ok(mut map) = self.ptys.lock() {
                    if let Some(entry) = map.get_mut(key) {
                        entry.buffer = delta.text;
                    }
                }
            }
            return;
        }
        if let Ok(mut map) = self.ptys.lock() {
            if let Some(entry) = map.get_mut(key) {
                entry.buffer.push_str(data);
                if data.contains('\r') || data.contains('\n') {
                    let cmd = data.trim();
                    if !cmd.is_empty() {
                        if let Ok(res) = Self::run_bash_command(Path::new(&entry.cwd), cmd) {
                            if let Some(out) = res.get("stdout").and_then(|v| v.as_str()) {
                                entry.buffer.push_str(out);
                            }
                            if let Some(err) = res.get("stderr").and_then(|v| v.as_str()) {
                                entry.buffer.push_str(err);
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn read_pty(&self, key: &str, from_offset: usize) -> Value {
        if let Ok(delta) = self.terminals.read_delta("", key, from_offset) {
            return serde_json::to_value(delta).unwrap_or_else(|_| json!({}));
        }
        if let Ok(map) = self.ptys.lock() {
            if let Some(entry) = map.get(key) {
                let total = entry.buffer.len();
                let start = from_offset.min(total);
                return json!({
                    "id": key,
                    "text": &entry.buffer[start..],
                    "nextOffset": total,
                    "totalBytes": total,
                    "lossy": false,
                    "status": "running",
                });
            }
        }
        json!({"error": "Terminal session not found"})
    }

    pub fn resize_pty(&self, key: &str, cols: u16, rows: u16) {
        self.terminals.resize(key, cols, rows);
        if let Ok(mut map) = self.ptys.lock() {
            if let Some(entry) = map.get_mut(key) {
                entry.cols = cols;
                entry.rows = rows;
            }
        }
    }

    pub fn stop_pty(&self, key: &str) {
        let _ = self.terminals.close("", key);
        if let Ok(mut map) = self.ptys.lock() {
            map.remove(key);
        }
    }

    pub fn get_busy_pty_keys(&self) -> Vec<String> {
        if let Ok(map) = self.ptys.lock() {
            map.values()
                .filter(|e| e.busy)
                .map(|e| e.key.clone())
                .collect()
        } else {
            vec![]
        }
    }

    pub fn run_bash_command(cwd: &Path, command: &str) -> Result<Value> {
        Self::run_bash_command_cancellable(cwd, command, &std::sync::atomic::AtomicBool::new(false), std::time::Duration::from_secs(120))
    }

    pub fn run_bash_command_cancellable(cwd: &Path, command: &str, cancelled: &std::sync::atomic::AtomicBool, timeout: std::time::Duration) -> Result<Value> {
        Self::run_bash_command_with_policy(cwd, command, cancelled, timeout, None)
    }

    pub fn run_bash_command_with_policy(
        cwd: &Path,
        command: &str,
        cancelled: &std::sync::atomic::AtomicBool,
        timeout: std::time::Duration,
        sandbox_policy: Option<&quixi_platform::sandbox::SandboxPolicy>,
    ) -> Result<Value> {
        use std::{io::Read, process::Stdio, sync::{atomic::Ordering, mpsc}, time::{Duration, Instant}};
        if cancelled.load(Ordering::SeqCst) { anyhow::bail!("Command cancelled before starting"); }
        let shell = Self::default_shell();
        let base_args: Vec<String> = if cfg!(target_os = "windows") {
            vec!["/D".into(), "/S".into(), "/C".into(), command.into()]
        } else {
            vec!["-c".into(), command.into()]
        };
        let confined = if let Some(policy) = sandbox_policy {
            Some(quixi_platform::sandbox::ProcessSandbox::confine(&shell, &base_args, policy))
        } else {
            None
        };
        let (prog, prog_args) = match &confined {
            Some(c) => (c.program.as_str(), c.args.as_slice()),
            None => (shell.as_str(), base_args.as_slice()),
        };

        let mut cmd = Command::new(prog);
        if quixi_proto::paths::test_profile().is_some() { cmd.env_clear(); }
        #[cfg(unix)]
        { use std::os::unix::process::CommandExt; cmd.process_group(0); }
        let mut envs = Self::resolve_user_shell_env();
        if let Some(Some(ref tmp)) = confined.as_ref().map(|c| &c.temp_dir_override) {
            let s = tmp.to_string_lossy().into_owned();
            envs.insert("TMPDIR".into(), s.clone());
            envs.insert("TMP".into(), s.clone());
            envs.insert("TEMP".into(), s);
        }
        let mut child = cmd.args(prog_args).current_dir(cwd).envs(envs)
            .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
        let pid = child.id();
        let (tx, rx) = mpsc::channel();
        fn drain(mut stream: impl Read + Send + 'static, tx: mpsc::Sender<(bool, Vec<u8>)>, stderr: bool) {
            std::thread::spawn(move || { let mut retained=Vec::new(); let mut buf=[0;8192];
                while let Ok(n)=stream.read(&mut buf) { if n==0 {break;} let keep=n.min(1_048_576usize.saturating_sub(retained.len())); retained.extend_from_slice(&buf[..keep]); }
                let _=tx.send((stderr,retained)); });
        }
        drain(child.stdout.take().unwrap(),tx.clone(),false); drain(child.stderr.take().unwrap(),tx,true);
        let started=Instant::now(); let mut output=[None,None]; let mut status=None;
        loop {
            while let Ok((err,bytes))=rx.try_recv() { output[usize::from(err)]=Some(bytes); }
            if status.is_none() { status=child.try_wait()?; }
            if status.is_some() && output.iter().all(Option::is_some) { break; }
            if cancelled.load(Ordering::SeqCst) || started.elapsed()>=timeout {
                #[cfg(unix)]
                unsafe { libc::kill(-(pid as i32),libc::SIGKILL); }
                #[cfg(target_os="windows")]
                { let _=Command::new("taskkill").args(["/PID",&pid.to_string(),"/T","/F"]).stdout(Stdio::null()).stderr(Stdio::null()).status(); }
                let _=child.kill(); let _=child.wait();
                anyhow::bail!(if cancelled.load(Ordering::SeqCst) {"Command stopped; its process tree was terminated."} else {"Command timed out; its process tree was terminated."});
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let status=status.unwrap();
        let stdout_str = String::from_utf8_lossy(output[0].as_ref().unwrap()).into_owned();
        let stderr_str = String::from_utf8_lossy(output[1].as_ref().unwrap()).into_owned();
        let code = status.code().unwrap_or(-1);
        let sandbox_meta = confined.map(|c| {
            let outcome = c.classify_stderr(code, &stderr_str);
            json!({
                "mode": c.mode,
                "enforcement": c.enforcement,
                "outcome": outcome,
            })
        });
        let mut res = json!({
            "stdout": stdout_str,
            "stderr": stderr_str,
            "exitCode": code,
            "ok": status.success(),
        });
        if let Some(sb) = sandbox_meta {
            res["sandbox"] = sb;
        }
        Ok(res)
    }
}

/// 4. TranscriptSearchEngine & CLI Session Scanner (replaces `transcriptSearchWorker.js` & `cliImport`)
pub struct TranscriptSearchEngine;

impl TranscriptSearchEngine {
    pub fn search_transcripts(query: &str, limit: usize) -> Vec<Value> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return vec![];
        }
        let mut roots: Vec<PathBuf> = Vec::new();
        {
            let home = quixi_proto::paths::account_home();
            roots.push(quixi_proto::paths::data_dir().join("sessions"));
            roots.push(quixi_proto::paths::data_dir().join("projects"));
            roots.push(home.join(".quixi/cli/projects"));
        }

        let mut jsonl_files = Vec::new();
        for r in roots {
            if !r.exists() {
                continue;
            }
            for entry in WalkBuilder::new(&r).max_depth(Some(4)).build().flatten() {
                if entry.path().extension().and_then(|s| s.to_str()) == Some("jsonl") {
                    jsonl_files.push(entry.path().to_path_buf());
                }
            }
        }

        let matches: Vec<Value> = jsonl_files
            .par_iter()
            .filter_map(|file_path| {
                let content = fs::read_to_string(file_path).ok()?;
                if content.to_lowercase().contains(&q) {
                    let snippet = content
                        .lines()
                        .find(|l| l.to_lowercase().contains(&q))
                        .unwrap_or("")
                        .chars()
                        .take(240)
                        .collect::<String>();
                    Some(json!({
                        "filePath": file_path.to_string_lossy(),
                        "sessionId": file_path.file_stem().and_then(|s| s.to_str()).unwrap_or(""),
                        "snippet": snippet,
                    }))
                } else {
                    None
                }
            })
            .collect();

        matches.into_iter().take(limit).collect()
    }

    /// Discovers existing CLI sessions in `~/.quixi/cli/projects` and `~/.quixi/code_sessions`
    pub fn list_cli_sessions(limit: usize) -> Vec<Value> {
        use std::io::BufRead;
        let mut raw_sessions: Vec<(u64, String, Value)> = Vec::new();
        let home = quixi_proto::paths::account_home();
        let cli_projects = home.join(".quixi/cli/projects");
        if cli_projects.exists() {
            for entry in WalkBuilder::new(&cli_projects)
                .max_depth(Some(3))
                .build()
                .flatten()
            {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                    let meta = match entry.metadata() {
                        Ok(m) if m.len() > 32 => m,
                        _ => continue,
                    };
                    let sid = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_string();
                    if sid.is_empty() {
                        continue;
                    }
                    let mtime = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0);
                    let raw_project_dir = path
                        .parent()
                        .and_then(|p| p.file_name())
                        .and_then(|s| s.to_str())
                        .unwrap_or("workspace");
                    let clean_project = raw_project_dir
                        .trim_start_matches('-')
                        .strip_prefix("Users-eric-")
                        .or_else(|| raw_project_dir.trim_start_matches('-').strip_prefix("Users-"))
                        .unwrap_or(raw_project_dir.trim_start_matches('-'));
                    let candidate_cwd = home.join(clean_project.replace('-', "/"));
                    let cwd = if candidate_cwd.is_dir() {
                        candidate_cwd.to_string_lossy().to_string()
                    } else {
                        quixi_proto::paths::default_workspace_str().to_string()
                    };

                    let mut extracted_title: Option<String> = None;
                    if let Ok(file) = std::fs::File::open(path) {
                        let reader = std::io::BufReader::new(file);
                        for line in reader.lines().take(25).flatten() {
                            if let Ok(v) = serde_json::from_str::<Value>(&line) {
                                let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
                                if ty == "custom-title" || ty == "summary" {
                                    if let Some(s) = v
                                        .get("customTitle")
                                        .or_else(|| v.get("summary"))
                                        .or_else(|| v.get("title"))
                                        .and_then(|x| x.as_str())
                                    {
                                        let t = s.trim();
                                        if !t.is_empty() {
                                            extracted_title = Some(t.chars().take(54).collect());
                                            break;
                                        }
                                    }
                                } else if ty == "user" && extracted_title.is_none() {
                                    let msg_content = v
                                        .get("message")
                                        .and_then(|m| m.get("content"))
                                        .or_else(|| v.get("content"));
                                    let txt = if let Some(s) = msg_content.and_then(|c| c.as_str()) {
                                        s.to_string()
                                    } else if let Some(arr) = msg_content.and_then(|c| c.as_array()) {
                                        arr.iter()
                                            .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                                            .collect::<Vec<_>>()
                                            .join(" ")
                                    } else {
                                        String::new()
                                    };
                                    let first_line = txt
                                        .lines()
                                        .map(|l| l.trim())
                                        .find(|l| !l.is_empty() && !l.starts_with('<'))
                                        .unwrap_or("");
                                    if first_line.len() > 3 && !first_line.eq_ignore_ascii_case("continue") {
                                        let truncated: String = first_line.chars().take(54).collect();
                                        extracted_title = Some(truncated);
                                    }
                                }
                            }
                        }
                    }

                    let Some(title) = extracted_title else {
                        continue;
                    };
                    raw_sessions.push((
                        mtime,
                        title.clone(),
                        json!({
                            "handle": sid,
                            "sessionId": sid,
                            "id": sid,
                            "title": title,
                            "cwd": cwd,
                            "lastActivityAt": mtime,
                            "liveness": "not_running",
                            "updatedAt": mtime,
                            "createdAt": mtime,
                            "filePath": path.to_string_lossy(),
                        }),
                    ));
                }
            }
        }
        raw_sessions.sort_by(|a, b| b.0.cmp(&a.0));
        let mut seen_titles = std::collections::HashSet::new();
        let mut sessions = Vec::new();
        for (_mtime, title, val) in raw_sessions {
            if seen_titles.insert(title) {
                sessions.push(val);
                if sessions.len() >= limit {
                    break;
                }
            }
        }
        sessions
    }

    pub fn read_cli_transcript(session_id: &str) -> Value {
        let home = quixi_proto::paths::account_home();
        let cli_projects = home.join(".quixi/cli/projects");
        if cli_projects.exists() {
            for entry in WalkBuilder::new(&cli_projects)
                .max_depth(Some(3))
                .build()
                .flatten()
            {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("jsonl")
                    && path.file_stem().and_then(|s| s.to_str()) == Some(session_id)
                {
                    if let Ok(raw) = fs::read_to_string(path) {
                        let msgs: Vec<Value> = raw
                            .lines()
                            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
                            .collect();
                        let count = msgs.len();
                        return json!({
                            "messages": msgs,
                            "totalMessages": count,
                        });
                    }
                }
            }
        }
        json!({ "messages": [], "totalMessages": 0 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_index_git_and_shell_engines() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
        let files = FileIndexEngine::list_files(root, Some("Cargo.toml"), 20);
        assert!(!files.is_empty());

        let children = FileIndexEngine::list_directory_children(root);
        assert!(children.iter().any(|c| c.name == "Cargo.toml"));

        let _git = GitPoolEngine::inspect_repo(root);
        let _diff = GitPoolEngine::get_git_diff_structured(root);
        let _stats = GitPoolEngine::get_git_diff_stats(root);
        let res = ShellHostEngine::run_bash_command(root, "echo quixi-shell-test").unwrap();
        assert_eq!(res["ok"], true);
        assert!(res["stdout"]
            .as_str()
            .unwrap_or("")
            .contains("quixi-shell-test"));
    }
}

#[cfg(test)] mod cancellation_tests {
    use super::*;
    use std::{sync::atomic::{AtomicBool,Ordering},time::{Duration,Instant}};
    #[test] fn cancellation_terminates_delayed_descendants() {
        let root=std::env::temp_dir().join(format!("quixi-cancel-{}",std::process::id()));fs::create_dir_all(&root).unwrap();let marker=root.join("should-not-exist");let _=fs::remove_file(&marker);
        let signal=Arc::new(AtomicBool::new(false));let worker_signal=signal.clone();let cwd=root.clone();
        #[cfg(unix)] let command="sleep 2; echo BAD > should-not-exist";
        #[cfg(windows)] let command="ping -n 4 127.0.0.1 > nul & echo BAD > should-not-exist";
        let start=Instant::now();let worker=std::thread::spawn(move||ShellHostEngine::run_bash_command_cancellable(&cwd,command,&worker_signal,Duration::from_secs(10)));
        std::thread::sleep(Duration::from_millis(150));signal.store(true,Ordering::SeqCst);assert!(worker.join().unwrap().unwrap_err().to_string().contains("stopped"));assert!(start.elapsed()<Duration::from_secs(2));
        std::thread::sleep(Duration::from_secs(3));assert!(!marker.exists());fs::remove_dir_all(root).unwrap();
    }
    #[test] fn timeout_and_large_output_are_bounded() {
        #[cfg(unix)] let command="sleep 10";
        #[cfg(windows)] let command="ping -n 12 127.0.0.1 > nul";
        let started=Instant::now();let result=ShellHostEngine::run_bash_command_cancellable(&std::env::temp_dir(),command,&AtomicBool::new(false),Duration::from_millis(100));assert!(result.unwrap_err().to_string().contains("timed out"));assert!(started.elapsed()<Duration::from_secs(3));
    }
}
