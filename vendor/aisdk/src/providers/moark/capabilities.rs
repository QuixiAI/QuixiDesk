//! Capabilities for moark models.
//!
//! This module defines model types and their capabilities for moark providers.
//! Users can implement additional traits on custom models.

use crate::core::capabilities::*;
use crate::model_capabilities;
use crate::providers::moark::Moark;

model_capabilities! {
    provider: Moark,
    models: {
        Glm47 {
            model_name: "GLM-4.7",
            constructor_name: glm_4_7,
            display_name: "GLM-4.7",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MinimaxM21 {
            model_name: "MiniMax-M2.1",
            constructor_name: minimax_m2_1,
            display_name: "MiniMax-M2.1",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
    }
}
