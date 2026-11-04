//! This module provides the GithubCopilot provider, wrapping OpenAI Chat Completions for GithubCopilot requests.

pub mod capabilities;

// Generate the settings module
crate::openai_compatible_settings!(
    GithubCopilotProviderSettings,
    GithubCopilotProviderSettingsBuilder,
    "GithubCopilot",
    "https://api.githubcopilot.com",
    "GITHUB_TOKEN"
);

// Generate the provider struct and builder
crate::openai_compatible_provider!(
    GithubCopilot,
    GithubCopilotBuilder,
    GithubCopilotProviderSettings,
    "github-copilot"
);

// Generate the language model implementation
crate::openai_compatible_language_model!(GithubCopilot);
