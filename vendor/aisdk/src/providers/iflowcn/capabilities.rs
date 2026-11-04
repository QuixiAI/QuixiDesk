//! Capabilities for iflowcn models.
//!
//! This module defines model types and their capabilities for iflowcn providers.
//! Users can implement additional traits on custom models.

use crate::core::capabilities::*;
use crate::model_capabilities;
use crate::providers::iflowcn::Iflowcn;

model_capabilities! {
    provider: Iflowcn,
    models: {
        DeepseekR1 {
            model_name: "deepseek-r1",
            constructor_name: deepseek_r1,
            display_name: "DeepSeek-R1",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        DeepseekV3 {
            model_name: "deepseek-v3",
            constructor_name: deepseek_v3,
            display_name: "DeepSeek-V3",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        DeepseekV32 {
            model_name: "deepseek-v3.2",
            constructor_name: deepseek_v3_2,
            display_name: "DeepSeek-V3.2-Exp",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Glm46 {
            model_name: "glm-4.6",
            constructor_name: glm_4_6,
            display_name: "GLM-4.6",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        KimiK2 {
            model_name: "kimi-k2",
            constructor_name: kimi_k2,
            display_name: "Kimi-K2",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        KimiK20905 {
            model_name: "kimi-k2-0905",
            constructor_name: kimi_k2_0905,
            display_name: "Kimi-K2-0905",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Qwen3235b {
            model_name: "qwen3-235b",
            constructor_name: qwen3_235b,
            display_name: "Qwen3-235B-A22B",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Qwen3235bA22bInstruct {
            model_name: "qwen3-235b-a22b-instruct",
            constructor_name: qwen3_235b_a22b_instruct,
            display_name: "Qwen3-235B-A22B-Instruct",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Qwen3235bA22bThinking2507 {
            model_name: "qwen3-235b-a22b-thinking-2507",
            constructor_name: qwen3_235b_a22b_thinking_2507,
            display_name: "Qwen3-235B-A22B-Thinking",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Qwen332b {
            model_name: "qwen3-32b",
            constructor_name: qwen3_32b,
            display_name: "Qwen3-32B",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Qwen3CoderPlus {
            model_name: "qwen3-coder-plus",
            constructor_name: qwen3_coder_plus,
            display_name: "Qwen3-Coder-Plus",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Qwen3Max {
            model_name: "qwen3-max",
            constructor_name: qwen3_max,
            display_name: "Qwen3-Max",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Qwen3MaxPreview {
            model_name: "qwen3-max-preview",
            constructor_name: qwen3_max_preview,
            display_name: "Qwen3-Max-Preview",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Qwen3VlPlus {
            model_name: "qwen3-vl-plus",
            constructor_name: qwen3_vl_plus,
            display_name: "Qwen3-VL-Plus",
            capabilities: [ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
    }
}
