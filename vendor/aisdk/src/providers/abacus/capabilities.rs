//! Capabilities for abacus models.
//!
//! This module defines model types and their capabilities for abacus providers.
//! Users can implement additional traits on custom models.

use crate::core::capabilities::*;
use crate::model_capabilities;
use crate::providers::abacus::Abacus;

model_capabilities! {
    provider: Abacus,
    models: {
        QwenQwq32b {
            model_name: "Qwen/QwQ-32B",
            constructor_name: qwen_qwq_32b,
            display_name: "QwQ 32B",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        QwenQwen2572bInstruct {
            model_name: "Qwen/Qwen2.5-72B-Instruct",
            constructor_name: qwen_qwen2_5_72b_instruct,
            display_name: "Qwen 2.5 72B Instruct",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        QwenQwen3235bA22bInstruct2507 {
            model_name: "Qwen/Qwen3-235B-A22B-Instruct-2507",
            constructor_name: qwen_qwen3_235b_a22b_instruct_2507,
            display_name: "Qwen3 235B A22B Instruct",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        QwenQwen332b {
            model_name: "Qwen/Qwen3-32B",
            constructor_name: qwen_qwen3_32b,
            display_name: "Qwen3 32B",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        QwenQwen3Coder480bA35bInstruct {
            model_name: "Qwen/qwen3-coder-480b-a35b-instruct",
            constructor_name: qwen_qwen3_coder_480b_a35b_instruct,
            display_name: "Qwen3 Coder 480B A35B Instruct",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Claude37Sonnet20250219 {
            model_name: "claude-3-7-sonnet-20250219",
            constructor_name: claude_3_7_sonnet_20250219,
            display_name: "Claude Sonnet 3.7",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        ClaudeHaiku4520251001 {
            model_name: "claude-haiku-4-5-20251001",
            constructor_name: claude_haiku_4_5_20251001,
            display_name: "Claude Haiku 4.5",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        ClaudeOpus4120250805 {
            model_name: "claude-opus-4-1-20250805",
            constructor_name: claude_opus_4_1_20250805,
            display_name: "Claude Opus 4.1",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        ClaudeOpus420250514 {
            model_name: "claude-opus-4-20250514",
            constructor_name: claude_opus_4_20250514,
            display_name: "Claude Opus 4",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        ClaudeOpus4520251101 {
            model_name: "claude-opus-4-5-20251101",
            constructor_name: claude_opus_4_5_20251101,
            display_name: "Claude Opus 4.5",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        ClaudeSonnet420250514 {
            model_name: "claude-sonnet-4-20250514",
            constructor_name: claude_sonnet_4_20250514,
            display_name: "Claude Sonnet 4",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        ClaudeSonnet4520250929 {
            model_name: "claude-sonnet-4-5-20250929",
            constructor_name: claude_sonnet_4_5_20250929,
            display_name: "Claude Sonnet 4.5",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        DeepseekAiDeepseekR1 {
            model_name: "deepseek-ai/DeepSeek-R1",
            constructor_name: deepseek_ai_deepseek_r1,
            display_name: "DeepSeek R1",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        DeepseekAiDeepseekV31Terminus {
            model_name: "deepseek-ai/DeepSeek-V3.1-Terminus",
            constructor_name: deepseek_ai_deepseek_v3_1_terminus,
            display_name: "DeepSeek V3.1 Terminus",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        DeepseekAiDeepseekV32 {
            model_name: "deepseek-ai/DeepSeek-V3.2",
            constructor_name: deepseek_ai_deepseek_v3_2,
            display_name: "DeepSeek V3.2",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        DeepseekDeepseekV31 {
            model_name: "deepseek/deepseek-v3.1",
            constructor_name: deepseek_deepseek_v3_1,
            display_name: "DeepSeek V3.1",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Gemini20Flash001 {
            model_name: "gemini-2.0-flash-001",
            constructor_name: gemini_2_0_flash_001,
            display_name: "Gemini 2.0 Flash",
            capabilities: [AudioInputSupport, ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport, VideoInputSupport]
        },
        Gemini20ProExp0205 {
            model_name: "gemini-2.0-pro-exp-02-05",
            constructor_name: gemini_2_0_pro_exp_02_05,
            display_name: "Gemini 2.0 Pro Exp",
            capabilities: [AudioInputSupport, ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport, VideoInputSupport]
        },
        Gemini25Flash {
            model_name: "gemini-2.5-flash",
            constructor_name: gemini_2_5_flash,
            display_name: "Gemini 2.5 Flash",
            capabilities: [AudioInputSupport, ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport, VideoInputSupport]
        },
        Gemini25Pro {
            model_name: "gemini-2.5-pro",
            constructor_name: gemini_2_5_pro,
            display_name: "Gemini 2.5 Pro",
            capabilities: [AudioInputSupport, ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport, VideoInputSupport]
        },
        Gemini3FlashPreview {
            model_name: "gemini-3-flash-preview",
            constructor_name: gemini_3_flash_preview,
            display_name: "Gemini 3 Flash Preview",
            capabilities: [AudioInputSupport, ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport, VideoInputSupport]
        },
        Gemini3ProPreview {
            model_name: "gemini-3-pro-preview",
            constructor_name: gemini_3_pro_preview,
            display_name: "Gemini 3 Pro Preview",
            capabilities: [AudioInputSupport, ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport, VideoInputSupport]
        },
        Gpt41 {
            model_name: "gpt-4.1",
            constructor_name: gpt_4_1,
            display_name: "GPT-4.1",
            capabilities: [ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Gpt41Mini {
            model_name: "gpt-4.1-mini",
            constructor_name: gpt_4_1_mini,
            display_name: "GPT-4.1 Mini",
            capabilities: [ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Gpt41Nano {
            model_name: "gpt-4.1-nano",
            constructor_name: gpt_4_1_nano,
            display_name: "GPT-4.1 Nano",
            capabilities: [ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Gpt4o20241120 {
            model_name: "gpt-4o-2024-11-20",
            constructor_name: gpt_4o_2024_11_20,
            display_name: "GPT-4o (2024-11-20)",
            capabilities: [AudioInputSupport, ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Gpt4oMini {
            model_name: "gpt-4o-mini",
            constructor_name: gpt_4o_mini,
            display_name: "GPT-4o Mini",
            capabilities: [ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Gpt5 {
            model_name: "gpt-5",
            constructor_name: gpt_5,
            display_name: "GPT-5",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Gpt5Mini {
            model_name: "gpt-5-mini",
            constructor_name: gpt_5_mini,
            display_name: "GPT-5 Mini",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Gpt5Nano {
            model_name: "gpt-5-nano",
            constructor_name: gpt_5_nano,
            display_name: "GPT-5 Nano",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Gpt51 {
            model_name: "gpt-5.1",
            constructor_name: gpt_5_1,
            display_name: "GPT-5.1",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Gpt51ChatLatest {
            model_name: "gpt-5.1-chat-latest",
            constructor_name: gpt_5_1_chat_latest,
            display_name: "GPT-5.1 Chat Latest",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Gpt52 {
            model_name: "gpt-5.2",
            constructor_name: gpt_5_2,
            display_name: "GPT-5.2",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Gpt52ChatLatest {
            model_name: "gpt-5.2-chat-latest",
            constructor_name: gpt_5_2_chat_latest,
            display_name: "GPT-5.2 Chat Latest",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Grok40709 {
            model_name: "grok-4-0709",
            constructor_name: grok_4_0709,
            display_name: "Grok 4",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Grok41FastNonReasoning {
            model_name: "grok-4-1-fast-non-reasoning",
            constructor_name: grok_4_1_fast_non_reasoning,
            display_name: "Grok 4.1 Fast (Non-Reasoning)",
            capabilities: [ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Grok4FastNonReasoning {
            model_name: "grok-4-fast-non-reasoning",
            constructor_name: grok_4_fast_non_reasoning,
            display_name: "Grok 4 Fast (Non-Reasoning)",
            capabilities: [ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        GrokCodeFast1 {
            model_name: "grok-code-fast-1",
            constructor_name: grok_code_fast_1,
            display_name: "Grok Code Fast 1",
            capabilities: [ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        KimiK2TurboPreview {
            model_name: "kimi-k2-turbo-preview",
            constructor_name: kimi_k2_turbo_preview,
            display_name: "Kimi K2 Turbo Preview",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Llama3370bVersatile {
            model_name: "llama-3.3-70b-versatile",
            constructor_name: llama_3_3_70b_versatile,
            display_name: "Llama 3.3 70B Versatile",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MetaLlamaLlama4Maverick17b128eInstructFp8 {
            model_name: "meta-llama/Llama-4-Maverick-17B-128E-Instruct-FP8",
            constructor_name: meta_llama_llama_4_maverick_17b_128e_instruct_fp8,
            display_name: "Llama 4 Maverick 17B 128E Instruct FP8",
            capabilities: [ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MetaLlamaMetaLlama31405bInstructTurbo {
            model_name: "meta-llama/Meta-Llama-3.1-405B-Instruct-Turbo",
            constructor_name: meta_llama_meta_llama_3_1_405b_instruct_turbo,
            display_name: "Llama 3.1 405B Instruct Turbo",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MetaLlamaMetaLlama3170bInstruct {
            model_name: "meta-llama/Meta-Llama-3.1-70B-Instruct",
            constructor_name: meta_llama_meta_llama_3_1_70b_instruct,
            display_name: "Llama 3.1 70B Instruct",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MetaLlamaMetaLlama318bInstruct {
            model_name: "meta-llama/Meta-Llama-3.1-8B-Instruct",
            constructor_name: meta_llama_meta_llama_3_1_8b_instruct,
            display_name: "Llama 3.1 8B Instruct",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        O3 {
            model_name: "o3",
            constructor_name: o3,
            display_name: "o3",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        O3Mini {
            model_name: "o3-mini",
            constructor_name: o3_mini,
            display_name: "o3-mini",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        O3Pro {
            model_name: "o3-pro",
            constructor_name: o3_pro,
            display_name: "o3-pro",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        O4Mini {
            model_name: "o4-mini",
            constructor_name: o4_mini,
            display_name: "o4-mini",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        OpenaiGptOss120b {
            model_name: "openai/gpt-oss-120b",
            constructor_name: openai_gpt_oss_120b,
            display_name: "GPT-OSS 120B",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Qwen25Coder32b {
            model_name: "qwen-2.5-coder-32b",
            constructor_name: qwen_2_5_coder_32b,
            display_name: "Qwen 2.5 Coder 32B",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Qwen3Max {
            model_name: "qwen3-max",
            constructor_name: qwen3_max,
            display_name: "Qwen3 Max",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        RouteLlm {
            model_name: "route-llm",
            constructor_name: route_llm,
            display_name: "Route LLM",
            capabilities: [ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        ZaiOrgGlm45 {
            model_name: "zai-org/glm-4.5",
            constructor_name: zai_org_glm_4_5,
            display_name: "GLM-4.5",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        ZaiOrgGlm46 {
            model_name: "zai-org/glm-4.6",
            constructor_name: zai_org_glm_4_6,
            display_name: "GLM-4.6",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        ZaiOrgGlm47 {
            model_name: "zai-org/glm-4.7",
            constructor_name: zai_org_glm_4_7,
            display_name: "GLM-4.7",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
    }
}
