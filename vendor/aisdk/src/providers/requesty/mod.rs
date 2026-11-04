//! This module provides the Requesty provider, wrapping OpenAI Chat Completions for Requesty requests.

pub mod capabilities;

// Generate the settings module
crate::openai_compatible_settings!(
    RequestyProviderSettings,
    RequestyProviderSettingsBuilder,
    "Requesty",
    "https://router.requesty.ai/v1",
    "REQUESTY_API_KEY"
);

// Generate the provider struct and builder
crate::openai_compatible_provider!(
    Requesty,
    RequestyBuilder,
    RequestyProviderSettings,
    "requesty"
);

// Generate the language model implementation
crate::openai_compatible_language_model!(Requesty);
