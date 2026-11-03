//! Persistent PTY session service inspired by DeepSeek Harness (`dsh-terminal`).
//!
//! Provides owner-fenced persistent shell sessions over real Unix `openpty` pseudo-terminals
//! (with line-runner fallback on non-Unix hosts), retaining a bounded scrollback buffer,
//! non-consuming UI observer offsets, and quiescence-aware interactive `send`.

use anyhow::{bail, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

const MAX_SCROLLBACK_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSnapshot {
    pub id: String,
    pub owner: String,
    pub name: String,
    pub cwd: String,
    pub shell: String,
    pub pid: Option<u32>,
    pub status: String, // "running" | "exited"
    pub exit_code: Option<i32>,
    pub cols: u16,
    pub rows: u16,
    pub total_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSendResult {
    pub id: String,
    pub viewport: String,
    pub wait_reason: String, // "inferred_idle" | "timeout" | "session_exit"
    pub status: String,
    pub exit_code: Option<i32>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalReadResult {
    pub id: String,
    pub text: String,
    pub next_offset: usize,
    pub total_bytes: usize,
    pub lossy: bool,
    pub status: String,
}

struct ScrollbackRing {
    buf: String,
    base_offset: usize,
    last_output_at: Option<Instant>,
}

impl ScrollbackRing {
    fn new() -> Self {
        Self {
            buf: String::new(),
            base_offset: 0,
            last_output_at: None,
        }
    }

    fn total_bytes(&self) -> usize {
        self.base_offset + self.buf.len()
    }

    fn append(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.buf.push_str(text);
        self.last_output_at = Some(Instant::now());
        if self.buf.len() > MAX_SCROLLBACK_BYTES {
            let excess = self.buf.len() - MAX_SCROLLBACK_BYTES;
            let mut cut = excess;
            while cut < self.buf.len() && !self.buf.is_char_boundary(cut) {
                cut += 1;
            }
            self.buf.drain(..cut);
            self.base_offset += cut;
        }
    }

    fn read_from(&self, from_offset: usize) -> (String, usize, bool) {
        let total = self.total_bytes();
        if from_offset >= total {
            return (String::new(), total, false);
        }
        if from_offset < self.base_offset {
            return (self.buf.clone(), total, true);
        }
        let rel = from_offset - self.base_offset;
        let mut idx = rel;
        while idx < self.buf.len() && !self.buf.is_char_boundary(idx) {
            idx += 1;
        }
        (self.buf[idx..].to_string(), total, false)
    }

    fn read_lines(&self, offset_from_end: usize, limit_lines: usize) -> String {
        let lines: Vec<&str> = self.buf.lines().collect();
        let end = lines.len().saturating_sub(offset_from_end);
        let start = end.saturating_sub(limit_lines);
        lines[start..end].join("\n")
    }
}

struct LivePtySession {
    id: String,
    owner: String,
    name: String,
    cwd: PathBuf,
    shell: String,
    cols: u16,
    rows: u16,
    ring: Arc<Mutex<ScrollbackRing>>,
    #[cfg(unix)]
    master_fd: i32,
    #[cfg(unix)]
    child: std::process::Child,
    exited: Option<i32>,
}

#[cfg(unix)]
impl Drop for LivePtySession {
    fn drop(&mut self) {
        if self.exited.is_none() {
            let pid = self.child.id() as i32;
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        if self.master_fd >= 0 {
            unsafe {
                libc::close(self.master_fd);
            }
            self.master_fd = -1;
        }
    }
}

#[derive(Clone, Default)]
pub struct TerminalSessionManager {
    sessions: Arc<Mutex<HashMap<String, LivePtySession>>>,
}

impl TerminalSessionManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(
        &self,
        owner: &str,
        id_hint: Option<&str>,
        name: Option<&str>,
        cwd: &Path,
        shell: &str,
        env: &HashMap<String, String>,
    ) -> Result<(TerminalSnapshot, String)> {
        let id = id_hint
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("pty-{}", &uuid::Uuid::new_v4().to_string()[..12]));
        let label = name
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("shell")
            .to_string();
        let workdir = if cwd.is_dir() {
            cwd.to_path_buf()
        } else {
            PathBuf::from(quixi_proto::paths::default_workspace())
        };
        let ring = Arc::new(Mutex::new(ScrollbackRing::new()));

        #[cfg(unix)]
        {
            use std::io::Read;
            use std::os::unix::io::FromRawFd;
            use std::os::unix::process::CommandExt;
            use std::process::{Command, Stdio};

            let mut master: libc::c_int = -1;
            let mut slave: libc::c_int = -1;
            let mut ws = libc::winsize {
                ws_row: 32,
                ws_col: 120,
                ws_xpixel: 0,
                ws_ypixel: 0,
            };
            let rc = unsafe {
                libc::openpty(
                    &mut master,
                    &mut slave,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    &mut ws,
                )
            };
            if rc != 0 || master < 0 || slave < 0 {
                bail!("openpty failed: {}", std::io::Error::last_os_error());
            }

            let slave_in = unsafe { libc::dup(slave) };
            let slave_out = unsafe { libc::dup(slave) };
            let slave_err = slave;
            if slave_in < 0 || slave_out < 0 {
                unsafe {
                    if slave_in >= 0 {
                        libc::close(slave_in);
                    }
                    if slave_out >= 0 {
                        libc::close(slave_out);
                    }
                    libc::close(slave_err);
                    libc::close(master);
                }
                bail!("dup on pty slave failed");
            }

            let mut cmd = Command::new(shell);
            if quixi_proto::paths::test_profile().is_some() {
                cmd.env_clear();
            }
            cmd.current_dir(&workdir)
                .envs(env)
                .env("TERM", "xterm-256color")
                .stdin(unsafe { Stdio::from_raw_fd(slave_in) })
                .stdout(unsafe { Stdio::from_raw_fd(slave_out) })
                .stderr(unsafe { Stdio::from_raw_fd(slave_err) });

            unsafe {
                cmd.pre_exec(move || {
                    libc::setsid();
                    #[allow(clippy::useless_conversion)]
                    let _ = libc::ioctl(0, libc::TIOCSCTTY.into(), 0);
                    Ok(())
                });
            }

            let child = match cmd.spawn() {
                Ok(c) => c,
                Err(err) => {
                    unsafe {
                        libc::close(master);
                    }
                    bail!("Failed to spawn PTY shell `{shell}`: {err}");
                }
            };
            let pid = child.id();

            let reader_fd = unsafe { libc::dup(master) };
            if reader_fd >= 0 {
                let ring_clone = ring.clone();
                std::thread::spawn(move || {
                    let mut file = unsafe { std::fs::File::from_raw_fd(reader_fd) };
                    let mut buf = [0u8; 4096];
                    loop {
                        match file.read(&mut buf) {
                            Ok(0) => break,
                            Ok(n) => {
                                let text = String::from_utf8_lossy(&buf[..n]);
                                ring_clone.lock().append(&text);
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                            Err(_) => break,
                        }
                    }
                });
            }

            // Brief settle to capture shell prompt / MOTD
            std::thread::sleep(Duration::from_millis(80));
            let motd = ring.lock().read_from(0).0;

            let session = LivePtySession {
                id: id.clone(),
                owner: owner.to_string(),
                name: label.clone(),
                cwd: workdir.clone(),
                shell: shell.to_string(),
                cols: 120,
                rows: 32,
                ring,
                master_fd: master,
                child,
                exited: None,
            };
            let snap = Self::snapshot_of(&session, Some(pid));
            self.sessions.lock().insert(id, session);
            Ok((snap, motd))
        }

        #[cfg(not(unix))]
        {
            let _ = env;
            let motd = format!("QuixiDesk Terminal ({})\r\n> ", workdir.display());
            ring.lock().append(&motd);
            let session = LivePtySession {
                id: id.clone(),
                owner: owner.to_string(),
                name: label,
                cwd: workdir,
                shell: shell.to_string(),
                cols: 120,
                rows: 32,
                ring,
                exited: None,
            };
            let snap = Self::snapshot_of(&session, None);
            self.sessions.lock().insert(id, session);
            Ok((snap, motd))
        }
    }

    fn snapshot_of(s: &LivePtySession, pid_override: Option<u32>) -> TerminalSnapshot {
        #[cfg(unix)]
        let pid = pid_override.or(Some(s.child.id()));
        #[cfg(not(unix))]
        let pid = pid_override;
        TerminalSnapshot {
            id: s.id.clone(),
            owner: s.owner.clone(),
            name: s.name.clone(),
            cwd: s.cwd.to_string_lossy().into_owned(),
            shell: s.shell.clone(),
            pid,
            status: if s.exited.is_some() {
                "exited".into()
            } else {
                "running".into()
            },
            exit_code: s.exited,
            cols: s.cols,
            rows: s.rows,
            total_bytes: s.ring.lock().total_bytes(),
        }
    }

    fn poll_exit(s: &mut LivePtySession) {
        #[cfg(unix)]
        if s.exited.is_none() {
            if let Ok(Some(st)) = s.child.try_wait() {
                s.exited = Some(st.code().unwrap_or(-1));
            }
        }
    }

    pub fn check_owner(&self, owner: &str, id: &str) -> Result<()> {
        let map = self.sessions.lock();
        let Some(s) = map.get(id) else {
            bail!("Unknown terminal session `{id}`.");
        };
        if !owner.is_empty() && !s.owner.is_empty() && s.owner != owner {
            bail!("Terminal session `{id}` belongs to another conversation.");
        }
        Ok(())
    }

    pub fn write_raw(&self, owner: &str, id: &str, input: &str) -> Result<usize> {
        self.check_owner(owner, id)?;
        let start_offset = {
            let mut map = self.sessions.lock();
            let s = map.get_mut(id).unwrap();
            Self::poll_exit(s);
            if s.exited.is_some() {
                bail!("Terminal session `{id}` has already exited.");
            }
            let offset = s.ring.lock().total_bytes();
            #[cfg(unix)]
            {
                let bytes = input.as_bytes();
                let mut written = 0usize;
                while written < bytes.len() {
                    let rc = unsafe {
                        libc::write(
                            s.master_fd,
                            bytes[written..].as_ptr() as *const libc::c_void,
                            bytes.len() - written,
                        )
                    };
                    if rc > 0 {
                        written += rc as usize;
                    } else {
                        let err = std::io::Error::last_os_error();
                        if err.kind() == std::io::ErrorKind::Interrupted {
                            continue;
                        }
                        bail!("Write to PTY failed: {err}");
                    }
                }
            }
            #[cfg(not(unix))]
            {
                s.ring.lock().append(input);
                if input.contains('\n') || input.contains('\r') {
                    let cmd = input.trim();
                    if !cmd.is_empty() {
                        if let Ok(res) = crate::ShellHostEngine::run_bash_command(&s.cwd, cmd) {
                            let out = res["stdout"].as_str().unwrap_or("");
                            let err = res["stderr"].as_str().unwrap_or("");
                            let mut combined = String::new();
                            combined.push_str(out);
                            combined.push_str(err);
                            combined.push_str("\r\n> ");
                            s.ring.lock().append(&combined);
                        }
                    }
                }
            }
            offset
        };
        Ok(start_offset)
    }

    /// Sends input to an interactive PTY and waits until idle, session exit, or timeout.
    pub fn send_and_wait(
        &self,
        owner: &str,
        id: &str,
        input: &str,
        submit: bool,
        timeout: Duration,
        cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<TerminalSendResult> {
        let mut payload = input.to_string();
        if submit && !payload.ends_with('\n') && !payload.ends_with('\r') {
            payload.push('\n');
        }
        let start_offset = self.write_raw(owner, id, &payload)?;
        let started = Instant::now();
        let idle_quiet = Duration::from_millis(260);
        let min_wait = Duration::from_millis(90);

        let wait_reason = loop {
            if cancelled.load(std::sync::atomic::Ordering::SeqCst) {
                let _ = self.signal(owner, id, "SIGINT");
                bail!("Terminal send cancelled.");
            }
            let (exited, has_new_output, quiet_elapsed) = {
                let mut map = self.sessions.lock();
                let Some(s) = map.get_mut(id) else {
                    bail!("Terminal session `{id}` was closed.");
                };
                Self::poll_exit(s);
                let ring = s.ring.lock();
                let total = ring.total_bytes();
                let quiet = ring
                    .last_output_at
                    .map(|t| t.elapsed())
                    .unwrap_or(Duration::ZERO);
                (s.exited, total > start_offset, quiet)
            };
            if exited.is_some() {
                break "session_exit";
            }
            if started.elapsed() >= min_wait && has_new_output && quiet_elapsed >= idle_quiet {
                break "inferred_idle";
            }
            if started.elapsed() >= timeout {
                break "timeout";
            }
            std::thread::sleep(Duration::from_millis(30));
        };

        let mut map = self.sessions.lock();
        let Some(s) = map.get_mut(id) else {
            bail!("Terminal session `{id}` was closed.");
        };
        Self::poll_exit(s);
        let (viewport, _, lossy) = s.ring.lock().read_from(start_offset);
        let capped: String = viewport.chars().take(16_000).collect();
        let truncated = lossy || capped.len() < viewport.len();
        Ok(TerminalSendResult {
            id: id.to_string(),
            viewport: capped,
            wait_reason: wait_reason.to_string(),
            status: if s.exited.is_some() {
                "exited".into()
            } else {
                "running".into()
            },
            exit_code: s.exited,
            truncated,
        })
    }

    pub fn read_delta(&self, owner: &str, id: &str, from_offset: usize) -> Result<TerminalReadResult> {
        self.check_owner(owner, id)?;
        let mut map = self.sessions.lock();
        let s = map.get_mut(id).unwrap();
        Self::poll_exit(s);
        let (text, next_offset, lossy) = s.ring.lock().read_from(from_offset);
        Ok(TerminalReadResult {
            id: id.to_string(),
            text,
            next_offset,
            total_bytes: next_offset,
            lossy,
            status: if s.exited.is_some() {
                "exited".into()
            } else {
                "running".into()
            },
        })
    }

    pub fn read_scrollback(
        &self,
        owner: &str,
        id: &str,
        offset_lines: usize,
        limit_lines: usize,
    ) -> Result<Value> {
        self.check_owner(owner, id)?;
        let mut map = self.sessions.lock();
        let s = map.get_mut(id).unwrap();
        Self::poll_exit(s);
        let ring = s.ring.lock();
        let text = ring.read_lines(offset_lines, limit_lines.clamp(1, 500));
        Ok(json!({
            "id": id,
            "text": text,
            "totalBytes": ring.total_bytes(),
            "status": if s.exited.is_some() { "exited" } else { "running" },
            "exitCode": s.exited,
        }))
    }

    pub fn signal(&self, owner: &str, id: &str, sig_name: &str) -> Result<Value> {
        self.check_owner(owner, id)?;
        #[cfg(unix)]
        {
            let mut map = self.sessions.lock();
            let s = map.get_mut(id).unwrap();
            Self::poll_exit(s);
            if s.exited.is_some() {
                return Ok(json!({"ok": true, "status": "exited"}));
            }
            if sig_name.eq_ignore_ascii_case("EOF") {
                let eof = [4u8];
                unsafe {
                    let _ = libc::write(s.master_fd, eof.as_ptr() as *const libc::c_void, 1);
                }
                return Ok(json!({"ok": true, "signal": "EOF"}));
            }
            let sig = match sig_name.to_ascii_uppercase().as_str() {
                "SIGINT" | "INT" => libc::SIGINT,
                "SIGTERM" | "TERM" => libc::SIGTERM,
                "SIGKILL" | "KILL" => libc::SIGKILL,
                "SIGHUP" | "HUP" => libc::SIGHUP,
                other => bail!("Unsupported terminal signal `{other}`. Use SIGINT, SIGTERM, SIGKILL, or EOF."),
            };
            let fg_pgrp = unsafe { libc::tcgetpgrp(s.master_fd) };
            let target_pgid = if fg_pgrp > 0 {
                fg_pgrp
            } else {
                s.child.id() as i32
            };
            unsafe {
                libc::kill(-target_pgid, sig);
            }
            Ok(json!({"ok": true, "signal": sig_name, "pgid": target_pgid}))
        }
        #[cfg(not(unix))]
        {
            let _ = sig_name;
            Ok(json!({"ok": false, "error": "POSIX signals require Unix PTY"}))
        }
    }

    pub fn resize(&self, id: &str, cols: u16, rows: u16) {
        if let Some(s) = self.sessions.lock().get_mut(id) {
            s.cols = cols.max(20);
            s.rows = rows.max(5);
            #[cfg(unix)]
            if s.master_fd >= 0 {
                let ws = libc::winsize {
                    ws_row: s.rows,
                    ws_col: s.cols,
                    ws_xpixel: 0,
                    ws_ypixel: 0,
                };
                unsafe {
                    #[allow(clippy::useless_conversion)]
                    let _ = libc::ioctl(s.master_fd, libc::TIOCSWINSZ.into(), &ws);
                }
            }
        }
    }

    pub fn close(&self, owner: &str, id: &str) -> Result<bool> {
        self.check_owner(owner, id)?;
        Ok(self.sessions.lock().remove(id).is_some())
    }

    pub fn close_owner(&self, owner: &str) {
        self.sessions.lock().retain(|_, s| s.owner != owner);
    }

    pub fn list(&self, owner: &str) -> Vec<TerminalSnapshot> {
        let mut map = self.sessions.lock();
        let mut out = Vec::new();
        for s in map.values_mut() {
            if owner.is_empty() || s.owner == owner {
                Self::poll_exit(s);
                out.push(Self::snapshot_of(s, None));
            }
        }
        out
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn persistent_pty_retains_working_directory_and_env_across_sends() {
        let mgr = TerminalSessionManager::new();
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("nested_dir");
        std::fs::create_dir_all(&sub).unwrap();
        let env: HashMap<String, String> = std::env::vars().collect();
        let (snap, _) = mgr
            .open("sess-1", None, Some("dev"), dir.path(), "/bin/sh", &env)
            .unwrap();
        let r1 = mgr
            .send_and_wait(
                "sess-1",
                &snap.id,
                "cd nested_dir && QUIXI_PTY_VAR=persisted_42",
                true,
                Duration::from_secs(3),
                &AtomicBool::new(false),
            )
            .unwrap();
        assert_eq!(r1.status, "running");
        let r2 = mgr
            .send_and_wait(
                "sess-1",
                &snap.id,
                "printf '%s|%s\\n' \"$(pwd)\" \"$QUIXI_PTY_VAR\"",
                true,
                Duration::from_secs(3),
                &AtomicBool::new(false),
            )
            .unwrap();
        assert!(
            r2.viewport.contains("nested_dir") && r2.viewport.contains("persisted_42"),
            "unexpected PTY viewport: {}",
            r2.viewport
        );
        // Foreign session cannot read or close
        assert!(mgr.read_scrollback("foreign-sess", &snap.id, 0, 20).is_err());
        assert!(mgr.close("sess-1", &snap.id).unwrap());
    }
}
