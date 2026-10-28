//! Routstr / Cashu integration.
//!
//! QuixiDesk does not re-implement a Cashu wallet. It manages the official
//! `routstrd` daemon (the fork with confidential upstream support), exposes its
//! OpenAI-compatible endpoint as two providers (`routstr` and
//! `routstr-confidential`), and surfaces wallet operations (Lightning top-up,
//! Cashu receive, seed backup, NPC address) in the provider settings screen.

use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub const DEFAULT_PORT: u16 = 8008;

pub struct RoutstrManager;

impl RoutstrManager {
    /// Location of the `routstrd` executable. Checks, in order:
    /// 1. `QUIXI_ROUTSTRD_BIN`
    /// 2. the app's bundled `resources/bin/routstrd` (baked-in build)
    /// 3. `~/.local/bin/routstrd`
    /// 4. `routstrd` on `PATH`
    pub fn binary() -> Option<PathBuf> {
        if let Ok(path) = std::env::var("QUIXI_ROUTSTRD_BIN") {
            let p = PathBuf::from(path);
            if p.is_file() {
                return Some(p);
            }
        }
        if let Ok(resources) = std::env::var("QUIXI_RESOURCES_DIR") {
            let p = PathBuf::from(resources).join("bin/routstrd");
            if p.is_file() {
                return Some(p);
            }
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                for candidate in [
                    dir.join("routstrd"),
                    dir.join("../Resources/bin/routstrd"),
                    dir.join("../share/quixidesk/bin/routstrd"),
                    dir.join("../../resources/bin/routstrd"),
                ] {
                    if candidate.is_file() {
                        return Some(candidate);
                    }
                }
            }
        }
        if let Some(home) = dirs::home_dir() {
            let p = home.join(".local/bin/routstrd");
            if p.is_file() {
                return Some(p);
            }
        }
        which("routstrd")
    }

    fn home() -> PathBuf {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
    }

    pub fn dir() -> PathBuf {
        std::env::var("ROUTSTRD_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| Self::home().join(".routstrd"))
    }

    pub fn wallet_dir() -> PathBuf {
        std::env::var("ROUTSTRD_WALLET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| Self::dir().join("wallet"))
    }

    fn config_path() -> PathBuf {
        Self::dir().join("config.json")
    }

    fn run(args: &[&str]) -> Result<String, String> {
        let bin = Self::binary().ok_or_else(|| {
            "routstrd is not installed. Install it from https://github.com/Routstr/routstrd/releases \
             (or build the confidential-upstream branch) and place the binary in ~/.local/bin/routstrd."
                .to_string()
        })?;
        let out = Command::new(&bin)
            .args(args)
            .env("ROUTSTRD_DIR", Self::dir())
            .env("NO_COLOR", "1")
            .output()
            .map_err(|e| format!("failed to run {}: {}", bin.display(), e))?;
        let mut text = String::from_utf8_lossy(&out.stdout).to_string();
        text.push_str(&String::from_utf8_lossy(&out.stderr));
        if !out.status.success() {
            return Err(if text.trim().is_empty() {
                format!("routstrd {} exited with {}", args.join(" "), out.status)
            } else {
                text.trim().to_string()
            });
        }
        Ok(text)
    }

    pub fn version() -> Option<String> {
        Self::run(&["--version"])
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    fn daemon_health() -> bool {
        let cfg = Self::read_config();
        let port = cfg
            .get("port")
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_PORT as u64);
        reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_millis(800))
            .build()
            .ok()
            .and_then(|c| c.get(format!("http://127.0.0.1:{}/health", port)).send().ok())
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    pub fn read_config() -> Value {
        fs::read_to_string(Self::config_path())
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .unwrap_or_else(|| json!({}))
    }

    fn write_config(cfg: &Value) -> Result<(), String> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let text = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
        fs::write(&path, text).map_err(|e| format!("failed to write {}: {}", path.display(), e))
    }

    fn parse_balance() -> Option<u64> {
        let text = Self::run(&["balance"]).ok()?;
        for line in text.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("Total:") {
                let digits: String = rest
                    .chars()
                    .skip_while(|c| !c.is_ascii_digit())
                    .take_while(|c| c.is_ascii_digit() || *c == ',')
                    .filter(|c| *c != ',')
                    .collect();
                if let Ok(v) = digits.parse::<u64>() {
                    return Some(v);
                }
            }
        }
        None
    }

    pub fn status() -> Value {
        let bin = Self::binary();
        let installed = bin.is_some();
        let version = if installed { Self::version() } else { None };
        let running = Self::daemon_health();
        let mut cli_status = Value::Null;
        if installed {
            if let Ok(text) = Self::run(&["status"]) {
                if let Ok(v) = serde_json::from_str::<Value>(text.trim()) {
                    cli_status = v;
                }
            }
        }
        let balance = if running { Self::parse_balance() } else { None };
        let cfg = Self::read_config();
        let confidential_enabled = cfg
            .pointer("/confidentialUpstream/enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let prover = find_prover();
        let wallet_config = Self::wallet_dir().join("config.json");
        json!({
            "installed": installed,
            "binary": bin.map(|p| p.to_string_lossy().to_string()),
            "version": version,
            "running": running,
            "daemon": cli_status,
            "balanceSats": balance,
            "port": cfg.get("port").and_then(|v| v.as_u64()).unwrap_or(DEFAULT_PORT as u64),
            "baseUrl": format!("http://127.0.0.1:{}/v1", cfg.get("port").and_then(|v| v.as_u64()).unwrap_or(DEFAULT_PORT as u64)),
            "walletInitialized": wallet_config.is_file(),
            "walletDir": Self::wallet_dir().to_string_lossy().to_string(),
            "configDir": Self::dir().to_string_lossy().to_string(),
            "confidential": {
                "enabled": confidential_enabled,
                "proverPath": prover.as_ref().map(|p| p.to_string_lossy().to_string()),
                "proverInstalled": prover.is_some(),
                "twoPartyInstalled": find_tool("cu-2pc").is_some(),
                "distroHint": "Confidential mode needs routstrd from the confidential-upstream branch plus the cu-prover and cu-2pc binaries."
            },
            "installHint": "curl -fsSL https://github.com/Routstr/routstrd/releases/latest/download/install.sh | sh",
        })
    }

    pub fn start() -> Value {
        if Self::binary().is_none() {
            return json!({ "ok": false, "error": "routstrd is not installed", "status": Self::status() });
        }
        if !Self::config_path().is_file() {
            // First run: create the config + in-process Cashu wallet without configuring
            // any third-party client integration.
            let _ = Self::run(&["onboard", "--skip-integration"]);
        }
        let result = Self::run(&["start"]);
        let ok = result.is_ok() || Self::daemon_health();
        json!({
            "ok": ok,
            "output": result.unwrap_or_default(),
            "status": Self::status(),
        })
    }

    pub fn stop() -> Value {
        let result = Self::run(&["stop"]);
        json!({
            "ok": result.is_ok(),
            "output": result.unwrap_or_default(),
            "status": Self::status(),
        })
    }

    /// Creates a Lightning invoice and returns it. `routstrd wallet receive bolt11`
    /// prints the invoice and exits; payment is credited to the in-process wallet.
    pub fn receive_lightning(amount_sats: u64) -> Value {
        if amount_sats == 0 {
            return json!({ "ok": false, "error": "amountSats must be greater than zero" });
        }
        if !Self::daemon_health() {
            let _ = Self::start();
        }
        let amount = amount_sats.to_string();
        match Self::run(&["wallet", "receive", "bolt11", &amount]) {
            Ok(text) => {
                let invoice = extract_invoice(&text);
                json!({
                    "ok": invoice.is_some(),
                    "amountSats": amount_sats,
                    "invoice": invoice,
                    "raw": if invoice.is_none() { json!(text) } else { Value::Null },
                })
            }
            Err(err) => json!({ "ok": false, "error": err }),
        }
    }

    pub fn receive_cashu(token: &str) -> Value {
        if token.trim().is_empty() {
            return json!({ "ok": false, "error": "Cashu token is required" });
        }
        match Self::run(&["wallet", "receive", "cashu", token.trim()]) {
            Ok(text) => json!({ "ok": true, "output": text, "status": Self::status() }),
            Err(err) => json!({ "ok": false, "error": err }),
        }
    }

    /// Returns the BIP39 mnemonic backing the Cashu wallet. Requires `confirm: true`
    /// so a stray request cannot silently exfiltrate it.
    pub fn seed_phrase(confirm: bool) -> Value {
        if !confirm {
            return json!({
                "ok": false,
                "requiresConfirmation": true,
                "error": "Set confirm=true to reveal the wallet seed phrase."
            });
        }
        let path = Self::wallet_dir().join("config.json");
        let cfg: Value = fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| json!({}));
        let mnemonic = cfg.get("mnemonic").and_then(|v| v.as_str()).unwrap_or("");
        if mnemonic.trim().is_empty() {
            return json!({ "ok": false, "error": "No wallet mnemonic found. Run routstrd onboard first." });
        }
        json!({
            "ok": true,
            "mnemonic": mnemonic,
            "encrypted": cfg.get("encrypted").and_then(|v| v.as_bool()).unwrap_or(false),
            "path": path.to_string_lossy().to_string(),
            "warning": "Anyone with this phrase can spend the wallet. Store it offline."
        })
    }

    pub fn npc_address() -> Value {
        match Self::run(&["wallet", "npc", "address"]) {
            Ok(text) => json!({ "ok": true, "address": extract_lightning_address(&text), "raw": text }),
            Err(err) => json!({ "ok": false, "error": err }),
        }
    }

    /// Enables confidential upstream inference and pins the Venice upstream hosts.
    pub fn enable_confidential() -> Value {
        let mut cfg = Self::read_config();
        if cfg.as_object().is_none() {
            return json!({ "ok": false, "error": "routstrd is not initialized. Start it once first." });
        }
        let prover = find_prover();
        let mut conf = json!({
            "enabled": true,
            "trusted_hosts": ["api.venice.ai", "*.venice.ai", "api.deepseek.com", "api.openai.com", "api.anthropic.com", "openrouter.ai"],
            "prewarm": { "enabled": true }
        });
        if let Some(p) = &prover {
            conf["prover_path"] = json!(p.to_string_lossy());
        }
        cfg["confidentialUpstream"] = conf;
        cfg["autoModelPath"] = json!(true);
        if let Err(err) = Self::write_config(&cfg) {
            return json!({ "ok": false, "error": err });
        }
        // Restart so the daemon picks up the new transport configuration.
        let _ = Self::stop();
        let started = Self::start();
        json!({
            "ok": started.get("ok").and_then(|v| v.as_bool()).unwrap_or(false),
            "proverInstalled": prover.is_some(),
            "warnings": if prover.is_some() {
                json!([])
            } else {
                json!(["cu-prover was not found; confidential requests will fail until it is installed and configured."])
            },
            "status": Self::status(),
        })
    }
}

fn extract_invoice(text: &str) -> Option<String> {
    for token in text.split(|c: char| {
        c.is_whitespace() || c == '"' || c == '\'' || c == ',' || c == ':' || c == '\u{1b}'
    }) {
        let t = token.trim();
        if t.starts_with("lnbc") || t.starts_with("lntb") || t.starts_with("lnbcrt") {
            if t.len() > 20 {
                return Some(t.to_string());
            }
        }
    }
    None
}

fn extract_lightning_address(text: &str) -> Option<String> {
    for token in text.split_whitespace() {
        let t = token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '@' && c != '.' && c != '_' && c != '-');
        if t.contains('@') && t.split('@').count() == 2 {
            return Some(t.to_string());
        }
    }
    None
}

fn find_prover() -> Option<PathBuf> {
    for var in ["QUIXI_CU_PROVER_BIN", "CU_PROVER_BIN", "CU_PROVER"] {
        if let Ok(p) = std::env::var(var) {
            let path = PathBuf::from(p);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    find_tool("cu-prover")
}

fn find_tool(name: &str) -> Option<PathBuf> {
    which(name)
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    if let Some(home) = dirs::home_dir() {
        for rel in [
            format!(".local/bin/{name}"),
            format!(".cargo/bin/{name}"),
            format!(".routstrd/bin/{name}"),
        ] {
            let p = home.join(rel);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

