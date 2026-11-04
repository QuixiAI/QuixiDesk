//! This module provides the GithubModels provider, wrapping OpenAI Chat Completions for GithubModels requests.

pub mod capabilities;

// Generate the settings module
crate::openai_compatible_settings!(
    GithubModelsProviderSettings,
    GithubModelsProviderSettingsBuilder,
    "GithubModels",
    "https://models.github.ai/inference",
    "GITHUB_TOKEN"
);

// Generate the provider struct and builder
crate::openai_compatible_provider!(
    GithubModels,
    GithubModelsBuilder,
    GithubModelsProviderSettings,
    "github-models"
);

// Generate the language model implementation
crate::openai_compatible_language_model!(GithubModels);
