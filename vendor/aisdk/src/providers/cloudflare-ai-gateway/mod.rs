//! This module provides the CloudflareAiGateway provider, wrapping OpenAI Chat Completions for CloudflareAiGateway requests.

pub mod capabilities;

// Generate the settings module
crate::openai_compatible_settings!(
    CloudflareAiGatewayProviderSettings,
    CloudflareAiGatewayProviderSettingsBuilder,
    "CloudflareAiGateway",
    "https://gateway.ai.cloudflare.com/v1/${CLOUDFLARE_ACCOUNT_ID}/${CLOUDFLARE_GATEWAY_ID}/compat/",
    "CLOUDFLARE_API_TOKEN"
);

// Generate the provider struct and builder
crate::openai_compatible_provider!(
    CloudflareAiGateway,
    CloudflareAiGatewayBuilder,
    CloudflareAiGatewayProviderSettings,
    "cloudflare-ai-gateway"
);

// Generate the language model implementation
crate::openai_compatible_language_model!(CloudflareAiGateway);
