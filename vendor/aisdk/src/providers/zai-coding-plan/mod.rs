//! This module provides the ZaiCodingPlan provider, wrapping OpenAI Chat Completions for ZaiCodingPlan requests.

pub mod capabilities;

// Generate the settings module
crate::openai_compatible_settings!(
    ZaiCodingPlanProviderSettings,
    ZaiCodingPlanProviderSettingsBuilder,
    "ZaiCodingPlan",
    "https://api.z.ai/api/coding/paas/v4",
    "ZHIPU_API_KEY"
);

// Generate the provider struct and builder
crate::openai_compatible_provider!(
    ZaiCodingPlan,
    ZaiCodingPlanBuilder,
    ZaiCodingPlanProviderSettings,
    "zai-coding-plan"
);

// Generate the language model implementation
crate::openai_compatible_language_model!(ZaiCodingPlan);
