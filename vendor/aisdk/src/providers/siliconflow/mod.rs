//! This module provides the Siliconflow provider, wrapping OpenAI Chat Completions for Siliconflow requests.

pub mod capabilities;

// Generate the settings module
crate::openai_compatible_settings!(
    SiliconflowProviderSettings,
    SiliconflowProviderSettingsBuilder,
    "Siliconflow",
    "https://api.siliconflow.com/v1",
    "SILICONFLOW_API_KEY"
);

// Generate the provider struct and builder
crate::openai_compatible_provider!(
    Siliconflow,
    SiliconflowBuilder,
    SiliconflowProviderSettings,
    "siliconflow"
);

// Generate the language model implementation
crate::openai_compatible_language_model!(Siliconflow);
