//! Capabilities for xiaomi models.
//!
//! This module defines model types and their capabilities for xiaomi providers.
//! Users can implement additional traits on custom models.

use crate::core::capabilities::*;
use crate::model_capabilities;
use crate::providers::xiaomi::Xiaomi;

model_capabilities! {
    provider: Xiaomi,
    models: {
        MimoV2Flash {
            model_name: "mimo-v2-flash",
            constructor_name: mimo_v2_flash,
            display_name: "MiMo-V2-Flash",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
    }
}
