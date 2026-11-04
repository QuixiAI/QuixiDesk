//! This module provides the Submodel provider, wrapping OpenAI Chat Completions for Submodel requests.

pub mod capabilities;

// Generate the settings module
crate::openai_compatible_settings!(
    SubmodelProviderSettings,
    SubmodelProviderSettingsBuilder,
    "Submodel",
    "https://llm.submodel.ai/v1",
    "SUBMODEL_INSTAGEN_ACCESS_KEY"
);

// Generate the provider struct and builder
crate::openai_compatible_provider!(
    Submodel,
    SubmodelBuilder,
    SubmodelProviderSettings,
    "submodel"
);

// Generate the language model implementation
crate::openai_compatible_language_model!(Submodel);
