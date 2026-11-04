//! Capabilities for llama models.
//!
//! This module defines model types and their capabilities for llama providers.
//! Users can implement additional traits on custom models.

use crate::core::capabilities::*;
use crate::model_capabilities;
use crate::providers::llama::Llama;

model_capabilities! {
    provider: Llama,
    models: {
        CerebrasLlama4Maverick17b128eInstruct {
            model_name: "cerebras-llama-4-maverick-17b-128e-instruct",
            constructor_name: cerebras_llama_4_maverick_17b_128e_instruct,
            display_name: "Cerebras-Llama-4-Maverick-17B-128E-Instruct",
            capabilities: [ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        CerebrasLlama4Scout17b16eInstruct {
            model_name: "cerebras-llama-4-scout-17b-16e-instruct",
            constructor_name: cerebras_llama_4_scout_17b_16e_instruct,
            display_name: "Cerebras-Llama-4-Scout-17B-16E-Instruct",
            capabilities: [ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        GroqLlama4Maverick17b128eInstruct {
            model_name: "groq-llama-4-maverick-17b-128e-instruct",
            constructor_name: groq_llama_4_maverick_17b_128e_instruct,
            display_name: "Groq-Llama-4-Maverick-17B-128E-Instruct",
            capabilities: [ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Llama3370bInstruct {
            model_name: "llama-3.3-70b-instruct",
            constructor_name: llama_3_3_70b_instruct,
            display_name: "Llama-3.3-70B-Instruct",
            capabilities: [ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Llama338bInstruct {
            model_name: "llama-3.3-8b-instruct",
            constructor_name: llama_3_3_8b_instruct,
            display_name: "Llama-3.3-8B-Instruct",
            capabilities: [ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Llama4Maverick17b128eInstructFp8 {
            model_name: "llama-4-maverick-17b-128e-instruct-fp8",
            constructor_name: llama_4_maverick_17b_128e_instruct_fp8,
            display_name: "Llama-4-Maverick-17B-128E-Instruct-FP8",
            capabilities: [ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        Llama4Scout17b16eInstructFp8 {
            model_name: "llama-4-scout-17b-16e-instruct-fp8",
            constructor_name: llama_4_scout_17b_16e_instruct_fp8,
            display_name: "Llama-4-Scout-17B-16E-Instruct-FP8",
            capabilities: [ImageInputSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
    }
}
