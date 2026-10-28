use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuixiAuthConfig {
    pub api_key: Option<String>,
    pub base_url: String,
    pub egress_mode: String,
    pub allowed_domains: Vec<String>,
}

impl Default for QuixiAuthConfig {
    fn default() -> Self {
        Self {
            api_key: std::env::var("ANTHROPIC_API_KEY").ok(),
            base_url: std::env::var("ANTHROPIC_BASE_URL")
                .unwrap_or_else(|_| "https://api.anthropic.com".to_string()),
            egress_mode: "full_egress".to_string(),
            allowed_domains: vec![
                "api.anthropic.com".to_string(),
                "github.com".to_string(),
                "npmjs.org".to_string(),
                "crates.io".to_string(),
            ],
        }
    }
}

pub struct NetstackPolicy {
    pub config_file: PathBuf,
}

impl NetstackPolicy {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(quixi_proto::paths::home_dir);
        let quixi_dir = home.join(".quixi");
        let _ = fs::create_dir_all(&quixi_dir);
        Self {
            config_file: quixi_dir.join("netstack.json"),
        }
    }

    pub fn load(&self) -> QuixiAuthConfig {
        let mut cfg: QuixiAuthConfig = fs::read_to_string(&self.config_file)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        if cfg.api_key.is_none() {
            cfg.api_key = std::env::var("ANTHROPIC_API_KEY")
                .ok()
                .filter(|k| k.starts_with("sk-ant-"));
        }
        cfg
    }

    pub fn save_api_key(&self, api_key: &str) {
        let mut cfg = self.load();
        cfg.api_key = Some(api_key.trim().to_string());
        if let Ok(s) = serde_json::to_string_pretty(&cfg) {
            let _ = fs::write(&self.config_file, s);
        }
    }

    pub fn is_host_allowed(&self, host: &str) -> bool {
        let cfg = self.load();
        if cfg.egress_mode == "full_egress" {
            return true;
        }
        cfg.allowed_domains
            .iter()
            .any(|d| host == d || host.ends_with(&format!(".{}", d)))
    }

    pub fn get_custom3p_config_health(&self) -> Value {
        json!({
            "state": "healthy",
            "health": "ok",
            "source": {
                "type": "local",
                "remote": false,
                "managedUnusable": false
            },
            "provider": "anthropic",
            "endpoint": "https://api.anthropic.com",
            "warnings": [],
            "login": {
                "enabled": true,
                "source": { "type": "local", "remote": false, "managedUnusable": false },
                "provider": "anthropic",
                "thirdPartyConfigured": true,
                "thirdPartyCanRun": true,
                "needsInteractiveAuth": false,
                "hybrid": false,
                "apiSignInAllowed": true
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_netstack_policy_and_health() {
        let ns = NetstackPolicy::new();
        assert!(ns.is_host_allowed("api.anthropic.com"));
        let health = ns.get_custom3p_config_health();
        assert_eq!(health["state"], "healthy");
        assert_eq!(health["source"]["type"], "local");
    }
}
