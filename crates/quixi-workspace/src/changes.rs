//! Turn-level workspace change recorder and Git worktree manager.
//!
//! Inspired by DeepSeek Harness (`workspace-changes` and `tool-worktree`):
//! - Uses a shadow Git index and temporary object store (`GIT_INDEX_FILE`,
//!   `GIT_OBJECT_DIRECTORY`, `GIT_ALTERNATE_OBJECT_DIRECTORIES`) so turn-start
//!   and turn-end working-tree snapshots never touch `.git/index` or `.git/objects`,
//!   and pre-existing dirty edits made before the turn are excluded.
//! - Supplements Git snapshots with pre-mutation file captures so untracked/ignored
//!   files and non-Git workspaces still produce exact line counts and unified diffs.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const MAX_CAPTURE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_SUMMARY_FILES: usize = 250;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffHunk {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFileEntry {
    pub path: String,
    pub display: String,
    pub status: String, // "added" | "modified" | "deleted"
    pub added: usize,
    pub deleted: usize,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub binary: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub oversized: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hunks: Vec<DiffHunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnChangesSummary {
    pub turn_id: String,
    pub cwd: String,
    pub files: Vec<ChangedFileEntry>,
    pub total_files: usize,
    pub added: usize,
    pub deleted: usize,
}

#[derive(Debug, Clone)]
enum CapturedBaseline {
    Missing,
    Oversized,
    Binary,
    Text(String),
}

pub struct TurnChangeTracker {
    turn_id: String,
    cwd: PathBuf,
    git_root: Option<PathBuf>,
    git_objects_dir: Option<PathBuf>,
    scratch_dir: Option<tempfile::TempDir>,
    before_tree: Option<String>,
    file_baselines: HashMap<PathBuf, CapturedBaseline>,
}

impl TurnChangeTracker {
    pub fn start(cwd: &Path) -> Self {
        let turn_id = uuid::Uuid::new_v4().to_string();
        if cwd.as_os_str().is_empty() || !cwd.is_dir() {
            return Self {
                turn_id,
                cwd: cwd.to_path_buf(),
                git_root: None,
                git_objects_dir: None,
                scratch_dir: None,
                before_tree: None,
                file_baselines: HashMap::new(),
            };
        }
        let canon_cwd = fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
        let (git_root, git_dir) = locate_git_dirs(&canon_cwd);
        let scratch_dir = tempfile::Builder::new()
            .prefix("quixi-turn-git-")
            .tempdir()
            .ok();
        let git_objects_dir = git_dir.as_ref().map(|gd| gd.join("objects"));
        let mut tracker = Self {
            turn_id,
            cwd: canon_cwd,
            git_root,
            git_objects_dir,
            scratch_dir,
            before_tree: None,
            file_baselines: HashMap::new(),
        };
        tracker.before_tree = tracker.snapshot_git_tree(git_dir.as_deref());
        tracker
    }

    /// Captures a file's baseline before its first file-tool mutation in this turn.
    pub fn capture_before_edit(&mut self, target: &Path) {
        let key = normalize_key_path(&self.cwd, target);
        if self.file_baselines.contains_key(&key) {
            return;
        }
        let baseline = read_file_state(&key);
        self.file_baselines.insert(key, baseline);
    }

    fn snapshot_git_tree(&self, git_dir: Option<&Path>) -> Option<String> {
        let root = self.git_root.as_ref()?;
        let objects = self.git_objects_dir.as_ref()?;
        let scratch = self.scratch_dir.as_ref()?;
        let tmp_objects = scratch.path().join("objects");
        let _ = fs::create_dir_all(&tmp_objects);
        let tmp_index = scratch
            .path()
            .join(format!("index-{}", uuid::Uuid::new_v4()));
        if let Some(gd) = git_dir {
            let real_index = gd.join("index");
            if real_index.is_file() {
                let _ = fs::copy(&real_index, &tmp_index);
            }
        }
        let add = shadow_git_cmd(root, &tmp_index, &tmp_objects, objects)
            .args(["add", "--all", "--ignore-errors"])
            .output()
            .ok()?;
        if !add.status.success() && add.status.code() != Some(1) {
            let _ = fs::remove_file(&tmp_index);
            return None;
        }
        let tree_out = shadow_git_cmd(root, &tmp_index, &tmp_objects, objects)
            .arg("write-tree")
            .output()
            .ok()?;
        let _ = fs::remove_file(&tmp_index);
        if !tree_out.status.success() {
            return None;
        }
        let sha = String::from_utf8_lossy(&tree_out.stdout).trim().to_string();
        (!sha.is_empty()).then_some(sha)
    }

    fn read_blob(&self, tree: &str, repo_rel: &str) -> CapturedBaseline {
        let (Some(root), Some(objects), Some(scratch)) = (
            self.git_root.as_ref(),
            self.git_objects_dir.as_ref(),
            self.scratch_dir.as_ref(),
        ) else {
            return CapturedBaseline::Missing;
        };
        let tmp_objects = scratch.path().join("objects");
        let tmp_index = scratch.path().join("readonly-index");
        let spec = format!("{tree}:{repo_rel}");
        let size_out = shadow_git_cmd(root, &tmp_index, &tmp_objects, objects)
            .args(["cat-file", "-s", &spec])
            .output();
        let Ok(size_res) = size_out else {
            return CapturedBaseline::Missing;
        };
        if !size_res.status.success() {
            return CapturedBaseline::Missing;
        }
        let size: u64 = String::from_utf8_lossy(&size_res.stdout)
            .trim()
            .parse()
            .unwrap_or(0);
        if size > MAX_CAPTURE_BYTES {
            return CapturedBaseline::Oversized;
        }
        let Ok(blob) = shadow_git_cmd(root, &tmp_index, &tmp_objects, objects)
            .args(["cat-file", "blob", &spec])
            .output()
        else {
            return CapturedBaseline::Missing;
        };
        if !blob.status.success() {
            return CapturedBaseline::Missing;
        }
        classify_bytes(&blob.stdout)
    }

    pub fn finish(&mut self) -> Option<TurnChangesSummary> {
        let mut by_display: BTreeMap<String, ChangedFileEntry> = BTreeMap::new();
        let mut covered_abs: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();

        if let (Some(root), Some(objects), Some(scratch), Some(before)) = (
            self.git_root.clone(),
            self.git_objects_dir.clone(),
            self.scratch_dir.as_ref(),
            self.before_tree.clone(),
        ) {
            let (_, git_dir) = locate_git_dirs(&root);
            if let Some(after) = self.snapshot_git_tree(git_dir.as_deref()) {
                if before != after {
                    let tmp_objects = scratch.path().join("objects");
                    let tmp_index = scratch.path().join("diff-index");
                    if let Ok(out) = shadow_git_cmd(&root, &tmp_index, &tmp_objects, &objects)
                        .args(["diff-tree", "-r", "--no-renames", "--numstat", &before, &after])
                        .output()
                    {
                        if out.status.success() {
                            for line in String::from_utf8_lossy(&out.stdout).lines() {
                                let parts: Vec<&str> = line.splitn(3, '\t').collect();
                                if parts.len() != 3 {
                                    continue;
                                }
                                let repo_rel = parts[2].trim();
                                if repo_rel.is_empty() {
                                    continue;
                                }
                                let abs = root.join(repo_rel);
                                covered_abs.insert(normalize_key_path(&self.cwd, &abs));
                                let binary = parts[0] == "-" || parts[1] == "-";
                                let added_stat = parts[0].parse::<usize>().unwrap_or(0);
                                let deleted_stat = parts[1].parse::<usize>().unwrap_or(0);
                                let old_state = self.read_blob(&before, repo_rel);
                                let new_state = self.read_blob(&after, repo_rel);
                                if let Some(mut entry) =
                                    build_changed_entry(&self.cwd, &abs, &old_state, &new_state)
                                {
                                    if !binary && !entry.oversized {
                                        entry.added = added_stat;
                                        entry.deleted = deleted_stat;
                                    }
                                    by_display.insert(entry.display.clone(), entry);
                                }
                            }
                        }
                    }
                }
            }
        }

        for (abs_path, old_state) in &self.file_baselines {
            if covered_abs.contains(abs_path) {
                continue;
            }
            let new_state = read_file_state(abs_path);
            if let Some(entry) = build_changed_entry(&self.cwd, abs_path, old_state, &new_state) {
                by_display.insert(entry.display.clone(), entry);
            }
        }

        if by_display.is_empty() {
            return None;
        }

        let total_files = by_display.len();
        let added: usize = by_display.values().map(|f| f.added).sum();
        let deleted: usize = by_display.values().map(|f| f.deleted).sum();
        let files: Vec<ChangedFileEntry> =
            by_display.into_values().take(MAX_SUMMARY_FILES).collect();

        Some(TurnChangesSummary {
            turn_id: self.turn_id.clone(),
            cwd: self.cwd.to_string_lossy().into_owned(),
            files,
            total_files,
            added,
            deleted,
        })
    }
}

fn locate_git_dirs(cwd: &Path) -> (Option<PathBuf>, Option<PathBuf>) {
    let top = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(cwd)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()));
    let common = Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .current_dir(cwd)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            let p = PathBuf::from(String::from_utf8_lossy(&o.stdout).trim());
            if p.is_absolute() {
                p
            } else {
                cwd.join(p)
            }
        });
    (top, common)
}

fn shadow_git_cmd(
    root: &Path,
    tmp_index: &Path,
    tmp_objects: &Path,
    real_objects: &Path,
) -> Command {
    let mut cmd = Command::new("git");
    cmd.current_dir(root)
        .env("GIT_INDEX_FILE", tmp_index)
        .env("GIT_OBJECT_DIRECTORY", tmp_objects)
        .env("GIT_ALTERNATE_OBJECT_DIRECTORIES", real_objects)
        .env("GIT_CONFIG_COUNT", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    cmd
}

fn normalize_key_path(cwd: &Path, target: &Path) -> PathBuf {
    let joined = if target.is_absolute() {
        target.to_path_buf()
    } else {
        cwd.join(target)
    };
    if let Ok(canon) = fs::canonicalize(&joined) {
        return canon;
    }
    if let (Some(parent), Some(name)) = (joined.parent(), joined.file_name()) {
        if let Ok(canon_parent) = fs::canonicalize(parent) {
            return canon_parent.join(name);
        }
    }
    joined
}

fn read_file_state(path: &Path) -> CapturedBaseline {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return CapturedBaseline::Missing;
    };
    if !meta.is_file() {
        return CapturedBaseline::Missing;
    }
    if meta.len() > MAX_CAPTURE_BYTES {
        return CapturedBaseline::Oversized;
    }
    match fs::read(path) {
        Ok(bytes) => classify_bytes(&bytes),
        Err(_) => CapturedBaseline::Missing,
    }
}

fn classify_bytes(bytes: &[u8]) -> CapturedBaseline {
    if bytes.contains(&0) {
        return CapturedBaseline::Binary;
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => CapturedBaseline::Text(s.replace("\r\n", "\n")),
        Err(_) => CapturedBaseline::Binary,
    }
}

fn display_path(cwd: &Path, abs: &Path) -> (String, String) {
    if let Ok(rel) = abs.strip_prefix(cwd) {
        let s = rel.to_string_lossy().replace('\\', "/");
        return (s.clone(), s);
    }
    let full = abs.to_string_lossy().into_owned();
    let home = quixi_proto::paths::home_dir();
    if let Ok(rel) = abs.strip_prefix(&home) {
        return (full, format!("~/{}", rel.to_string_lossy().replace('\\', "/")));
    }
    (full.clone(), full.replace('\\', "/"))
}

fn build_changed_entry(
    cwd: &Path,
    abs: &Path,
    old: &CapturedBaseline,
    new: &CapturedBaseline,
) -> Option<ChangedFileEntry> {
    let (path, display) = display_path(cwd, abs);
    let status = match (old, new) {
        (CapturedBaseline::Missing, CapturedBaseline::Missing) => return None,
        (CapturedBaseline::Missing, _) => "added",
        (_, CapturedBaseline::Missing) => "deleted",
        _ => "modified",
    }
    .to_string();

    match (old, new) {
        (CapturedBaseline::Oversized, _) | (_, CapturedBaseline::Oversized) => {
            Some(ChangedFileEntry {
                path,
                display,
                status,
                added: 0,
                deleted: 0,
                binary: false,
                oversized: true,
                hunks: vec![],
            })
        }
        (CapturedBaseline::Binary, _) | (_, CapturedBaseline::Binary) => Some(ChangedFileEntry {
            path,
            display,
            status,
            added: 0,
            deleted: 0,
            binary: true,
            oversized: false,
            hunks: vec![],
        }),
        _ => {
            let old_text = match old {
                CapturedBaseline::Text(t) => t.as_str(),
                _ => "",
            };
            let new_text = match new {
                CapturedBaseline::Text(t) => t.as_str(),
                _ => "",
            };
            if old_text == new_text {
                return None;
            }
            let (added, deleted, hunks) = compute_unified_hunks(old_text, new_text, 3);
            Some(ChangedFileEntry {
                path,
                display,
                status,
                added,
                deleted,
                binary: false,
                oversized: false,
                hunks,
            })
        }
    }
}

/// Computes unified diff hunks with `context` lines and bounded runtime.
pub fn compute_unified_hunks(old_text: &str, new_text: &str, context: usize) -> (usize, usize, Vec<DiffHunk>) {
    let old_lines: Vec<&str> = if old_text.is_empty() {
        vec![]
    } else {
        old_text.lines().collect()
    };
    let new_lines: Vec<&str> = if new_text.is_empty() {
        vec![]
    } else {
        new_text.lines().collect()
    };

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Op {
        Equal(usize, usize),
        Delete(usize),
        Insert(usize),
    }

    // Trim shared prefix & suffix first so typical code edits are tiny in DP matrix.
    let mut prefix = 0usize;
    while prefix < old_lines.len()
        && prefix < new_lines.len()
        && old_lines[prefix] == new_lines[prefix]
    {
        prefix += 1;
    }
    let mut suffix = 0usize;
    while suffix + prefix < old_lines.len()
        && suffix + prefix < new_lines.len()
        && old_lines[old_lines.len() - 1 - suffix] == new_lines[new_lines.len() - 1 - suffix]
    {
        suffix += 1;
    }

    let mid_old = &old_lines[prefix..old_lines.len() - suffix];
    let mid_new = &new_lines[prefix..new_lines.len() - suffix];

    let mut ops = Vec::with_capacity(old_lines.len() + new_lines.len());
    for i in 0..prefix {
        ops.push(Op::Equal(i, i));
    }

    // Bounded LCS on the middle window; falls back to coarse replacement for huge middle spans.
    if mid_old.len().saturating_mul(mid_new.len()) <= 400_000 {
        let n = mid_old.len();
        let m = mid_new.len();
        let stride = m + 1;
        let mut dp = vec![0u32; (n + 1) * stride];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                dp[i * stride + j] = if mid_old[i] == mid_new[j] {
                    dp[(i + 1) * stride + j + 1] + 1
                } else {
                    dp[(i + 1) * stride + j].max(dp[i * stride + j + 1])
                };
            }
        }
        let (mut i, mut j) = (0usize, 0usize);
        while i < n && j < m {
            if mid_old[i] == mid_new[j] {
                ops.push(Op::Equal(prefix + i, prefix + j));
                i += 1;
                j += 1;
            } else if dp[(i + 1) * stride + j] >= dp[i * stride + j + 1] {
                ops.push(Op::Delete(prefix + i));
                i += 1;
            } else {
                ops.push(Op::Insert(prefix + j));
                j += 1;
            }
        }
        while i < n {
            ops.push(Op::Delete(prefix + i));
            i += 1;
        }
        while j < m {
            ops.push(Op::Insert(prefix + j));
            j += 1;
        }
    } else {
        for i in 0..mid_old.len() {
            ops.push(Op::Delete(prefix + i));
        }
        for j in 0..mid_new.len() {
            ops.push(Op::Insert(prefix + j));
        }
    }

    for k in 0..suffix {
        ops.push(Op::Equal(
            old_lines.len() - suffix + k,
            new_lines.len() - suffix + k,
        ));
    }

    let added = ops.iter().filter(|o| matches!(o, Op::Insert(_))).count();
    let deleted = ops.iter().filter(|o| matches!(o, Op::Delete(_))).count();

    let change_indices: Vec<usize> = ops
        .iter()
        .enumerate()
        .filter_map(|(idx, op)| (!matches!(op, Op::Equal(_, _))).then_some(idx))
        .collect();
    if change_indices.is_empty() {
        return (0, 0, vec![]);
    }

    let mut groups: Vec<(usize, usize)> = Vec::new();
    for idx in change_indices {
        let start = idx.saturating_sub(context);
        let end = (idx + context + 1).min(ops.len());
        if let Some(last) = groups.last_mut() {
            if start <= last.1 {
                last.1 = end;
                continue;
            }
        }
        groups.push((start, end));
    }

    let mut hunks = Vec::new();
    for (start, end) in groups.into_iter().take(40) {
        let mut old_start = None;
        let mut new_start = None;
        let mut old_count = 0usize;
        let mut new_count = 0usize;
        let mut hunk_lines = Vec::new();
        for op in &ops[start..end] {
            match *op {
                Op::Equal(oi, ni) => {
                    old_start.get_or_insert(oi + 1);
                    new_start.get_or_insert(ni + 1);
                    old_count += 1;
                    new_count += 1;
                    hunk_lines.push(format!(" {}", old_lines[oi]));
                }
                Op::Delete(oi) => {
                    old_start.get_or_insert(oi + 1);
                    old_count += 1;
                    hunk_lines.push(format!("-{}", old_lines[oi]));
                }
                Op::Insert(ni) => {
                    new_start.get_or_insert(ni + 1);
                    new_count += 1;
                    hunk_lines.push(format!("+{}", new_lines[ni]));
                }
            }
        }
        hunks.push(DiffHunk {
            old_start: old_start.unwrap_or(1),
            old_lines: old_count,
            new_start: new_start.unwrap_or(1),
            new_lines: new_count,
            lines: hunk_lines,
        });
    }

    (added, deleted, hunks)
}

pub struct WorktreeManager;

impl WorktreeManager {
    /// Creates a clean Git worktree under `<repo_root>/.agents/worktrees/<slug>` pinned to `HEAD`.
    pub fn create_worktree(cwd: &Path, raw_name: &str) -> Result<serde_json::Value> {
        let (Some(root), _) = locate_git_dirs(cwd) else {
            bail!("Workspace is not inside a Git repository.");
        };
        let slug: String = raw_name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .trim_matches('-')
            .chars()
            .take(48)
            .collect();
        let slug = if slug.is_empty() {
            format!("wt-{}", &uuid::Uuid::new_v4().to_string()[..8])
        } else {
            slug
        };
        let wt_base = root.join(".agents").join("worktrees");
        fs::create_dir_all(&wt_base)?;
        let gitignore = wt_base.join(".gitignore");
        if !gitignore.exists() {
            let _ = fs::write(&gitignore, "*\n");
        }
        let target = wt_base.join(&slug);
        if target.exists() {
            bail!("Worktree path already exists: {}", target.display());
        }
        let branch = format!("worktree/{slug}");
        let head = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&root)
            .output()
            .context("Failed to resolve HEAD")?;
        if !head.status.success() {
            bail!("Repository has no committed HEAD to branch a worktree from.");
        }
        let base_commit = String::from_utf8_lossy(&head.stdout).trim().to_string();
        let out = Command::new("git")
            .args([
                "-c",
                "filter.lfs.smudge=",
                "-c",
                "filter.lfs.process=",
                "-c",
                "filter.lfs.required=false",
                "worktree",
                "add",
                "-b",
                &branch,
                &target.to_string_lossy(),
                &base_commit,
            ])
            .current_dir(&root)
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .context("Failed to execute git worktree add")?;
        if !out.status.success() {
            bail!(
                "git worktree add failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(serde_json::json!({
            "path": target.to_string_lossy(),
            "branch": branch,
            "baseCommit": base_commit,
            "repoRoot": root.to_string_lossy(),
        }))
    }

    pub fn list_worktrees(cwd: &Path) -> Vec<serde_json::Value> {
        let Ok(out) = Command::new("git")
            .args(["worktree", "list", "--porcelain"])
            .current_dir(cwd)
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
        else {
            return vec![];
        };
        if !out.status.success() {
            return vec![];
        }
        let mut items = Vec::new();
        let mut cur_path = String::new();
        let mut cur_head = String::new();
        let mut cur_branch = String::new();
        for line in String::from_utf8_lossy(&out.stdout).lines().chain(std::iter::once("")) {
            if line.is_empty() {
                if !cur_path.is_empty() {
                    items.push(serde_json::json!({
                        "path": cur_path,
                        "head": cur_head,
                        "branch": cur_branch,
                    }));
                    cur_path.clear();
                    cur_head.clear();
                    cur_branch.clear();
                }
            } else if let Some(rest) = line.strip_prefix("worktree ") {
                cur_path = rest.trim().to_string();
            } else if let Some(rest) = line.strip_prefix("HEAD ") {
                cur_head = rest.trim().to_string();
            } else if let Some(rest) = line.strip_prefix("branch ") {
                cur_branch = rest.trim().trim_start_matches("refs/heads/").to_string();
            }
        }
        items
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shadow_git_snapshot_captures_turn_changes_without_dirtying_repo_index() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        assert!(Command::new("git").arg("init").current_dir(repo).status().unwrap().success());
        assert!(Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo)
            .status()
            .unwrap()
            .success());
        assert!(Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(repo)
            .status()
            .unwrap()
            .success());
        fs::write(repo.join("tracked.txt"), "line1\nline2\n").unwrap();
        fs::write(repo.join("pre_dirty.txt"), "before turn\n").unwrap();
        assert!(Command::new("git")
            .args(["add", "tracked.txt", "pre_dirty.txt"])
            .current_dir(repo)
            .status()
            .unwrap()
            .success());
        assert!(Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(repo)
            .status()
            .unwrap()
            .success());

        // User already modified `pre_dirty.txt` before the turn started:
        fs::write(repo.join("pre_dirty.txt"), "dirty before turn\n").unwrap();

        let mut tracker = TurnChangeTracker::start(repo);
        // Turn modifies `tracked.txt` and creates `new_file.rs`:
        fs::write(repo.join("tracked.txt"), "line1\nline2_changed\nline3\n").unwrap();
        fs::write(repo.join("new_file.rs"), "fn main() {}\n").unwrap();

        let summary = tracker.finish().expect("expected turn changes");
        // `pre_dirty.txt` must NOT appear in the turn's changes!
        assert_eq!(summary.files.len(), 2);
        assert!(summary.files.iter().all(|f| f.display != "pre_dirty.txt"));
        let tracked = summary.files.iter().find(|f| f.display == "tracked.txt").unwrap();
        assert_eq!(tracked.status, "modified");
        assert_eq!(tracked.added, 2);
        assert_eq!(tracked.deleted, 1);
        assert!(!tracked.hunks.is_empty());

        // Verify `.git` staging index was never mutated (`new_file.rs` is still untracked `??`):
        let status = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(repo)
            .output()
            .unwrap();
        let status_str = String::from_utf8_lossy(&status.stdout);
        assert!(status_str.contains("?? new_file.rs"), "got: {status_str}");
    }
}
