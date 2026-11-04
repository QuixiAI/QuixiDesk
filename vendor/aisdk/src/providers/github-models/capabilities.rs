//! Capabilities for github_models models.
//!
//! This module defines model types and their capabilities for github_models providers.
//! Users can implement additional traits on custom models.

use crate::core::capabilities::*;
use crate::model_capabilities;
use crate::providers::github_models::GithubModels;

model_capabilities! {
    provider: GithubModels,
    models: {
        Ai21LabsAi21Jamba15Large {
            model_name: "ai21-labs/ai21-jamba-1.5-large",
            constructor_name: ai21_labs_ai21_jamba_1_5_large,
            display_name: "AI21 Jamba 1.5 Large",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Ai21LabsAi21Jamba15Mini {
            model_name: "ai21-labs/ai21-jamba-1.5-mini",
            constructor_name: ai21_labs_ai21_jamba_1_5_mini,
            display_name: "AI21 Jamba 1.5 Mini",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        CohereCohereCommandA {
            model_name: "cohere/cohere-command-a",
            constructor_name: cohere_cohere_command_a,
            display_name: "Cohere Command A",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        CohereCohereCommandR {
            model_name: "cohere/cohere-command-r",
            constructor_name: cohere_cohere_command_r,
            display_name: "Cohere Command R",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        CohereCohereCommandR082024 {
            model_name: "cohere/cohere-command-r-08-2024",
            constructor_name: cohere_cohere_command_r_08_2024,
            display_name: "Cohere Command R 08-2024",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        CohereCohereCommandRPlus {
            model_name: "cohere/cohere-command-r-plus",
            constructor_name: cohere_cohere_command_r_plus,
            display_name: "Cohere Command R+",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        CohereCohereCommandRPlus082024 {
            model_name: "cohere/cohere-command-r-plus-08-2024",
            constructor_name: cohere_cohere_command_r_plus_08_2024,
            display_name: "Cohere Command R+ 08-2024",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Core42Jais30bChat {
            model_name: "core42/jais-30b-chat",
            constructor_name: core42_jais_30b_chat,
            display_name: "JAIS 30b Chat",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        DeepseekDeepseekR1 {
            model_name: "deepseek/deepseek-r1",
            constructor_name: deepseek_deepseek_r1,
            display_name: "DeepSeek-R1",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        DeepseekDeepseekR10528 {
            model_name: "deepseek/deepseek-r1-0528",
            constructor_name: deepseek_deepseek_r1_0528,
            display_name: "DeepSeek-R1-0528",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        DeepseekDeepseekV30324 {
            model_name: "deepseek/deepseek-v3-0324",
            constructor_name: deepseek_deepseek_v3_0324,
            display_name: "DeepSeek-V3-0324",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MetaLlama3211bVisionInstruct {
            model_name: "meta/llama-3.2-11b-vision-instruct",
            constructor_name: meta_llama_3_2_11b_vision_instruct,
            display_name: "Llama-3.2-11B-Vision-Instruct",
            capabilities: [AudioInputSupport, ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MetaLlama3290bVisionInstruct {
            model_name: "meta/llama-3.2-90b-vision-instruct",
            constructor_name: meta_llama_3_2_90b_vision_instruct,
            display_name: "Llama-3.2-90B-Vision-Instruct",
            capabilities: [AudioInputSupport, ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MetaLlama3370bInstruct {
            model_name: "meta/llama-3.3-70b-instruct",
            constructor_name: meta_llama_3_3_70b_instruct,
            display_name: "Llama-3.3-70B-Instruct",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MetaLlama4Maverick17b128eInstructFp8 {
            model_name: "meta/llama-4-maverick-17b-128e-instruct-fp8",
            constructor_name: meta_llama_4_maverick_17b_128e_instruct_fp8,
            display_name: "Llama 4 Maverick 17B 128E Instruct FP8",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MetaLlama4Scout17b16eInstruct {
            model_name: "meta/llama-4-scout-17b-16e-instruct",
            constructor_name: meta_llama_4_scout_17b_16e_instruct,
            display_name: "Llama 4 Scout 17B 16E Instruct",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MetaMetaLlama370bInstruct {
            model_name: "meta/meta-llama-3-70b-instruct",
            constructor_name: meta_meta_llama_3_70b_instruct,
            display_name: "Meta-Llama-3-70B-Instruct",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MetaMetaLlama38bInstruct {
            model_name: "meta/meta-llama-3-8b-instruct",
            constructor_name: meta_meta_llama_3_8b_instruct,
            display_name: "Meta-Llama-3-8B-Instruct",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MetaMetaLlama31405bInstruct {
            model_name: "meta/meta-llama-3.1-405b-instruct",
            constructor_name: meta_meta_llama_3_1_405b_instruct,
            display_name: "Meta-Llama-3.1-405B-Instruct",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MetaMetaLlama3170bInstruct {
            model_name: "meta/meta-llama-3.1-70b-instruct",
            constructor_name: meta_meta_llama_3_1_70b_instruct,
            display_name: "Meta-Llama-3.1-70B-Instruct",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MetaMetaLlama318bInstruct {
            model_name: "meta/meta-llama-3.1-8b-instruct",
            constructor_name: meta_meta_llama_3_1_8b_instruct,
            display_name: "Meta-Llama-3.1-8B-Instruct",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MicrosoftMaiDsR1 {
            model_name: "microsoft/mai-ds-r1",
            constructor_name: microsoft_mai_ds_r1,
            display_name: "MAI-DS-R1",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MicrosoftPhi3Medium128kInstruct {
            model_name: "microsoft/phi-3-medium-128k-instruct",
            constructor_name: microsoft_phi_3_medium_128k_instruct,
            display_name: "Phi-3-medium instruct (128k)",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MicrosoftPhi3Medium4kInstruct {
            model_name: "microsoft/phi-3-medium-4k-instruct",
            constructor_name: microsoft_phi_3_medium_4k_instruct,
            display_name: "Phi-3-medium instruct (4k)",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MicrosoftPhi3Mini128kInstruct {
            model_name: "microsoft/phi-3-mini-128k-instruct",
            constructor_name: microsoft_phi_3_mini_128k_instruct,
            display_name: "Phi-3-mini instruct (128k)",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MicrosoftPhi3Mini4kInstruct {
            model_name: "microsoft/phi-3-mini-4k-instruct",
            constructor_name: microsoft_phi_3_mini_4k_instruct,
            display_name: "Phi-3-mini instruct (4k)",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MicrosoftPhi3Small128kInstruct {
            model_name: "microsoft/phi-3-small-128k-instruct",
            constructor_name: microsoft_phi_3_small_128k_instruct,
            display_name: "Phi-3-small instruct (128k)",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MicrosoftPhi3Small8kInstruct {
            model_name: "microsoft/phi-3-small-8k-instruct",
            constructor_name: microsoft_phi_3_small_8k_instruct,
            display_name: "Phi-3-small instruct (8k)",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MicrosoftPhi35MiniInstruct {
            model_name: "microsoft/phi-3.5-mini-instruct",
            constructor_name: microsoft_phi_3_5_mini_instruct,
            display_name: "Phi-3.5-mini instruct (128k)",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MicrosoftPhi35MoeInstruct {
            model_name: "microsoft/phi-3.5-moe-instruct",
            constructor_name: microsoft_phi_3_5_moe_instruct,
            display_name: "Phi-3.5-MoE instruct (128k)",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MicrosoftPhi35VisionInstruct {
            model_name: "microsoft/phi-3.5-vision-instruct",
            constructor_name: microsoft_phi_3_5_vision_instruct,
            display_name: "Phi-3.5-vision instruct (128k)",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MicrosoftPhi4 {
            model_name: "microsoft/phi-4",
            constructor_name: microsoft_phi_4,
            display_name: "Phi-4",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MicrosoftPhi4MiniInstruct {
            model_name: "microsoft/phi-4-mini-instruct",
            constructor_name: microsoft_phi_4_mini_instruct,
            display_name: "Phi-4-mini-instruct",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MicrosoftPhi4MiniReasoning {
            model_name: "microsoft/phi-4-mini-reasoning",
            constructor_name: microsoft_phi_4_mini_reasoning,
            display_name: "Phi-4-mini-reasoning",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MicrosoftPhi4MultimodalInstruct {
            model_name: "microsoft/phi-4-multimodal-instruct",
            constructor_name: microsoft_phi_4_multimodal_instruct,
            display_name: "Phi-4-multimodal-instruct",
            capabilities: [AudioInputSupport, ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MicrosoftPhi4Reasoning {
            model_name: "microsoft/phi-4-reasoning",
            constructor_name: microsoft_phi_4_reasoning,
            display_name: "Phi-4-Reasoning",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MistralAiCodestral2501 {
            model_name: "mistral-ai/codestral-2501",
            constructor_name: mistral_ai_codestral_2501,
            display_name: "Codestral 25.01",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MistralAiMinistral3b {
            model_name: "mistral-ai/ministral-3b",
            constructor_name: mistral_ai_ministral_3b,
            display_name: "Ministral 3B",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MistralAiMistralLarge2411 {
            model_name: "mistral-ai/mistral-large-2411",
            constructor_name: mistral_ai_mistral_large_2411,
            display_name: "Mistral Large 24.11",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MistralAiMistralMedium2505 {
            model_name: "mistral-ai/mistral-medium-2505",
            constructor_name: mistral_ai_mistral_medium_2505,
            display_name: "Mistral Medium 3 (25.05)",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MistralAiMistralNemo {
            model_name: "mistral-ai/mistral-nemo",
            constructor_name: mistral_ai_mistral_nemo,
            display_name: "Mistral Nemo",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MistralAiMistralSmall2503 {
            model_name: "mistral-ai/mistral-small-2503",
            constructor_name: mistral_ai_mistral_small_2503,
            display_name: "Mistral Small 3.1",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
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
            display_name: "GPT-4.1-mini",
            capabilities: [ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        OpenaiGpt41Nano {
            model_name: "openai/gpt-4.1-nano",
            constructor_name: openai_gpt_4_1_nano,
            display_name: "GPT-4.1-nano",
            capabilities: [ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        OpenaiGpt4o {
            model_name: "openai/gpt-4o",
            constructor_name: openai_gpt_4o,
            display_name: "GPT-4o",
            capabilities: [AudioInputSupport, ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        OpenaiGpt4oMini {
            model_name: "openai/gpt-4o-mini",
            constructor_name: openai_gpt_4o_mini,
            display_name: "GPT-4o mini",
            capabilities: [AudioInputSupport, ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        OpenaiO1 {
            model_name: "openai/o1",
            constructor_name: openai_o1,
            display_name: "OpenAI o1",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport]
        },
        OpenaiO1Mini {
            model_name: "openai/o1-mini",
            constructor_name: openai_o1_mini,
            display_name: "OpenAI o1-mini",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport]
        },
        OpenaiO1Preview {
            model_name: "openai/o1-preview",
            constructor_name: openai_o1_preview,
            display_name: "OpenAI o1-preview",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport]
        },
        OpenaiO3 {
            model_name: "openai/o3",
            constructor_name: openai_o3,
            display_name: "OpenAI o3",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport]
        },
        OpenaiO3Mini {
            model_name: "openai/o3-mini",
            constructor_name: openai_o3_mini,
            display_name: "OpenAI o3-mini",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport]
        },
        OpenaiO4Mini {
            model_name: "openai/o4-mini",
            constructor_name: openai_o4_mini,
            display_name: "OpenAI o4-mini",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport]
        },
        XaiGrok3 {
            model_name: "xai/grok-3",
            constructor_name: xai_grok_3,
            display_name: "Grok 3",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        XaiGrok3Mini {
            model_name: "xai/grok-3-mini",
            constructor_name: xai_grok_3_mini,
            display_name: "Grok 3 Mini",
            capabilities: [ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
    }
}
