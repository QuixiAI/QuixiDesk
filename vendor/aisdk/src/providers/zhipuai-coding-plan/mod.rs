//! This module provides the ZhipuaiCodingPlan provider, wrapping OpenAI Chat Completions for ZhipuaiCodingPlan requests.

pub mod capabilities;

// Generate the settings module
crate::openai_compatible_settings!(
    ZhipuaiCodingPlanProviderSettings,
    ZhipuaiCodingPlanProviderSettingsBuilder,
    "ZhipuaiCodingPlan",
    "https://open.bigmodel.cn/api/coding/paas/v4",
    "ZHIPU_API_KEY"
);

// Generate the provider struct and builder
crate::openai_compatible_provider!(
    ZhipuaiCodingPlan,
    ZhipuaiCodingPlanBuilder,
    ZhipuaiCodingPlanProviderSettings,
    "zhipuai-coding-plan"
);

// Generate the language model implementation
crate::openai_compatible_language_model!(ZhipuaiCodingPlan);
