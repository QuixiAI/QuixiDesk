//! This module provides the Firmware provider, wrapping OpenAI Chat Completions for Firmware requests.

pub mod capabilities;

// Generate the settings module
crate::openai_compatible_settings!(
    FirmwareProviderSettings,
    FirmwareProviderSettingsBuilder,
    "Firmware",
    "https://app.firmware.ai/api/v1",
    "FIRMWARE_API_KEY"
);

// Generate the provider struct and builder
crate::openai_compatible_provider!(
    Firmware,
    FirmwareBuilder,
    FirmwareProviderSettings,
    "firmware"
);

// Generate the language model implementation
crate::openai_compatible_language_model!(Firmware);
