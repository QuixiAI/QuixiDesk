//! Capabilities for fastrouter models.
//!
//! This module defines model types and their capabilities for fastrouter providers.
//! Users can implement additional traits on custom models.

use crate::core::capabilities::*;
use crate::model_capabilities;
use crate::providers::fastrouter::Fastrouter;

model_capabilities! {
    provider: Fastrouter,
    models: {
        AnthropicClaudeOpus41 {
            model_name: "anthropic/claude-opus-4.1",
            constructor_name: anthropic_claude_opus_4_1,
            display_name: "Claude Opus 4.1",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        AnthropicClaudeSonnet4 {
            model_name: "anthropic/claude-sonnet-4",
            constructor_name: anthropic_claude_sonnet_4,
            display_name: "Claude Sonnet 4",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        DeepseekAiDeepseekR1DistillLlama70b {
            model_name: "deepseek-ai/deepseek-r1-distill-llama-70b",
            constructor_name: deepseek_ai_deepseek_r1_distill_llama_70b,
            display_name: "DeepSeek R1 Distill Llama 70B",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport]
        },
        GoogleGemini25Flash {
            model_name: "google/gemini-2.5-flash",
            constructor_name: google_gemini_2_5_flash,
            display_name: "Gemini 2.5 Flash",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        GoogleGemini25Pro {
            model_name: "google/gemini-2.5-pro",
            constructor_name: google_gemini_2_5_pro,
            display_name: "Gemini 2.5 Pro",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MoonshotaiKimiK2 {
            model_name: "moonshotai/kimi-k2",
            constructor_name: moonshotai_kimi_k2,
            display_name: "Kimi K2",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        OpenaiGpt41 {
            model_name: "openai/gpt-4.1",
            constructor_name: openai_gpt_4_1,
            display_name: "GPT-4.1",
            capabilities: [ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        OpenaiGpt5 {
            model_name: "openai/gpt-5",
            constructor_name: openai_gpt_5,
            display_name: "GPT-5",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        OpenaiGpt5Mini {
            model_name: "openai/gpt-5-mini",
            constructor_name: openai_gpt_5_mini,
            display_name: "GPT-5 Mini",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        OpenaiGpt5Nano {
            model_name: "openai/gpt-5-nano",
            constructor_name: openai_gpt_5_nano,
            display_name: "GPT-5 Nano",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        OpenaiGptOss120b {
            model_name: "openai/gpt-oss-120b",
            constructor_name: openai_gpt_oss_120b,
            display_name: "GPT OSS 120B",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        OpenaiGptOss20b {
            model_name: "openai/gpt-oss-20b",
            constructor_name: openai_gpt_oss_20b,
            display_name: "GPT OSS 20B",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        QwenQwen3Coder {
            model_name: "qwen/qwen3-coder",
            constructor_name: qwen_qwen3_coder,
            display_name: "Qwen3 Coder",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        XAiGrok4 {
            model_name: "x-ai/grok-4",
            constructor_name: x_ai_grok_4,
            display_name: "Grok 4",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
    }
}
