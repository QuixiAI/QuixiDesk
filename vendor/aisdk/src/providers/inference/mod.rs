//! This module provides the Inference provider, wrapping OpenAI Chat Completions for Inference requests.

pub mod capabilities;

// Generate the settings module
crate::openai_compatible_settings!(
    InferenceProviderSettings,
    InferenceProviderSettingsBuilder,
    "Inference",
    "https://inference.net/v1",
    "INFERENCE_API_KEY"
);

// Generate the provider struct and builder
crate::openai_compatible_provider!(
    Inference,
    InferenceBuilder,
    InferenceProviderSettings,
    "inference"
);

// Generate the language model implementation
crate::openai_compatible_language_model!(Inference);
