//! This module provides the OllamaCloud provider, wrapping OpenAI Chat Completions for OllamaCloud requests.

pub mod capabilities;

// Generate the settings module
crate::openai_compatible_settings!(
    OllamaCloudProviderSettings,
    OllamaCloudProviderSettingsBuilder,
    "OllamaCloud",
    "https://ollama.com/v1",
    "OLLAMA_API_KEY"
);

// Generate the provider struct and builder
crate::openai_compatible_provider!(
    OllamaCloud,
    OllamaCloudBuilder,
    OllamaCloudProviderSettings,
    "ollama-cloud"
);

// Generate the language model implementation
crate::openai_compatible_language_model!(OllamaCloud);
