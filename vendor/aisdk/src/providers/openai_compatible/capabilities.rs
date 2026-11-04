//! Model capabilities for the OpenAI-compatible provider.

use crate::core::capabilities::*;
use crate::model_capabilities;
use crate::providers::OpenAICompatible;

model_capabilities! {
    provider: OpenAICompatible,
    models: {}
}
