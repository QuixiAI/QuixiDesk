//! Capabilities for vultr models.
//!
//! This module defines model types and their capabilities for vultr providers.
//! Users can implement additional traits on custom models.

use crate::core::capabilities::*;
use crate::model_capabilities;
use crate::providers::vultr::Vultr;

model_capabilities! {
    provider: Vultr,
    models: {
        DeepseekR1DistillLlama70b {
            model_name: "deepseek-r1-distill-llama-70b",
            constructor_name: deepseek_r1_distill_llama_70b,
            display_name: "DeepSeek R1 Distill Llama 70B",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        DeepseekR1DistillQwen32b {
            model_name: "deepseek-r1-distill-qwen-32b",
            constructor_name: deepseek_r1_distill_qwen_32b,
            display_name: "DeepSeek R1 Distill Qwen 32B",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        GptOss120b {
            model_name: "gpt-oss-120b",
            constructor_name: gpt_oss_120b,
            display_name: "GPT OSS 120B",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        KimiK2Instruct {
            model_name: "kimi-k2-instruct",
            constructor_name: kimi_k2_instruct,
            display_name: "Kimi K2 Instruct",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Qwen25Coder32bInstruct {
            model_name: "qwen2.5-coder-32b-instruct",
            constructor_name: qwen2_5_coder_32b_instruct,
            display_name: "Qwen2.5 Coder 32B Instruct",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
    }
}
