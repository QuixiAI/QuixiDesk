//! This module provides the AlibabaCn provider, wrapping OpenAI Chat Completions for AlibabaCn requests.

pub mod capabilities;

// Generate the settings module
crate::openai_compatible_settings!(
    AlibabaCnProviderSettings,
    AlibabaCnProviderSettingsBuilder,
    "AlibabaCn",
    "https://dashscope.aliyuncs.com/compatible-mode/v1",
    "DASHSCOPE_API_KEY"
);

// Generate the provider struct and builder
crate::openai_compatible_provider!(
    AlibabaCn,
    AlibabaCnBuilder,
    AlibabaCnProviderSettings,
    "alibaba-cn"
);

// Generate the language model implementation
crate::openai_compatible_language_model!(AlibabaCn);
