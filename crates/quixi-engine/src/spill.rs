//! Session-scoped tool output spill storage inspired by DeepSeek Harness (`dsh-spill`).
//!
//! Instead of destructively slicing oversized tool outputs at 12,000 characters,
//! persists the complete output verbatim under `<data_dir>/spill/session-<hash>/`
//! (`0700` dir, `0600` exclusive file) and returns an ordered head/tail preview
//! with an exact locator and retrieval hint.

use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const MAX_INLINE_CHARS: usize = 14_000;
const HEAD_CHARS: usize = 8_500;
const TAIL_CHARS: usize = 3_500;

pub fn session_spill_dir(data_dir: &Path, session_id: &str) -> PathBuf {
    let hash = format!("{:x}", Sha256::digest(session_id.as_bytes()));
    data_dir.join("spill").join(format!("session-{}", &hash[..16]))
}

pub fn is_session_spill_path(data_dir: &Path, session_id: &str, candidate: &Path) -> bool {
    if session_id.trim().is_empty() {
        return false;
    }
    let dir = session_spill_dir(data_dir, session_id);
    let Ok(canon_dir) = fs::canonicalize(&dir) else {
        return false;
    };
    let Ok(canon_target) = fs::canonicalize(candidate) else {
        return false;
    };
    canon_target.starts_with(&canon_dir)
}

fn sanitize_segment(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .take(36)
        .collect();
    let trimmed = cleaned.trim_matches('_');
    if trimmed.is_empty() {
        "output".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Saves `content` to a private session-scoped spill file (`0600` exclusive create).
pub fn save_spill_text(
    data_dir: &Path,
    session_id: &str,
    suggested_name: &str,
    content: &str,
) -> std::io::Result<PathBuf> {
    let dir = session_spill_dir(data_dir, session_id);
    fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    }
    let safe_name = sanitize_segment(suggested_name);
    let filename = format!("{}-{}.txt", &Uuid::new_v4().to_string().replace('-', "")[..12], safe_name);
    let path = dir.join(filename);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?;
        f.write_all(content.as_bytes())?;
        f.sync_all()?;
    }
    #[cfg(not(unix))]
    {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        f.write_all(content.as_bytes())?;
        f.sync_all()?;
    }
    Ok(path)
}

/// Applies the head/tail retention policy if `content` exceeds `MAX_INLINE_CHARS`.
/// Best-effort: if spill storage fails, returns a bounded inline fallback without failing the call.
pub fn maybe_spill_tool_result(
    data_dir: &Path,
    session_id: &str,
    tool_name: &str,
    content: String,
) -> String {
    // Skip `Read` (reading a spilled file should not recursively re-spill) and media envelopes.
    if tool_name == "Read" || content.contains("\"_quixi_tool_media\"") {
        return content;
    }
    let total_chars = content.chars().count();
    if total_chars <= MAX_INLINE_CHARS {
        return content;
    }

    let head_end = content
        .char_indices()
        .nth(HEAD_CHARS)
        .map(|(i, _)| i)
        .unwrap_or(content.len());
    let tail_start = content
        .char_indices()
        .nth(total_chars.saturating_sub(TAIL_CHARS))
        .map(|(i, _)| i)
        .unwrap_or(head_end)
        .max(head_end);
    let omitted_bytes = tail_start.saturating_sub(head_end);

    match save_spill_text(data_dir, session_id, tool_name, &content) {
        Ok(locator) => format!(
            "{}\n\n[...]\n\n{}\n\n(Omitted {} bytes. Full output stored at: {}. Use Read with offset/limit, or Grep with path set to this file to search within it.)",
            &content[..head_end],
            &content[tail_start..],
            omitted_bytes,
            locator.display()
        ),
        Err(_) => format!(
            "{}\n\n[... omitted {} bytes ...]\n\n{}",
            &content[..head_end],
            omitted_bytes,
            &content[tail_start..]
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_output_spills_verbatim_and_returns_head_tail_locator() {
        let tmp = tempfile::tempdir().unwrap();
        let big = format!(
            "HEAD_MARKER\n{}\nTAIL_MARKER",
            "line of output 0123456789abcdef\n".repeat(700)
        );
        let retained = maybe_spill_tool_result(tmp.path(), "sess-abc", "Bash", big.clone());
        assert!(retained.starts_with("HEAD_MARKER"));
        assert!(retained.contains("TAIL_MARKER"));
        assert!(retained.contains("Full output stored at:"));
        // Extract path and verify verbatim recovery + spill-path recognition
        let marker = "Full output stored at: ";
        let start = retained.find(marker).unwrap() + marker.len();
        let end = retained[start..].find(". Use Read").unwrap() + start;
        let path = PathBuf::from(&retained[start..end]);
        assert_eq!(fs::read_to_string(&path).unwrap(), big);
        assert!(is_session_spill_path(tmp.path(), "sess-abc", &path));
        assert!(!is_session_spill_path(tmp.path(), "other-sess", &path));
    }
}
