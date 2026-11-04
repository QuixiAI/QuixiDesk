//! This module provides the Aihubmix provider, wrapping OpenAI Chat Completions for Aihubmix requests.

pub mod capabilities;

// Generate the settings module
crate::openai_compatible_settings!(
    AihubmixProviderSettings,
    AihubmixProviderSettingsBuilder,
    "Aihubmix",
    "https://aihubmix.com/v1",
    "AIHUBMIX_API_KEY"
);

// Generate the provider struct and builder
crate::openai_compatible_provider!(
    Aihubmix,
    AihubmixBuilder,
    AihubmixProviderSettings,
    "aihubmix"
);

// Generate the language model implementation
crate::openai_compatible_language_model!(Aihubmix);
