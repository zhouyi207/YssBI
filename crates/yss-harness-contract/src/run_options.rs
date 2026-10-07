//! User-selected execution options and provider-reported consumption.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessMode {
    Ask,
    #[default]
    Write,
}

impl HarnessMode {
    pub fn allows(self, capability: crate::CapabilityId) -> bool {
        self == Self::Write || capability.descriptor().effect == crate::ToolEffect::Inspect
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    Low,
    Medium,
    High,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HarnessTurnOptions {
    pub mode: HarnessMode,
    pub reasoning_effort: Option<ReasoningEffort>,
}

/// Input includes cache tokens; output includes reasoning. Unknown is not zero.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelTokenUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cached_input_tokens: Option<u64>,
    pub cache_creation_input_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelCallPurpose {
    Response,
    Compaction,
}
