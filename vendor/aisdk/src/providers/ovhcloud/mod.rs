//! This module provides the Ovhcloud provider, wrapping OpenAI Chat Completions for Ovhcloud requests.

pub mod capabilities;

// Generate the settings module
crate::openai_compatible_settings!(
    OvhcloudProviderSettings,
    OvhcloudProviderSettingsBuilder,
    "Ovhcloud",
    "https://oai.endpoints.kepler.ai.cloud.ovh.net/v1",
    "OVHCLOUD_API_KEY"
);

// Generate the provider struct and builder
crate::openai_compatible_provider!(
    Ovhcloud,
    OvhcloudBuilder,
    OvhcloudProviderSettings,
    "ovhcloud"
);

// Generate the language model implementation
crate::openai_compatible_language_model!(Ovhcloud);
