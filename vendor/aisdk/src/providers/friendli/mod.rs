//! This module provides the Friendli provider, wrapping OpenAI Chat Completions for Friendli requests.

pub mod capabilities;

// Generate the settings module
crate::openai_compatible_settings!(
    FriendliProviderSettings,
    FriendliProviderSettingsBuilder,
    "Friendli",
    "https://api.friendli.ai/serverless/v1",
    "FRIENDLI_TOKEN"
);

// Generate the provider struct and builder
crate::openai_compatible_provider!(
    Friendli,
    FriendliBuilder,
    FriendliProviderSettings,
    "friendli"
);

// Generate the language model implementation
crate::openai_compatible_language_model!(Friendli);
