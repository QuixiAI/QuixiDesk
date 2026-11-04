//! Capabilities for nova models.
//!
//! This module defines model types and their capabilities for nova providers.
//! Users can implement additional traits on custom models.

use crate::core::capabilities::*;
use crate::model_capabilities;
use crate::providers::nova::Nova;

model_capabilities! {
    provider: Nova,
    models: {
        Nova2LiteV1 {
            model_name: "nova-2-lite-v1",
            constructor_name: nova_2_lite_v1,
            display_name: "Nova 2 Lite",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport, VideoInputSupport]
        },
        Nova2ProV1 {
            model_name: "nova-2-pro-v1",
            constructor_name: nova_2_pro_v1,
            display_name: "Nova 2 Pro",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport, VideoInputSupport]
        },
    }
}
