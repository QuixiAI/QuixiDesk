//! This module provides the Fastrouter provider, wrapping OpenAI Chat Completions for Fastrouter requests.

pub mod capabilities;

// Generate the settings module
crate::openai_compatible_settings!(
    FastrouterProviderSettings,
    FastrouterProviderSettingsBuilder,
    "Fastrouter",
    "https://go.fastrouter.ai/api/v1",
    "FASTROUTER_API_KEY"
);

// Generate the provider struct and builder
crate::openai_compatible_provider!(
    Fastrouter,
    FastrouterBuilder,
    FastrouterProviderSettings,
    "fastrouter"
);

// Generate the language model implementation
crate::openai_compatible_language_model!(Fastrouter);
