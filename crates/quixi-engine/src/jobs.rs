//! Background Job Registry & Persistent PTY tool bindings inspired by DeepSeek Harness (`dsh-jobs`).
//!
//! Every background shell command or background PTY send owns a bounded output ring (`256 KiB`
//! live, `16 KiB` settled) plus a full spill file on disk. The model reads via a consuming cursor
//! (`JobOutput`), while UI panels inspect via a non-consuming byte offset (`read_at`), so neither
//! disturbs the other.

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};
use uuid::Uuid;

const LIVE_RING_CAP: usize = 256 * 1024;
const SETTLED_RING_CAP: usize = 16 * 1024;
const MAX_JOBS_PER_OWNER: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobView {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub owner: String,
    pub status: String, // "running" | "stopping" | "completed" | "killed" | "failed"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub started_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<i64>,
    pub total_bytes: usize,
    pub earliest_byte: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spill_path: Option<String>,
}

struct JobRecord {
    id: String,
    kind: String,
    label: String,
    owner: String,
    status: String,
    detail: Option<String>,
    started_at: i64,
    finished_at: Option<i64>,
    buf: String,
    earliest_byte: usize,
    model_cursor: usize,
    spill_path: Option<PathBuf>,
    cancel: Arc<AtomicBool>,
    pid: Option<u32>,
    settlement_reported: bool,
}

impl JobRecord {
    fn total_bytes(&self) -> usize {
        self.earliest_byte + self.buf.len()
    }

    fn append(&mut self, chunk: &str, cap: usize) {
        if chunk.is_empty() {
            return;
        }
        if let Some(ref spill) = self.spill_path {
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(spill) {
                let _ = f.write_all(chunk.as_bytes());
            }
        }
        self.buf.push_str(chunk);
        if self.buf.len() > cap {
            let drop_len = self.buf.len() - cap;
            let mut cut = drop_len;
            while cut < self.buf.len() && !self.buf.is_char_boundary(cut) {
                cut += 1;
            }
            self.buf.drain(..cut);
            self.earliest_byte += cut;
        }
    }

    fn trim_settled(&mut self) {
        if self.buf.len() > SETTLED_RING_CAP {
            let drop_len = self.buf.len() - SETTLED_RING_CAP;
            let mut cut = drop_len;
            while cut < self.buf.len() && !self.buf.is_char_boundary(cut) {
                cut += 1;
            }
            self.buf.drain(..cut);
            self.earliest_byte += cut;
        }
    }

    fn read_slice(&self, from: usize) -> (String, usize, bool) {
        let total = self.total_bytes();
        if from >= total {
            return (String::new(), total, false);
        }
        if from < self.earliest_byte {
            return (self.buf.clone(), total, true);
        }
        let rel = from - self.earliest_byte;
        let mut idx = rel;
        while idx < self.buf.len() && !self.buf.is_char_boundary(idx) {
            idx += 1;
        }
        (self.buf[idx..].to_string(), total, false)
    }

    fn view(&self) -> JobView {
        JobView {
            id: self.id.clone(),
            kind: self.kind.clone(),
            label: self.label.clone(),
            owner: self.owner.clone(),
            status: self.status.clone(),
            detail: self.detail.clone(),
            started_at: self.started_at,
            finished_at: self.finished_at,
            total_bytes: self.total_bytes(),
            earliest_byte: self.earliest_byte,
            spill_path: self
                .spill_path
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned()),
        }
    }
}

#[derive(Clone, Default)]
pub struct JobRegistry {
    jobs: Arc<Mutex<HashMap<String, JobRecord>>>,
}

impl JobRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn active_count(&self) -> usize {
        self.jobs
            .lock()
            .values()
            .filter(|j| matches!(j.status.as_str(), "running" | "stopping"))
            .count()
    }

    pub fn start_bash(
        &self,
        owner: &str,
        label: &str,
        cwd: &Path,
        command: &str,
        data_dir: &Path,
        sandbox_policy: Option<&quixi_platform::sandbox::SandboxPolicy>,
    ) -> Result<JobView, String> {
        {
            let map = self.jobs.lock();
            let running = map
                .values()
                .filter(|j| j.owner == owner && matches!(j.status.as_str(), "running" | "stopping"))
                .count();
            if running >= MAX_JOBS_PER_OWNER {
                return Err(format!(
                    "Maximum concurrent background jobs ({MAX_JOBS_PER_OWNER}) reached for this conversation. Use JobKill or wait for a job to finish."
                ));
            }
        }

        let job_id = format!(
            "bash-{}",
            &Uuid::new_v4().to_string().replace('-', "")[..16]
        );
        let spill_dir = crate::spill::session_spill_dir(data_dir, owner);
        let _ = fs::create_dir_all(&spill_dir);
        let spill_path = spill_dir.join(format!("{job_id}.log"));
        let _ = File::create(&spill_path);
        use std::fs::File;

        let shell = quixi_workspace::ShellHostEngine::posix_bash_shell();
        let base_args: Vec<String> = if cfg!(target_os = "windows") {
            vec!["/D".into(), "/S".into(), "/C".into(), command.into()]
        } else {
            vec!["-c".into(), command.into()]
        };
        let confined =
            sandbox_policy.map(|p| quixi_platform::sandbox::ProcessSandbox::confine(&shell, &base_args, p));
        let (prog, prog_args) = match &confined {
            Some(c) => (c.program.as_str(), c.args.as_slice()),
            None => (shell.as_str(), base_args.as_slice()),
        };

        let mut cmd = Command::new(prog);
        if quixi_proto::paths::test_profile().is_some() {
            cmd.env_clear();
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        let mut envs = quixi_workspace::ShellHostEngine::resolve_user_shell_env();
        if let Some(Some(ref tmp)) = confined.as_ref().map(|c| &c.temp_dir_override) {
            let s = tmp.to_string_lossy().into_owned();
            envs.insert("TMPDIR".into(), s.clone());
            envs.insert("TMP".into(), s.clone());
            envs.insert("TEMP".into(), s);
        }
        let mut child = cmd
            .args(prog_args)
            .current_dir(cwd)
            .envs(envs)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start background job: {e}"))?;

        let pid = child.id();
        let cancel = Arc::new(AtomicBool::new(false));
        let now = chrono::Utc::now().timestamp_millis();
        let record = JobRecord {
            id: job_id.clone(),
            kind: "bash".to_string(),
            label: if label.trim().is_empty() {
                command.chars().take(80).collect()
            } else {
                label.chars().take(80).collect()
            },
            owner: owner.to_string(),
            status: "running".to_string(),
            detail: None,
            started_at: now,
            finished_at: None,
            buf: String::new(),
            earliest_byte: 0,
            model_cursor: 0,
            spill_path: Some(spill_path),
            cancel: cancel.clone(),
            pid: Some(pid),
            settlement_reported: false,
        };
        let initial_view = record.view();
        self.jobs.lock().insert(job_id.clone(), record);

        let jobs_ref = self.jobs.clone();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let (tx, rx) = std::sync::mpsc::channel::<(bool, String)>();

        fn pump(mut r: impl Read + Send + 'static, tx: std::sync::mpsc::Sender<(bool, String)>, is_err: bool) {
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                while let Ok(n) = r.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    let text = String::from_utf8_lossy(&buf[..n]).into_owned();
                    if tx.send((is_err, text)).is_err() {
                        break;
                    }
                }
            });
        }
        pump(stdout, tx.clone(), false);
        pump(stderr, tx, true);

        std::thread::spawn(move || {
            loop {
                while let Ok((is_err, chunk)) = rx.try_recv() {
                    if let Some(rec) = jobs_ref.lock().get_mut(&job_id) {
                        if is_err {
                            rec.append(&chunk, LIVE_RING_CAP);
                        } else {
                            rec.append(&chunk, LIVE_RING_CAP);
                        }
                    }
                }
                if cancel.load(Ordering::SeqCst) {
                    #[cfg(unix)]
                    unsafe {
                        libc::kill(-(pid as i32), libc::SIGKILL);
                    }
                    #[cfg(target_os = "windows")]
                    {
                        let _ = Command::new("taskkill")
                            .args(["/PID", &pid.to_string(), "/T", "/F"])
                            .stdout(Stdio::null())
                            .stderr(Stdio::null())
                            .status();
                    }
                    let _ = child.kill();
                    let _ = child.wait();
                    while let Ok((_, chunk)) = rx.try_recv() {
                        if let Some(rec) = jobs_ref.lock().get_mut(&job_id) {
                            rec.append(&chunk, LIVE_RING_CAP);
                        }
                    }
                    if let Some(rec) = jobs_ref.lock().get_mut(&job_id) {
                        rec.status = "killed".into();
                        if rec.detail.is_none() {
                            rec.detail = Some("Killed by request".into());
                        }
                        rec.finished_at = Some(chrono::Utc::now().timestamp_millis());
                        rec.pid = None;
                        rec.trim_settled();
                    }
                    break;
                }
                match child.try_wait() {
                    Ok(Some(st)) => {
                        std::thread::sleep(Duration::from_millis(25));
                        while let Ok((_, chunk)) = rx.try_recv() {
                            if let Some(rec) = jobs_ref.lock().get_mut(&job_id) {
                                rec.append(&chunk, LIVE_RING_CAP);
                            }
                        }
                        let code = st.code().unwrap_or(-1);
                        if let Some(rec) = jobs_ref.lock().get_mut(&job_id) {
                            if st.success() {
                                rec.status = "completed".into();
                                rec.detail = Some("exit code: 0".into());
                            } else {
                                rec.status = "failed".into();
                                rec.detail = Some(format!("exit code: {code}"));
                            }
                            rec.finished_at = Some(chrono::Utc::now().timestamp_millis());
                            rec.pid = None;
                            rec.trim_settled();
                        }
                        break;
                    }
                    Ok(None) => std::thread::sleep(Duration::from_millis(40)),
                    Err(e) => {
                        if let Some(rec) = jobs_ref.lock().get_mut(&job_id) {
                            rec.status = "failed".into();
                            rec.detail = Some(format!("wait error: {e}"));
                            rec.finished_at = Some(chrono::Utc::now().timestamp_millis());
                            rec.pid = None;
                            rec.trim_settled();
                        }
                        break;
                    }
                }
            }
        });

        Ok(initial_view)
    }

    pub fn list(&self, owner: &str) -> Vec<JobView> {
        let map = self.jobs.lock();
        let mut out: Vec<JobView> = map
            .values()
            .filter(|j| j.owner == owner)
            .map(JobRecord::view)
            .collect();
        out.sort_by_key(|j| j.started_at);
        out
    }

    /// Runs a foreground bash command up to `foreground_timeout`. If it completes before the
    /// deadline, cleans up the transient job entry and returns `(formatted_output, is_error)`.
    /// If the deadline expires while still running, promotes the process into a persistent
    /// background job (`dsh-tool-bash` timeout-to-background mechanic) instead of killing it!
    #[allow(clippy::too_many_arguments)]
    pub fn run_bash_with_timeout_promotion(
        &self,
        owner: &str,
        label: &str,
        cwd: &Path,
        command: &str,
        data_dir: &Path,
        foreground_timeout: Duration,
        cancelled: &AtomicBool,
        sandbox_policy: Option<&quixi_platform::sandbox::SandboxPolicy>,
    ) -> Result<(String, bool, Option<String>), String> {
        self.run_bash_with_timeout_promotion_and_progress(
            owner,
            label,
            cwd,
            command,
            data_dir,
            foreground_timeout,
            cancelled,
            sandbox_policy,
            |_, _| {},
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn run_bash_with_timeout_promotion_and_progress<P>(
        &self,
        owner: &str,
        label: &str,
        cwd: &Path,
        command: &str,
        data_dir: &Path,
        foreground_timeout: Duration,
        cancelled: &AtomicBool,
        sandbox_policy: Option<&quixi_platform::sandbox::SandboxPolicy>,
        mut on_progress: P,
    ) -> Result<(String, bool, Option<String>), String>
    where
        P: FnMut(&str, u64),
    {
        let view = self.start_bash(owner, label, cwd, command, data_dir, sandbox_policy)?;
        let job_id = view.id.clone();
        let started = Instant::now();
        let mut last_progress = Instant::now();
        let mut last_seen_len = 0usize;

        while started.elapsed() < foreground_timeout {
            if cancelled.load(Ordering::SeqCst) {
                let _ = self.kill(owner, &job_id, Some("Cancelled during foreground wait"));
                return Err("Task stopped while waiting on command.".into());
            }
            let (settled, latest_line, cur_len) = {
                let map = self.jobs.lock();
                if let Some(j) = map.get(&job_id) {
                    let done = !matches!(j.status.as_str(), "running" | "stopping");
                    let tail = j
                        .buf
                        .lines()
                        .rev()
                        .map(str::trim)
                        .find(|l| !l.is_empty())
                        .map(str::to_owned);
                    (done, tail, j.buf.len())
                } else {
                    (true, None, 0)
                }
            };
            if last_progress.elapsed() >= Duration::from_millis(320) && cur_len != last_seen_len {
                last_seen_len = cur_len;
                last_progress = Instant::now();
                if let Some(ref line) = latest_line {
                    let preview: String = line.chars().take(160).collect();
                    on_progress(&preview, started.elapsed().as_millis() as u64);
                }
            }
            if settled {
                break;
            }
            std::thread::sleep(Duration::from_millis(45));
        }

        match self.read_consuming(owner, &job_id, None, cancelled) {
            Err(err) => {
                let _ = self.kill(owner, &job_id, Some("Cancelled during foreground wait"));
                Err(err)
            }
            Ok(val) => {
                let status = val["job"]["status"].as_str().unwrap_or("failed").to_owned();
                let output = val["output"].as_str().unwrap_or("").trim_end().to_owned();
                let detail = val["job"]["detail"].as_str().unwrap_or("").to_owned();
                if matches!(status.as_str(), "running" | "stopping") {
                    let secs = foreground_timeout.as_secs().max(1);
                    let mut msg = output;
                    if !msg.is_empty() {
                        msg.push('\n');
                    }
                    msg.push_str(&format!(
                        "[Foreground timeout ({secs}s) reached; command is still running and was promoted to background job `{job_id}` rather than killed. Call `JobOutput(job_id: \"{job_id}\", wait_seconds: 15)` to monitor or `JobKill(job_id: \"{job_id}\")` to stop it.]"
                    ));
                    Ok((msg, false, Some(job_id)))
                } else {
                    if let Some(removed) = self.jobs.lock().remove(&job_id) {
                        if let Some(sp) = removed.spill_path {
                            let _ = fs::remove_file(sp);
                        }
                    }
                    let exit_line = if detail.starts_with("exit code:") {
                        let code = detail.trim_start_matches("exit code:").trim();
                        format!("[exit code {code}]")
                    } else if status == "completed" {
                        "[exit code 0]".to_owned()
                    } else {
                        format!("[{status}: {detail}]")
                    };
                    let formatted = if output.is_empty() {
                        exit_line
                    } else {
                        format!("{output}\n{exit_line}")
                    };
                    Ok((formatted, status != "completed", None))
                }
            }
        }
    }

    /// Consuming model read (`JobOutput`), optionally waiting up to `wait_timeout` for settlement.
    pub fn read_consuming(
        &self,
        owner: &str,
        job_id: &str,
        wait_timeout: Option<Duration>,
        cancelled: &AtomicBool,
    ) -> Result<Value, String> {
        if let Some(wait) = wait_timeout.filter(|d| !d.is_zero()) {
            let started = Instant::now();
            loop {
                if cancelled.load(Ordering::SeqCst) {
                    return Err("Task stopped while waiting on job.".into());
                }
                let settled = {
                    let map = self.jobs.lock();
                    let Some(j) = map.get(job_id).filter(|j| j.owner == owner) else {
                        return Err(format!("Unknown job `{job_id}` for this conversation."));
                    };
                    !matches!(j.status.as_str(), "running" | "stopping")
                };
                if settled || started.elapsed() >= wait {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }

        let mut map = self.jobs.lock();
        let Some(j) = map.get_mut(job_id).filter(|j| j.owner == owner) else {
            return Err(format!("Unknown job `{job_id}` for this conversation."));
        };
        let (text, next, lossy) = j.read_slice(j.model_cursor);
        j.model_cursor = next;
        if !matches!(j.status.as_str(), "running" | "stopping") {
            j.settlement_reported = true;
        }
        let capped: String = text.chars().take(12_000).collect();
        Ok(json!({
            "job": j.view(),
            "output": capped,
            "lossy": lossy || capped.len() < text.len(),
        }))
    }

    /// Non-consuming observer read for UI live follow.
    pub fn read_at(&self, owner: &str, job_id: &str, from_offset: usize) -> Result<Value, String> {
        let map = self.jobs.lock();
        let Some(j) = map.get(job_id).filter(|j| j.owner == owner) else {
            return Err(format!("Unknown job `{job_id}`."));
        };
        let (text, next, lossy) = j.read_slice(from_offset);
        Ok(json!({
            "job": j.view(),
            "text": text,
            "next": next,
            "lossy": lossy,
        }))
    }

    pub fn kill(&self, owner: &str, job_id: &str, reason: Option<&str>) -> Result<Value, String> {
        let mut map = self.jobs.lock();
        let Some(j) = map.get_mut(job_id).filter(|j| j.owner == owner) else {
            return Err(format!("Unknown job `{job_id}`."));
        };
        if !matches!(j.status.as_str(), "running" | "stopping") {
            return Ok(json!({"status": "already-finished", "job": j.view()}));
        }
        j.status = "stopping".into();
        if let Some(r) = reason.filter(|s| !s.trim().is_empty()) {
            j.detail = Some(r.to_string());
        }
        j.cancel.store(true, Ordering::SeqCst);
        Ok(json!({"status": "requested", "job": j.view()}))
    }

    pub fn kill_all_for_owner(&self, owner: &str) {
        let mut map = self.jobs.lock();
        for j in map.values_mut().filter(|j| j.owner == owner) {
            if matches!(j.status.as_str(), "running" | "stopping") {
                j.status = "stopping".into();
                j.detail = Some("Owner session stopped or deleted".into());
                j.cancel.store(true, Ordering::SeqCst);
            }
        }
    }

    /// Drains one-time completion notices for settled background jobs not yet collected by `JobOutput`.
    pub fn take_settlement_notices(&self, owner: &str) -> Vec<String> {
        let mut map = self.jobs.lock();
        let mut notices = Vec::new();
        for j in map.values_mut().filter(|j| j.owner == owner) {
            if !j.settlement_reported && !matches!(j.status.as_str(), "running" | "stopping") {
                j.settlement_reported = true;
                let detail = j.detail.as_deref().unwrap_or("");
                notices.push(format!(
                    "[Background job `{}` ({}) settled with status `{}`{}. Call `JobOutput(job_id: \"{}\")` to inspect its output.]",
                    j.id,
                    j.label,
                    j.status,
                    if detail.is_empty() { String::new() } else { format!(" ({detail})") },
                    j.id
                ));
            }
        }
        notices
    }
}

pub fn job_and_terminal_tool_schemas() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "JobList",
                "description": "List background jobs started in this conversation (running and completed).",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": [],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "JobOutput",
                "description": "Read incremental output produced by a background job since the last read, optionally waiting up to 30 seconds for completion.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "job_id": { "type": "string", "description": "Job id returned by Bash(run_in_background: true)." },
                        "wait_seconds": { "type": "integer", "description": "Optional seconds (0-30) to wait for job completion before returning." }
                    },
                    "required": ["job_id"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "JobKill",
                "description": "Terminate a running background job and its process group.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "job_id": { "type": "string" },
                        "reason": { "type": "string" }
                    },
                    "required": ["job_id"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "TerminalOpen",
                "description": "Open a persistent interactive PTY shell session owned by this conversation (retains cd, exported variables, and REPL state across calls).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "Optional short label for the terminal." },
                        "cwd": { "type": "string", "description": "Optional working directory inside workspace." }
                    },
                    "required": [],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "TerminalSend",
                "description": "Send input to an open persistent PTY session and wait until output settles, the shell exits, or timeout expires.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "terminal_id": { "type": "string" },
                        "input": { "type": "string", "description": "Command or input text to send." },
                        "submit": { "type": "boolean", "description": "Append newline to submit (default true)." },
                        "timeout_seconds": { "type": "integer", "description": "Wait timeout in seconds (1-120, default 15)." }
                    },
                    "required": ["terminal_id", "input"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "TerminalRead",
                "description": "Read scrollback lines from a persistent PTY session.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "terminal_id": { "type": "string" },
                        "offset_lines": { "type": "integer", "description": "Lines from newest end to skip (default 0)." },
                        "limit_lines": { "type": "integer", "description": "Max lines to read (default 120)." }
                    },
                    "required": ["terminal_id"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "TerminalSignal",
                "description": "Send SIGINT, SIGTERM, SIGKILL, or EOF to a persistent PTY foreground process group.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "terminal_id": { "type": "string" },
                        "signal": { "type": "string", "description": "SIGINT, SIGTERM, SIGKILL, or EOF." }
                    },
                    "required": ["terminal_id", "signal"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "TerminalClose",
                "description": "Close a persistent PTY session and terminate its process tree.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "terminal_id": { "type": "string" }
                    },
                    "required": ["terminal_id"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "TerminalList",
                "description": "List open and recently exited persistent PTY terminal sessions owned by this conversation.",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": [],
                    "additionalProperties": false
                }
            }
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_bash_job_streams_to_both_observer_and_consuming_model_cursor() {
        let tmp = tempfile::tempdir().unwrap();
        let reg = JobRegistry::new();
        let view = reg
            .start_bash(
                "sess-1",
                "echo test",
                tmp.path(),
                "printf 'alpha\\nbeta\\n'",
                tmp.path(),
                None,
            )
            .unwrap();
        assert!(view.id.starts_with("bash-"));
        // Consuming read waits up to 2s for completion
        let r1 = reg
            .read_consuming(
                "sess-1",
                &view.id,
                Some(Duration::from_secs(2)),
                &AtomicBool::new(false),
            )
            .unwrap();
        assert_eq!(r1["job"]["status"], "completed");
        assert!(r1["output"].as_str().unwrap().contains("alpha"));
        // Second consuming read sees empty delta because cursor advanced
        let r2 = reg
            .read_consuming("sess-1", &view.id, None, &AtomicBool::new(false))
            .unwrap();
        assert_eq!(r2["output"], "");
        // Observer read from offset 0 still sees full retained output!
        let obs = reg.read_at("sess-1", &view.id, 0).unwrap();
        assert!(obs["text"].as_str().unwrap().contains("alpha\nbeta"));
        // Foreign session rejected
        assert!(reg.read_at("other-sess", &view.id, 0).is_err());
    }

    #[test]
    fn foreground_bash_promotes_to_background_job_on_timeout_without_killing_child() {
        let tmp = tempfile::tempdir().unwrap();
        let reg = JobRegistry::new();
        let (out, is_err, promoted) = reg
            .run_bash_with_timeout_promotion(
                "sess-promote",
                "long build",
                tmp.path(),
                "printf 'phase-1\\n'; sleep 0.5; printf 'phase-2\\n'",
                tmp.path(),
                Duration::from_millis(120),
                &AtomicBool::new(false),
                None,
            )
            .unwrap();
        assert!(!is_err);
        let jid = promoted.expect("must promote to background job");
        assert!(out.contains("promoted to background job"));
        let settled = reg
            .read_consuming(
                "sess-promote",
                &jid,
                Some(Duration::from_secs(3)),
                &AtomicBool::new(false),
            )
            .unwrap();
        assert_eq!(settled["job"]["status"], "completed");
        assert!(settled["output"].as_str().unwrap().contains("phase-2"));
    }
}
