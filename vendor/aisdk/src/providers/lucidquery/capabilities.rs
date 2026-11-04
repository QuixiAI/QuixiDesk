//! Capabilities for lucidquery models.
//!
//! This module defines model types and their capabilities for lucidquery providers.
//! Users can implement additional traits on custom models.

use crate::core::capabilities::*;
use crate::model_capabilities;
use crate::providers::lucidquery::Lucidquery;

model_capabilities! {
    provider: Lucidquery,
    models: {
        LucidnovaRf1100b {
            model_name: "lucidnova-rf1-100b",
            constructor_name: lucidnova_rf1_100b,
            display_name: "LucidNova RF1 100B",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
        LucidqueryNexusCoder {
            model_name: "lucidquery-nexus-coder",
            constructor_name: lucidquery_nexus_coder,
            display_name: "LucidQuery Nexus Coder",
            capabilities: [ImageInputSupport, ReasoningSupport, TextInputSupport, TextOutputSupport, ToolCallSupport]
        },
    }
}
