//! This module provides the Lucidquery provider, wrapping OpenAI Chat Completions for Lucidquery requests.

pub mod capabilities;

// Generate the settings module
crate::openai_compatible_settings!(
    LucidqueryProviderSettings,
    LucidqueryProviderSettingsBuilder,
    "Lucidquery",
    "https://lucidquery.com/api/v1",
    "LUCIDQUERY_API_KEY"
);

// Generate the provider struct and builder
crate::openai_compatible_provider!(
    Lucidquery,
    LucidqueryBuilder,
    LucidqueryProviderSettings,
    "lucidquery"
);

// Generate the language model implementation
crate::openai_compatible_language_model!(Lucidquery);
