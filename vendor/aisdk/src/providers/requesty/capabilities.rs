//! Capabilities for requesty models.
//!
//! This module defines model types and their capabilities for requesty providers.
//! Users can implement additional traits on custom models.

use crate::core::capabilities::*;
use crate::model_capabilities;
use crate::providers::requesty::Requesty;

model_capabilities! {
    provider: Requesty,
    models: {
        AnthropicClaude37Sonnet {
            model_name: "anthropic/claude-3-7-sonnet",
            constructor_name: anthropic_claude_3_7_sonnet,
            display_name: "Claude Sonnet 3.7",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        AnthropicClaudeHaiku45 {
            model_name: "anthropic/claude-haiku-4-5",
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
            model_name: "anthropic/claude-opus-4-1",
            constructor_name: anthropic_claude_opus_4_1,
            display_name: "Claude Opus 4.1",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        AnthropicClaudeOpus45 {
            model_name: "anthropic/claude-opus-4-5",
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
            model_name: "anthropic/claude-sonnet-4-5",
            constructor_name: anthropic_claude_sonnet_4_5,
            display_name: "Claude Sonnet 4.5",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        GoogleGemini25Flash {
            model_name: "google/gemini-2.5-flash",
            constructor_name: google_gemini_2_5_flash,
            display_name: "Gemini 2.5 Flash",
            capabilities: [AudioInputSupport, ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport, VideoInputSupport]
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
            display_name: "Gemini 3 Flash",
            capabilities: [AudioInputSupport, ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport, VideoInputSupport]
        },
        GoogleGemini3ProPreview {
            model_name: "google/gemini-3-pro-preview",
            constructor_name: google_gemini_3_pro_preview,
            display_name: "Gemini 3 Pro",
            capabilities: [AudioInputSupport, ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport, VideoInputSupport]
        },
        OpenaiGpt41 {
            model_name: "openai/gpt-4.1",
            constructor_name: openai_gpt_4_1,
            display_name: "GPT-4.1",
            capabilities: [ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        OpenaiGpt41Mini {
            model_name: "openai/gpt-4.1-mini",
            constructor_name: openai_gpt_4_1_mini,
            display_name: "GPT-4.1 Mini",
            capabilities: [ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        OpenaiGpt4oMini {
            model_name: "openai/gpt-4o-mini",
            constructor_name: openai_gpt_4o_mini,
            display_name: "GPT-4o Mini",
            capabilities: [ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        OpenaiGpt5 {
            model_name: "openai/gpt-5",
            constructor_name: openai_gpt_5,
            display_name: "GPT-5",
            capabilities: [AudioInputSupport, AudioOutputSupport, ImageInputSupport, ImageOutputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport, VideoInputSupport]
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
        OpenaiO4Mini {
            model_name: "openai/o4-mini",
            constructor_name: openai_o4_mini,
            display_name: "o4 Mini",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        XaiGrok4 {
            model_name: "xai/grok-4",
            constructor_name: xai_grok_4,
            display_name: "Grok 4",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        XaiGrok4Fast {
            model_name: "xai/grok-4-fast",
            constructor_name: xai_grok_4_fast,
            display_name: "Grok 4 Fast",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
    }
}
