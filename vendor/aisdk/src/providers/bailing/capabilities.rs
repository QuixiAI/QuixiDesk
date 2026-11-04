//! Capabilities for bailing models.
//!
//! This module defines model types and their capabilities for bailing providers.
//! Users can implement additional traits on custom models.

use crate::core::capabilities::*;
use crate::model_capabilities;
use crate::providers::bailing::Bailing;

model_capabilities! {
    provider: Bailing,
    models: {
        Ling1t {
            model_name: "Ling-1T",
            constructor_name: ling_1t,
            display_name: "Ling-1T",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Ring1t {
            model_name: "Ring-1T",
            constructor_name: ring_1t,
            display_name: "Ring-1T",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport]
        },
    }
}
