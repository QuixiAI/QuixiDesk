//! This module provides the Lmstudio provider, wrapping OpenAI Chat Completions for Lmstudio requests.

pub mod capabilities;

// Generate the settings module
crate::openai_compatible_settings!(
    LmstudioProviderSettings,
    LmstudioProviderSettingsBuilder,
    "Lmstudio",
    "http://127.0.0.1:1234/v1",
    "LMSTUDIO_API_KEY"
);

// Generate the provider struct and builder
crate::openai_compatible_provider!(
    Lmstudio,
    LmstudioBuilder,
    LmstudioProviderSettings,
    "lmstudio"
);

// Generate the language model implementation
crate::openai_compatible_language_model!(Lmstudio);
