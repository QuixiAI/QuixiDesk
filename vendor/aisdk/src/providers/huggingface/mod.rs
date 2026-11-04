//! This module provides the Huggingface provider, wrapping OpenAI Chat Completions for Huggingface requests.

pub mod capabilities;

// Generate the settings module
crate::openai_compatible_settings!(
    HuggingfaceProviderSettings,
    HuggingfaceProviderSettingsBuilder,
    "Huggingface",
    "https://router.huggingface.co/v1",
    "HF_TOKEN"
);

// Generate the provider struct and builder
crate::openai_compatible_provider!(
    Huggingface,
    HuggingfaceBuilder,
    HuggingfaceProviderSettings,
    "huggingface"
);

// Generate the language model implementation
crate::openai_compatible_language_model!(Huggingface);
