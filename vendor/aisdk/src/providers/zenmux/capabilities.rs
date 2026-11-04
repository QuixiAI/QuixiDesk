//! Capabilities for zenmux models.
//!
//! This module defines model types and their capabilities for zenmux providers.
//! Users can implement additional traits on custom models.

use crate::core::capabilities::*;
use crate::model_capabilities;
use crate::providers::zenmux::Zenmux;

model_capabilities! {
    provider: Zenmux,
    models: {
        AnthropicClaudeHaiku45 {
            model_name: "anthropic/claude-haiku-4.5",
            constructor_name: anthropic_claude_haiku_4_5,
            display_name: "Claude Haiku 4.5",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        AnthropicClaudeOpus4 {
            model_name: "anthropic/claude-opus-4",
            constructor_name: anthropic_claude_opus_4,
            display_name: "Claude Opus 4",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        AnthropicClaudeOpus41 {
            model_name: "anthropic/claude-opus-4.1",
            constructor_name: anthropic_claude_opus_4_1,
            display_name: "Claude Opus 4.1",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        AnthropicClaudeOpus45 {
            model_name: "anthropic/claude-opus-4.5",
            constructor_name: anthropic_claude_opus_4_5,
            display_name: "Claude Opus 4.5",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        AnthropicClaudeSonnet4 {
            model_name: "anthropic/claude-sonnet-4",
            constructor_name: anthropic_claude_sonnet_4,
            display_name: "Claude Sonnet 4",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        AnthropicClaudeSonnet45 {
            model_name: "anthropic/claude-sonnet-4.5",
            constructor_name: anthropic_claude_sonnet_4_5,
            display_name: "Claude Sonnet 4.5",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        BaiduErnie50ThinkingPreview {
            model_name: "baidu/ernie-5.0-thinking-preview",
            constructor_name: baidu_ernie_5_0_thinking_preview,
            display_name: "ERNIE-5.0-Thinking-Preview",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport, VideoInputSupport]
        },
        DeepseekDeepseekChat {
            model_name: "deepseek/deepseek-chat",
            constructor_name: deepseek_deepseek_chat,
            display_name: "DeepSeek-V3.2 (Non-thinking Mode)",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        DeepseekDeepseekReasoner {
            model_name: "deepseek/deepseek-reasoner",
            constructor_name: deepseek_deepseek_reasoner,
            display_name: "DeepSeek-V3.2 (Thinking Mode)",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        DeepseekDeepseekV32 {
            model_name: "deepseek/deepseek-v3.2",
            constructor_name: deepseek_deepseek_v3_2,
            display_name: "DeepSeek V3.2",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        DeepseekDeepseekV32Exp {
            model_name: "deepseek/deepseek-v3.2-exp",
            constructor_name: deepseek_deepseek_v3_2_exp,
            display_name: "DeepSeek-V3.2-Exp",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        GoogleGemini25Flash {
            model_name: "google/gemini-2.5-flash",
            constructor_name: google_gemini_2_5_flash,
            display_name: "Gemini 2.5 Flash",
            capabilities: [AudioInputSupport, ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        GoogleGemini25FlashLite {
            model_name: "google/gemini-2.5-flash-lite",
            constructor_name: google_gemini_2_5_flash_lite,
            display_name: "Gemini 2.5 Flash Lite",
            capabilities: [AudioInputSupport, ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        GoogleGemini25Pro {
            model_name: "google/gemini-2.5-pro",
            constructor_name: google_gemini_2_5_pro,
            display_name: "Gemini 2.5 Pro",
            capabilities: [AudioInputSupport, ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport, VideoInputSupport]
        },
        GoogleGemini3FlashPreview {
            model_name: "google/gemini-3-flash-preview",
            constructor_name: google_gemini_3_flash_preview,
            display_name: "Gemini 3 Flash Preview",
            capabilities: [AudioInputSupport, ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        GoogleGemini3FlashPreviewFree {
            model_name: "google/gemini-3-flash-preview-free",
            constructor_name: google_gemini_3_flash_preview_free,
            display_name: "Gemini 3 Flash Preview Free",
            capabilities: [AudioInputSupport, ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        GoogleGemini3ProPreview {
            model_name: "google/gemini-3-pro-preview",
            constructor_name: google_gemini_3_pro_preview,
            display_name: "Gemini 3 Pro Preview",
            capabilities: [AudioInputSupport, ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport, VideoInputSupport]
        },
        InclusionaiLing1t {
            model_name: "inclusionai/ling-1t",
            constructor_name: inclusionai_ling_1t,
            display_name: "Ling-1T",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        InclusionaiRing1t {
            model_name: "inclusionai/ring-1t",
            constructor_name: inclusionai_ring_1t,
            display_name: "Ring-1T",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        KuaishouKatCoderProV1 {
            model_name: "kuaishou/kat-coder-pro-v1",
            constructor_name: kuaishou_kat_coder_pro_v1,
            display_name: "KAT-Coder-Pro-V1",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        KuaishouKatCoderProV1Free {
            model_name: "kuaishou/kat-coder-pro-v1-free",
            constructor_name: kuaishou_kat_coder_pro_v1_free,
            display_name: "KAT-Coder-Pro-V1 Free",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MinimaxMinimaxM2 {
            model_name: "minimax/minimax-m2",
            constructor_name: minimax_minimax_m2,
            display_name: "MiniMax M2",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MinimaxMinimaxM21 {
            model_name: "minimax/minimax-m2.1",
            constructor_name: minimax_minimax_m2_1,
            display_name: "MiniMax M2.1",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MoonshotaiKimiK20905 {
            model_name: "moonshotai/kimi-k2-0905",
            constructor_name: moonshotai_kimi_k2_0905,
            display_name: "Kimi K2 0905",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MoonshotaiKimiK2Thinking {
            model_name: "moonshotai/kimi-k2-thinking",
            constructor_name: moonshotai_kimi_k2_thinking,
            display_name: "Kimi K2 Thinking",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MoonshotaiKimiK2ThinkingTurbo {
            model_name: "moonshotai/kimi-k2-thinking-turbo",
            constructor_name: moonshotai_kimi_k2_thinking_turbo,
            display_name: "Kimi K2 Thinking Turbo",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MoonshotaiKimiK25 {
            model_name: "moonshotai/kimi-k2.5",
            constructor_name: moonshotai_kimi_k2_5,
            display_name: "Kimi K2.5",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport, VideoInputSupport]
        },
        OpenaiGpt5 {
            model_name: "openai/gpt-5",
            constructor_name: openai_gpt_5,
            display_name: "GPT-5",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        OpenaiGpt5Codex {
            model_name: "openai/gpt-5-codex",
            constructor_name: openai_gpt_5_codex,
            display_name: "GPT-5 Codex",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        OpenaiGpt51 {
            model_name: "openai/gpt-5.1",
            constructor_name: openai_gpt_5_1,
            display_name: "GPT-5.1",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        OpenaiGpt51Chat {
            model_name: "openai/gpt-5.1-chat",
            constructor_name: openai_gpt_5_1_chat,
            display_name: "GPT-5.1 Chat",
            capabilities: [ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        OpenaiGpt51Codex {
            model_name: "openai/gpt-5.1-codex",
            constructor_name: openai_gpt_5_1_codex,
            display_name: "GPT-5.1-Codex",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        OpenaiGpt51CodexMini {
            model_name: "openai/gpt-5.1-codex-mini",
            constructor_name: openai_gpt_5_1_codex_mini,
            display_name: "GPT-5.1-Codex-Mini",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        OpenaiGpt52 {
            model_name: "openai/gpt-5.2",
            constructor_name: openai_gpt_5_2,
            display_name: "GPT-5.2",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        OpenaiGpt52Codex {
            model_name: "openai/gpt-5.2-codex",
            constructor_name: openai_gpt_5_2_codex,
            display_name: "GPT-5.2-Codex",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        QwenQwen3CoderPlus {
            model_name: "qwen/qwen3-coder-plus",
            constructor_name: qwen_qwen3_coder_plus,
            display_name: "Qwen3-Coder-Plus",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        QwenQwen3MaxThinking {
            model_name: "qwen/qwen3-max-thinking",
            constructor_name: qwen_qwen3_max_thinking,
            display_name: "Qwen3-Max-Thinking",
            capabilities: [ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        StepfunStep3 {
            model_name: "stepfun/step-3",
            constructor_name: stepfun_step_3,
            display_name: "Step-3",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        VolcengineDoubaoSeed18 {
            model_name: "volcengine/doubao-seed-1.8",
            constructor_name: volcengine_doubao_seed_1_8,
            display_name: "Doubao-Seed-1.8",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        VolcengineDoubaoSeedCode {
            model_name: "volcengine/doubao-seed-code",
            constructor_name: volcengine_doubao_seed_code,
            display_name: "Doubao-Seed-Code",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        XAiGrok4 {
            model_name: "x-ai/grok-4",
            constructor_name: x_ai_grok_4,
            display_name: "Grok 4",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        XAiGrok4Fast {
            model_name: "x-ai/grok-4-fast",
            constructor_name: x_ai_grok_4_fast,
            display_name: "Grok 4 Fast",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        XAiGrok41Fast {
            model_name: "x-ai/grok-4.1-fast",
            constructor_name: x_ai_grok_4_1_fast,
            display_name: "Grok 4.1 Fast",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        XAiGrok41FastNonReasoning {
            model_name: "x-ai/grok-4.1-fast-non-reasoning",
            constructor_name: x_ai_grok_4_1_fast_non_reasoning,
            display_name: "Grok 4.1 Fast Non Reasoning",
            capabilities: [ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        XAiGrokCodeFast1 {
            model_name: "x-ai/grok-code-fast-1",
            constructor_name: x_ai_grok_code_fast_1,
            display_name: "Grok Code Fast 1",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        XiaomiMimoV2Flash {
            model_name: "xiaomi/mimo-v2-flash",
            constructor_name: xiaomi_mimo_v2_flash,
            display_name: "MiMo-V2-Flash",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        XiaomiMimoV2FlashFree {
            model_name: "xiaomi/mimo-v2-flash-free",
            constructor_name: xiaomi_mimo_v2_flash_free,
            display_name: "MiMo-V2-Flash Free",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        ZAiGlm45 {
            model_name: "z-ai/glm-4.5",
            constructor_name: z_ai_glm_4_5,
            display_name: "GLM 4.5",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        ZAiGlm45Air {
            model_name: "z-ai/glm-4.5-air",
            constructor_name: z_ai_glm_4_5_air,
            display_name: "GLM 4.5 Air",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        ZAiGlm46 {
            model_name: "z-ai/glm-4.6",
            constructor_name: z_ai_glm_4_6,
            display_name: "GLM 4.6",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        ZAiGlm46v {
            model_name: "z-ai/glm-4.6v",
            constructor_name: z_ai_glm_4_6v,
            display_name: "GLM 4.6V",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport, VideoInputSupport]
        },
        ZAiGlm46vFlash {
            model_name: "z-ai/glm-4.6v-flash",
            constructor_name: z_ai_glm_4_6v_flash,
            display_name: "GLM 4.6V FlashX",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport, VideoInputSupport]
        },
        ZAiGlm46vFlashFree {
            model_name: "z-ai/glm-4.6v-flash-free",
            constructor_name: z_ai_glm_4_6v_flash_free,
            display_name: "GLM 4.6V Flash (Free)",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport, VideoInputSupport]
        },
        ZAiGlm47 {
            model_name: "z-ai/glm-4.7",
            constructor_name: z_ai_glm_4_7,
            display_name: "GLM 4.7",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        ZAiGlm47Flashx {
            model_name: "z-ai/glm-4.7-flashx",
            constructor_name: z_ai_glm_4_7_flashx,
            display_name: "GLM 4.7 FlashX",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
    }
}
