//! Provider-neutral LLM failure normalization and retry policy (`@deepseek-ai/dsh-llm-retry`).
//!
//! - Unwraps nested provider error JSON (including OpenRouter's stringified `error.metadata.raw`)
//!   into clean human-readable diagnostics with stable machine-routable failure codes
//!   (`SERVER`, `RATE_LIMIT`, `TIMEOUT`, `TRANSPORT`, `EMPTY_RESPONSE`, `CONTEXT_WINDOW_EXCEEDED`,
//!   `QUOTA`, `AUTH`, `BAD_REQUEST`).
//! - Executes bounded (`normal`, default `maxRetries = 10`) or unbounded (`always`) exponential
//!   backoff with symmetric jitter and `Retry-After` header support, cancellable within 40ms.

use parking_lot::RwLock;
use quixi_cordis::ServiceKey;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub const LLM_RETRY_SERVICE: ServiceKey<LlmRetryService> = ServiceKey::new(
    "llmRetry",
    "Exponential-backoff model request retry policy (`dsh-llm-retry`)",
);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmRetryConfig {
    /// `"normal"` (bounded by `max_retries`) or `"always"` (retry until success or user Stop).
    pub mode: String,
    /// Maximum retry attempts per model step after the initial attempt (default `10`).
    pub max_retries: u32,
    /// Initial exponential backoff delay in milliseconds (default `1000`).
    pub initial_delay_ms: u64,
    /// Maximum backoff delay in milliseconds (default `15000`).
    pub max_delay_ms: u64,
    /// Symmetric jitter ratio in `0.0..=1.0` (default `0.15`).
    pub jitter_ratio: f64,
}

impl Default for LlmRetryConfig {
    fn default() -> Self {
        Self {
            mode: "normal".into(),
            max_retries: 10,
            initial_delay_ms: 1_000,
            max_delay_ms: 15_000,
            jitter_ratio: 0.15,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmFailure {
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    pub provider: String,
    pub model: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrySchedule {
    pub retry: u32,
    pub max_retries: u32,
    pub mode: String,
    pub delay_ms: u64,
}

pub struct LlmRetryService {
    pub config: RwLock<LlmRetryConfig>,
    rng_counter: AtomicU64,
}

impl Default for LlmRetryService {
    fn default() -> Self {
        Self::new(LlmRetryConfig::default())
    }
}

impl LlmRetryService {
    pub fn new(cfg: LlmRetryConfig) -> Self {
        Self {
            config: RwLock::new(cfg),
            rng_counter: AtomicU64::new(0x9E37_79B9_7F4A_7C15),
        }
    }

    fn next_unit_sample(&self) -> f64 {
        let prev = self
            .rng_counter
            .fetch_add(0xBF58_476D_1CE4_E5B9, Ordering::Relaxed);
        let mut z = prev ^ (prev >> 30);
        z = z.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z ^= z >> 27;
        z = z.wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        ((z >> 11) as f64) / ((1u64 << 53) as f64)
    }

    pub fn local_delay_ms(&self, cfg: &LlmRetryConfig, retry: u32) -> u64 {
        let exp = retry.saturating_sub(1).min(20);
        let base = (cfg.initial_delay_ms as f64) * 2f64.powi(exp as i32);
        let capped = base.min(cfg.max_delay_ms as f64);
        let jitter = cfg.jitter_ratio.clamp(0.0, 1.0);
        let factor = 1.0 - jitter + 2.0 * jitter * self.next_unit_sample();
        (capped * factor).round().clamp(1.0, cfg.max_delay_ms as f64) as u64
    }

    pub fn is_retryable_code(code: &str) -> bool {
        matches!(
            code,
            "SERVER" | "RATE_LIMIT" | "TIMEOUT" | "TRANSPORT" | "EMPTY_RESPONSE"
        )
    }

    /// Evaluates whether `failure` should be retried given `previous_retries` already spent in this step.
    pub fn decide_retry(&self, failure: &LlmFailure, previous_retries: u32) -> Option<RetrySchedule> {
        let cfg = self.config.read().clone();
        let always = cfg.mode == "always";
        if !always {
            if !Self::is_retryable_code(&failure.code) {
                return None;
            }
            if previous_retries >= cfg.max_retries {
                return None;
            }
        } else if matches!(failure.code.as_str(), "AUTH" | "BAD_REQUEST") {
            // Even in `always` mode, invalid credentials or malformed 400 schemas cannot heal via sleep.
            return None;
        }

        let retry = previous_retries + 1;
        let delay_ms = match failure.retry_after_ms {
            Some(ms) if ms > 0 && ms <= cfg.max_delay_ms => ms,
            Some(ms) if ms > cfg.max_delay_ms && !always => return None,
            _ => self.local_delay_ms(&cfg, retry),
        };

        Some(RetrySchedule {
            retry,
            max_retries: cfg.max_retries,
            mode: cfg.mode,
            delay_ms,
        })
    }

    /// Sleeps up to `delay_ms` in 40ms slices, returning `false` immediately if `cancelled` fires.
    pub fn cancellable_sleep(delay_ms: u64, cancelled: &AtomicBool) -> bool {
        let deadline = Instant::now() + Duration::from_millis(delay_ms);
        while Instant::now() < deadline {
            if cancelled.load(Ordering::SeqCst) {
                return false;
            }
            let rem = deadline.saturating_duration_since(Instant::now());
            std::thread::sleep(rem.min(Duration::from_millis(40)));
        }
        !cancelled.load(Ordering::SeqCst)
    }
}

/// Normalizes a raw adapter/HTTP failure string into a clean `LlmFailure` with a stable code
/// and human-readable message (unwrapping OpenRouter `metadata.raw` and stripping raw JSON noise).
pub fn normalize_llm_failure(
    raw_error: &str,
    provider: &str,
    model: &str,
    retry_after_ms: Option<u64>,
) -> LlmFailure {
    let trimmed = raw_error.trim().trim_start_matches("Provider error:").trim();

    if trimmed == "EMPTY_RESPONSE" {
        return LlmFailure {
            code: "EMPTY_RESPONSE".into(),
            status: None,
            provider: provider.to_owned(),
            model: model.to_owned(),
            message: format!(
                "{} returned an empty response for `{}`.",
                if provider.is_empty() { "Provider" } else { provider },
                model
            ),
            retry_after_ms,
        };
    }

    // 1. Extract HTTP status code if present (`HTTP 503`, `code":503`, etc.)
    let http_re = Regex::new(r"\bHTTP\s+(\d{3})\b").unwrap();
    let mut status: Option<u16> = http_re
        .captures(trimmed)
        .and_then(|c| c[1].parse::<u16>().ok());

    // 2. Parse any embedded JSON payload (`{ ... }`) and unwrap nested `metadata.raw`
    let mut extracted_msg: Option<String> = None;
    let mut upstream_provider: Option<String> = None;
    let mut json_code_str = String::new();

    if let Some(brace_idx) = trimmed.find('{') {
        let json_candidate = &trimmed[brace_idx..];
        let parsed = serde_json::from_str::<Value>(json_candidate).or_else(|_| {
            // Handle truncated JSON tails by attempting to parse up to last `}`
            json_candidate
                .rfind('}')
                .ok_or(())
                .and_then(|end| serde_json::from_str::<Value>(&json_candidate[..=end]).map_err(|_| ()))
        });

        if let Ok(val) = parsed {
            let err_obj = val.get("error").unwrap_or(&val);
            if status.is_none() {
                status = err_obj
                    .get("code")
                    .and_then(Value::as_u64)
                    .and_then(|n| u16::try_from(n).ok())
                    .filter(|c| (100..=599).contains(c));
            }
            if let Some(c_str) = err_obj.get("code").and_then(Value::as_str) {
                json_code_str.push_str(c_str);
                json_code_str.push(' ');
            }
            if let Some(t_str) = err_obj.get("type").and_then(Value::as_str) {
                json_code_str.push_str(t_str);
                json_code_str.push(' ');
            }
            if let Some(s_str) = err_obj.get("status").and_then(Value::as_str) {
                json_code_str.push_str(s_str);
                json_code_str.push(' ');
            }

            if let Some(p_name) = err_obj
                .pointer("/metadata/provider_name")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
            {
                upstream_provider = Some(p_name.trim().to_owned());
            }

            // Unwrap OpenRouter's stringified inner JSON (`error.metadata.raw`):
            if let Some(raw_inner) = err_obj
                .pointer("/metadata/raw")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
            {
                if let Ok(inner_val) = serde_json::from_str::<Value>(raw_inner) {
                    let inner_err = inner_val.get("error").unwrap_or(&inner_val);
                    if let Some(inner_msg) = inner_err
                        .get("message")
                        .and_then(Value::as_str)
                        .filter(|s| !s.trim().is_empty())
                    {
                        extracted_msg = Some(inner_msg.trim().to_owned());
                    }
                    if let Some(inner_status) = inner_err.get("status").and_then(Value::as_str) {
                        json_code_str.push_str(inner_status);
                        json_code_str.push(' ');
                    }
                } else if extracted_msg.is_none() && !raw_inner.trim_start().starts_with('{') {
                    extracted_msg = Some(raw_inner.trim().to_owned());
                }
            }

            if extracted_msg.is_none() {
                if let Some(outer_msg) = err_obj
                    .get("message")
                    .and_then(Value::as_str)
                    .filter(|s| !s.trim().is_empty() && *s != "Provider returned error")
                {
                    extracted_msg = Some(outer_msg.trim().to_owned());
                }
            }
        } else {
            // Even when outer JSON was truncated at 400 chars (as in legacy errors!),
            // extract `\"message\": \"...\"` cleanly with regex!
            let escaped_msg_re = Regex::new(r#"\\"message\\":\s*\\"([^\\"]+)\\""#).unwrap();
            let plain_msg_re = Regex::new(r#""message"\s*:\s*"([^"]+)""#).unwrap();
            let prov_re = Regex::new(r#""provider_name"\s*:\s*"([^"]+)""#).unwrap();
            if let Some(c) = prov_re.captures(json_candidate) {
                upstream_provider = Some(c[1].trim().to_owned());
            }
            if let Some(c) = escaped_msg_re.captures(json_candidate) {
                extracted_msg = Some(c[1].trim().to_owned());
            } else if let Some(c) = plain_msg_re.captures(json_candidate) {
                if &c[1] != "Provider returned error" {
                    extracted_msg = Some(c[1].trim().to_owned());
                }
            }
        }
    }

    let detail_joined = format!(
        "{} {} {}",
        trimmed,
        json_code_str,
        extracted_msg.as_deref().unwrap_or("")
    );
    let lower = detail_joined.to_lowercase();

    // 3. Classify into DeepSeek Harness canonical failure codes:
    let code = if is_context_window_exceeded(&lower) {
        "CONTEXT_WINDOW_EXCEEDED"
    } else if status == Some(402) || is_quota_exceeded(&lower) {
        "QUOTA"
    } else if matches!(status, Some(401 | 403))
        || lower.contains("invalid api key")
        || lower.contains("incorrect api key")
        || lower.contains("unauthorized")
        || lower.contains("authentication failed")
    {
        "AUTH"
    } else if status == Some(429)
        || lower.contains("rate limit")
        || lower.contains("rate_limit")
        || lower.contains("too many requests")
    {
        "RATE_LIMIT"
    } else if matches!(status, Some(500 | 502 | 503 | 504 | 520..=529))
        || lower.contains("experiencing high demand")
        || lower.contains("overloaded")
        || lower.contains("unavailable")
        || lower.contains("bad gateway")
        || lower.contains("service unavailable")
        || lower.contains("server_error")
    {
        "SERVER"
    } else if status == Some(408)
        || lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("deadline exceeded")
    {
        "TIMEOUT"
    } else if lower.contains("unreachable")
        || lower.contains("connection reset")
        || lower.contains("connection refused")
        || lower.contains("dns error")
        || lower.contains("broken pipe")
        || lower.contains("unexpected eof")
        || lower.contains("stream ended before")
    {
        "TRANSPORT"
    } else if matches!(status, Some(400 | 404 | 422)) {
        "BAD_REQUEST"
    } else {
        "UNKNOWN"
    };

    // 4. Construct a concise, clean human-readable message with zero JSON noise:
    let provider_label = match (provider.trim(), upstream_provider.as_deref()) {
        ("", Some(up)) => up.to_owned(),
        (p, Some(up)) if !p.eq_ignore_ascii_case(up) => format!("{up} (via {p})"),
        (p, Some(_)) => p.to_owned(),
        ("", None) => "Model provider".to_owned(),
        (p, None) => p.to_owned(),
    };

    let status_suffix = status
        .map(|s| format!(" (HTTP {s})"))
        .unwrap_or_default();

    let clean_reason = if let Some(msg) = extracted_msg {
        msg
    } else if let Some(brace_idx) = trimmed.find('{') {
        let prefix = trimmed[..brace_idx].trim().trim_end_matches(':').trim();
        if prefix.is_empty() {
            default_reason_for_code(code)
        } else {
            prefix.to_owned()
        }
    } else {
        trimmed.chars().take(280).collect()
    };

    let message = if clean_reason.contains(&provider_label) {
        clean_reason
    } else {
        format!("{provider_label}{status_suffix}: {clean_reason}")
    };

    LlmFailure {
        code: code.to_owned(),
        status,
        provider: provider.to_owned(),
        model: model.to_owned(),
        message,
        retry_after_ms,
    }
}

fn default_reason_for_code(code: &str) -> String {
    match code {
        "SERVER" => "Temporary upstream server overload. Retrying automatically.".into(),
        "RATE_LIMIT" => "Rate limit reached. Waiting before retrying.".into(),
        "TIMEOUT" => "Model request timed out.".into(),
        "TRANSPORT" => "Network connection interrupted before response completed.".into(),
        "QUOTA" => "Account balance or request quota is exhausted.".into(),
        "AUTH" => "API key or authentication credential was rejected.".into(),
        _ => "Model request failed.".into(),
    }
}

fn is_context_window_exceeded(lower: &str) -> bool {
    lower.contains("context_length_exceeded")
        || lower.contains("context_window_exceeded")
        || lower.contains("maximum context length")
        || lower.contains("max context length")
        || lower.contains("prompt is too long")
        || lower.contains("input is too long")
        || lower.contains("request is too large for")
}

fn is_quota_exceeded(lower: &str) -> bool {
    lower.contains("insufficient_quota")
        || lower.contains("insufficient balance")
        || lower.contains("insufficient credits")
        || lower.contains("quota exceeded")
        || lower.contains("out of credits")
        || lower.contains("balance exhausted")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwraps_openrouter_truncated_503_nested_json_and_schedules_10_retries() {
        // Exact truncated OpenRouter 503 payload from live screenshot:
        let raw = r#"OpenRouter returned HTTP 503 for model `google/gemini-4-carbon-eap`: {"error":{"message":"Provider returned error","code":503,"metadata":{"raw":"{\n \"error\": {\n \"code\": 503,\n \"message\": \"This model is currently experiencing high demand. Spikes in demand are usually temporary. Please try again later.\",\n \"status\": \"UNAVAILABLE\"\n }\n}\n","provider_name":"Google AI Studio","is_byok":false,"provider_error_code":"503"}},"user_id":"org_2uMVNwONq"#;

        let f = normalize_llm_failure(raw, "OpenRouter", "google/gemini-4-carbon-eap", None);
        assert_eq!(f.code, "SERVER");
        assert_eq!(f.status, Some(503));
        assert!(f.message.contains("Google AI Studio (via OpenRouter)"));
        assert!(f.message.contains("currently experiencing high demand"));
        assert!(!f.message.contains("user_id"));
        assert!(!f.message.contains("is_byok"));

        let svc = LlmRetryService::default();
        for prev in 0..10 {
            let sched = svc.decide_retry(&f, prev).expect("must schedule retries 1..=10");
            assert_eq!(sched.retry, prev + 1);
            assert_eq!(sched.max_retries, 10);
            assert!(sched.delay_ms >= 800 && sched.delay_ms <= 15_000);
        }
        assert!(
            svc.decide_retry(&f, 10).is_none(),
            "11th retry must stop under normal maxRetries=10"
        );

        // Non-retryable AUTH / QUOTA errors never waste 10 retries:
        let quota = normalize_llm_failure(
            r#"HTTP 402: {"error":{"message":"Insufficient Balance"}}"#,
            "DeepSeek",
            "deepseek-chat",
            None,
        );
        assert_eq!(quota.code, "QUOTA");
        assert!(svc.decide_retry(&quota, 0).is_none());
    }
}
