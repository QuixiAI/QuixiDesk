//! This module provides the Modelscope provider, wrapping OpenAI Chat Completions for Modelscope requests.

pub mod capabilities;

// Generate the settings module
crate::openai_compatible_settings!(
    ModelscopeProviderSettings,
    ModelscopeProviderSettingsBuilder,
    "Modelscope",
    "https://api-inference.modelscope.cn/v1",
    "MODELSCOPE_API_KEY"
);

// Generate the provider struct and builder
crate::openai_compatible_provider!(
    Modelscope,
    ModelscopeBuilder,
    ModelscopeProviderSettings,
    "modelscope"
);

// Generate the language model implementation
crate::openai_compatible_language_model!(Modelscope);
