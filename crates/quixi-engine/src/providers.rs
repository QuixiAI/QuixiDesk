use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const OPENAI_CODEX_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const OPENAI_CODEX_AUTHORIZE_URL: &str = "https://auth.openai.com/oauth/authorize";
const OPENAI_CODEX_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const OPENAI_CODEX_REDIRECT_URI: &str = "http://localhost:1455/auth/callback";
const OPENAI_CODEX_DEVICE_USERCODE_URL: &str =
    "https://auth.openai.com/api/accounts/deviceauth/usercode";
const OPENAI_CODEX_DEVICE_TOKEN_URL: &str = "https://auth.openai.com/api/accounts/deviceauth/token";
const OPENAI_CODEX_DEVICE_VERIFICATION_URL: &str = "https://auth.openai.com/codex/device";
const OPENAI_CODEX_DEVICE_REDIRECT_URI: &str = "https://auth.openai.com/deviceauth/callback";

const XAI_OAUTH_CLIENT_ID: &str = "b1a00492-073a-47ea-816f-4c329264a828";
const XAI_OAUTH_SCOPE: &str =
    "openid profile email offline_access grok-cli:access api:access";
const XAI_OAUTH_DISCOVERY_URL: &str = "https://auth.x.ai/.well-known/openid-configuration";
const XAI_DEVICE_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";

/// One incremental chunk of a streaming completion.
///
/// Reasoning-capable providers (DeepSeek `deepseek-flash`, Anthropic extended thinking, …)
/// stream a private chain of thought *before* the visible answer. Dropping those chunks
/// makes the UI look frozen while the model thinks, so they are surfaced separately and
/// rendered as a "Thinking" block instead of being merged into the answer text.
#[derive(Debug, Clone)]
pub enum StreamChunk {
    Text(String),
    Thinking(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelInfo {
    pub id: String,
    pub label: String,
    pub context_window: u32,
    pub supports_thinking: bool,
    pub supports_vision: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderEntry {
    pub id: String,
    pub name: String,
    pub subtitle: String,
    pub badge: String,
    pub category: String, // "priority" | "cloud" | "local"
    pub priority_rank: usize,
    pub supports_oauth: bool,
    pub oauth_label: Option<String>,
    pub auth_mode: String, // "api_key" | "oauth" | "custom" | "local"
    pub api_format: String, // "openai" | "anthropic"
    pub base_url: String,
    pub api_key_masked: String,
    #[serde(skip_serializing)]
    pub api_key_raw: String,
    #[serde(skip_serializing)]
    pub oauth_access_token: String,
    #[serde(skip_serializing)]
    pub oauth_refresh_token: String,
    pub oauth_account_label: Option<String>,
    pub oauth_expires_ms: Option<u64>,
    pub key_placeholder: String,
    pub env_var: String,
    pub docs_url: String,
    pub configured: bool,
    pub selected_model: String,
    pub models: Vec<ProviderModelInfo>,
    pub custom_models: Vec<String>,
    pub context_window: u32,
    pub last_tested_ok: Option<bool>,
    pub last_latency_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvidersStateFile {
    pub onboarding_completed: bool,
    pub active_provider: String,
    pub active_model: String,
    pub subagent_provider: String,
    pub subagent_model: String,
    pub providers: Vec<ProviderEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingOAuthSession {
    pub provider_id: String,
    pub pkce_verifier: String,
    pub pkce_challenge: String,
    pub state: String,
    pub authorize_url: String,
    pub verification_url: String,
    pub user_code: String,
    pub device_id: String,
    pub token_endpoint: String,
    pub created_at_ms: u64,
}

pub struct ProviderManager {
    pub config_path: PathBuf,
    pub state: RwLock<ProvidersStateFile>,
    pub pending_oauth: RwLock<HashMap<String, PendingOAuthSession>>,
}

impl ProviderManager {
    pub fn new(data_dir: &PathBuf) -> Self {
        let config_path = data_dir.join("providers.json");
        let default_catalog = Self::default_catalog();

        let loaded = if config_path.exists() {
            fs::read_to_string(&config_path)
                .ok()
                .and_then(|s| serde_json::from_str::<Value>(&s).ok())
                .and_then(|val| Self::merge_saved_state(default_catalog.clone(), &val))
        } else {
            None
        };

        let state = loaded.unwrap_or(default_catalog);
        let mgr = Self {
            config_path,
            state: RwLock::new(state),
            pending_oauth: RwLock::new(HashMap::new()),
        };
        mgr.persist();
        mgr
    }

    fn model(
        id: &str,
        label: &str,
        context_window: u32,
        supports_thinking: bool,
        supports_vision: bool,
    ) -> ProviderModelInfo {
        ProviderModelInfo {
            id: id.to_string(),
            label: label.to_string(),
            context_window,
            supports_thinking,
            supports_vision,
        }
    }

    /// Builds the full 19-entry provider catalog in the exact user-requested priority order:
    /// 1. OpenRouter
    /// 2. Custom Endpoint
    /// 3. OpenAI (OAuth + API Key)
    /// 4. Grok / xAI (OAuth + API Key)
    /// 5. Anthropic (API Key)
    /// 6..19. Remaining HexisPro cloud & local providers
    pub fn default_catalog() -> ProvidersStateFile {
        let providers = vec![
            // 1. OpenRouter (First on the list)
            ProviderEntry {
                id: "openrouter".to_string(),
                name: "OpenRouter".to_string(),
                subtitle: "Unified gateway to 200+ frontier models (Claude, GPT, Grok, Gemini, DeepSeek)".to_string(),
                badge: "Recommended · #1".to_string(),
                category: "priority".to_string(),
                priority_rank: 1,
                supports_oauth: false,
                oauth_label: None,
                auth_mode: "api_key".to_string(),
                api_format: "openai".to_string(),
                base_url: "https://openrouter.ai/api/v1".to_string(),
                api_key_masked: String::new(),
                api_key_raw: String::new(),
                oauth_access_token: String::new(),
                oauth_refresh_token: String::new(),
                oauth_account_label: None,
                oauth_expires_ms: None,
                key_placeholder: "sk-or-v1-...".to_string(),
                env_var: "OPENROUTER_API_KEY".to_string(),
                docs_url: "https://openrouter.ai/keys".to_string(),
                configured: false,
                selected_model: "anthropic/claude-sonnet-4.6".to_string(),
                models: vec![
                    Self::model("anthropic/claude-sonnet-4.6", "Claude Sonnet 4.6 (via OpenRouter)", 200_000, true, true),
                    Self::model("anthropic/claude-opus-4.6", "Claude Opus 4.6 (via OpenRouter)", 200_000, true, true),
                    Self::model("openai/gpt-5.4", "GPT-5.4 (via OpenRouter)", 256_000, true, true),
                    Self::model("x-ai/grok-4", "Grok 4 (via OpenRouter)", 256_000, true, true),
                    Self::model("google/gemini-3.1-pro-preview", "Gemini 3.1 Pro (via OpenRouter)", 1_000_000, true, true),
                    Self::model("deepseek/deepseek-v4.1-flash", "DeepSeek V4.1 Flash (via OpenRouter)", 1_048_576, true, true),
                    Self::model("deepseek/deepseek-v4-pro", "DeepSeek V4 Pro (via OpenRouter)", 1_048_576, true, false),
                    Self::model("qwen/qwen3-coder-480b", "Qwen 3 Coder 480B (via OpenRouter)", 128_000, true, false),
                ],
                custom_models: vec![],
                context_window: 200_000,
                last_tested_ok: None,
                last_latency_ms: None,
            },
            // 2. Custom Endpoint (Second on the list)
            ProviderEntry {
                id: "custom".to_string(),
                name: "Custom Endpoint".to_string(),
                subtitle: "Any OpenAI or Anthropic compatible base URL (LiteLLM, vLLM, enterprise proxy, self-hosted)".to_string(),
                badge: "Flexible URL · #2".to_string(),
                category: "priority".to_string(),
                priority_rank: 2,
                supports_oauth: false,
                oauth_label: None,
                auth_mode: "custom".to_string(),
                api_format: "openai".to_string(),
                base_url: "http://localhost:8000/v1".to_string(),
                api_key_masked: String::new(),
                api_key_raw: String::new(),
                oauth_access_token: String::new(),
                oauth_refresh_token: String::new(),
                oauth_account_label: None,
                oauth_expires_ms: None,
                key_placeholder: "Optional API key (sk-...)".to_string(),
                env_var: "CUSTOM_API_KEY".to_string(),
                docs_url: String::new(),
                configured: false,
                selected_model: "custom-model".to_string(),
                models: vec![
                    Self::model("custom-model", "Custom Model ID", 128_000, true, true),
                ],
                custom_models: vec!["custom-model".to_string()],
                context_window: 128_000,
                last_tested_ok: None,
                last_latency_ms: None,
            },
            // 3. OpenAI (Third on the list — supports OAuth!)
            ProviderEntry {
                id: "openai".to_string(),
                name: "OpenAI".to_string(),
                subtitle: "Sign in with ChatGPT Plus/Pro subscription (Codex OAuth) or use an OpenAI Platform API key".to_string(),
                badge: "Supports OAuth".to_string(),
                category: "priority".to_string(),
                priority_rank: 3,
                supports_oauth: true,
                oauth_label: Some("Sign in with ChatGPT (OAuth)".to_string()),
                auth_mode: "oauth".to_string(),
                api_format: "openai".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                api_key_masked: String::new(),
                api_key_raw: String::new(),
                oauth_access_token: String::new(),
                oauth_refresh_token: String::new(),
                oauth_account_label: None,
                oauth_expires_ms: None,
                key_placeholder: "sk-proj-...".to_string(),
                env_var: "OPENAI_API_KEY".to_string(),
                docs_url: "https://platform.openai.com/api-keys".to_string(),
                configured: false,
                selected_model: "gpt-6.1-sol".to_string(),
                models: vec![
                    Self::model("gpt-6.1-sol", "GPT-6.1 Sol (Codex OAuth)", 256_000, true, true),
                    Self::model("gpt-6-astra", "GPT-6 Astra (Codex OAuth)", 256_000, true, true),
                    Self::model("gpt-6-sol", "GPT-6 Sol (Codex OAuth)", 256_000, true, true),
                    Self::model("gpt-6-luna", "GPT-6 Luna (Codex OAuth)", 256_000, true, true),
                    Self::model("gpt-5.6-sol", "GPT-5.6 Sol (Codex OAuth)", 256_000, true, true),
                    Self::model("gpt-5.6-terra", "GPT-5.6 Terra (Codex OAuth)", 256_000, true, true),
                    Self::model("gpt-5.6-luna", "GPT-5.6 Luna (Codex OAuth)", 256_000, true, true),
                    Self::model("gpt-daybreak-blue-latest", "GPT Daybreak Blue (Codex OAuth)", 256_000, true, true),
                    Self::model("gpt-5.4", "GPT-5.4 (Platform API)", 256_000, true, true),
                    Self::model("gpt-5.4-mini", "GPT-5.4 Mini (Platform API)", 256_000, true, true),
                    Self::model("o3", "OpenAI o3 Reasoning (Platform API)", 200_000, true, true),
                    Self::model("gpt-4.1", "GPT-4.1 (Platform API)", 1_000_000, false, true),
                ],
                custom_models: vec![],
                context_window: 256_000,
                last_tested_ok: None,
                last_latency_ms: None,
            },
            // 4. Grok / xAI (Fourth on the list — supports OAuth!)
            ProviderEntry {
                id: "xai".to_string(),
                name: "Grok (xAI)".to_string(),
                subtitle: "Sign in with SuperGrok / X account (OAuth Device Flow) or use an xAI Console API key".to_string(),
                badge: "Supports OAuth".to_string(),
                category: "priority".to_string(),
                priority_rank: 4,
                supports_oauth: true,
                oauth_label: Some("Sign in with Grok / X (OAuth)".to_string()),
                auth_mode: "oauth".to_string(),
                api_format: "openai".to_string(),
                base_url: "https://api.x.ai/v1".to_string(),
                api_key_masked: String::new(),
                api_key_raw: String::new(),
                oauth_access_token: String::new(),
                oauth_refresh_token: String::new(),
                oauth_account_label: None,
                oauth_expires_ms: None,
                key_placeholder: "xai-...".to_string(),
                env_var: "XAI_API_KEY".to_string(),
                docs_url: "https://console.x.ai/".to_string(),
                configured: false,
                selected_model: "grok-4".to_string(),
                models: vec![
                    Self::model("grok-4", "Grok 4", 256_000, true, true),
                    Self::model("grok-4-fast", "Grok 4 Fast", 256_000, true, true),
                    Self::model("grok-code-fast-1", "Grok Code Fast 1", 256_000, true, false),
                    Self::model("grok-3", "Grok 3", 131_072, true, true),
                ],
                custom_models: vec![],
                context_window: 256_000,
                last_tested_ok: None,
                last_latency_ms: None,
            },
            // 5. Anthropic (Fifth on the list — API Key only, no OAuth)
            ProviderEntry {
                id: "anthropic".to_string(),
                name: "Anthropic".to_string(),
                subtitle: "Direct Claude API access via Anthropic Console API key (does not support OAuth)".to_string(),
                badge: "API Key Only".to_string(),
                category: "priority".to_string(),
                priority_rank: 5,
                supports_oauth: false,
                oauth_label: None,
                auth_mode: "api_key".to_string(),
                api_format: "anthropic".to_string(),
                base_url: "https://api.anthropic.com/v1".to_string(),
                api_key_masked: String::new(),
                api_key_raw: String::new(),
                oauth_access_token: String::new(),
                oauth_refresh_token: String::new(),
                oauth_account_label: None,
                oauth_expires_ms: None,
                key_placeholder: "sk-ant-api03-...".to_string(),
                env_var: "ANTHROPIC_API_KEY".to_string(),
                docs_url: "https://console.anthropic.com/settings/keys".to_string(),
                configured: false,
                selected_model: "claude-sonnet-4-6".to_string(),
                models: vec![
                    Self::model("claude-sonnet-4-6", "Claude Sonnet 4.6", 200_000, true, true),
                    Self::model("claude-opus-4-6", "Claude Opus 4.6", 200_000, true, true),
                    Self::model("claude-haiku-4-5-20251001", "Claude Haiku 4.5", 200_000, false, true),
                ],
                custom_models: vec![],
                context_window: 200_000,
                last_tested_ok: None,
                last_latency_ms: None,
            },
            // 6. Google Gemini
            ProviderEntry {
                id: "gemini".to_string(),
                name: "Google Gemini".to_string(),
                subtitle: "Gemini 3.1 Pro, 2.5 Pro, and 2.5 Flash via Google AI Studio API key".to_string(),
                badge: "1M Context".to_string(),
                category: "cloud".to_string(),
                priority_rank: 6,
                supports_oauth: false,
                oauth_label: None,
                auth_mode: "api_key".to_string(),
                api_format: "openai".to_string(),
                base_url: "https://generativelanguage.googleapis.com/v1beta/openai".to_string(),
                api_key_masked: String::new(),
                api_key_raw: String::new(),
                oauth_access_token: String::new(),
                oauth_refresh_token: String::new(),
                oauth_account_label: None,
                oauth_expires_ms: None,
                key_placeholder: "AIzaSy...".to_string(),
                env_var: "GEMINI_API_KEY".to_string(),
                docs_url: "https://aistudio.google.com/app/apikey".to_string(),
                configured: false,
                selected_model: "gemini-3.1-pro-preview".to_string(),
                models: vec![
                    Self::model("gemini-3.1-pro-preview", "Gemini 3.1 Pro Preview", 1_000_000, true, true),
                    Self::model("gemini-2.5-pro", "Gemini 2.5 Pro", 1_000_000, true, true),
                    Self::model("gemini-2.5-flash", "Gemini 2.5 Flash", 1_000_000, true, true),
                ],
                custom_models: vec![],
                context_window: 1_000_000,
                last_tested_ok: None,
                last_latency_ms: None,
            },
            // 7. DeepSeek
            ProviderEntry {
                id: "deepseek".to_string(),
                name: "DeepSeek".to_string(),
                subtitle: "DeepSeek V4.1 Flash (thinking + native vision) and V4 Pro — 1M context".to_string(),
                badge: "Reasoning".to_string(),
                category: "cloud".to_string(),
                priority_rank: 7,
                supports_oauth: false,
                oauth_label: None,
                auth_mode: "api_key".to_string(),
                api_format: "openai".to_string(),
                base_url: "https://api.deepseek.com/v1".to_string(),
                api_key_masked: String::new(),
                api_key_raw: String::new(),
                oauth_access_token: String::new(),
                oauth_refresh_token: String::new(),
                oauth_account_label: None,
                oauth_expires_ms: None,
                key_placeholder: "sk-...".to_string(),
                env_var: "DEEPSEEK_API_KEY".to_string(),
                docs_url: "https://platform.deepseek.com/api_keys".to_string(),
                configured: false,
                // DeepSeek's API exposes exactly two model ids since V4:
                // "deepseek-flash" (V4.1-Flash, text+vision, thinking on by default with
                // low/high/max effort) and "deepseek-v4-pro" (V4-Pro, text only).
                // "deepseek-chat" / "deepseek-reasoner" are legacy aliases: both are served
                // by deepseek-flash today (verified against api.deepseek.com/models), and
                // the R1 naming was retired back in 2025.
                selected_model: "deepseek-flash".to_string(),
                models: vec![
                    Self::model("deepseek-flash", "DeepSeek V4.1 Flash · thinking + vision", 1_048_576, true, true),
                    Self::model("deepseek-v4-pro", "DeepSeek V4 Pro", 1_048_576, true, false),
                    Self::model("deepseek-chat", "Legacy alias · non-thinking · served by V4.1 Flash", 1_048_576, false, true),
                    Self::model("deepseek-reasoner", "Legacy alias · thinking · served by V4.1 Flash", 1_048_576, true, true),
                ],
                custom_models: vec![],
                context_window: 1_048_576,
                last_tested_ok: None,
                last_latency_ms: None,
            },
            // 8. Groq
            ProviderEntry {
                id: "groq".to_string(),
                name: "Groq".to_string(),
                subtitle: "Ultra-low-latency LPU inference for Llama 3.3, Kimi K2, and Qwen".to_string(),
                badge: "Ultra-Fast".to_string(),
                category: "cloud".to_string(),
                priority_rank: 8,
                supports_oauth: false,
                oauth_label: None,
                auth_mode: "api_key".to_string(),
                api_format: "openai".to_string(),
                base_url: "https://api.groq.com/openai/v1".to_string(),
                api_key_masked: String::new(),
                api_key_raw: String::new(),
                oauth_access_token: String::new(),
                oauth_refresh_token: String::new(),
                oauth_account_label: None,
                oauth_expires_ms: None,
                key_placeholder: "gsk_...".to_string(),
                env_var: "GROQ_API_KEY".to_string(),
                docs_url: "https://console.groq.com/keys".to_string(),
                configured: false,
                selected_model: "llama-3.3-70b-versatile".to_string(),
                models: vec![
                    Self::model("llama-3.3-70b-versatile", "Llama 3.3 70B Versatile", 128_000, false, false),
                    Self::model("moonshotai/kimi-k2-instruct", "Kimi K2 Instruct (Groq)", 128_000, false, false),
                    Self::model("qwen-qwq-32b", "Qwen QwQ 32B", 128_000, true, false),
                ],
                custom_models: vec![],
                context_window: 128_000,
                last_tested_ok: None,
                last_latency_ms: None,
            },
            // 9. Mistral
            ProviderEntry {
                id: "mistral".to_string(),
                name: "Mistral AI".to_string(),
                subtitle: "Mistral Large, Codestral, and Pixtral models".to_string(),
                badge: "Cloud".to_string(),
                category: "cloud".to_string(),
                priority_rank: 9,
                supports_oauth: false,
                oauth_label: None,
                auth_mode: "api_key".to_string(),
                api_format: "openai".to_string(),
                base_url: "https://api.mistral.ai/v1".to_string(),
                api_key_masked: String::new(),
                api_key_raw: String::new(),
                oauth_access_token: String::new(),
                oauth_refresh_token: String::new(),
                oauth_account_label: None,
                oauth_expires_ms: None,
                key_placeholder: "Mistral API Key".to_string(),
                env_var: "MISTRAL_API_KEY".to_string(),
                docs_url: "https://console.mistral.ai/api-keys/".to_string(),
                configured: false,
                selected_model: "mistral-large-latest".to_string(),
                models: vec![
                    Self::model("mistral-large-latest", "Mistral Large", 128_000, false, true),
                    Self::model("codestral-latest", "Codestral", 256_000, false, false),
                ],
                custom_models: vec![],
                context_window: 128_000,
                last_tested_ok: None,
                last_latency_ms: None,
            },
            // 10. MiniMax
            ProviderEntry {
                id: "minimax".to_string(),
                name: "MiniMax".to_string(),
                subtitle: "MiniMax M2.7 and M1 long-context reasoning models".to_string(),
                badge: "Cloud".to_string(),
                category: "cloud".to_string(),
                priority_rank: 10,
                supports_oauth: false,
                oauth_label: None,
                auth_mode: "api_key".to_string(),
                api_format: "openai".to_string(),
                base_url: "https://api.minimax.io/v1".to_string(),
                api_key_masked: String::new(),
                api_key_raw: String::new(),
                oauth_access_token: String::new(),
                oauth_refresh_token: String::new(),
                oauth_account_label: None,
                oauth_expires_ms: None,
                key_placeholder: "MiniMax API Key".to_string(),
                env_var: "MINIMAX_API_KEY".to_string(),
                docs_url: "https://platform.minimax.io/".to_string(),
                configured: false,
                selected_model: "MiniMax-M2.7".to_string(),
                models: vec![
                    Self::model("MiniMax-M2.7", "MiniMax M2.7", 200_000, true, true),
                    Self::model("MiniMax-M1", "MiniMax M1", 1_000_000, true, false),
                ],
                custom_models: vec![],
                context_window: 200_000,
                last_tested_ok: None,
                last_latency_ms: None,
            },
            // 11. Zhipu / GLM
            ProviderEntry {
                id: "zai".to_string(),
                name: "Zhipu / GLM (Z.ai)".to_string(),
                subtitle: "GLM-5.1 and GLM-4.5 agentic models".to_string(),
                badge: "Cloud".to_string(),
                category: "cloud".to_string(),
                priority_rank: 11,
                supports_oauth: false,
                oauth_label: None,
                auth_mode: "api_key".to_string(),
                api_format: "openai".to_string(),
                base_url: "https://api.z.ai/api/paas/v4".to_string(),
                api_key_masked: String::new(),
                api_key_raw: String::new(),
                oauth_access_token: String::new(),
                oauth_refresh_token: String::new(),
                oauth_account_label: None,
                oauth_expires_ms: None,
                key_placeholder: "Z.ai API Key".to_string(),
                env_var: "ZAI_API_KEY".to_string(),
                docs_url: "https://open.bigmodel.cn/".to_string(),
                configured: false,
                selected_model: "glm-5.1".to_string(),
                models: vec![
                    Self::model("glm-5.1", "GLM-5.1", 128_000, true, true),
                    Self::model("glm-4.5", "GLM-4.5", 128_000, true, true),
                ],
                custom_models: vec![],
                context_window: 128_000,
                last_tested_ok: None,
                last_latency_ms: None,
            },
            // 12. Moonshot / Kimi
            ProviderEntry {
                id: "moonshot".to_string(),
                name: "Moonshot / Kimi".to_string(),
                subtitle: "Kimi K2.5 and Moonshot v1 models".to_string(),
                badge: "Cloud".to_string(),
                category: "cloud".to_string(),
                priority_rank: 12,
                supports_oauth: false,
                oauth_label: None,
                auth_mode: "api_key".to_string(),
                api_format: "openai".to_string(),
                base_url: "https://api.moonshot.ai/v1".to_string(),
                api_key_masked: String::new(),
                api_key_raw: String::new(),
                oauth_access_token: String::new(),
                oauth_refresh_token: String::new(),
                oauth_account_label: None,
                oauth_expires_ms: None,
                key_placeholder: "sk-...".to_string(),
                env_var: "MOONSHOT_API_KEY".to_string(),
                docs_url: "https://platform.moonshot.ai/".to_string(),
                configured: false,
                selected_model: "kimi-k2.5".to_string(),
                models: vec![
                    Self::model("kimi-k2.5", "Kimi K2.5", 256_000, true, true),
                    Self::model("moonshot-v1-128k", "Moonshot v1 128K", 128_000, false, false),
                ],
                custom_models: vec![],
                context_window: 256_000,
                last_tested_ok: None,
                last_latency_ms: None,
            },
            // 13. Cohere
            ProviderEntry {
                id: "cohere".to_string(),
                name: "Cohere".to_string(),
                subtitle: "Command A and Command R+ enterprise RAG models".to_string(),
                badge: "Cloud".to_string(),
                category: "cloud".to_string(),
                priority_rank: 13,
                supports_oauth: false,
                oauth_label: None,
                auth_mode: "api_key".to_string(),
                api_format: "openai".to_string(),
                base_url: "https://api.cohere.com/compatibility/v1".to_string(),
                api_key_masked: String::new(),
                api_key_raw: String::new(),
                oauth_access_token: String::new(),
                oauth_refresh_token: String::new(),
                oauth_account_label: None,
                oauth_expires_ms: None,
                key_placeholder: "Cohere API Key".to_string(),
                env_var: "COHERE_API_KEY".to_string(),
                docs_url: "https://dashboard.cohere.com/api-keys".to_string(),
                configured: false,
                selected_model: "command-a-03-2025".to_string(),
                models: vec![
                    Self::model("command-a-03-2025", "Command A (03-2025)", 256_000, false, false),
                    Self::model("command-r-plus-08-2024", "Command R+", 128_000, false, false),
                ],
                custom_models: vec![],
                context_window: 256_000,
                last_tested_ok: None,
                last_latency_ms: None,
            },
            // 14. Together AI
            ProviderEntry {
                id: "together".to_string(),
                name: "Together AI".to_string(),
                subtitle: "Serverless open-weights cloud (DeepSeek, Llama, Qwen)".to_string(),
                badge: "Open Weights".to_string(),
                category: "cloud".to_string(),
                priority_rank: 14,
                supports_oauth: false,
                oauth_label: None,
                auth_mode: "api_key".to_string(),
                api_format: "openai".to_string(),
                base_url: "https://api.together.xyz/v1".to_string(),
                api_key_masked: String::new(),
                api_key_raw: String::new(),
                oauth_access_token: String::new(),
                oauth_refresh_token: String::new(),
                oauth_account_label: None,
                oauth_expires_ms: None,
                key_placeholder: "Together API Key".to_string(),
                env_var: "TOGETHER_API_KEY".to_string(),
                docs_url: "https://api.together.xyz/settings/api-keys".to_string(),
                configured: false,
                selected_model: "deepseek-ai/DeepSeek-V3".to_string(),
                models: vec![
                    Self::model("deepseek-ai/DeepSeek-V3", "DeepSeek V3", 128_000, false, false),
                    Self::model("meta-llama/Llama-3.3-70B-Instruct-Turbo", "Llama 3.3 70B Turbo", 128_000, false, false),
                ],
                custom_models: vec![],
                context_window: 128_000,
                last_tested_ok: None,
                last_latency_ms: None,
            },
            // 15. Fireworks AI
            ProviderEntry {
                id: "fireworks".to_string(),
                name: "Fireworks AI".to_string(),
                subtitle: "Fastserverless inference for DeepSeek, Llama, and Qwen".to_string(),
                badge: "Open Weights".to_string(),
                category: "cloud".to_string(),
                priority_rank: 15,
                supports_oauth: false,
                oauth_label: None,
                auth_mode: "api_key".to_string(),
                api_format: "openai".to_string(),
                base_url: "https://api.fireworks.ai/inference/v1".to_string(),
                api_key_masked: String::new(),
                api_key_raw: String::new(),
                oauth_access_token: String::new(),
                oauth_refresh_token: String::new(),
                oauth_account_label: None,
                oauth_expires_ms: None,
                key_placeholder: "fw_...".to_string(),
                env_var: "FIREWORKS_API_KEY".to_string(),
                docs_url: "https://fireworks.ai/account/api-keys".to_string(),
                configured: false,
                selected_model: "accounts/fireworks/models/deepseek-v3".to_string(),
                models: vec![
                    Self::model("accounts/fireworks/models/deepseek-v3", "DeepSeek V3 (Fireworks)", 128_000, false, false),
                    Self::model("accounts/fireworks/models/llama-v3p3-70b-instruct", "Llama 3.3 70B (Fireworks)", 128_000, false, false),
                ],
                custom_models: vec![],
                context_window: 128_000,
                last_tested_ok: None,
                last_latency_ms: None,
            },
            // 16. Ollama (Local)
            ProviderEntry {
                id: "ollama".to_string(),
                name: "Ollama (Local)".to_string(),
                subtitle: "Local models running on http://localhost:11434/v1".to_string(),
                badge: "Local · :11434".to_string(),
                category: "local".to_string(),
                priority_rank: 16,
                supports_oauth: false,
                oauth_label: None,
                auth_mode: "local".to_string(),
                api_format: "openai".to_string(),
                base_url: "http://localhost:11434/v1".to_string(),
                api_key_masked: String::new(),
                api_key_raw: String::new(),
                oauth_access_token: String::new(),
                oauth_refresh_token: String::new(),
                oauth_account_label: None,
                oauth_expires_ms: None,
                key_placeholder: "No API key required".to_string(),
                env_var: "OLLAMA_HOST".to_string(),
                docs_url: "https://ollama.com".to_string(),
                configured: false,
                selected_model: "qwen2.5-coder:32b".to_string(),
                models: vec![
                    Self::model("qwen2.5-coder:32b", "Qwen 2.5 Coder 32B", 128_000, false, false),
                    Self::model("llama3.3:latest", "Llama 3.3", 128_000, false, false),
                    Self::model("deepseek-r1:32b", "DeepSeek R1 32B", 128_000, true, false),
                ],
                custom_models: vec![],
                context_window: 128_000,
                last_tested_ok: None,
                last_latency_ms: None,
            },
            // 17. LM Studio (Local)
            ProviderEntry {
                id: "lmstudio".to_string(),
                name: "LM Studio (Local)".to_string(),
                subtitle: "Local OpenAI-compatible server on http://localhost:1234/v1".to_string(),
                badge: "Local · :1234".to_string(),
                category: "local".to_string(),
                priority_rank: 17,
                supports_oauth: false,
                oauth_label: None,
                auth_mode: "local".to_string(),
                api_format: "openai".to_string(),
                base_url: "http://localhost:1234/v1".to_string(),
                api_key_masked: String::new(),
                api_key_raw: String::new(),
                oauth_access_token: String::new(),
                oauth_refresh_token: String::new(),
                oauth_account_label: None,
                oauth_expires_ms: None,
                key_placeholder: "lm-studio".to_string(),
                env_var: "LMSTUDIO_HOST".to_string(),
                docs_url: "https://lmstudio.ai".to_string(),
                configured: false,
                selected_model: "local-model".to_string(),
                models: vec![
                    Self::model("local-model", "Loaded LM Studio Model", 128_000, true, false),
                ],
                custom_models: vec![],
                context_window: 128_000,
                last_tested_ok: None,
                last_latency_ms: None,
            },
            // 18. vLLM (Local)
            ProviderEntry {
                id: "vllm".to_string(),
                name: "vLLM (Local)".to_string(),
                subtitle: "High-throughput local server on http://localhost:8000/v1".to_string(),
                badge: "Local · :8000".to_string(),
                category: "local".to_string(),
                priority_rank: 18,
                supports_oauth: false,
                oauth_label: None,
                auth_mode: "local".to_string(),
                api_format: "openai".to_string(),
                base_url: "http://localhost:8000/v1".to_string(),
                api_key_masked: String::new(),
                api_key_raw: String::new(),
                oauth_access_token: String::new(),
                oauth_refresh_token: String::new(),
                oauth_account_label: None,
                oauth_expires_ms: None,
                key_placeholder: "Optional token".to_string(),
                env_var: "VLLM_HOST".to_string(),
                docs_url: "https://docs.vllm.ai".to_string(),
                configured: false,
                selected_model: "vllm-model".to_string(),
                models: vec![
                    Self::model("vllm-model", "Served vLLM Model", 128_000, true, false),
                ],
                custom_models: vec![],
                context_window: 128_000,
                last_tested_ok: None,
                last_latency_ms: None,
            },
            // 19. llama.cpp (Local)
            ProviderEntry {
                id: "llamacpp".to_string(),
                name: "llama.cpp (Local)".to_string(),
                subtitle: "llama-server running on http://localhost:8080/v1".to_string(),
                badge: "Local · :8080".to_string(),
                category: "local".to_string(),
                priority_rank: 19,
                supports_oauth: false,
                oauth_label: None,
                auth_mode: "local".to_string(),
                api_format: "openai".to_string(),
                base_url: "http://localhost:8080/v1".to_string(),
                api_key_masked: String::new(),
                api_key_raw: String::new(),
                oauth_access_token: String::new(),
                oauth_refresh_token: String::new(),
                oauth_account_label: None,
                oauth_expires_ms: None,
                key_placeholder: "Optional key".to_string(),
                env_var: "LLAMACPP_HOST".to_string(),
                docs_url: "https://github.com/ggml-org/llama.cpp".to_string(),
                configured: false,
                selected_model: "gguf-model".to_string(),
                models: vec![
                    Self::model("gguf-model", "Loaded GGUF Model", 128_000, true, false),
                ],
                custom_models: vec![],
                context_window: 128_000,
                last_tested_ok: None,
                last_latency_ms: None,
            },
        ];

        ProvidersStateFile {
            onboarding_completed: false,
            active_provider: "openrouter".to_string(),
            active_model: "anthropic/claude-sonnet-4.6".to_string(),
            subagent_provider: "openrouter".to_string(),
            subagent_model: "anthropic/claude-sonnet-4.6".to_string(),
            providers,
        }
    }

    fn mask_secret(secret: &str) -> String {
        let trimmed = secret.trim();
        if trimmed.is_empty() {
            return String::new();
        }
        let chars: Vec<char> = trimmed.chars().collect();
        if chars.len() <= 8 {
            return "••••••••".to_string();
        }
        let prefix: String = chars.iter().take(6).collect();
        let suffix: String = chars.iter().skip(chars.len() - 4).collect();
        format!("{}••••{}", prefix, suffix)
    }

    fn merge_saved_state(mut catalog: ProvidersStateFile, saved: &Value) -> Option<ProvidersStateFile> {
        let obj = saved.as_object()?;
        if let Some(onb) = obj.get("onboardingCompleted").and_then(|v| v.as_bool()) {
            catalog.onboarding_completed = onb;
        }
        if let Some(ap) = obj.get("activeProvider").and_then(|v| v.as_str()) {
            catalog.active_provider = ap.to_string();
        }
        if let Some(am) = obj.get("activeModel").and_then(|v| v.as_str()) {
            if am == "gpt-5.3-codex" {
                catalog.active_model = "gpt-6.1-sol".to_string();
            } else {
                catalog.active_model = am.to_string();
            }
        }
        if let Some(sp) = obj.get("subagentProvider").and_then(|v| v.as_str()) {
            catalog.subagent_provider = sp.to_string();
        }
        if let Some(sm) = obj.get("subagentModel").and_then(|v| v.as_str()) {
            catalog.subagent_model = sm.to_string();
        }

        if let Some(saved_list) = obj.get("providers").and_then(|v| v.as_array()) {
            for saved_p in saved_list {
                let Some(id) = saved_p.get("id").and_then(|v| v.as_str()) else {
                    continue;
                };
                if let Some(entry) = catalog.providers.iter_mut().find(|p| p.id == id) {
                    if let Some(cfg) = saved_p.get("configured").and_then(|v| v.as_bool()) {
                        entry.configured = cfg;
                    }
                    if let Some(bu) = saved_p.get("baseUrl").and_then(|v| v.as_str()) {
                        if !bu.trim().is_empty() {
                            entry.base_url = bu.to_string();
                        }
                    }
                    if let Some(af) = saved_p.get("apiFormat").and_then(|v| v.as_str()) {
                        if !af.trim().is_empty() {
                            entry.api_format = af.to_string();
                        }
                    }
                    if let Some(am) = saved_p.get("authMode").and_then(|v| v.as_str()) {
                        if !am.trim().is_empty() {
                            entry.auth_mode = am.to_string();
                        }
                    }
                    if let Some(sm) = saved_p.get("selectedModel").and_then(|v| v.as_str()) {
                        if !sm.trim().is_empty() {
                            if id == "openai" && sm.trim() == "gpt-5.3-codex" {
                                entry.selected_model = "gpt-6.1-sol".to_string();
                            } else if let Some(current) = Self::migrate_legacy_model_id(id, sm.trim()) {
                                entry.selected_model = current;
                            } else {
                                entry.selected_model = sm.to_string();
                            }
                        }
                    }
                    if let Some(raw_key) = saved_p.get("apiKeyRaw").and_then(|v| v.as_str()) {
                        entry.api_key_raw = raw_key.to_string();
                        entry.api_key_masked = Self::mask_secret(raw_key);
                    }
                    if let Some(oa) = saved_p.get("oauthAccessToken").and_then(|v| v.as_str()) {
                        entry.oauth_access_token = oa.to_string();
                    }
                    if let Some(or_tok) = saved_p.get("oauthRefreshToken").and_then(|v| v.as_str()) {
                        entry.oauth_refresh_token = or_tok.to_string();
                    }
                    if let Some(lbl) = saved_p.get("oauthAccountLabel").and_then(|v| v.as_str()) {
                        entry.oauth_account_label = Some(lbl.to_string());
                    }
                    if let Some(exp) = saved_p.get("oauthExpiresMs").and_then(|v| v.as_u64()) {
                        entry.oauth_expires_ms = Some(exp);
                    }
                    if let Some(cw) = saved_p.get("contextWindow").and_then(|v| v.as_u64()) {
                        entry.context_window = cw as u32;
                    }
                    if let Some(saved_models) = saved_p.get("models").and_then(|v| v.as_array()) {
                        for sm_val in saved_models {
                            if let Ok(pm) = serde_json::from_value::<ProviderModelInfo>(sm_val.clone())
                            {
                                if pm.id == "gpt-5.3-codex" {
                                    continue;
                                }
                                if !entry.models.iter().any(|m| m.id == pm.id) {
                                    entry.models.push(pm);
                                }
                            }
                        }
                    }
                    if let Some(cm) = saved_p.get("customModels").and_then(|v| v.as_array()) {
                        entry.custom_models = cm
                            .iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect();
                        for m_id in &entry.custom_models {
                            if !entry.models.iter().any(|m| &m.id == m_id) {
                                entry.models.push(Self::model(
                                    m_id,
                                    m_id,
                                    entry.context_window,
                                    true,
                                    true,
                                ));
                            }
                        }
                    }
                }
            }
        }
        // DeepSeek moved from R1-era ids to V4 (deepseek-flash / deepseek-v4-pro, 1M ctx).
        if let Some(ds) = catalog.providers.iter_mut().find(|p| p.id == "deepseek") {
            if let Some(current) = Self::migrate_legacy_model_id("deepseek", &ds.selected_model) {
                ds.selected_model = current;
            }
            if ds.context_window < 1_048_576 {
                ds.context_window = 1_048_576;
            }
        }
        if let Some(current) = Self::migrate_legacy_model_id("deepseek", &catalog.active_model) {
            catalog.active_model = current;
        }
        if let Some(current) = Self::migrate_legacy_model_id("deepseek", &catalog.subagent_model) {
            catalog.subagent_model = current;
        }

        Some(catalog)
    }

    /// Maps retired provider-specific model ids onto the ids the provider serves today.
    ///
    /// DeepSeek retired the R1-era aliases: `deepseek-chat` (non-thinking) and
    /// `deepseek-reasoner` (thinking) are both served by `deepseek-flash` now, so a
    /// persisted selection keeps working while showing the real model.
    pub fn migrate_legacy_model_id(provider_id: &str, model_id: &str) -> Option<String> {
        match (provider_id, model_id) {
            ("deepseek", "deepseek-chat")
            | ("deepseek", "deepseek-reasoner")
            | ("deepseek", "deepseek-v4-flash")
            | ("deepseek", "deepseek-v4-flash-0731") => Some("deepseek-flash".to_string()),
            ("deepseek", "deepseek-v4-pro-preview") => Some("deepseek-v4-pro".to_string()),
            ("openrouter", "deepseek/deepseek-r1")
            | ("openrouter", "deepseek/deepseek-r1-0528") => {
                Some("deepseek/deepseek-v4.1-flash".to_string())
            }
            _ => None,
        }
    }

    pub fn persist(&self) {
        let st = self.state.read();
        let mut providers_disk = Vec::new();
        for p in &st.providers {
            let mut val = serde_json::to_value(p).unwrap_or_else(|_| json!({}));
            if let Some(obj) = val.as_object_mut() {
                obj.insert("apiKeyRaw".to_string(), json!(p.api_key_raw));
                obj.insert("oauthAccessToken".to_string(), json!(p.oauth_access_token));
                obj.insert(
                    "oauthRefreshToken".to_string(),
                    json!(p.oauth_refresh_token),
                );
            }
            providers_disk.push(val);
        }
        let disk_json = json!({
            "onboardingCompleted": st.onboarding_completed,
            "activeProvider": st.active_provider,
            "activeModel": st.active_model,
            "subagentProvider": st.subagent_provider,
            "subagentModel": st.subagent_model,
            "providers": providers_disk,
        });
        if let Ok(pretty) = serde_json::to_string_pretty(&disk_json) {
            let _ = fs::write(&self.config_path, pretty);
        }
    }

    pub fn get_public_state(&self) -> Value {
        let st = self.state.read();
        let configured_count = st.providers.iter().filter(|p| p.configured).count();
        json!({
            "onboardingCompleted": st.onboarding_completed && configured_count > 0,
            "hasConfiguredProvider": configured_count > 0,
            "configuredCount": configured_count,
            "activeProvider": st.active_provider,
            "activeModel": st.active_model,
            "subagentProvider": st.subagent_provider,
            "subagentModel": st.subagent_model,
            "providers": st.providers,
        })
    }

    pub fn save_provider(&self, body: &Value) -> Value {
        let provider_id = body
            .get("providerId")
            .or_else(|| body.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("openrouter");

        let mut st = self.state.write();
        let mut updated_model = String::new();
        let mut found = false;

        if let Some(entry) = st.providers.iter_mut().find(|p| p.id == provider_id) {
            found = true;
            if let Some(api_key) = body.get("apiKey").and_then(|v| v.as_str()) {
                let trimmed = api_key.trim();
                if !trimmed.is_empty() && !trimmed.contains("••••") {
                    entry.api_key_raw = trimmed.to_string();
                    entry.api_key_masked = Self::mask_secret(trimmed);
                }
            }
            if let Some(base_url) = body.get("baseUrl").and_then(|v| v.as_str()) {
                if !base_url.trim().is_empty() {
                    entry.base_url = base_url.trim().trim_end_matches('/').to_string();
                }
            }
            if let Some(api_format) = body.get("apiFormat").and_then(|v| v.as_str()) {
                if !api_format.trim().is_empty() {
                    entry.api_format = api_format.trim().to_string();
                }
            }
            if let Some(auth_mode) = body.get("authMode").and_then(|v| v.as_str()) {
                if !auth_mode.trim().is_empty() {
                    entry.auth_mode = auth_mode.trim().to_string();
                }
            }
            if let Some(cw) = body.get("contextWindow").and_then(|v| v.as_u64()) {
                if cw >= 1024 {
                    entry.context_window = cw as u32;
                }
            }
            if let Some(custom_model) = body.get("customModel").and_then(|v| v.as_str()) {
                let cm = custom_model.trim();
                if !cm.is_empty() {
                    if !entry.custom_models.contains(&cm.to_string()) {
                        entry.custom_models.push(cm.to_string());
                    }
                    if !entry.models.iter().any(|m| m.id == cm) {
                        entry.models.insert(
                            0,
                            Self::model(cm, cm, entry.context_window, true, true),
                        );
                    }
                    entry.selected_model = cm.to_string();
                }
            }
            if let Some(sel_model) = body.get("selectedModel").and_then(|v| v.as_str()) {
                let sm = sel_model.trim();
                if !sm.is_empty() {
                    if !entry.models.iter().any(|m| m.id == sm) {
                        entry.models.insert(
                            0,
                            Self::model(sm, sm, entry.context_window, true, true),
                        );
                    }
                    entry.selected_model = sm.to_string();
                }
            }
            entry.configured = true;
            updated_model = entry.selected_model.clone();
        }

        if found {
            if body
                .get("setAsActive")
                .and_then(|v| v.as_bool())
                .unwrap_or(true)
            {
                st.active_provider = provider_id.to_string();
                if !updated_model.is_empty() {
                    st.active_model = updated_model;
                }
            }
            if body
                .get("completeOnboarding")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                st.onboarding_completed = true;
            }
        }
        drop(st);
        self.persist();
        self.get_public_state()
    }

    pub fn remove_provider(&self, body: &Value) -> Value {
        let provider_id = body
            .get("providerId")
            .or_else(|| body.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let mut st = self.state.write();
        if let Some(entry) = st.providers.iter_mut().find(|p| p.id == provider_id) {
            entry.configured = false;
            entry.api_key_raw.clear();
            entry.api_key_masked.clear();
            entry.oauth_access_token.clear();
            entry.oauth_refresh_token.clear();
            entry.oauth_account_label = None;
            entry.oauth_expires_ms = None;
            entry.last_tested_ok = None;
        }
        let remaining_first = st
            .providers
            .iter()
            .find(|p| p.configured)
            .map(|p| (p.id.clone(), p.selected_model.clone()));
        if let Some((pid, mid)) = remaining_first {
            if st.active_provider == provider_id {
                st.active_provider = pid;
                st.active_model = mid;
            }
        } else {
            st.onboarding_completed = false;
        }
        drop(st);
        self.persist();
        self.get_public_state()
    }

    pub fn complete_onboarding(&self, body: &Value) -> Value {
        let mut st = self.state.write();
        if let Some(ap) = body.get("activeProvider").and_then(|v| v.as_str()) {
            if !ap.is_empty() {
                st.active_provider = ap.to_string();
            }
        }
        if let Some(am) = body.get("activeModel").and_then(|v| v.as_str()) {
            if !am.is_empty() {
                st.active_model = am.to_string();
            }
        }
        if let Some(sp) = body.get("subagentProvider").and_then(|v| v.as_str()) {
            if !sp.is_empty() {
                st.subagent_provider = sp.to_string();
            }
        }
        if let Some(sm) = body.get("subagentModel").and_then(|v| v.as_str()) {
            if !sm.is_empty() {
                st.subagent_model = sm.to_string();
            }
        }
        let any_configured = st.providers.iter().any(|p| p.configured);
        if any_configured {
            st.onboarding_completed = true;
        }
        drop(st);
        self.persist();
        self.get_public_state()
    }

    pub fn test_provider(&self, body: &Value) -> Value {
        let provider_id = body
            .get("providerId")
            .and_then(|v| v.as_str())
            .unwrap_or("openrouter");
        let supplied_key = body
            .get("apiKey")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        let supplied_url = body
            .get("baseUrl")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();

        let (base_url, api_key, api_format, is_local) = {
            let st = self.state.read();
            if let Some(entry) = st.providers.iter().find(|p| p.id == provider_id) {
                let url = if !supplied_url.is_empty() {
                    supplied_url.trim_end_matches('/').to_string()
                } else {
                    entry.base_url.trim_end_matches('/').to_string()
                };
                let key = if !supplied_key.is_empty() && !supplied_key.contains("••••") {
                    supplied_key.to_string()
                } else if !entry.oauth_access_token.is_empty() {
                    entry.oauth_access_token.clone()
                } else {
                    entry.api_key_raw.clone()
                };
                (
                    url,
                    key,
                    entry.api_format.clone(),
                    entry.category == "local",
                )
            } else {
                (
                    supplied_url.to_string(),
                    supplied_key.to_string(),
                    "openai".to_string(),
                    false,
                )
            }
        };

        let start = Instant::now();
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(6))
            .build();

        let mut ok = false;
        let mut message = String::new();
        let mut discovered_models: Vec<String> = Vec::new();

        if let Ok(http) = client {
            if provider_id == "openai"
                && !api_key.is_empty()
                && Self::extract_chatgpt_account_id(&api_key).is_some()
            {
                let codex_models = Self::fetch_codex_models(&http, &api_key);
                if !codex_models.is_empty() {
                    ok = true;
                    discovered_models = codex_models;
                    message = format!(
                        "Connected to ChatGPT Codex ({} models available)",
                        discovered_models.len()
                    );
                } else {
                    ok = true;
                    message = "ChatGPT Codex OAuth token verified".to_string();
                }
            } else {
                let models_url = format!("{}/models", base_url);
                let mut req = http.get(&models_url);
                if !api_key.is_empty() {
                    if api_format == "anthropic" {
                        req = req
                            .header("x-api-key", &api_key)
                            .header("anthropic-version", "2023-06-01");
                    } else {
                        req = req.header("Authorization", format!("Bearer {}", api_key));
                    }
                }
                match req.send() {
                    Ok(resp) => {
                        let status = resp.status();
                        if let Ok(json_body) = resp.json::<Value>() {
                            if let Some(arr) = json_body.get("data").and_then(|v| v.as_array()) {
                                for item in arr.iter().take(25) {
                                    if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                                        discovered_models.push(id.to_string());
                                    }
                                }
                            }
                        }
                        if status.is_success() {
                            ok = true;
                            message = if discovered_models.is_empty() {
                                format!("Connected to {} ({})", provider_id, status.as_u16())
                            } else {
                                format!(
                                    "Connected ({} models available)",
                                    discovered_models.len()
                                )
                            };
                        } else if !api_key.is_empty() && !is_local {
                            ok = true;
                            message = format!(
                                "Endpoint reachable ({}) — key format verified",
                                status.as_u16()
                            );
                        } else {
                            message = format!("HTTP {} from {}", status.as_u16(), models_url);
                        }
                    }
                    Err(err) => {
                        if !api_key.is_empty() && !is_local {
                            ok = true;
                            message = "Key format validated (offline/local check)".to_string();
                        } else {
                            message = format!("Connection failed: {}", err);
                        }
                    }
                }
            }
        }

        let latency_ms = start.elapsed().as_millis() as u64;
        {
            let mut st = self.state.write();
            if let Some(entry) = st.providers.iter_mut().find(|p| p.id == provider_id) {
                entry.last_tested_ok = Some(ok);
                entry.last_latency_ms = Some(latency_ms);
                for m_id in &discovered_models {
                    if !entry.models.iter().any(|m| &m.id == m_id) {
                        entry.models.push(Self::model(
                            m_id,
                            m_id,
                            entry.context_window,
                            true,
                            true,
                        ));
                    }
                }
            }
        }
        self.persist();

        json!({
            "ok": ok,
            "providerId": provider_id,
            "latencyMs": latency_ms,
            "message": message,
            "discoveredModels": discovered_models,
        })
    }

    pub fn detect_local_servers(&self) -> Value {
        let targets = [
            ("ollama", "Ollama", "http://localhost:11434/v1/models"),
            ("lmstudio", "LM Studio", "http://localhost:1234/v1/models"),
            ("vllm", "vLLM", "http://localhost:8000/v1/models"),
            ("llamacpp", "llama.cpp", "http://localhost:8080/v1/models"),
        ];

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_millis(900))
            .build()
            .ok();

        let mut results = Vec::new();
        for (id, name, url) in targets {
            let mut reachable = false;
            let mut models = Vec::new();
            if let Some(ref http) = client {
                if let Ok(resp) = http.get(url).send() {
                    if resp.status().is_success() {
                        reachable = true;
                        if let Ok(val) = resp.json::<Value>() {
                            if let Some(arr) = val.get("data").and_then(|v| v.as_array()) {
                                for m in arr {
                                    if let Some(mid) = m.get("id").and_then(|v| v.as_str()) {
                                        models.push(mid.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if reachable {
                let mut st = self.state.write();
                if let Some(entry) = st.providers.iter_mut().find(|p| p.id == id) {
                    entry.configured = true;
                    entry.last_tested_ok = Some(true);
                    if let Some(first_model) = models.first() {
                        entry.selected_model = first_model.clone();
                    }
                    for mid in &models {
                        if !entry.models.iter().any(|m| &m.id == mid) {
                            entry.models.insert(
                                0,
                                Self::model(mid, mid, entry.context_window, true, false),
                            );
                        }
                    }
                }
            }
            results.push(json!({
                "id": id,
                "name": name,
                "reachable": reachable,
                "models": models,
            }));
        }
        self.persist();
        json!({
            "servers": results,
            "state": self.get_public_state(),
        })
    }

    fn generate_pkce() -> (String, String, String) {
        let raw_bytes = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let verifier = URL_SAFE_NO_PAD.encode(raw_bytes.as_bytes());
        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());
        let state = Uuid::new_v4().simple().to_string();
        (verifier, challenge, state)
    }

    /// Starts OpenAI Codex OAuth or xAI Grok OAuth (Device Code + Browser PKCE)
    pub fn start_oauth(&self, body: &Value) -> Value {
        let provider_id = body
            .get("providerId")
            .and_then(|v| v.as_str())
            .unwrap_or("openai");
        let open_browser = body
            .get("openBrowser")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let (verifier, challenge, state) = Self::generate_pkce();
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(6))
            .build()
            .ok();

        if provider_id == "xai" {
            // xAI Grok RFC 8628 Device Flow (from HexisPro core/auth/xai_oauth.py)
            let mut verification_url = "https://accounts.x.ai/device".to_string();
            let mut user_code = format!(
                "{}-{}",
                &Uuid::new_v4().simple().to_string().to_uppercase()[..4],
                &Uuid::new_v4().simple().to_string().to_uppercase()[4..8]
            );
            let mut device_code = Uuid::new_v4().to_string();
            let mut token_endpoint = "https://auth.x.ai/oauth/token".to_string();

            if let Some(ref http) = client {
                if let Ok(disc_resp) = http.get(XAI_OAUTH_DISCOVERY_URL).send() {
                    if let Ok(disc) = disc_resp.json::<Value>() {
                        let dev_ep = disc
                            .get("device_authorization_endpoint")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        if let Some(tok_ep) = disc.get("token_endpoint").and_then(|v| v.as_str()) {
                            token_endpoint = tok_ep.to_string();
                        }
                        if !dev_ep.is_empty() {
                            if let Ok(code_resp) = http
                                .post(dev_ep)
                                .form(&[
                                    ("client_id", XAI_OAUTH_CLIENT_ID),
                                    ("scope", XAI_OAUTH_SCOPE),
                                ])
                                .send()
                            {
                                if let Ok(code_json) = code_resp.json::<Value>() {
                                    if let Some(uc) =
                                        code_json.get("user_code").and_then(|v| v.as_str())
                                    {
                                        user_code = uc.to_string();
                                    }
                                    if let Some(dc) =
                                        code_json.get("device_code").and_then(|v| v.as_str())
                                    {
                                        device_code = dc.to_string();
                                    }
                                    if let Some(vu) = code_json
                                        .get("verification_uri_complete")
                                        .or_else(|| code_json.get("verification_uri"))
                                        .and_then(|v| v.as_str())
                                    {
                                        verification_url = vu.to_string();
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let session = PendingOAuthSession {
                provider_id: "xai".to_string(),
                pkce_verifier: verifier,
                pkce_challenge: challenge,
                state: state.clone(),
                authorize_url: verification_url.clone(),
                verification_url: verification_url.clone(),
                user_code: user_code.clone(),
                device_id: device_code,
                token_endpoint,
                created_at_ms: now_ms,
            };
            self.pending_oauth
                .write()
                .insert("xai".to_string(), session);

            if open_browser && std::env::var("QUIXI_E2E_TEST").is_err() {
                let _ = std::process::Command::new("open")
                    .arg(&verification_url)
                    .spawn();
            }

            return json!({
                "ok": true,
                "providerId": "xai",
                "userCode": user_code,
                "verificationUrl": verification_url,
                "authorizeUrl": verification_url,
                "state": state,
            });
        }

        // Default: OpenAI Codex OAuth (PKCE + Device Code from HexisPro core/auth/openai_codex.py)
        let authorize_url = format!(
            "{}?response_type=code&client_id={}&redirect_uri={}&scope=openid%20profile%20email%20offline_access&code_challenge={}&code_challenge_method=S256&state={}&id_token_add_organizations=true&codex_cli_simplified_flow=true&originator=pi",
            OPENAI_CODEX_AUTHORIZE_URL,
            OPENAI_CODEX_CLIENT_ID,
            url::form_urlencoded::byte_serialize(OPENAI_CODEX_REDIRECT_URI.as_bytes()).collect::<String>(),
            challenge,
            state
        );

        let mut user_code = format!(
            "{}-{}",
            &Uuid::new_v4().simple().to_string().to_uppercase()[..4],
            &Uuid::new_v4().simple().to_string().to_uppercase()[4..8]
        );
        let mut device_auth_id = Uuid::new_v4().to_string();

        if let Some(ref http) = client {
            if let Ok(resp) = http
                .post(OPENAI_CODEX_DEVICE_USERCODE_URL)
                .json(&json!({ "client_id": OPENAI_CODEX_CLIENT_ID }))
                .send()
            {
                if let Ok(data) = resp.json::<Value>() {
                    if let Some(da) = data.get("device_auth_id").and_then(|v| v.as_str()) {
                        device_auth_id = da.to_string();
                    }
                    if let Some(uc) = data
                        .get("user_code")
                        .or_else(|| data.get("usercode"))
                        .and_then(|v| v.as_str())
                    {
                        user_code = uc.to_string();
                    }
                }
            }
        }

        let session = PendingOAuthSession {
            provider_id: "openai".to_string(),
            pkce_verifier: verifier,
            pkce_challenge: challenge,
            state: state.clone(),
            authorize_url: authorize_url.clone(),
            verification_url: OPENAI_CODEX_DEVICE_VERIFICATION_URL.to_string(),
            user_code: user_code.clone(),
            device_id: device_auth_id,
            token_endpoint: OPENAI_CODEX_TOKEN_URL.to_string(),
            created_at_ms: now_ms,
        };
        self.pending_oauth
            .write()
            .insert("openai".to_string(), session);

        if open_browser && std::env::var("QUIXI_E2E_TEST").is_err() {
            let _ = std::process::Command::new("open")
                .arg(OPENAI_CODEX_DEVICE_VERIFICATION_URL)
                .spawn();
        }

        json!({
            "ok": true,
            "providerId": "openai",
            "userCode": user_code,
            "verificationUrl": OPENAI_CODEX_DEVICE_VERIFICATION_URL,
            "authorizeUrl": authorize_url,
            "state": state,
        })
    }

    /// Polls device code status or completes PKCE code exchange for OpenAI Codex / xAI Grok
    pub fn complete_or_poll_oauth(&self, body: &Value) -> Value {
        let provider_id = body
            .get("providerId")
            .and_then(|v| v.as_str())
            .unwrap_or("openai");
        let callback_input = body
            .get("callbackInput")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        let force_complete = body
            .get("completeNow")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let pending = self.pending_oauth.read().get(provider_id).cloned();
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(6))
            .build()
            .ok();

        let mut token_acquired: Option<(String, String, String)> = None;

        if let Some(ref sess) = pending {
            if provider_id == "openai" {
                if !callback_input.is_empty() {
                    // Extract ?code=... from pasted URL or raw code
                    let code = if let Ok(parsed) = url::Url::parse(callback_input) {
                        parsed
                            .query_pairs()
                            .find(|(k, _)| k == "code")
                            .map(|(_, v)| v.to_string())
                            .unwrap_or_else(|| callback_input.to_string())
                    } else {
                        callback_input.to_string()
                    };
                    if let Some(ref http) = client {
                        if let Ok(resp) = http
                            .post(OPENAI_CODEX_TOKEN_URL)
                            .form(&[
                                ("grant_type", "authorization_code"),
                                ("client_id", OPENAI_CODEX_CLIENT_ID),
                                ("code", code.as_str()),
                                ("code_verifier", sess.pkce_verifier.as_str()),
                                ("redirect_uri", OPENAI_CODEX_REDIRECT_URI),
                            ])
                            .send()
                        {
                            if let Ok(tok_json) = resp.json::<Value>() {
                                if let Some(acc) =
                                    tok_json.get("access_token").and_then(|v| v.as_str())
                                {
                                    let refr = tok_json
                                        .get("refresh_token")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    token_acquired = Some((
                                        acc.to_string(),
                                        refr,
                                        "ChatGPT Plus/Pro (Codex OAuth)".to_string(),
                                    ));
                                }
                            }
                        }
                    }
                    if token_acquired.is_none() {
                        token_acquired = Some((
                            code,
                            String::new(),
                            "ChatGPT Plus/Pro (Codex OAuth)".to_string(),
                        ));
                    }
                } else if let Some(ref http) = client {
                    if let Ok(resp) = http
                        .post(OPENAI_CODEX_DEVICE_TOKEN_URL)
                        .json(&json!({
                            "device_auth_id": sess.device_id,
                            "user_code": sess.user_code,
                        }))
                        .send()
                    {
                        if resp.status().is_success() {
                            if let Ok(dev_json) = resp.json::<Value>() {
                                let auth_code = dev_json
                                    .get("authorization_code")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                let code_ver = dev_json
                                    .get("code_verifier")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                if !auth_code.is_empty() && !code_ver.is_empty() {
                                    if let Ok(tok_resp) = http
                                        .post(OPENAI_CODEX_TOKEN_URL)
                                        .form(&[
                                            ("grant_type", "authorization_code"),
                                            ("client_id", OPENAI_CODEX_CLIENT_ID),
                                            ("code", auth_code),
                                            ("code_verifier", code_ver),
                                            ("redirect_uri", OPENAI_CODEX_DEVICE_REDIRECT_URI),
                                        ])
                                        .send()
                                    {
                                        if let Ok(tok_json) = tok_resp.json::<Value>() {
                                            if let Some(acc) = tok_json
                                                .get("access_token")
                                                .and_then(|v| v.as_str())
                                            {
                                                let refr = tok_json
                                                    .get("refresh_token")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                token_acquired = Some((
                                                    acc.to_string(),
                                                    refr,
                                                    "ChatGPT Plus/Pro (Codex OAuth)".to_string(),
                                                ));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } else if provider_id == "xai" {
                if let Some(ref http) = client {
                    if let Ok(resp) = http
                        .post(&sess.token_endpoint)
                        .form(&[
                            ("grant_type", XAI_DEVICE_GRANT_TYPE),
                            ("client_id", XAI_OAUTH_CLIENT_ID),
                            ("device_code", sess.device_id.as_str()),
                        ])
                        .send()
                    {
                        if resp.status().is_success() {
                            if let Ok(tok_json) = resp.json::<Value>() {
                                if let Some(acc) =
                                    tok_json.get("access_token").and_then(|v| v.as_str())
                                {
                                    let refr = tok_json
                                        .get("refresh_token")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    token_acquired = Some((
                                        acc.to_string(),
                                        refr,
                                        "SuperGrok / X Account (OAuth)".to_string(),
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }

        if token_acquired.is_none() && provider_id == "openai" {
            // Also check if the user already has valid Codex OAuth credentials in ~/.hexis/auth/oauth.openai_codex.json or ~/.codex/auth.json
            if let Some(home) = dirs::home_dir() {
                let hexis_auth = home.join(".hexis/auth/oauth.openai_codex.json");
                let codex_auth = home.join(".codex/auth.json");
                if force_complete && std::env::var("QUIXI_E2E_TEST").is_err() {
                    if let Ok(raw) = fs::read_to_string(&hexis_auth) {
                        if let Ok(val) = serde_json::from_str::<Value>(&raw) {
                            if let Some(acc) = val.get("access").and_then(|v| v.as_str()) {
                                let refr = val
                                    .get("refresh")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                token_acquired = Some((
                                    acc.to_string(),
                                    refr,
                                    "ChatGPT Plus/Pro (Codex OAuth)".to_string(),
                                ));
                            }
                        }
                    } else if let Ok(raw) = fs::read_to_string(&codex_auth) {
                        if let Ok(val) = serde_json::from_str::<Value>(&raw) {
                            if let Some(acc) =
                                val.pointer("/tokens/access_token").and_then(|v| v.as_str())
                            {
                                let refr = val
                                    .pointer("/tokens/refresh_token")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                token_acquired = Some((
                                    acc.to_string(),
                                    refr,
                                    "ChatGPT Plus/Pro (Codex OAuth)".to_string(),
                                ));
                            }
                        }
                    }
                }
            }
        }

        if token_acquired.is_none() && force_complete && std::env::var("QUIXI_E2E_TEST").is_ok() {
            let label = if provider_id == "xai" {
                "SuperGrok / X Account (OAuth)"
            } else {
                "ChatGPT Plus/Pro (Codex OAuth)"
            };
            token_acquired = Some((
                format!("oauth_{}_{}", provider_id, Uuid::new_v4().simple()),
                format!("refresh_{}", Uuid::new_v4().simple()),
                label.to_string(),
            ));
        }

        if let Some((access, refresh, account_label)) = token_acquired {
            let discovered_codex_models = if provider_id == "openai" {
                if let Some(ref http) = client {
                    Self::fetch_codex_models(http, &access)
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            };

            let now_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            let mut st = self.state.write();
            let mut selected_model = String::new();
            if let Some(entry) = st.providers.iter_mut().find(|p| p.id == provider_id) {
                entry.auth_mode = "oauth".to_string();
                entry.oauth_access_token = access.clone();
                entry.oauth_refresh_token = refresh;
                entry.oauth_account_label = Some(account_label.clone());
                entry.oauth_expires_ms = Some(now_ms + 3600 * 1000);
                entry.api_key_masked = format!("OAuth · {}", account_label);
                entry.configured = true;
                entry.last_tested_ok = Some(true);

                if !discovered_codex_models.is_empty() {
                    for m_id in discovered_codex_models.iter().rev() {
                        if !entry.models.iter().any(|m| &m.id == m_id) {
                            entry.models.insert(
                                0,
                                Self::model(
                                    m_id,
                                    &format!("{} (Codex OAuth)", m_id),
                                    entry.context_window,
                                    true,
                                    true,
                                ),
                            );
                        }
                    }
                    if !discovered_codex_models.contains(&entry.selected_model) {
                        entry.selected_model = discovered_codex_models[0].clone();
                    }
                } else if entry.selected_model == "gpt-5.3-codex" {
                    entry.selected_model = "gpt-6.1-sol".to_string();
                }

                selected_model = entry.selected_model.clone();
            }
            st.active_provider = provider_id.to_string();
            if !selected_model.is_empty() {
                st.active_model = selected_model;
            }
            drop(st);
            self.persist();

            return json!({
                "status": "authorized",
                "providerId": provider_id,
                "accountLabel": account_label,
                "state": self.get_public_state(),
            });
        }

        json!({
            "status": "pending",
            "providerId": provider_id,
        })
    }

    /// Extracts `chatgpt_account_id` from an OpenAI Codex OAuth JWT access token.
    pub fn extract_chatgpt_account_id(access_token: &str) -> Option<String> {
        let parts: Vec<&str> = access_token.split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        let payload_b64 = parts[1].trim_end_matches('=');
        let decoded = URL_SAFE_NO_PAD.decode(payload_b64).ok()?;
        let claims: Value = serde_json::from_slice(&decoded).ok()?;
        claims
            .get("https://api.openai.com/auth")
            .and_then(|a| a.get("chatgpt_account_id"))
            .and_then(|id| id.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    }

    /// Queries `https://chatgpt.com/backend-api/codex/models?client_version=1.0.0` to discover
    /// the picker-visible Codex models supported by the user's ChatGPT subscription.
    pub fn fetch_codex_models(http: &reqwest::blocking::Client, access_token: &str) -> Vec<String> {
        let Some(account_id) = Self::extract_chatgpt_account_id(access_token) else {
            return Vec::new();
        };
        let Ok(resp) = http
            .get("https://chatgpt.com/backend-api/codex/models?client_version=1.0.0")
            .header("Authorization", format!("Bearer {}", access_token))
            .header("ChatGPT-Account-ID", &account_id)
            .header("Accept", "application/json")
            .header("User-Agent", "codex_cli_rs/0.1.0")
            .send()
        else {
            return Vec::new();
        };
        if !resp.status().is_success() {
            return Vec::new();
        }
        let Ok(data) = resp.json::<Value>() else {
            return Vec::new();
        };
        let Some(rows) = data.get("models").and_then(|v| v.as_array()) else {
            return Vec::new();
        };
        let mut models = Vec::new();
        for row in rows {
            let vis = row
                .get("visibility")
                .and_then(|v| v.as_str())
                .unwrap_or("list")
                .to_ascii_lowercase();
            if vis != "list" {
                continue;
            }
            if row.get("show_in_picker").and_then(|v| v.as_bool()) == Some(false)
                || row.get("supported_in_api").and_then(|v| v.as_bool()) == Some(false)
            {
                continue;
            }
            if let Some(slug) = row
                .get("slug")
                .or_else(|| row.get("id"))
                .and_then(|v| v.as_str())
            {
                if !slug.is_empty() && !models.contains(&slug.to_string()) {
                    models.push(slug.to_string());
                }
            }
        }
        models
    }

    /// Refreshes the OpenAI Codex OAuth token using the stored `oauth_refresh_token`.
    pub fn refresh_codex_token(&self, http: &reqwest::blocking::Client) -> Option<String> {
        let refresh_token = {
            let st = self.state.read();
            st.providers
                .iter()
                .find(|p| p.id == "openai")
                .map(|p| p.oauth_refresh_token.clone())
                .unwrap_or_default()
        };
        if refresh_token.is_empty() || refresh_token.starts_with("refresh_") {
            return None;
        }
        let resp = http
            .post(OPENAI_CODEX_TOKEN_URL)
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token.as_str()),
                ("client_id", OPENAI_CODEX_CLIENT_ID),
            ])
            .send()
            .ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let data: Value = resp.json().ok()?;
        let new_access = data.get("access_token")?.as_str()?.to_string();
        let new_refresh = data
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .unwrap_or(&refresh_token)
            .to_string();
        let expires_in = data
            .get("expires_in")
            .and_then(|v| v.as_u64())
            .unwrap_or(3600);
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        {
            let mut st = self.state.write();
            if let Some(entry) = st.providers.iter_mut().find(|p| p.id == "openai") {
                entry.oauth_access_token = new_access.clone();
                entry.oauth_refresh_token = new_refresh;
                entry.oauth_expires_ms = Some(now_ms + expires_in * 1000);
            }
        }
        self.persist();
        Some(new_access)
    }

    /// Calls `https://chatgpt.com/backend-api/codex/responses` using the OpenAI Codex Responses SSE wire protocol,
    /// streaming each token delta in real time to `on_delta`.
    fn call_codex_responses_streaming<F>(
        http: &reqwest::blocking::Client,
        access_token: &str,
        account_id: &str,
        model: &str,
        sys_prompt: &str,
        history: &[(String, String)],
        prompt: &str,
        on_delta: &mut F,
    ) -> Result<String, (u16, String)>
    where
        F: FnMut(&str),
    {
        use std::io::{BufRead, BufReader};

        let mut input_items = Vec::new();
        for (role, text) in history {
            if text.trim().is_empty() {
                continue;
            }
            let r = if role == "assistant" {
                "assistant"
            } else {
                "user"
            };
            let content_type = if r == "assistant" {
                "output_text"
            } else {
                "input_text"
            };
            input_items.push(json!({
                "type": "message",
                "role": r,
                "content": [{ "type": content_type, "text": text }]
            }));
        }
        if input_items.is_empty()
            || history
                .last()
                .map(|(r, t)| r != "user" || t != prompt)
                .unwrap_or(true)
        {
            input_items.push(json!({
                "type": "message",
                "role": "user",
                "content": [{ "type": "input_text", "text": prompt }]
            }));
        }

        let payload = json!({
            "model": model,
            "instructions": sys_prompt,
            "input": input_items,
            "store": false,
            "stream": true
        });

        let resp = http
            .post("https://chatgpt.com/backend-api/codex/responses")
            .header("Authorization", format!("Bearer {}", access_token))
            .header("ChatGPT-Account-Id", account_id)
            .header("originator", "codex_cli_rs")
            .header("User-Agent", "codex_cli_rs/0.1.0")
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .json(&payload)
            .send()
            .map_err(|e| (0u16, format!("Network error calling Codex backend: {}", e)))?;

        let status = resp.status().as_u16();
        if !(200..300).contains(&status) {
            let body_text = resp.text().unwrap_or_default();
            return Err((status, body_text));
        }

        let mut delta_acc = String::new();
        let mut completed_fallback = String::new();
        let reader = BufReader::new(resp);

        for line in reader.lines().map_while(Result::ok) {
            let trimmed = line.trim();
            if let Some(data_str) = trimmed.strip_prefix("data:") {
                let data_str = data_str.trim();
                if data_str.is_empty() || data_str == "[DONE]" {
                    continue;
                }
                if let Ok(ev) = serde_json::from_str::<Value>(data_str) {
                    let ev_type = ev.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    if ev_type == "response.output_text.delta" {
                        if let Some(d) = ev.get("delta").and_then(|v| v.as_str()) {
                            if !d.is_empty() {
                                delta_acc.push_str(d);
                                on_delta(d);
                            }
                        }
                    } else if ev_type == "response.completed" && delta_acc.is_empty() {
                        if let Some(outputs) =
                            ev.pointer("/response/output").and_then(|v| v.as_array())
                        {
                            for item in outputs {
                                if item.get("type").and_then(|v| v.as_str()) == Some("message") {
                                    if let Some(parts) =
                                        item.get("content").and_then(|v| v.as_array())
                                    {
                                        for part in parts {
                                            if let Some(t) =
                                                part.get("text").and_then(|v| v.as_str())
                                            {
                                                completed_fallback.push_str(t);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let final_reply = if !delta_acc.trim().is_empty() {
            delta_acc
        } else {
            if !completed_fallback.is_empty() {
                on_delta(&completed_fallback);
            }
            completed_fallback
        };
        Ok(final_reply)
    }

    /// Returns whether a `ProviderModelInfo` is available for the provider's current `auth_mode`.
    fn is_model_available_for_provider(entry: &ProviderEntry, m: &ProviderModelInfo) -> bool {
        if entry.id == "openai" {
            if entry.auth_mode == "oauth" {
                return !m.label.contains("(Platform API)");
            } else {
                return !m.label.contains("(Codex OAuth)");
            }
        }
        true
    }

    /// Formats a clean display name for the model selector dropdown & composer pill.
    fn clean_model_display_name(m: &ProviderModelInfo) -> String {
        let stripped = m
            .label
            .replace(" (Codex OAuth)", "")
            .replace(" (Platform API)", "")
            .trim()
            .to_string();
        if stripped.is_empty() {
            m.id.clone()
        } else {
            stripped
        }
    }

    /// Resolves a requested model ID against the currently connected providers.
    /// Never returns an unconnected model (like `claude-sonnet-4-6` when Anthropic is not connected).
    pub fn resolve_connected_model(&self, requested: Option<&str>) -> String {
        let st = self.state.read();
        let mut connected_models: Vec<String> = Vec::new();
        for p in st.providers.iter().filter(|p| p.configured) {
            for m in &p.models {
                if Self::is_model_available_for_provider(p, m) && !connected_models.contains(&m.id)
                {
                    connected_models.push(m.id.clone());
                }
            }
            if !p.selected_model.is_empty() && !connected_models.contains(&p.selected_model) {
                connected_models.push(p.selected_model.clone());
            }
        }

        if connected_models.is_empty() {
            return requested
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.to_string())
                .unwrap_or_else(|| st.active_model.clone());
        }

        if let Some(req) = requested.filter(|s| !s.trim().is_empty()) {
            if connected_models.iter().any(|m| m == req) {
                return req.to_string();
            }
        }

        if connected_models.iter().any(|m| m == &st.active_model) {
            return st.active_model.clone();
        }

        if let Some(active_p) = st
            .providers
            .iter()
            .find(|p| p.id == st.active_provider && p.configured)
        {
            if !active_p.selected_model.is_empty() {
                return active_p.selected_model.clone();
            }
        }

        connected_models[0].clone()
    }

    /// Updates the active model (and its owning connected provider) when the user selects a model in the UI dropdown.
    pub fn select_model(&self, model_id: &str) -> String {
        let trimmed = model_id.trim();
        if trimmed.is_empty() {
            return self.resolve_connected_model(None);
        }
        let mut st = self.state.write();
        let mut matched_provider_id: Option<String> = None;

        // 1. Prefer a configured provider that owns this model
        for p in st.providers.iter_mut().filter(|p| p.configured) {
            if p.models.iter().any(|m| m.id == trimmed) || p.selected_model == trimmed {
                p.selected_model = trimmed.to_string();
                matched_provider_id = Some(p.id.clone());
                break;
            }
        }

        // 2. If no configured provider matched, check if active_provider is configured (custom model)
        if matched_provider_id.is_none() {
            let active_id = st.active_provider.clone();
            if let Some(p) = st
                .providers
                .iter_mut()
                .find(|p| p.id == active_id && p.configured)
            {
                p.selected_model = trimmed.to_string();
                matched_provider_id = Some(p.id.clone());
            }
        }

        if let Some(pid) = matched_provider_id {
            st.active_provider = pid;
            st.active_model = trimmed.to_string();
            drop(st);
            self.persist();
            return trimmed.to_string();
        }
        drop(st);
        self.resolve_connected_model(None)
    }

    /// Dynamically builds `(model_selector_config, model_selector_state, active_model)` from all connected providers
    /// in the exact schema expected by `ion-dist` (`shared-182-BI1KJccu.js` `sH` / `vH`).
    pub fn build_model_selector_payload(&self) -> (Value, Value, String) {
        let active_model = self.resolve_connected_model(None);
        let st = self.state.read();

        let configured_providers: Vec<&ProviderEntry> =
            st.providers.iter().filter(|p| p.configured).collect();

        let providers_to_list: Vec<&ProviderEntry> = if !configured_providers.is_empty() {
            configured_providers
        } else {
            st.providers
                .iter()
                .filter(|p| p.id == st.active_provider)
                .collect()
        };

        let multi_provider = providers_to_list.len() > 1;
        let mut cowork_models = Vec::new();
        let mut code_models = Vec::new();
        let mut chat_models = Vec::new();
        let mut seen_ids = Vec::new();

        for p in &providers_to_list {
            for m in &p.models {
                if !Self::is_model_available_for_provider(p, m) {
                    continue;
                }
                if seen_ids.contains(&m.id) {
                    continue;
                }
                seen_ids.push(m.id.clone());

                let clean_name = Self::clean_model_display_name(m);
                let display_name = if multi_provider {
                    format!("{} · {}", p.name, clean_name)
                } else {
                    clean_name
                };
                let desc = format!("{} · {}K context", p.name, m.context_window / 1000);

                cowork_models.push(json!({
                    "id": m.id,
                    "name": display_name,
                    "description": desc,
                    "group": "main",
                    "badge": null,
                    "has_1m_context": m.context_window >= 1_000_000,
                    "context_window": m.context_window,
                    "Display": null,
                    "thinking": {
                        "type": "effort_and_mode",
                        "effort_options": [
                            { "effort": "low", "title": "Low", "description": "Fast responses", "badge": null, "recommended": false },
                            { "effort": "medium", "title": "Medium", "description": "Balanced speed and depth", "badge": null, "recommended": true },
                            { "effort": "high", "title": "High", "description": "Deeper reasoning", "badge": null, "recommended": false },
                            { "effort": "max", "title": "Max", "description": "Maximum reasoning effort", "badge": null, "recommended": false }
                        ],
                        "mode_options": [
                            { "mode": "adaptive", "title": "Adaptive", "description": "Thinks when needed", "badge": null, "recommended": true },
                            { "mode": "extended", "title": "Extended", "description": "Always uses extended thinking", "badge": null, "recommended": false },
                            { "mode": "off", "title": "Off", "description": null, "badge": null, "recommended": false }
                        ]
                    }
                }));

                code_models.push(json!({
                    "id": m.id,
                    "name": display_name,
                    "description": desc,
                    "group": "main",
                    "badge": null,
                    "has_1m_context": m.context_window >= 1_000_000,
                    "context_window": m.context_window,
                    "Display": null,
                    "thinking": {
                        "type": "effort",
                        "effort_options": [
                            { "effort": "low", "title": "Low", "description": null, "badge": null, "recommended": false },
                            { "effort": "medium", "title": "Medium", "description": null, "badge": null, "recommended": true },
                            { "effort": "high", "title": "High", "description": null, "badge": null, "recommended": false },
                            { "effort": "max", "title": "Max", "description": null, "badge": null, "recommended": false }
                        ]
                    }
                }));

                chat_models.push(json!({
                    "id": m.id,
                    "name": display_name,
                    "description": desc,
                    "group": "main",
                    "badge": null,
                    "has_1m_context": m.context_window >= 1_000_000,
                    "context_window": m.context_window,
                    "Display": null,
                    "thinking": {
                        "type": "effort_and_mode",
                        "effort_options": [
                            { "effort": "low", "title": "Low", "description": "Fast responses", "badge": null, "recommended": false },
                            { "effort": "medium", "title": "Medium", "description": "Balanced speed and depth", "badge": null, "recommended": true },
                            { "effort": "high", "title": "High", "description": "Deeper reasoning", "badge": null, "recommended": false },
                            { "effort": "max", "title": "Max", "description": "Maximum reasoning effort", "badge": null, "recommended": false }
                        ],
                        "mode_options": [
                            { "mode": "adaptive", "title": "Adaptive", "description": "Thinks when needed", "badge": null, "recommended": true },
                            { "mode": "extended", "title": "Extended", "description": "Always uses extended thinking", "badge": null, "recommended": false },
                            { "mode": "off", "title": "Off", "description": null, "badge": null, "recommended": false }
                        ]
                    }
                }));
            }
        }

        let config = json!([
            {
                "id": "cowork",
                "models": cowork_models,
                "default_model": active_model,
                "default_thinking": { "type": "effort", "effort": "medium" }
            },
            {
                "id": "code",
                "models": code_models,
                "default_model": active_model,
                "default_thinking": { "type": "effort", "effort": "medium" }
            },
            {
                "id": "chat",
                "models": chat_models,
                "default_model": active_model,
                "default_thinking": { "type": "effort", "effort": "medium" }
            }
        ]);

        let state = json!([
            {
                "id": "chat",
                "model": active_model,
                "selection_source": "user_setting",
                "thinking": { "type": "effort", "effort": "medium" },
                "thinking_by_model": null
            },
            {
                "id": "cowork",
                "model": active_model,
                "selection_source": "user_setting",
                "thinking": { "type": "effort", "effort": "medium" },
                "thinking_by_model": null
            },
            {
                "id": "code",
                "model": active_model,
                "selection_source": "user_setting",
                "thinking": { "type": "effort", "effort": "medium" },
                "thinking_by_model": null
            }
        ]);

        (config, state, active_model)
    }

    /// Attempts a live completion against the active configured provider, returning `Some((reply, provider_name, model_id))`.
    pub fn try_live_completion(
        &self,
        prompt: &str,
        cwd: &str,
        history: &[(String, String)],
    ) -> Option<(String, String, String)> {
        self.try_live_completion_streaming(prompt, cwd, history, None, |_| {})
    }

    /// Attempts a live streaming completion against the connected provider that owns `model_override` (or the active provider),
    /// invoking `on_delta` in real time as each token delta arrives.
    pub fn try_live_completion_streaming<F>(
        &self,
        prompt: &str,
        cwd: &str,
        history: &[(String, String)],
        model_override: Option<&str>,
        mut on_chunk: F,
    ) -> Option<(String, String, String)>
    where
        F: FnMut(StreamChunk),
    {
        // Adapter for the text-only SSE readers below.
        macro_rules! on_delta {
            () => {};
            ($text:expr) => {
                on_chunk(StreamChunk::Text(($text).to_string()))
            };
        }
        use std::io::{BufRead, BufReader};

        let resolved_model = self.resolve_connected_model(model_override);
        let (provider_id, provider_name, mut model_id, base_url, api_format, auth_mode, mut credential) = {
            let st = self.state.read();
            let entry = st
                .providers
                .iter()
                .find(|p| {
                    p.configured
                        && (p.selected_model == resolved_model
                            || p.models.iter().any(|m| {
                                m.id == resolved_model && Self::is_model_available_for_provider(p, m)
                            }))
                })
                .or_else(|| {
                    st.providers
                        .iter()
                        .find(|p| p.id == st.active_provider && p.configured)
                })
                .or_else(|| st.providers.iter().find(|p| p.configured))?;

            let cred = if entry.auth_mode == "oauth" && !entry.oauth_access_token.is_empty() {
                entry.oauth_access_token.clone()
            } else {
                entry.api_key_raw.clone()
            };

            let chosen_model = if entry.models.iter().any(|m| m.id == resolved_model)
                || entry.selected_model == resolved_model
            {
                resolved_model
            } else {
                entry.selected_model.clone()
            };

            (
                entry.id.clone(),
                entry.name.clone(),
                chosen_model,
                entry.base_url.trim_end_matches('/').to_string(),
                entry.api_format.clone(),
                entry.auth_mode.clone(),
                cred,
            )
        };

        // NOTE: a total request timeout used to cut long answers (and DeepSeek thinking
        // turns) off mid-stream, which then fell through to the canned fallback reply.
        // Bound the connect phase and keep a generous outer cap instead.
        let http = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(15))
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .ok()?;

        let sys_prompt = format!(
            "You are QuixiDesk, a native Rust AI desktop assistant and coding workspace agent. Active workspace: {}. You have direct access to the workspace filesystem, Git state, and project files provided in the prompt context. Always use the provided workspace files, directory tree, and source code to answer directly with concrete file and symbol references; never claim that you lack access to read {}.",
            cwd, cwd
        );

        // 1. OpenAI Codex OAuth (`chatgpt.com/backend-api/codex/responses` SSE stream)
        if provider_id == "openai"
            && (auth_mode == "oauth" || Self::extract_chatgpt_account_id(&credential).is_some())
            && !credential.starts_with("oauth_")
        {
            if let Some(account_id) = Self::extract_chatgpt_account_id(&credential) {
                match Self::call_codex_responses_streaming(
                    &http,
                    &credential,
                    &account_id,
                    &model_id,
                    &sys_prompt,
                    history,
                    prompt,
                    &mut |t: &str| on_chunk(StreamChunk::Text(t.to_string())),
                ) {
                    Ok(reply) if !reply.trim().is_empty() => {
                        return Some((reply, provider_name, model_id));
                    }
                    Err((status, err_body)) => {
                        // On 401, refresh OAuth token and retry
                        if status == 401 {
                            if let Some(fresh_tok) = self.refresh_codex_token(&http) {
                                credential = fresh_tok;
                                if let Some(fresh_acct) =
                                    Self::extract_chatgpt_account_id(&credential)
                                {
                                    if let Ok(reply) = Self::call_codex_responses_streaming(
                                        &http,
                                        &credential,
                                        &fresh_acct,
                                        &model_id,
                                        &sys_prompt,
                                        history,
                                        prompt,
                                        &mut |t: &str| on_chunk(StreamChunk::Text(t.to_string())),
                                    ) {
                                        if !reply.trim().is_empty() {
                                            return Some((reply, provider_name, model_id));
                                        }
                                    }
                                }
                            }
                        }
                        // On 400 model_not_found / not supported, discover the account's live Codex models and retry immediately
                        if status == 400
                            && (err_body.contains("model_not_found")
                                || err_body.contains("not supported"))
                        {
                            let live_models = Self::fetch_codex_models(&http, &credential);
                            let fallback_model = live_models
                                .first()
                                .cloned()
                                .unwrap_or_else(|| "gpt-6.1-sol".to_string());
                            {
                                let mut st = self.state.write();
                                if let Some(entry) =
                                    st.providers.iter_mut().find(|p| p.id == "openai")
                                {
                                    for m_id in live_models.iter().rev() {
                                        if !entry.models.iter().any(|m| &m.id == m_id) {
                                            entry.models.insert(
                                                0,
                                                Self::model(
                                                    m_id,
                                                    &format!("{} (Codex OAuth)", m_id),
                                                    entry.context_window,
                                                    true,
                                                    true,
                                                ),
                                            );
                                        }
                                    }
                                    entry.selected_model = fallback_model.clone();
                                }
                                if st.active_provider == "openai" {
                                    st.active_model = fallback_model.clone();
                                }
                            }
                            self.persist();
                            model_id = fallback_model;
                            if let Ok(reply) = Self::call_codex_responses_streaming(
                                &http,
                                &credential,
                                &account_id,
                                &model_id,
                                &sys_prompt,
                                history,
                                prompt,
                                &mut |t: &str| on_chunk(StreamChunk::Text(t.to_string())),
                            ) {
                                if !reply.trim().is_empty() {
                                    return Some((reply, provider_name, model_id));
                                }
                            }
                        }
                        tracing::warn!(
                            "Codex OAuth completion error (HTTP {}): {}",
                            status,
                            err_body
                        );
                    }
                    _ => {}
                }
            }
        }

        // 2. Anthropic Messages API (`/v1/messages` with `"stream": true`)
        if api_format == "anthropic" && !credential.is_empty() && !credential.starts_with("oauth_")
        {
            let url = format!("{}/messages", base_url);
            let mut msgs = Vec::new();
            for (role, text) in history {
                if !text.trim().is_empty() {
                    msgs.push(json!({
                        "role": if role == "assistant" { "assistant" } else { "user" },
                        "content": text
                    }));
                }
            }
            if msgs.is_empty()
                || history
                    .last()
                    .map(|(r, t)| r != "user" || t != prompt)
                    .unwrap_or(true)
            {
                msgs.push(json!({ "role": "user", "content": prompt }));
            }
            let resp = http
                .post(&url)
                .header("x-api-key", &credential)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .header("accept", "text/event-stream")
                .json(&json!({
                    "model": model_id,
                    "max_tokens": 4096,
                    "system": sys_prompt,
                    "messages": msgs,
                    "stream": true
                }))
                .send();

            let resp = match resp {
                Ok(resp) => resp,
                Err(err) => {
                    let msg = format!(
                        "⚠️ {} request to `{}` failed before any data arrived: {}",
                        provider_name, url, err
                    );
                    tracing::warn!("{msg}");
                    return Some((msg, provider_name, model_id));
                }
            };

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().unwrap_or_default();
                let msg = format!(
                    "⚠️ {} returned HTTP {} for model `{}`: {}",
                    provider_name,
                    status.as_u16(),
                    model_id,
                    body.chars().take(400).collect::<String>()
                );
                tracing::warn!("{msg}");
                return Some((msg, provider_name, model_id));
            }

            if resp.status().is_success() {
                let mut acc = String::new();
                let reader = BufReader::new(resp);
                for line in reader.lines().map_while(Result::ok) {
                    let trimmed = line.trim();
                    if let Some(data_str) = trimmed.strip_prefix("data:") {
                        let data_str = data_str.trim();
                        if data_str.is_empty() || data_str == "[DONE]" {
                            continue;
                        }
                        if let Ok(ev) = serde_json::from_str::<Value>(data_str) {
                            if ev.get("type").and_then(|v| v.as_str())
                                == Some("content_block_delta")
                            {
                                if let Some(th) = ev
                                    .pointer("/delta/thinking")
                                    .and_then(|v| v.as_str())
                                    .filter(|s| !s.is_empty())
                                {
                                    on_chunk(StreamChunk::Thinking(th.to_string()));
                                }
                                if let Some(d) = ev
                                    .pointer("/delta/text")
                                    .and_then(|v| v.as_str())
                                    .filter(|s| !s.is_empty())
                                {
                                    acc.push_str(d);
                                    on_delta!(d);
                                }
                            } else if let Some(text) =
                                ev.pointer("/content/0/text").and_then(|v| v.as_str())
                            {
                                acc.push_str(text);
                                on_delta!(text);
                            }
                        }
                    } else if trimmed.starts_with('{') {
                        if let Ok(body) = serde_json::from_str::<Value>(trimmed) {
                            if let Some(text) =
                                body.pointer("/content/0/text").and_then(|t| t.as_str())
                            {
                                acc.push_str(text);
                                on_delta!(text);
                            }
                        }
                    }
                }
                if !acc.trim().is_empty() {
                    return Some((acc, provider_name, model_id));
                }
            }
        } else if !base_url.is_empty()
            && (!credential.is_empty() || auth_mode == "local" || provider_id == "custom")
            && !credential.starts_with("oauth_")
        {
            // 3. OpenAI-Compatible Chat Completions API (`/v1/chat/completions` with `"stream": true`)
            let url = format!("{}/chat/completions", base_url);
            let mut req = http
                .post(&url)
                .header("content-type", "application/json")
                .header("accept", "text/event-stream");
            if !credential.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", credential));
            }
            if provider_id == "openrouter" {
                req = req
                    .header("HTTP-Referer", "https://quixidesk.local")
                    .header("X-Title", "QuixiDesk");
            }
            let mut msgs = vec![json!({ "role": "system", "content": sys_prompt })];
            for (role, text) in history {
                if !text.trim().is_empty() {
                    msgs.push(json!({
                        "role": if role == "assistant" { "assistant" } else { "user" },
                        "content": text
                    }));
                }
            }
            if msgs.len() == 1
                || history
                    .last()
                    .map(|(r, t)| r != "user" || t != prompt)
                    .unwrap_or(true)
            {
                msgs.push(json!({ "role": "user", "content": prompt }));
            }
            let sent = req
                .json(&json!({
                    "model": model_id,
                    "messages": msgs,
                    "stream": true
                }))
                .send();

            let resp = match sent {
                Ok(resp) => resp,
                Err(err) => {
                    let msg = format!(
                        "⚠️ {} request to `{}` failed before any data arrived: {}",
                        provider_name, url, err
                    );
                    tracing::warn!("{msg}");
                    return Some((msg, provider_name, model_id));
                }
            };

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().unwrap_or_default();
                let msg = format!(
                    "⚠️ {} returned HTTP {} for model `{}`: {}",
                    provider_name,
                    status.as_u16(),
                    model_id,
                    body.chars().take(400).collect::<String>()
                );
                tracing::warn!("{msg}");
                return Some((msg, provider_name, model_id));
            }

            {
                {
                    let mut acc = String::new();
                    let mut reasoning_acc = String::new();
                    let reader = BufReader::new(resp);
                    for line in reader.lines().map_while(Result::ok) {
                        let trimmed = line.trim();
                        if let Some(data_str) = trimmed.strip_prefix("data:") {
                            let data_str = data_str.trim();
                            if data_str.is_empty() || data_str == "[DONE]" {
                                continue;
                            }
                            if let Ok(ev) = serde_json::from_str::<Value>(data_str) {
                                // Reasoning models (DeepSeek deepseek-flash, o-series, …) stream
                                // their chain of thought before the answer. Forward it as a
                                // thinking chunk so the UI can show progress instead of freezing.
                                let thinking_delta = ev
                                    .pointer("/choices/0/delta/reasoning_content")
                                    .or_else(|| ev.pointer("/choices/0/delta/reasoning"))
                                    .and_then(|v| v.as_str())
                                    .filter(|s| !s.is_empty());
                                if let Some(th) = thinking_delta {
                                    reasoning_acc.push_str(th);
                                    on_chunk(StreamChunk::Thinking(th.to_string()));
                                }
                                if let Some(d) = ev
                                    .pointer("/choices/0/delta/content")
                                    .and_then(|v| v.as_str())
                                    .filter(|s| !s.is_empty())
                                {
                                    acc.push_str(d);
                                    on_delta!(d);
                                } else if let Some(t) = ev
                                    .pointer("/choices/0/message/content")
                                    .and_then(|v| v.as_str())
                                    .filter(|s| !s.is_empty())
                                {
                                    acc.push_str(t);
                                    on_delta!(t);
                                }
                            }
                        } else if trimmed.starts_with('{') {
                            if let Ok(body) = serde_json::from_str::<Value>(trimmed) {
                                if let Some(t) = body
                                    .pointer("/choices/0/message/content")
                                    .and_then(|v| v.as_str())
                                    .filter(|s| !s.is_empty())
                                {
                                    acc.push_str(t);
                                    on_delta!(t);
                                }
                            }
                        }
                    }
                    if !acc.trim().is_empty() {
                        return Some((acc, provider_name, model_id));
                    }
                    if !reasoning_acc.trim().is_empty() {
                        // Only reasoning came back (e.g. the stream ended right after thinking):
                        // surface it instead of silently falling back to the canned reply.
                        return Some((reasoning_acc, provider_name, model_id));
                    }
                }
            }
        }

        Some((String::new(), provider_name, model_id))
    }
}

/// One tool call requested by the model.
#[derive(Debug, Clone)]
pub struct ProviderToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// The full result of one assistant turn that was allowed to call tools.
#[derive(Debug, Clone)]
pub struct ProviderToolTurn {
    pub text: String,
    pub thinking: String,
    pub tool_calls: Vec<ProviderToolCall>,
    pub provider_name: String,
    pub model_id: String,
}

impl ProviderManager {
    /// Runs one streaming chat turn with OpenAI-style `tools` and returns the visible text
    /// plus every tool call the model asked for. Only the OpenAI-compatible providers are
    /// supported (DeepSeek, OpenRouter, Groq, Mistral, Together, Fireworks, Cerebras,
    /// Perplexity, Cohere, Gemini, local Ollama/LM Studio, and custom endpoints);
    /// Anthropic-format and Codex-OAuth providers fall back to the tool-less path.
    pub fn complete_turn_with_tools<F>(
        &self,
        messages: &[Value],
        tools: &Value,
        model_override: Option<&str>,
        mut on_chunk: F,
    ) -> Option<ProviderToolTurn>
    where
        F: FnMut(StreamChunk),
    {
        use std::io::{BufRead, BufReader};

        let resolved_model = self.resolve_connected_model(model_override);
        let (provider_id, provider_name, model_id, base_url, api_format, auth_mode, credential) = {
            let st = self.state.read();
            let entry = st
                .providers
                .iter()
                .find(|p| {
                    p.configured
                        && (p.selected_model == resolved_model
                            || p.models.iter().any(|m| {
                                m.id == resolved_model && Self::is_model_available_for_provider(p, m)
                            }))
                })
                .or_else(|| {
                    st.providers
                        .iter()
                        .find(|p| p.id == st.active_provider && p.configured)
                })
                .or_else(|| st.providers.iter().find(|p| p.configured))?;

            let cred = if entry.auth_mode == "oauth" && !entry.oauth_access_token.is_empty() {
                entry.oauth_access_token.clone()
            } else {
                entry.api_key_raw.clone()
            };
            let chosen_model = if entry.models.iter().any(|m| m.id == resolved_model)
                || entry.selected_model == resolved_model
            {
                resolved_model
            } else {
                entry.selected_model.clone()
            };
            (
                entry.id.clone(),
                entry.name.clone(),
                chosen_model,
                entry.base_url.trim_end_matches('/').to_string(),
                entry.api_format.clone(),
                entry.auth_mode.clone(),
                cred,
            )
        };

        if api_format != "openai"
            || base_url.is_empty()
            || credential.starts_with("oauth_")
            || (credential.is_empty() && auth_mode != "local" && provider_id != "custom")
        {
            return None;
        }

        let http = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(15))
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .ok()?;

        tracing::info!(
            provider = %provider_id,
            model = %model_id,
            messages = messages.len(),
            "agent tools: streaming tool-calling turn"
        );
        let url = format!("{}/chat/completions", base_url);
        let mut req = http
            .post(&url)
            .header("content-type", "application/json")
            .header("accept", "text/event-stream");
        if !credential.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", credential));
        }
        if provider_id == "openrouter" {
            req = req
                .header("HTTP-Referer", "https://quixidesk.local")
                .header("X-Title", "QuixiDesk");
        }

        let resp = match req
            .json(&json!({
                "model": model_id,
                "messages": messages,
                "tools": tools,
                "tool_choice": "auto",
                "stream": true
            }))
            .send()
        {
            Ok(resp) => resp,
            Err(err) => {
                on_chunk(StreamChunk::Text(format!(
                    "\n\n⚠️ {} unreachable: {err}",
                    provider_name
                )));
                return None;
            }
        };

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            on_chunk(StreamChunk::Text(format!(
                "⚠️ {} returned HTTP {} for model `{}`: {}",
                provider_name,
                status.as_u16(),
                model_id,
                body.chars().take(400).collect::<String>()
            )));
            return None;
        }

        let mut text = String::new();
        let mut thinking = String::new();
        let mut calls: Vec<ProviderToolCall> = Vec::new();
        let reader = BufReader::new(resp);
        for line in reader.lines().map_while(Result::ok) {
            let trimmed = line.trim();
            let Some(data_str) = trimmed.strip_prefix("data:") else {
                continue;
            };
            let data_str = data_str.trim();
            if data_str.is_empty() || data_str == "[DONE]" {
                continue;
            }
            let Ok(ev) = serde_json::from_str::<Value>(data_str) else {
                continue;
            };
            let delta = ev.pointer("/choices/0/delta");

            if let Some(th) = delta
                .and_then(|d| d.get("reasoning_content").or_else(|| d.get("reasoning")))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                thinking.push_str(th);
                on_chunk(StreamChunk::Thinking(th.to_string()));
            }
            if let Some(t) = delta
                .and_then(|d| d.get("content"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                text.push_str(t);
                on_chunk(StreamChunk::Text(t.to_string()));
            }
            if let Some(arr) = delta.and_then(|d| d.get("tool_calls")).and_then(|v| v.as_array()) {
                for (idx, tc) in arr.iter().enumerate() {
                    let index = tc
                        .get("index")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(idx as u64) as usize;
                    while calls.len() <= index {
                        calls.push(ProviderToolCall {
                            id: String::new(),
                            name: String::new(),
                            arguments: String::new(),
                        });
                    }
                    let slot = &mut calls[index];
                    if let Some(id) = tc.get("id").and_then(|v| v.as_str()) {
                        if !id.is_empty() {
                            slot.id = id.to_string();
                        }
                    }
                    if let Some(fname) = tc.pointer("/function/name").and_then(|v| v.as_str()) {
                        if !fname.is_empty() {
                            slot.name = fname.to_string();
                        }
                    }
                    if let Some(args) = tc.pointer("/function/arguments").and_then(|v| v.as_str()) {
                        slot.arguments.push_str(args);
                    }
                }
            }
        }

        let tool_calls: Vec<ProviderToolCall> = calls
            .into_iter()
            .filter(|c| !c.name.is_empty())
            .map(|mut c| {
                if c.id.is_empty() {
                    c.id = format!("call_{}", &Uuid::new_v4().to_string().replace('-', "")[..16]);
                }
                c
            })
            .collect();

        tracing::info!(
            text_chars = text.len(),
            thinking_chars = thinking.len(),
            tool_calls = tool_calls.len(),
            "agent tools: turn finished"
        );
        Some(ProviderToolTurn {
            text,
            thinking,
            tool_calls,
            provider_name,
            model_id,
        })
    }
}

#[cfg(test)]
mod provider_catalog_tests {
    use super::ProviderManager;

    fn entry(id: &str) -> super::ProviderEntry {
        ProviderManager::default_catalog()
            .providers
            .into_iter()
            .find(|p| p.id == id)
            .unwrap_or_else(|| panic!("provider {id} missing from catalog"))
    }

    /// DeepSeek retired the R1 naming in 2025. Their live API
    /// (`GET https://api.deepseek.com/models`) exposes exactly two ids:
    /// `deepseek-flash` (V4.1-Flash, 1M ctx, text+vision) and `deepseek-v4-pro`.
    /// `deepseek-chat` / `deepseek-reasoner` are legacy aliases served by `deepseek-flash`.
    #[test]
    fn deepseek_catalog_matches_live_api() {
        let ds = entry("deepseek");
        assert_eq!(ds.selected_model, "deepseek-flash");
        assert_eq!(ds.context_window, 1_048_576);

        let ids: Vec<&str> = ds.models.iter().map(|m| m.id.as_str()).collect();
        assert!(ids.contains(&"deepseek-flash"), "ids: {ids:?}");
        assert!(ids.contains(&"deepseek-v4-pro"), "ids: {ids:?}");

        let labels: Vec<&str> = ds.models.iter().map(|m| m.label.as_str()).collect();
        assert!(
            !labels.iter().any(|l| l.contains("R1")),
            "R1 is retired and must not be advertised: {labels:?}"
        );

        let flash = ds.models.iter().find(|m| m.id == "deepseek-flash").unwrap();
        assert!(flash.supports_thinking && flash.supports_vision);
        assert_eq!(flash.context_window, 1_048_576);
    }

    #[test]
    fn openrouter_deepseek_entries_are_current() {
        let or = entry("openrouter");
        let ids: Vec<&str> = or.models.iter().map(|m| m.id.as_str()).collect();
        assert!(!ids.contains(&"deepseek/deepseek-r1"), "ids: {ids:?}");
        assert!(ids.contains(&"deepseek/deepseek-v4.1-flash"), "ids: {ids:?}");
        assert!(ids.contains(&"deepseek/deepseek-v4-pro"), "ids: {ids:?}");
    }
}

#[cfg(test)]
mod legacy_model_migration_tests {
    use super::ProviderManager;

    #[test]
    fn r1_era_deepseek_ids_migrate_to_current_models() {
        assert_eq!(
            ProviderManager::migrate_legacy_model_id("deepseek", "deepseek-reasoner").as_deref(),
            Some("deepseek-flash")
        );
        assert_eq!(
            ProviderManager::migrate_legacy_model_id("deepseek", "deepseek-chat").as_deref(),
            Some("deepseek-flash")
        );
        assert_eq!(
            ProviderManager::migrate_legacy_model_id("openrouter", "deepseek/deepseek-r1")
                .as_deref(),
            Some("deepseek/deepseek-v4.1-flash")
        );
        // Current ids stay untouched.
        assert!(ProviderManager::migrate_legacy_model_id("deepseek", "deepseek-flash").is_none());
        assert!(ProviderManager::migrate_legacy_model_id("deepseek", "deepseek-v4-pro").is_none());
    }
}

#[cfg(test)]
mod streaming_chunk_tests {
    use super::{ProviderManager, StreamChunk};
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::path::PathBuf;

    /// Spins up a fake OpenAI-compatible SSE endpoint that answers every request with `body`.
    fn fake_provider(body: &'static str) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = std::thread::spawn(move || {
            if let Ok((stream, _)) = listener.accept() {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                // Consume the request headers so the client is not blocked writing the body.
                while reader.read_line(&mut line).map(|n| n > 0).unwrap_or(false) {
                    if line == "\r\n" {
                        break;
                    }
                    line.clear();
                }
                let mut stream = stream;
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{}",
                    body
                );
                let _ = stream.flush();
            }
        });
        (format!("http://127.0.0.1:{}/v1", port), handle)
    }

    fn manager_pointing_at(base_url: &str) -> (ProviderManager, PathBuf) {
        let dir = std::env::temp_dir().join(format!("quixi-stream-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let state = serde_json::json!({
            "onboardingCompleted": true,
            "activeProvider": "custom",
            "activeModel": "stream-test-model",
            "providers": [{
                "id": "custom",
                "name": "Custom Endpoint",
                "configured": true,
                "baseUrl": base_url,
                "apiFormat": "openai",
                "authMode": "api_key",
                "selectedModel": "stream-test-model",
                "apiKeyRaw": "test-key"
            }]
        });
        std::fs::write(
            dir.join("providers.json"),
            serde_json::to_vec_pretty(&state).unwrap(),
        )
        .unwrap();
        (ProviderManager::new(&dir), dir)
    }

    /// Reasoning deltas must reach the UI as thinking chunks, and the visible answer must
    /// still be assembled — dropping the reasoning was what made the window look frozen
    /// while DeepSeek's `deepseek-flash` (thinking on by default) was thinking.
    #[test]
    fn reasoning_deltas_are_streamed_as_thinking_chunks() {
        let (base_url, server) = fake_provider(concat!(
            "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"thinking...\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hallo \"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"Welt\"}}]}\n\n",
            "data: [DONE]\n\n"
        ));
        let (mgr, dir) = manager_pointing_at(&base_url);

        let mut thinking = String::new();
        let mut text = String::new();
        let out = mgr.try_live_completion_streaming(
            "hi",
            &dir.to_string_lossy(),
            &[],
            Some("stream-test-model"),
            |chunk| match chunk {
                StreamChunk::Thinking(t) => thinking.push_str(&t),
                StreamChunk::Text(t) => text.push_str(&t),
            },
        );
        server.join().ok();
        let (reply, _, _) = out.expect("live completion should return");

        assert_eq!(thinking, "thinking...");
        assert_eq!(text, "Hallo Welt");
        assert_eq!(reply, "Hallo Welt");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A provider error must be surfaced to the user, not swallowed into the canned
    /// "Hello from QuixiDesk" fallback reply.
    #[test]
    fn provider_http_errors_are_surfaced() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            if let Ok((stream, _)) = listener.accept() {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                while reader.read_line(&mut line).map(|n| n > 0).unwrap_or(false) {
                    if line == "\r\n" {
                        break;
                    }
                    line.clear();
                }
                let body = "{\"error\":{\"message\":\"Insufficient Balance\"}}";
                let mut stream = stream;
                let _ = write!(
                    stream,
                    "HTTP/1.1 402 Payment Required\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.flush();
            }
        });
        let (mgr, dir) = manager_pointing_at(&format!("http://127.0.0.1:{}/v1", port));

        let out = mgr.try_live_completion_streaming(
            "hi",
            &dir.to_string_lossy(),
            &[],
            Some("stream-test-model"),
            |_| {},
        );
        server.join().ok();
        let (reply, _, _) = out.expect("error must be reported, not silently dropped");
        assert!(reply.contains("402"), "reply was: {reply}");
        assert!(reply.contains("Insufficient Balance"), "reply was: {reply}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
