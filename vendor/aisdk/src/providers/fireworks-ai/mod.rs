//! This module provides the FireworksAi provider, wrapping OpenAI Chat Completions for FireworksAi requests.

pub mod capabilities;

// Generate the settings module
crate::openai_compatible_settings!(
    FireworksAiProviderSettings,
    FireworksAiProviderSettingsBuilder,
    "FireworksAi",
    "https://api.fireworks.ai/inference/v1/",
    "FIREWORKS_API_KEY"
);

// Generate the provider struct and builder
crate::openai_compatible_provider!(
    FireworksAi,
    FireworksAiBuilder,
    FireworksAiProviderSettings,
    "fireworks-ai"
);

// Generate the language model implementation
crate::openai_compatible_language_model!(FireworksAi);
