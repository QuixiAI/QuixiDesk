//! Capabilities for lmstudio models.
//!
//! This module defines model types and their capabilities for lmstudio providers.
//! Users can implement additional traits on custom models.

use crate::core::capabilities::*;
use crate::model_capabilities;
use crate::providers::lmstudio::Lmstudio;

model_capabilities! {
    provider: Lmstudio,
    models: {
        OpenaiGptOss20b {
            model_name: "openai/gpt-oss-20b",
            constructor_name: openai_gpt_oss_20b,
            display_name: "GPT OSS 20B",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        QwenQwen330bA3b2507 {
            model_name: "qwen/qwen3-30b-a3b-2507",
            constructor_name: qwen_qwen3_30b_a3b_2507,
            display_name: "Qwen3 30B A3B 2507",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        QwenQwen3Coder30b {
            model_name: "qwen/qwen3-coder-30b",
            constructor_name: qwen_qwen3_coder_30b,
            display_name: "Qwen3 Coder 30B",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
    }
}
