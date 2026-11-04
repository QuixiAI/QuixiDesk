//! This module provides the Opencode provider, wrapping OpenAI Chat Completions for Opencode requests.

pub mod capabilities;

// Generate the settings module
crate::openai_compatible_settings!(
    OpencodeProviderSettings,
    OpencodeProviderSettingsBuilder,
    "Opencode",
    "https://opencode.ai/zen/v1",
    "OPENCODE_API_KEY"
);

// Generate the provider struct and builder
crate::openai_compatible_provider!(
    Opencode,
    OpencodeBuilder,
    OpencodeProviderSettings,
    "opencode"
);

// Generate the language model implementation
crate::openai_compatible_language_model!(Opencode);
