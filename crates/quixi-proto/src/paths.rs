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

static TEST_PROFILE: OnceLock<PathBuf> = OnceLock::new();

/// Opt-in account/data fixture root. Never changes the operating system's HOME.
pub fn configure_test_profile(root: &std::path::Path) -> Result<(), String> {
    if !root.is_absolute() { return Err("Test profile must be an absolute path".into()); }
    std::fs::create_dir_all(root).map_err(|e| format!("Cannot create test profile: {e}"))?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    // Windows canonical paths have an extended-length prefix; compare paths
    // in the same form so selecting the real account directory still fails.
    let home = home_dir().canonicalize().unwrap_or_else(|_| home_dir());
    let normal_data = home.join(".quixi");
    let normal_data = normal_data.canonicalize().unwrap_or(normal_data);
    if root == home || root.starts_with(normal_data) {
        return Err("Test profile must be separate from the normal account data".into());
    }
    if let Some(current) = TEST_PROFILE.get() {
        return if current == &root { Ok(()) } else { Err("Test profile already configured".into()) };
    }
    TEST_PROFILE.set(root.clone()).or_else(|_| if TEST_PROFILE.get() == Some(&root) { Ok(()) } else { Err(root) })
        .map_err(|_| "Test profile already configured".into())
}

pub fn test_profile() -> Option<&'static std::path::Path> {
    if TEST_PROFILE.get().is_none() {
        if let Some(root) = std::env::var_os("QUIXI_TEST_PROFILE") {
            configure_test_profile(std::path::Path::new(&root)).expect("Invalid QUIXI_TEST_PROFILE");
        }
    }
    TEST_PROFILE.get().map(PathBuf::as_path)
}

pub fn data_dir() -> PathBuf {
    test_profile().map(|p| p.join("data")).unwrap_or_else(|| home_dir().join(".quixi"))
}

/// Application-owned home fixtures, including CLI transcripts. OS HOME is untouched.
pub fn account_home() -> PathBuf {
    test_profile().map(|p| p.join("home")).unwrap_or_else(home_dir)
}

pub fn automation_port() -> u16 {
    std::env::var("QUIXI_AUTOMATION_PORT").ok().and_then(|p| p.parse().ok())
        .unwrap_or(if test_profile().is_some() { 19456 } else { 19455 })
}

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
    if let Some(profile) = test_profile() { return profile.join("workspace").to_string_lossy().into_owned(); }
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_profile_routes_app_data_without_changing_os_home() {
        assert!(configure_test_profile(std::path::Path::new("relative-profile")).is_err());
        let os_home = home_dir();
        assert!(configure_test_profile(&os_home).is_err());
        let root = std::env::temp_dir().join(format!("quixi-path-test-{}",std::process::id()));
        configure_test_profile(&root).unwrap();
        let root=root.canonicalize().unwrap();
        assert_eq!(data_dir(),root.join("data"));
        assert_eq!(account_home(),root.join("home"));
        assert_eq!(home_dir(),os_home);
        assert_eq!(test_profile(),Some(root.as_path()));
        configure_test_profile(&root).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
