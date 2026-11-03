//! OS-level process confinement seam inspired by DeepSeek Harness (`dsh-sandbox`).
//!
//! Separates per-call `SandboxPolicy` (`ReadOnly`, `WorkspaceWrite`, `FullAccess`),
//! functional runner probing (so nested-sandbox environments never silently misattribute
//! kernel `sandbox_apply` refusal as a command bug), and orthogonal stderr classification
//! (`RunnerFailed` vs `PolicyDenied` vs `NormalFailure`).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SandboxMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

impl SandboxMode {
    pub fn from_permission_mode(mode: &str) -> Self {
        match mode {
            "auto" | "bypassPermissions" => Self::DangerFullAccess,
            "acceptEdits" | "smartAuto" | "smartAutoApproved" => Self::WorkspaceWrite,
            _ => Self::ReadOnly,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SandboxEnforcement {
    Full,
    Partial,
    Unconfined,
}

#[derive(Debug, Clone)]
pub struct SandboxPolicy {
    pub mode: SandboxMode,
    pub workspace_root: PathBuf,
    pub additional_writable_roots: Vec<PathBuf>,
    pub session_temp_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "kebab-case")]
pub enum SandboxOutcome {
    Allowed,
    RunnerFailed(String),
    PolicyDenied(String),
}

#[derive(Debug, Clone)]
pub struct ConfinedCommand {
    pub program: String,
    pub args: Vec<String>,
    pub enforcement: SandboxEnforcement,
    pub mode: SandboxMode,
    pub temp_dir_override: Option<PathBuf>,
    denial_signatures: &'static [&'static str],
    runner_failure_signatures: &'static [&'static str],
}

impl ConfinedCommand {
    pub fn classify_stderr(&self, exit_code: i32, stderr: &str) -> SandboxOutcome {
        if exit_code == 0 || self.enforcement == SandboxEnforcement::Unconfined {
            return SandboxOutcome::Allowed;
        }
        for line in stderr.lines() {
            let lower = line.trim().to_ascii_lowercase();
            if lower.is_empty() {
                continue;
            }
            if self
                .runner_failure_signatures
                .iter()
                .any(|sig| lower.contains(sig))
            {
                return SandboxOutcome::RunnerFailed(line.trim().to_string());
            }
        }
        for line in stderr.lines() {
            let lower = line.trim().to_ascii_lowercase();
            if self
                .denial_signatures
                .iter()
                .any(|sig| lower.contains(sig))
            {
                return SandboxOutcome::PolicyDenied(line.trim().to_string());
            }
        }
        SandboxOutcome::Allowed
    }
}

pub struct ProcessSandbox;

impl ProcessSandbox {
    #[cfg(target_os = "macos")]
    fn seatbelt_usable() -> bool {
        static PROBE: OnceLock<bool> = OnceLock::new();
        *PROBE.get_or_init(|| {
            if !Path::new("/usr/bin/sandbox-exec").is_file() {
                return false;
            }
            Command::new("/usr/bin/sandbox-exec")
                .args(["-p", "(version 1)(allow default)", "/usr/bin/true"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|s| s.success())
        })
    }

    #[cfg(target_os = "linux")]
    fn bwrap_usable() -> bool {
        static PROBE: OnceLock<bool> = OnceLock::new();
        *PROBE.get_or_init(|| {
            if !Path::new("/usr/bin/bwrap").is_file() {
                return false;
            }
            Command::new("/usr/bin/bwrap")
                .args([
                    "--ro-bind",
                    "/",
                    "/",
                    "--dev",
                    "/dev",
                    "--proc",
                    "/proc",
                    "--",
                    "/bin/true",
                ])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|s| s.success())
        })
    }

    #[cfg(target_os = "macos")]
    fn escape_sbpl(path: &Path) -> String {
        path.to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
    }

    #[cfg(target_os = "macos")]
    pub fn build_seatbelt_profile(policy: &SandboxPolicy) -> String {
        let mut profile = String::from(
            "(version 1)\n(allow default)\n(deny file-write*)\n(allow file-write* (literal \"/dev/null\") (literal \"/dev/zero\") (literal \"/dev/tty\") (literal \"/dev/dtracehelper\"))\n",
        );
        if policy.mode == SandboxMode::WorkspaceWrite {
            if let Ok(canon) = fs::canonicalize(&policy.workspace_root) {
                profile.push_str(&format!(
                    "(allow file-write* (subpath \"{}\"))\n",
                    Self::escape_sbpl(&canon)
                ));
            }
            for extra in &policy.additional_writable_roots {
                if let Ok(canon) = fs::canonicalize(extra) {
                    if canon.is_dir() {
                        profile.push_str(&format!(
                            "(allow file-write* (subpath \"{}\"))\n",
                            Self::escape_sbpl(&canon)
                        ));
                    }
                }
            }
            if let Some(ref tmp) = policy.session_temp_dir {
                let _ = fs::create_dir_all(tmp);
                if let Ok(canon) = fs::canonicalize(tmp) {
                    profile.push_str(&format!(
                        "(allow file-write* (subpath \"{}\"))\n",
                        Self::escape_sbpl(&canon)
                    ));
                }
            }
        }
        profile
    }

    /// Wraps `program` + `args` in an OS confinement runner when available and requested.
    pub fn confine(program: &str, args: &[String], policy: &SandboxPolicy) -> ConfinedCommand {
        if policy.mode == SandboxMode::DangerFullAccess {
            return ConfinedCommand {
                program: program.to_string(),
                args: args.to_vec(),
                enforcement: SandboxEnforcement::Unconfined,
                mode: policy.mode,
                temp_dir_override: None,
                denial_signatures: &[],
                runner_failure_signatures: &[],
            };
        }

        #[cfg(target_os = "macos")]
        if Self::seatbelt_usable() {
            let profile = Self::build_seatbelt_profile(policy);
            let mut wrapped = vec![
                "-p".to_string(),
                profile,
                "--".to_string(),
                program.to_string(),
            ];
            wrapped.extend(args.iter().cloned());
            return ConfinedCommand {
                program: "/usr/bin/sandbox-exec".to_string(),
                args: wrapped,
                enforcement: SandboxEnforcement::Full,
                mode: policy.mode,
                temp_dir_override: policy.session_temp_dir.clone(),
                denial_signatures: &["operation not permitted", "permission denied"],
                runner_failure_signatures: &["sandbox-exec:", "sandbox_apply:"],
            };
        }

        #[cfg(target_os = "linux")]
        if Self::bwrap_usable() {
            let mut wrapped = vec![
                "--ro-bind".to_string(),
                "/".to_string(),
                "/".to_string(),
                "--dev".to_string(),
                "/dev".to_string(),
                "--proc".to_string(),
                "/proc".to_string(),
            ];
            if policy.mode == SandboxMode::WorkspaceWrite {
                if let Ok(canon) = fs::canonicalize(&policy.workspace_root) {
                    let s = canon.to_string_lossy().into_owned();
                    wrapped.extend(["--bind".to_string(), s.clone(), s]);
                }
                for extra in &policy.additional_writable_roots {
                    if let Ok(canon) = fs::canonicalize(extra) {
                        if canon.is_dir() {
                            let s = canon.to_string_lossy().into_owned();
                            wrapped.extend(["--bind".to_string(), s.clone(), s]);
                        }
                    }
                }
                if let Some(ref tmp) = policy.session_temp_dir {
                    let _ = fs::create_dir_all(tmp);
                    if let Ok(canon) = fs::canonicalize(tmp) {
                        let s = canon.to_string_lossy().into_owned();
                        wrapped.extend(["--bind".to_string(), s.clone(), s]);
                    }
                } else {
                    wrapped.extend(["--tmpfs".to_string(), "/tmp".to_string()]);
                }
            } else {
                wrapped.extend(["--tmpfs".to_string(), "/tmp".to_string()]);
            }
            wrapped.push("--".to_string());
            wrapped.push(program.to_string());
            wrapped.extend(args.iter().cloned());
            return ConfinedCommand {
                program: "/usr/bin/bwrap".to_string(),
                args: wrapped,
                enforcement: SandboxEnforcement::Full,
                mode: policy.mode,
                temp_dir_override: policy.session_temp_dir.clone(),
                denial_signatures: &["read-only file system", "permission denied"],
                runner_failure_signatures: &["bwrap:"],
            };
        }

        ConfinedCommand {
            program: program.to_string(),
            args: args.to_vec(),
            enforcement: SandboxEnforcement::Unconfined,
            mode: policy.mode,
            temp_dir_override: None,
            denial_signatures: &[],
            runner_failure_signatures: &[],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialect_classifier_distinguishes_runner_failure_from_policy_denial() {
        let cmd = ConfinedCommand {
            program: "/usr/bin/sandbox-exec".into(),
            args: vec![],
            enforcement: SandboxEnforcement::Full,
            mode: SandboxMode::ReadOnly,
            temp_dir_override: None,
            denial_signatures: &["operation not permitted"],
            runner_failure_signatures: &["sandbox-exec:", "sandbox_apply:"],
        };
        assert_eq!(cmd.classify_stderr(0, "Operation not permitted"), SandboxOutcome::Allowed);
        assert!(matches!(
            cmd.classify_stderr(71, "sandbox-exec: sandbox_apply: Operation not permitted"),
            SandboxOutcome::RunnerFailed(_)
        ));
        assert!(matches!(
            cmd.classify_stderr(1, "touch: /etc/test: Operation not permitted"),
            SandboxOutcome::PolicyDenied(_)
        ));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn seatbelt_profile_escapes_and_scopes_workspace_writes() {
        let root = std::env::temp_dir().join(format!("quixi-sbpl-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let ro = ProcessSandbox::build_seatbelt_profile(&SandboxPolicy {
            mode: SandboxMode::ReadOnly,
            workspace_root: root.clone(),
            additional_writable_roots: vec![],
            session_temp_dir: None,
        });
        assert!(ro.contains("(deny file-write*)"));
        assert!(!ro.contains("subpath"));
        let rw = ProcessSandbox::build_seatbelt_profile(&SandboxPolicy {
            mode: SandboxMode::WorkspaceWrite,
            workspace_root: root.clone(),
            additional_writable_roots: vec![],
            session_temp_dir: None,
        });
        assert!(rw.contains("subpath"));
        fs::remove_dir_all(root).unwrap();
    }
}
