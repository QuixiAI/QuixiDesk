//! This module provides the Moonshotai provider, wrapping OpenAI Chat Completions for Moonshotai requests.

pub mod capabilities;

// Generate the settings module
crate::openai_compatible_settings!(
    MoonshotaiProviderSettings,
    MoonshotaiProviderSettingsBuilder,
    "Moonshotai",
    "https://api.moonshot.ai/v1",
    "MOONSHOT_API_KEY"
);

// Generate the provider struct and builder
crate::openai_compatible_provider!(
    Moonshotai,
    MoonshotaiBuilder,
    MoonshotaiProviderSettings,
    "moonshotai"
);

// Generate the language model implementation
crate::openai_compatible_language_model!(Moonshotai);
