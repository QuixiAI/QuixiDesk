//! This module provides the NovitaAi provider, wrapping OpenAI Chat Completions for NovitaAi requests.

pub mod capabilities;

// Generate the settings module
crate::openai_compatible_settings!(
    NovitaAiProviderSettings,
    NovitaAiProviderSettingsBuilder,
    "NovitaAi",
    "https://api.novita.ai/openai",
    "NOVITA_API_KEY"
);

// Generate the provider struct and builder
crate::openai_compatible_provider!(
    NovitaAi,
    NovitaAiBuilder,
    NovitaAiProviderSettings,
    "novita-ai"
);

// Generate the language model implementation
crate::openai_compatible_language_model!(NovitaAi);
