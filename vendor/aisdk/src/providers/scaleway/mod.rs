//! This module provides the Scaleway provider, wrapping OpenAI Chat Completions for Scaleway requests.

pub mod capabilities;

// Generate the settings module
crate::openai_compatible_settings!(
    ScalewayProviderSettings,
    ScalewayProviderSettingsBuilder,
    "Scaleway",
    "https://api.scaleway.ai/v1",
    "SCALEWAY_API_KEY"
);

// Generate the provider struct and builder
crate::openai_compatible_provider!(
    Scaleway,
    ScalewayBuilder,
    ScalewayProviderSettings,
    "scaleway"
);

// Generate the language model implementation
crate::openai_compatible_language_model!(Scaleway);
