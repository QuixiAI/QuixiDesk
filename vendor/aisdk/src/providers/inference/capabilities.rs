//! Capabilities for inference models.
//!
//! This module defines model types and their capabilities for inference providers.
//! Users can implement additional traits on custom models.

use crate::core::capabilities::*;
use crate::model_capabilities;
use crate::providers::inference::Inference;

model_capabilities! {
    provider: Inference,
    models: {
        GoogleGemma3 {
            model_name: "google/gemma-3",
            constructor_name: google_gemma_3,
            display_name: "Google Gemma 3",
            capabilities: [ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MetaLlama318bInstruct {
            model_name: "meta/llama-3.1-8b-instruct",
            constructor_name: meta_llama_3_1_8b_instruct,
            display_name: "Llama 3.1 8B Instruct",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MetaLlama3211bVisionInstruct {
            model_name: "meta/llama-3.2-11b-vision-instruct",
            constructor_name: meta_llama_3_2_11b_vision_instruct,
            display_name: "Llama 3.2 11B Vision Instruct",
            capabilities: [ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MetaLlama321bInstruct {
            model_name: "meta/llama-3.2-1b-instruct",
            constructor_name: meta_llama_3_2_1b_instruct,
            display_name: "Llama 3.2 1B Instruct",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MetaLlama323bInstruct {
            model_name: "meta/llama-3.2-3b-instruct",
            constructor_name: meta_llama_3_2_3b_instruct,
            display_name: "Llama 3.2 3B Instruct",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        MistralMistralNemo12bInstruct {
            model_name: "mistral/mistral-nemo-12b-instruct",
            constructor_name: mistral_mistral_nemo_12b_instruct,
            display_name: "Mistral Nemo 12B Instruct",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        OsmosisOsmosisStructure06b {
            model_name: "osmosis/osmosis-structure-0.6b",
            constructor_name: osmosis_osmosis_structure_0_6b,
            display_name: "Osmosis Structure 0.6B",
            capabilities: [TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        QwenQwen257bVisionInstruct {
            model_name: "qwen/qwen-2.5-7b-vision-instruct",
            constructor_name: qwen_qwen_2_5_7b_vision_instruct,
            display_name: "Qwen 2.5 7B Vision Instruct",
            capabilities: [ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        QwenQwen3Embedding4b {
            model_name: "qwen/qwen3-embedding-4b",
            constructor_name: qwen_qwen3_embedding_4b,
            display_name: "Qwen 3 Embedding 4B",
            capabilities: [TextInputSupport, TextOutputSupport]
        },
    }
}
