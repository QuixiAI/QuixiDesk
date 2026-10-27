//! Host path resolution.
//!
//! Nothing in QuixiDesk may hard-code a developer machine path. Every default
//! (workspace folder, home directory, per-user data folders) is resolved here so
//! it works on any Mac and inside a shipped `.app` bundle.
//!
//! Resolution order for the default workspace:
//! 1. `QUIXI_WORKSPACE` environment variable
//! 2. The source checkout this build came from (dev builds only, if it still exists)
//! 3. `<home>/QuixiDesk`

use std::path::PathBuf;
use std::sync::OnceLock;

/// User home directory, honouring `HOME` and falling back to the OS lookup.
pub fn home_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME").filter(|h| !h.is_empty()) {
        return PathBuf::from(home);
    }
    #[allow(deprecated)]
    std::env::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

/// Same as [`home_dir`], borrowed for `&'static str` call sites.
pub fn home_dir_str() -> &'static str {
    static CACHE: OnceLock<String> = OnceLock::new();
    CACHE
        .get_or_init(|| home_dir().to_string_lossy().into_owned())
        .as_str()
}

/// The source checkout recorded at compile time, if it still exists on disk.
pub fn build_root() -> Option<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())?;
    root.exists().then_some(root)
}

/// Default folder the workspace engine opens when the user has not picked one.
pub fn default_workspace() -> String {
    if let Ok(explicit) = std::env::var("QUIXI_WORKSPACE") {
        if !explicit.trim().is_empty() {
            return explicit;
        }
    }
    if let Some(root) = build_root() {
        return root.to_string_lossy().into_owned();
    }
    home_dir().join("QuixiDesk").to_string_lossy().into_owned()
}

/// Same as [`default_workspace`], borrowed for `&'static str` call sites.
pub fn default_workspace_str() -> &'static str {
    static CACHE: OnceLock<String> = OnceLock::new();
    CACHE.get_or_init(default_workspace).as_str()
}

/// Joins a relative path onto [`default_workspace`].
pub fn workspace_join(relative: &str) -> String {
    PathBuf::from(default_workspace())
        .join(relative)
        .to_string_lossy()
        .into_owned()
}
