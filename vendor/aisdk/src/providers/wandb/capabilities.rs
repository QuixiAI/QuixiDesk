//! Capabilities for wandb models.
//!
//! This module defines model types and their capabilities for wandb providers.
//! Users can implement additional traits on custom models.

use crate::core::capabilities::*;
use crate::model_capabilities;
use crate::providers::wandb::Wandb;

model_capabilities! {
    provider: Wandb,
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
            display_name: "Qwen3-235B-A22B-Thinking-2507",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        QwenQwen3Coder480bA35bInstruct {
            model_name: "Qwen/Qwen3-Coder-480B-A35B-Instruct",
            constructor_name: qwen_qwen3_coder_480b_a35b_instruct,
            display_name: "Qwen3-Coder-480B-A35B-Instruct",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        DeepseekAiDeepseekR10528 {
            model_name: "deepseek-ai/DeepSeek-R1-0528",
            constructor_name: deepseek_ai_deepseek_r1_0528,
            display_name: "DeepSeek-R1-0528",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        DeepseekAiDeepseekV30324 {
            model_name: "deepseek-ai/DeepSeek-V3-0324",
            constructor_name: deepseek_ai_deepseek_v3_0324,
            display_name: "DeepSeek-V3-0324",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MetaLlamaLlama318bInstruct {
            model_name: "meta-llama/Llama-3.1-8B-Instruct",
            constructor_name: meta_llama_llama_3_1_8b_instruct,
            display_name: "Meta-Llama-3.1-8B-Instruct",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MetaLlamaLlama3370bInstruct {
            model_name: "meta-llama/Llama-3.3-70B-Instruct",
            constructor_name: meta_llama_llama_3_3_70b_instruct,
            display_name: "Llama-3.3-70B-Instruct",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MetaLlamaLlama4Scout17b16eInstruct {
            model_name: "meta-llama/Llama-4-Scout-17B-16E-Instruct",
            constructor_name: meta_llama_llama_4_scout_17b_16e_instruct,
            display_name: "Llama 4 Scout 17B 16E Instruct",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MicrosoftPhi4MiniInstruct {
            model_name: "microsoft/Phi-4-mini-instruct",
            constructor_name: microsoft_phi_4_mini_instruct,
            display_name: "Phi-4-mini-instruct",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MoonshotaiKimiK2Instruct {
            model_name: "moonshotai/Kimi-K2-Instruct",
            constructor_name: moonshotai_kimi_k2_instruct,
            display_name: "Kimi-K2-Instruct",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
    }
}
