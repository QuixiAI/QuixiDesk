//! Capabilities for morph models.
//!
//! This module defines model types and their capabilities for morph providers.
//! Users can implement additional traits on custom models.

use crate::core::capabilities::*;
use crate::model_capabilities;
use crate::providers::morph::Morph;

model_capabilities! {
    provider: Morph,
    models: {
        Auto {
            model_name: "auto",
            constructor_name: auto,
            display_name: "Auto",
            capabilities: [TextInputSupport, TextOutputSupport]
        },
        MorphV3Fast {
            model_name: "morph-v3-fast",
            constructor_name: morph_v3_fast,
            display_name: "Morph v3 Fast",
            capabilities: [TextInputSupport, TextOutputSupport]
        },
        MorphV3Large {
            model_name: "morph-v3-large",
            constructor_name: morph_v3_large,
            display_name: "Morph v3 Large",
            capabilities: [TextInputSupport, TextOutputSupport]
        },
    }
}
