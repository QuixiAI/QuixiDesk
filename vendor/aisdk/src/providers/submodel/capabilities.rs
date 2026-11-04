//! Capabilities for submodel models.
//!
//! This module defines model types and their capabilities for submodel providers.
//! Users can implement additional traits on custom models.

use crate::core::capabilities::*;
use crate::model_capabilities;
use crate::providers::submodel::Submodel;

model_capabilities! {
    provider: Submodel,
    models: {
        QwenQwen3235bA22bInstruct2507 {
            model_name: "Qwen/Qwen3-235B-A22B-Instruct-2507",
            constructor_name: qwen_qwen3_235b_a22b_instruct_2507,
            display_name: "Qwen3 235B A22B Instruct 2507",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        QwenQwen3235bA22bThinking2507 {
            model_name: "Qwen/Qwen3-235B-A22B-Thinking-2507",
            constructor_name: qwen_qwen3_235b_a22b_thinking_2507,
            display_name: "Qwen3 235B A22B Thinking 2507",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        QwenQwen3Coder480bA35bInstructFp8 {
            model_name: "Qwen/Qwen3-Coder-480B-A35B-Instruct-FP8",
            constructor_name: qwen_qwen3_coder_480b_a35b_instruct_fp8,
            display_name: "Qwen3 Coder 480B A35B Instruct",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        DeepseekAiDeepseekR10528 {
            model_name: "deepseek-ai/DeepSeek-R1-0528",
            constructor_name: deepseek_ai_deepseek_r1_0528,
            display_name: "DeepSeek R1 0528",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        DeepseekAiDeepseekV30324 {
            model_name: "deepseek-ai/DeepSeek-V3-0324",
            constructor_name: deepseek_ai_deepseek_v3_0324,
            display_name: "DeepSeek V3 0324",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        DeepseekAiDeepseekV31 {
            model_name: "deepseek-ai/DeepSeek-V3.1",
            constructor_name: deepseek_ai_deepseek_v3_1,
            display_name: "DeepSeek V3.1",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        OpenaiGptOss120b {
            model_name: "openai/gpt-oss-120b",
            constructor_name: openai_gpt_oss_120b,
            display_name: "GPT OSS 120B",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        ZaiOrgGlm45Air {
            model_name: "zai-org/GLM-4.5-Air",
            constructor_name: zai_org_glm_4_5_air,
            display_name: "GLM 4.5 Air",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        ZaiOrgGlm45Fp8 {
            model_name: "zai-org/GLM-4.5-FP8",
            constructor_name: zai_org_glm_4_5_fp8,
            display_name: "GLM 4.5 FP8",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
    }
}
