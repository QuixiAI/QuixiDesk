//! Capabilities for moonshotai models.
//!
//! This module defines model types and their capabilities for moonshotai providers.
//! Users can implement additional traits on custom models.

use crate::core::capabilities::*;
use crate::model_capabilities;
use crate::providers::moonshotai::Moonshotai;

model_capabilities! {
    provider: Moonshotai,
    models: {
        KimiK20711Preview {
            model_name: "kimi-k2-0711-preview",
            constructor_name: kimi_k2_0711_preview,
            display_name: "Kimi K2 0711",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        KimiK20905Preview {
            model_name: "kimi-k2-0905-preview",
            constructor_name: kimi_k2_0905_preview,
            display_name: "Kimi K2 0905",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        KimiK2Thinking {
            model_name: "kimi-k2-thinking",
            constructor_name: kimi_k2_thinking,
            display_name: "Kimi K2 Thinking",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        KimiK2ThinkingTurbo {
            model_name: "kimi-k2-thinking-turbo",
            constructor_name: kimi_k2_thinking_turbo,
            display_name: "Kimi K2 Thinking Turbo",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        KimiK2TurboPreview {
            model_name: "kimi-k2-turbo-preview",
            constructor_name: kimi_k2_turbo_preview,
            display_name: "Kimi K2 Turbo",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        KimiK25 {
            model_name: "kimi-k2.5",
            constructor_name: kimi_k2_5,
            display_name: "Kimi K2.5",
            capabilities: [ImageInputSupport, ReasoningSupport, StructuredOutputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport, VideoInputSupport]
        },
    }
}
