//! This module provides the Synthetic provider, wrapping OpenAI Chat Completions for Synthetic requests.

pub mod capabilities;

// Generate the settings module
crate::openai_compatible_settings!(
    SyntheticProviderSettings,
    SyntheticProviderSettingsBuilder,
    "Synthetic",
    "https://api.synthetic.new/v1",
    "SYNTHETIC_API_KEY"
);

// Generate the provider struct and builder
crate::openai_compatible_provider!(
    Synthetic,
    SyntheticBuilder,
    SyntheticProviderSettings,
    "synthetic"
);

// Generate the language model implementation
crate::openai_compatible_language_model!(Synthetic);
