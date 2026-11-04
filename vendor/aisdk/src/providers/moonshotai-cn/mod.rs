//! This module provides the MoonshotaiCn provider, wrapping OpenAI Chat Completions for MoonshotaiCn requests.

pub mod capabilities;

// Generate the settings module
crate::openai_compatible_settings!(
    MoonshotaiCnProviderSettings,
    MoonshotaiCnProviderSettingsBuilder,
    "MoonshotaiCn",
    "https://api.moonshot.cn/v1",
    "MOONSHOT_API_KEY"
);

// Generate the provider struct and builder
crate::openai_compatible_provider!(
    MoonshotaiCn,
    MoonshotaiCnBuilder,
    MoonshotaiCnProviderSettings,
    "moonshotai-cn"
);

// Generate the language model implementation
crate::openai_compatible_language_model!(MoonshotaiCn);
