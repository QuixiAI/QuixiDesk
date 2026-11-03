//! Advanced code navigation, patching, globbing, freshness tracking, and nested instructions.
//!
//! Inspired by DeepSeek Harness (`dsh-tool-fs-search`, `dsh-fs-patch`, `dsh-fs-observation-policy`,
//! `dsh-tool-lsp`, `dsh-agent-instructions`, and `dsh-experimental-tool-worktree`).

use globset::GlobBuilder;
use parking_lot::Mutex;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant, UNIX_EPOCH};

pub const TOOL_GLOB: &str = "Glob";
pub const TOOL_APPLY_PATCH: &str = "ApplyPatch";
pub const TOOL_LSP: &str = "Lsp";
pub const TOOL_CREATE_WORKTREE: &str = "CreateWorktree";

pub fn code_tool_schemas() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": TOOL_GLOB,
                "description": "Find files matching a glob pattern (e.g. `**/*.rs`, `src/**/*.ts`) sorted by most recently modified. Available in Manual mode without approval.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "pattern": { "type": "string", "description": "Glob pattern such as `**/*.rs` or `crates/*/Cargo.toml`." },
                        "path": { "type": "string", "description": "Optional directory within workspace (defaults to `.`)." }
                    },
                    "required": ["pattern"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": TOOL_APPLY_PATCH,
                "description": "Atomically apply a multi-file or multi-hunk patch (`*** Begin Patch` with `*** Add File:` / `*** Update File:` / `*** Delete File:`, or standard unified diff `--- a/...` / `+++ b/...`). Validates all hunks before writing any file.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "patch": { "type": "string", "description": "Complete patch text." }
                    },
                    "required": ["patch"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": TOOL_LSP,
                "description": "Query a language server for semantic code navigation (`goToDefinition`, `findReferences`, `goToImplementation`, `hover`). Line and character are 1-based. Available in Manual mode without approval.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "operation": {
                            "type": "string",
                            "enum": ["goToDefinition", "findReferences", "goToImplementation", "hover"],
                            "description": "LSP query operation."
                        },
                        "file_path": { "type": "string", "description": "Path to source file." },
                        "line": { "type": "integer", "description": "1-based line number." },
                        "character": { "type": "integer", "description": "1-based column offset." }
                    },
                    "required": ["operation", "file_path", "line", "character"],
                    "additionalProperties": false
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": TOOL_CREATE_WORKTREE,
                "description": "Create an isolated Git worktree and branch under `.agents/worktrees/<name>` from HEAD, optionally switching this conversation's working directory to it.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "Short worktree slug, e.g. `feature-refactor`." },
                        "enter": { "type": "boolean", "description": "Switch session working directory into the new worktree (default true)." }
                    },
                    "required": ["name"],
                    "additionalProperties": false
                }
            }
        }),
    ]
}

/// Runs `Glob` against `target` relative to `workspace_root`.
pub fn execute_glob(target: &Path, workspace_root: &Path, pattern: &str) -> (String, bool) {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return ("Glob `pattern` must not be empty.".into(), true);
    }
    let normalized = if !pattern.contains('/') && !pattern.starts_with("**") {
        format!("**/{pattern}")
    } else {
        pattern.to_string()
    };
    let matcher = match GlobBuilder::new(&normalized)
        .literal_separator(false)
        .build()
        .map(|g| g.compile_matcher())
    {
        Ok(m) => m,
        Err(e) => return (format!("Invalid glob pattern `{pattern}`: {e}"), true),
    };
    if !target.exists() {
        return (
            format!("Cannot search {}: path does not exist", target.display()),
            true,
        );
    }

    let mut hits: Vec<(u128, String)> = Vec::new();
    let walker = ignore::WalkBuilder::new(target)
        .hidden(false)
        .git_ignore(true)
        .follow_links(false)
        .max_depth(Some(12))
        .build();

    for entry in walker.flatten() {
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        if p.components().any(|c| {
            let s = c.as_os_str();
            s == ".git" || s == "node_modules" || s == "target" || s == "dist"
        }) {
            continue;
        }
        let rel_target = p.strip_prefix(target).unwrap_or(p);
        let rel_str = rel_target.to_string_lossy().replace('\\', "/");
        if matcher.is_match(&rel_str) || matcher.is_match(p.file_name().unwrap_or_default()) {
            let display = p
                .strip_prefix(workspace_root)
                .unwrap_or(p)
                .to_string_lossy()
                .replace('\\', "/");
            let mtime = fs::metadata(p)
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            hits.push((mtime, display));
        }
    }

    hits.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let total = hits.len();
    let capped = total > 200;
    let out: Vec<String> = hits.into_iter().take(200).map(|(_, p)| p).collect();
    if out.is_empty() {
        (format!("No files matched `{pattern}`."), false)
    } else if capped {
        (
            format!(
                "{}\n\n[Showing 200 most recently modified of {total} matching files]",
                out.join("\n")
            ),
            false,
        )
    } else {
        (out.join("\n"), false)
    }
}

#[derive(Debug, Clone)]
pub enum PatchFileOp {
    Add { path: PathBuf, content: String },
    Update { path: PathBuf, hunks: Vec<Vec<String>> },
    Delete { path: PathBuf },
}

/// Parses either `*** Begin Patch` format or standard unified diff (`--- a/...` / `+++ b/...`).
pub fn parse_patch_operations(patch: &str, cwd: &Path) -> Result<Vec<PatchFileOp>, String> {
    let normalized = patch.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized.lines().collect();
    if lines.iter().any(|l| {
        l.starts_with("*** Begin Patch")
            || l.starts_with("*** Add File:")
            || l.starts_with("*** Update File:")
            || l.starts_with("*** Delete File:")
    }) {
        parse_envelope_patch(&lines, cwd)
    } else {
        parse_unified_patch(&lines, cwd)
    }
}

fn resolve_rel(cwd: &Path, raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "/dev/null" {
        return Err("Patch file path must not be empty.".into());
    }
    let stripped = trimmed
        .strip_prefix("a/")
        .or_else(|| trimmed.strip_prefix("b/"))
        .unwrap_or(trimmed);
    let p = Path::new(stripped);
    Ok(if p.is_absolute() {
        p.to_path_buf()
    } else {
        cwd.join(p)
    })
}

fn parse_envelope_patch(lines: &[&str], cwd: &Path) -> Result<Vec<PatchFileOp>, String> {
    let mut ops = Vec::new();
    let mut i = 0usize;
    while i < lines.len() {
        let line = lines[i];
        if line.trim().is_empty() || line.starts_with("*** Begin Patch") {
            i += 1;
            continue;
        }
        if line.starts_with("*** End Patch") {
            break;
        }
        if let Some(rest) = line.strip_prefix("*** Add File:") {
            let path = resolve_rel(cwd, rest)?;
            i += 1;
            let mut body = Vec::new();
            while i < lines.len() && !lines[i].starts_with("*** ") {
                let l = lines[i];
                body.push(l.strip_prefix('+').unwrap_or(l));
                i += 1;
            }
            let mut content = body.join("\n");
            if !content.is_empty() {
                content.push('\n');
            }
            ops.push(PatchFileOp::Add { path, content });
        } else if let Some(rest) = line.strip_prefix("*** Delete File:") {
            let path = resolve_rel(cwd, rest)?;
            ops.push(PatchFileOp::Delete { path });
            i += 1;
        } else if let Some(rest) = line.strip_prefix("*** Update File:") {
            let path = resolve_rel(cwd, rest)?;
            i += 1;
            if i < lines.len() && lines[i].starts_with("*** Move to:") {
                return Err("File renames inside ApplyPatch are not supported yet; use Bash `git mv` or Write.".into());
            }
            let mut hunks: Vec<Vec<String>> = Vec::new();
            let mut cur: Vec<String> = Vec::new();
            while i < lines.len() && !lines[i].starts_with("*** ") {
                let l = lines[i];
                if l.starts_with("@@") {
                    if !cur.is_empty() {
                        hunks.push(std::mem::take(&mut cur));
                    }
                } else if l.starts_with('+') || l.starts_with('-') || l.starts_with(' ') || l.is_empty() {
                    cur.push(if l.is_empty() { " ".into() } else { l.to_string() });
                }
                i += 1;
            }
            if !cur.is_empty() {
                hunks.push(cur);
            }
            if hunks.is_empty() {
                return Err(format!("Update for {} contained no diff hunks.", path.display()));
            }
            ops.push(PatchFileOp::Update { path, hunks });
        } else {
            return Err(format!("Unexpected patch header line {}: `{line}`", i + 1));
        }
    }
    if ops.is_empty() {
        return Err("Patch contained no file operations.".into());
    }
    Ok(ops)
}

fn parse_unified_patch(lines: &[&str], cwd: &Path) -> Result<Vec<PatchFileOp>, String> {
    let mut ops = Vec::new();
    let mut i = 0usize;
    while i < lines.len() {
        if !lines[i].starts_with("--- ") {
            i += 1;
            continue;
        }
        let old_hdr = lines[i][4..].split('\t').next().unwrap_or("").trim();
        i += 1;
        if i >= lines.len() || !lines[i].starts_with("+++ ") {
            return Err("Malformed unified diff: missing `+++` header.".into());
        }
        let new_hdr = lines[i][4..].split('\t').next().unwrap_or("").trim();
        i += 1;

        let mut hunks: Vec<Vec<String>> = Vec::new();
        let mut cur: Vec<String> = Vec::new();
        while i < lines.len() && !lines[i].starts_with("--- ") && !lines[i].starts_with("diff ") {
            let l = lines[i];
            if l.starts_with("@@") {
                if !cur.is_empty() {
                    hunks.push(std::mem::take(&mut cur));
                }
            } else if l.starts_with('\\') {
                // "\ No newline at end of file"
            } else if l.starts_with('+') || l.starts_with('-') || l.starts_with(' ') {
                cur.push(l.to_string());
            }
            i += 1;
        }
        if !cur.is_empty() {
            hunks.push(cur);
        }

        if old_hdr == "/dev/null" {
            let path = resolve_rel(cwd, new_hdr)?;
            let mut added = Vec::new();
            for h in &hunks {
                for l in h {
                    if let Some(rest) = l.strip_prefix('+') {
                        added.push(rest);
                    }
                }
            }
            let mut content = added.join("\n");
            if !content.is_empty() {
                content.push('\n');
            }
            ops.push(PatchFileOp::Add { path, content });
        } else if new_hdr == "/dev/null" {
            let path = resolve_rel(cwd, old_hdr)?;
            ops.push(PatchFileOp::Delete { path });
        } else {
            let path = resolve_rel(cwd, new_hdr)?;
            if hunks.is_empty() {
                return Err(format!("No hunks found for {}", path.display()));
            }
            ops.push(PatchFileOp::Update { path, hunks });
        }
    }
    if ops.is_empty() {
        return Err("Patch contained no recognizable unified or `*** Begin Patch` hunks.".into());
    }
    Ok(ops)
}

fn apply_hunks_to_text(original: &str, hunks: &[Vec<String>], display: &Path) -> Result<String, String> {
    let use_crlf = original.contains("\r\n");
    let trailing_nl = original.ends_with('\n');
    let norm = original.replace("\r\n", "\n");
    let mut file_lines: Vec<String> = norm.lines().map(str::to_string).collect();
    let mut search_from = 0usize;

    for (hunk_idx, hunk) in hunks.iter().enumerate() {
        let mut expected: Vec<&str> = Vec::new();
        let mut replacement: Vec<String> = Vec::new();
        for l in hunk {
            if let Some(rest) = l.strip_prefix(' ') {
                expected.push(rest);
                replacement.push(rest.to_string());
            } else if let Some(rest) = l.strip_prefix('-') {
                expected.push(rest);
            } else if let Some(rest) = l.strip_prefix('+') {
                replacement.push(rest.to_string());
            }
        }
        if expected.is_empty() {
            file_lines.splice(search_from..search_from, replacement);
            continue;
        }
        let found = (search_from..=file_lines.len().saturating_sub(expected.len()))
            .find(|&pos| {
                file_lines[pos..pos + expected.len()]
                    .iter()
                    .zip(&expected)
                    .all(|(a, b)| a == b)
            })
            .or_else(|| {
                // Retry trimmed whitespace match if exact whitespace drifted slightly
                (search_from..=file_lines.len().saturating_sub(expected.len())).find(|&pos| {
                    file_lines[pos..pos + expected.len()]
                        .iter()
                        .zip(&expected)
                        .all(|(a, b)| a.trim_end() == b.trim_end())
                })
            })
            .ok_or_else(|| {
                format!(
                    "Hunk #{} failed to match current contents of {}. Read the file first and retry.",
                    hunk_idx + 1,
                    display.display()
                )
            })?;
        let new_len = replacement.len();
        file_lines.splice(found..found + expected.len(), replacement);
        search_from = found + new_len;
    }

    let sep = if use_crlf { "\r\n" } else { "\n" };
    let mut out = file_lines.join(sep);
    if trailing_nl && !out.is_empty() {
        out.push_str(sep);
    }
    Ok(out)
}

/// Validates every hunk in memory first, invokes `before_mutate` on each target path,
/// and only then commits writes/deletes so a partial failure leaves the workspace untouched.
pub fn execute_apply_patch(
    ops: &[PatchFileOp],
    mut before_mutate: impl FnMut(&Path),
) -> Result<Vec<PathBuf>, String> {
    enum Staged {
        Write(PathBuf, String),
        Delete(PathBuf),
    }
    let mut staged = Vec::with_capacity(ops.len());
    for op in ops {
        match op {
            PatchFileOp::Add { path, content } => {
                staged.push(Staged::Write(path.clone(), content.clone()));
            }
            PatchFileOp::Delete { path } => {
                if !path.exists() {
                    return Err(format!(
                        "Cannot delete {}: file does not exist.",
                        path.display()
                    ));
                }
                staged.push(Staged::Delete(path.clone()));
            }
            PatchFileOp::Update { path, hunks } => {
                let raw = fs::read_to_string(path)
                    .map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
                let updated = apply_hunks_to_text(&raw, hunks, path)?;
                staged.push(Staged::Write(path.clone(), updated));
            }
        }
    }

    let mut touched = Vec::with_capacity(staged.len());
    for item in staged {
        match item {
            Staged::Write(path, content) => {
                before_mutate(&path);
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| format!("Cannot create parent of {}: {e}", path.display()))?;
                }
                fs::write(&path, content)
                    .map_err(|e| format!("Cannot write {}: {e}", path.display()))?;
                touched.push(path);
            }
            Staged::Delete(path) => {
                before_mutate(&path);
                fs::remove_file(&path)
                    .map_err(|e| format!("Cannot delete {}: {e}", path.display()))?;
                touched.push(path);
            }
        }
    }
    Ok(touched)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileStamp {
    len: u64,
    mtime_ns: u128,
}

impl FileStamp {
    fn of(path: &Path) -> Option<Self> {
        let meta = fs::metadata(path).ok()?;
        let mtime_ns = meta
            .modified()
            .ok()?
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_nanos();
        Some(Self {
            len: meta.len(),
            mtime_ns,
        })
    }
}

/// Tracks per-session file observations (`Read`/`Write`/`Edit`/`ApplyPatch`) to detect
/// external concurrent modifications (`FS_STALE_VERSION`) and discover nested `AGENTS.md` files.
#[derive(Clone, Default)]
pub struct WorkspaceObservationService {
    stamps: Arc<Mutex<HashMap<String, HashMap<PathBuf, FileStamp>>>>,
    seen_agents_md: Arc<Mutex<HashMap<String, HashSet<String>>>>,
}

impl WorkspaceObservationService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&self, session_id: &str, path: &Path) {
        if session_id.is_empty() {
            return;
        }
        let canon = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        if let Some(stamp) = FileStamp::of(&canon) {
            self.stamps
                .lock()
                .entry(session_id.to_string())
                .or_default()
                .insert(canon, stamp);
        }
    }

    /// If `path` was previously observed by `session_id` and its disk stamp changed externally,
    /// returns a descriptive staleness error so the model re-reads before clobbering edits.
    pub fn verify_not_stale(&self, session_id: &str, path: &Path) -> Result<(), String> {
        if session_id.is_empty() {
            return Ok(());
        }
        let canon = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let map = self.stamps.lock();
        let Some(prev) = map.get(session_id).and_then(|m| m.get(&canon)).copied() else {
            return Ok(());
        };
        if let Some(cur) = FileStamp::of(&canon) {
            if cur != prev {
                return Err(format!(
                    "FS_STALE_VERSION: {} changed on disk since you last read or edited it. Call Read on it first so concurrent changes are not overwritten.",
                    path.display()
                ));
            }
        }
        Ok(())
    }

    /// Walks directories between `cwd` and `touched` to discover unseen nested `AGENTS.md` files.
    pub fn discover_nested_instructions(
        &self,
        session_id: &str,
        cwd: &Path,
        touched: &Path,
    ) -> Vec<(String, String)> {
        if session_id.is_empty() || !cwd.is_dir() {
            return vec![];
        }
        let Ok(canon_root) = fs::canonicalize(cwd) else {
            return vec![];
        };
        let start_dir = if touched.is_dir() {
            touched.to_path_buf()
        } else {
            touched.parent().unwrap_or(cwd).to_path_buf()
        };
        let Ok(canon_dir) = fs::canonicalize(&start_dir) else {
            return vec![];
        };
        if !canon_dir.starts_with(&canon_root) || canon_dir == canon_root {
            return vec![];
        }

        let mut dirs = Vec::new();
        let mut cur = canon_dir.as_path();
        while cur.starts_with(&canon_root) && cur != canon_root {
            dirs.push(cur.to_path_buf());
            let Some(parent) = cur.parent() else {
                break;
            };
            cur = parent;
        }
        dirs.reverse();

        let mut discovered = Vec::new();
        let mut seen = self.seen_agents_md.lock();
        let session_seen = seen.entry(session_id.to_string()).or_default();

        for dir in dirs {
            let candidate = dir.join("AGENTS.md");
            if !candidate.is_file() {
                continue;
            }
            let Ok(raw) = fs::read_to_string(&candidate) else {
                continue;
            };
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                continue;
            }
            let digest = format!("{:x}", Sha256::digest(trimmed.as_bytes()));
            if !session_seen.insert(digest) {
                continue;
            }
            let rel = candidate
                .strip_prefix(&canon_root)
                .unwrap_or(&candidate)
                .to_string_lossy()
                .replace('\\', "/");
            let safe_body: String = trimmed
                .replace("</system-reminder>", "<\\/system-reminder>")
                .chars()
                .take(16_000)
                .collect();
            discovered.push((rel, safe_body));
        }
        discovered
    }

    pub fn clear_session(&self, session_id: &str) {
        self.stamps.lock().remove(session_id);
        self.seen_agents_md.lock().remove(session_id);
    }
}

/// Executes a one-shot stdio LSP query (`goToDefinition`, `findReferences`, `goToImplementation`, `hover`).
pub fn execute_lsp_query(
    cwd: &Path,
    operation: &str,
    file_path: &Path,
    line_1based: u64,
    char_1based: u64,
) -> (String, bool) {
    if !file_path.is_file() {
        return (
            format!("LSP target file does not exist: {}", file_path.display()),
            true,
        );
    }
    let ext = file_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let (lang_id, candidates): (&str, &[(&str, &[&str])]) = match ext.as_str() {
        "rs" => ("rust", &[("rust-analyzer", &[])]),
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" => (
            "typescript",
            &[
                ("typescript-language-server", &["--stdio"]),
                ("vtsls", &["--stdio"]),
            ],
        ),
        "py" => (
            "python",
            &[
                ("pyright-langserver", &["--stdio"]),
                ("pylsp", &[]),
            ],
        ),
        "go" => ("go", &[("gopls", &["serve"])]),
        "c" | "cc" | "cpp" | "cxx" | "h" | "hpp" => ("cpp", &[("clangd", &["--log=error"])]),
        other => {
            return (
                format!("LSP_UNAVAILABLE: no built-in language server mapping for `.{other}` files."),
                true,
            )
        }
    };

    let envs = quixi_workspace::ShellHostEngine::resolve_user_shell_env();
    let path_var = envs.get("PATH").cloned().unwrap_or_default();
    let chosen = candidates.iter().find_map(|(bin, args)| {
        find_executable(bin, &path_var).map(|full| (full, *args))
    });
    let Some((server_bin, server_args)) = chosen else {
        let names: Vec<&str> = candidates.iter().map(|(b, _)| *b).collect();
        return (
            format!(
                "LSP_UNAVAILABLE: none of [{}] was found on PATH. Use Grep/Read or install the language server.",
                names.join(", ")
            ),
            true,
        );
    };

    let Ok(canon_file) = fs::canonicalize(file_path) else {
        return ("Cannot canonicalize file path for LSP.".into(), true);
    };
    let Ok(canon_root) = fs::canonicalize(cwd) else {
        return ("Cannot canonicalize workspace root for LSP.".into(), true);
    };
    let Ok(file_text) = fs::read_to_string(&canon_file) else {
        return ("Cannot read UTF-8 source file for LSP.".into(), true);
    };
    let Ok(root_uri) = url::Url::from_file_path(&canon_root) else {
        return ("Cannot construct workspace URI.".into(), true);
    };
    let Ok(file_uri) = url::Url::from_file_path(&canon_file) else {
        return ("Cannot construct file URI.".into(), true);
    };

    let method = match operation {
        "goToDefinition" => "textDocument/definition",
        "findReferences" => "textDocument/references",
        "goToImplementation" => "textDocument/implementation",
        "hover" => "textDocument/hover",
        other => return (format!("Unsupported LSP operation `{other}`."), true),
    };

    let mut child = match Command::new(&server_bin)
        .args(server_args)
        .current_dir(&canon_root)
        .envs(&envs)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return (format!("Failed to spawn {}: {e}", server_bin.display()), true),
    };

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel::<Value>();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        while let Some(msg) = read_lsp_frame(&mut reader) {
            if tx.send(msg).is_err() {
                break;
            }
        }
    });

    let run = (|| -> Result<Value, String> {
        send_lsp_frame(
            &mut stdin,
            &json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "processId": std::process::id(),
                    "rootUri": root_uri.as_str(),
                    "capabilities": {}
                }
            }),
        )?;
        let deadline = Instant::now() + Duration::from_secs(12);
        wait_lsp_response(&rx, 1, deadline)?;
        send_lsp_frame(
            &mut stdin,
            &json!({
                "jsonrpc": "2.0",
                "method": "initialized",
                "params": {}
            }),
        )?;
        send_lsp_frame(
            &mut stdin,
            &json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": file_uri.as_str(),
                        "languageId": lang_id,
                        "version": 1,
                        "text": file_text
                    }
                }
            }),
        )?;

        let line0 = line_1based.saturating_sub(1);
        let char0 = char_1based.saturating_sub(1);
        let params = if operation == "findReferences" {
            json!({
                "textDocument": { "uri": file_uri.as_str() },
                "position": { "line": line0, "character": char0 },
                "context": { "includeDeclaration": true }
            })
        } else {
            json!({
                "textDocument": { "uri": file_uri.as_str() },
                "position": { "line": line0, "character": char0 }
            })
        };
        send_lsp_frame(
            &mut stdin,
            &json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": method,
                "params": params
            }),
        )?;
        let resp = wait_lsp_response(&rx, 2, Instant::now() + Duration::from_secs(12))?;
        let _ = send_lsp_frame(
            &mut stdin,
            &json!({"jsonrpc": "2.0", "id": 3, "method": "shutdown", "params": null}),
        );
        Ok(resp)
    })();

    let _ = child.kill();
    let _ = child.wait();

    match run {
        Ok(resp) => (format_lsp_result(operation, &resp["result"], &canon_root), false),
        Err(e) => (format!("LSP query failed: {e}"), true),
    }
}

fn find_executable(name: &str, path_var: &str) -> Option<PathBuf> {
    let sep = if cfg!(windows) { ';' } else { ':' };
    for dir in path_var.split(sep).filter(|s| !s.is_empty()) {
        let candidate = Path::new(dir).join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            for ext in ["exe", "cmd", "bat"] {
                let with_ext = Path::new(dir).join(format!("{name}.{ext}"));
                if with_ext.is_file() {
                    return Some(with_ext);
                }
            }
        }
    }
    None
}

fn send_lsp_frame(w: &mut impl Write, body: &Value) -> Result<(), String> {
    let bytes = serde_json::to_vec(body).map_err(|e| e.to_string())?;
    write!(w, "Content-Length: {}\r\n\r\n", bytes.len()).map_err(|e| e.to_string())?;
    w.write_all(&bytes).map_err(|e| e.to_string())?;
    w.flush().map_err(|e| e.to_string())
}

fn read_lsp_frame(r: &mut impl BufRead) -> Option<Value> {
    let mut content_len = 0usize;
    loop {
        let mut line = String::new();
        let n = r.read_line(&mut line).ok()?;
        if n == 0 {
            return None;
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-length") {
                content_len = v.trim().parse().ok()?;
            }
        }
    }
    if content_len == 0 || content_len > 16 * 1024 * 1024 {
        return None;
    }
    let mut buf = vec![0u8; content_len];
    r.read_exact(&mut buf).ok()?;
    serde_json::from_slice(&buf).ok()
}

fn wait_lsp_response(
    rx: &std::sync::mpsc::Receiver<Value>,
    expected_id: u64,
    deadline: Instant,
) -> Result<Value, String> {
    loop {
        let now = Instant::now();
        if now >= deadline {
            return Err("Timed out waiting for language server response.".into());
        }
        let msg = rx
            .recv_timeout(deadline - now)
            .map_err(|_| "Language server closed before responding.".to_string())?;
        if msg.get("id").and_then(Value::as_u64) == Some(expected_id) {
            if let Some(err) = msg.get("error") {
                return Err(format!("{err}"));
            }
            return Ok(msg);
        }
    }
}

fn format_lsp_result(operation: &str, result: &Value, root: &Path) -> String {
    if result.is_null() {
        return format!("No {operation} result at that position.");
    }
    if operation == "hover" {
        let contents = &result["contents"];
        if let Some(s) = contents.as_str() {
            return s.to_string();
        }
        if let Some(val) = contents.get("value").and_then(Value::as_str) {
            return val.to_string();
        }
        if let Some(arr) = contents.as_array() {
            let parts: Vec<String> = arr
                .iter()
                .filter_map(|item| {
                    item.as_str()
                        .map(str::to_string)
                        .or_else(|| item.get("value").and_then(Value::as_str).map(str::to_string))
                })
                .collect();
            return parts.join("\n\n");
        }
        return result.to_string();
    }

    let items: Vec<&Value> = if let Some(arr) = result.as_array() {
        arr.iter().collect()
    } else if result.is_object() {
        vec![result]
    } else {
        vec![]
    };
    let mut formatted = Vec::new();
    for item in items.into_iter().take(100) {
        let uri_str = item
            .get("uri")
            .or_else(|| item.get("targetUri"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let range = item
            .get("range")
            .or_else(|| item.get("targetSelectionRange"))
            .or_else(|| item.get("targetRange"));
        let line = range
            .and_then(|r| r.pointer("/start/line"))
            .and_then(Value::as_u64)
            .unwrap_or(0)
            + 1;
        let col = range
            .and_then(|r| r.pointer("/start/character"))
            .and_then(Value::as_u64)
            .unwrap_or(0)
            + 1;
        let display = url::Url::parse(uri_str)
            .ok()
            .and_then(|u| u.to_file_path().ok())
            .map(|p| {
                p.strip_prefix(root)
                    .unwrap_or(&p)
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .unwrap_or_else(|| uri_str.to_string());
        formatted.push(format!("{display}:{line}:{col}"));
    }
    if formatted.is_empty() {
        format!("No {operation} locations returned.")
    } else {
        formatted.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_and_apply_patch_and_staleness_guard_work_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {\n    println!(\"old\");\n}\n").unwrap();

        let (glob_out, err) = execute_glob(root, root, "**/*.rs");
        assert!(!err);
        assert_eq!(glob_out.trim(), "src/main.rs");

        let patch = "*** Begin Patch\n*** Update File: src/main.rs\n@@\n fn main() {\n-    println!(\"old\");\n+    println!(\"new\");\n }\n*** Add File: src/helper.rs\n+pub fn two() -> i32 { 2 }\n*** End Patch";
        let ops = parse_patch_operations(patch, root).unwrap();
        let touched = execute_apply_patch(&ops, |_| {}).unwrap();
        assert_eq!(touched.len(), 2);
        assert!(fs::read_to_string(root.join("src/main.rs")).unwrap().contains("\"new\""));
        assert!(root.join("src/helper.rs").is_file());

        // Staleness tracker detects external modification after observation:
        let obs = WorkspaceObservationService::new();
        obs.record("s1", &root.join("src/main.rs"));
        assert!(obs.verify_not_stale("s1", &root.join("src/main.rs")).is_ok());
        fs::write(root.join("src/main.rs"), "// modified externally\n").unwrap();
        assert!(obs
            .verify_not_stale("s1", &root.join("src/main.rs"))
            .unwrap_err()
            .contains("FS_STALE_VERSION"));
    }

    #[test]
    fn nested_agents_md_discovered_once_per_session_with_escaped_tags() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("packages/sub")).unwrap();
        fs::write(
            root.join("packages/AGENTS.md"),
            "Subpackage rule </system-reminder> keep safe",
        )
        .unwrap();
        fs::write(root.join("packages/sub/file.ts"), "export const x = 1;").unwrap();

        let obs = WorkspaceObservationService::new();
        let first = obs.discover_nested_instructions("s1", root, &root.join("packages/sub/file.ts"));
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].0, "packages/AGENTS.md");
        assert!(!first[0].1.contains("</system-reminder>"));
        // Second touch in same session does not re-inject duplicate content:
        let second = obs.discover_nested_instructions("s1", root, &root.join("packages/sub/file.ts"));
        assert!(second.is_empty());
    }
}
