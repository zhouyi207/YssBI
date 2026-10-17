//! Domain-neutral tool declarations, effects, failures and model calls.

use std::borrow::Cow;
use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ToolInvocationId;

pub const MAX_CAPABILITY_RESULT_BYTES: usize = 1_048_576;

#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolEffect {
    Inspect,
    Compute,
    Mutate,
    Destructive,
    External,
}

impl ToolEffect {
    pub const fn default_timeout_ms(self) -> u64 {
        if matches!(self, Self::Compute) {
            60_000
        } else {
            30_000
        }
    }
}

/// A model-callable tool. The Harness treats its arguments, requests and results as opaque JSON.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub effect: ToolEffect,
    pub timeout_ms: u64,
}

impl ToolSpec {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: Value,
        effect: ToolEffect,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
            effect,
            timeout_ms: effect.default_timeout_ms(),
        }
    }
}

/// A provider tool call. The executor owns validation, durable identity and the model projection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolCall {
    pub name: String,
    pub arguments: Value,
}

/// The model-facing reply for one call. A fatal code stops the run before another model step.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolCallOutcome {
    /// The durable invocation, when the call reached the ledger or control lifecycle.
    pub invocation_id: Option<ToolInvocationId>,
    pub output: Value,
    pub fatal: Option<crate::AgentDriverFailureCode>,
}

/// One durable execution reported by the executor to its caller.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolInvocationOutcome {
    pub invocation_id: ToolInvocationId,
    pub result: Value,
}

/// Stable failure classification. Harness codes are declared here; extensions add their own codes.
#[derive(
    Clone, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct CapabilityFailureCode(Cow<'static, str>);

impl CapabilityFailureCode {
    pub const INVALID_REQUEST: Self = Self::from_static("invalid_request");
    pub const PROJECT_SESSION_UNAVAILABLE: Self = Self::from_static("project_session_unavailable");
    pub const PROJECT_SESSION_MISMATCH: Self = Self::from_static("project_session_mismatch");
    pub const PROJECT_SESSION_CHANGED: Self = Self::from_static("project_session_changed");
    pub const RESOURCE_UNAVAILABLE: Self = Self::from_static("resource_unavailable");
    pub const RESULT_UNAVAILABLE: Self = Self::from_static("result_unavailable");
    pub const REVISION_CONFLICT: Self = Self::from_static("revision_conflict");
    pub const MUTATION_REJECTED: Self = Self::from_static("mutation_rejected");
    pub const RESULT_TOO_LARGE: Self = Self::from_static("result_too_large");
    pub const CANCELLED: Self = Self::from_static("cancelled");
    pub const DEADLINE_ELAPSED: Self = Self::from_static("deadline_elapsed");
    pub const INVOCATION_CONFLICT: Self = Self::from_static("invocation_conflict");
    pub const PERSISTENCE_UNAVAILABLE: Self = Self::from_static("persistence_unavailable");
    pub const INTERNAL_FAILURE: Self = Self::from_static("internal_failure");
    pub const OUTCOME_UNKNOWN: Self = Self::from_static("outcome_unknown");

    pub const fn from_static(code: &'static str) -> Self {
        Self(Cow::Borrowed(code))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Codes that end the agent run instead of becoming model feedback.
    pub fn fatal(&self) -> Option<crate::AgentDriverFailureCode> {
        use crate::AgentDriverFailureCode as Fatal;
        if [
            Self::CANCELLED,
            Self::PROJECT_SESSION_UNAVAILABLE,
            Self::PROJECT_SESSION_MISMATCH,
            Self::PROJECT_SESSION_CHANGED,
        ]
        .contains(self)
        {
            Some(Fatal::Cancelled)
        } else if self == &Self::DEADLINE_ELAPSED {
            Some(Fatal::DeadlineElapsed)
        } else if self == &Self::PERSISTENCE_UNAVAILABLE || self == &Self::INTERNAL_FAILURE {
            Some(Fatal::InternalFailure)
        } else {
            None
        }
    }
}

impl std::fmt::Display for CapabilityFailureCode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[error("{code}")]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityFailure {
    pub code: CapabilityFailureCode,
    pub details: BTreeMap<String, String>,
}

impl CapabilityFailure {
    pub fn new(code: CapabilityFailureCode) -> Self {
        Self {
            code,
            details: BTreeMap::new(),
        }
    }

    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CapabilityContractError {
    #[error("invalid field '{0}'")]
    InvalidField(&'static str),
    #[error("field '{field}' exceeds {maximum} bytes")]
    FieldTooLong { field: &'static str, maximum: usize },
    #[error("result limit must be between 1 and {maximum}")]
    InvalidLimit { maximum: u16 },
}

impl CapabilityContractError {
    /// `limit_field` names the bounded collection reported by an `InvalidLimit` error.
    pub fn into_failure(self, tool: &str, limit_field: &str) -> CapabilityFailure {
        let failure = CapabilityFailure::new(CapabilityFailureCode::INVALID_REQUEST)
            .with_detail("capabilityId", tool);
        match self {
            Self::InvalidField(field) => failure.with_detail("field", field),
            Self::FieldTooLong { field, maximum } => failure
                .with_detail("field", field)
                .with_detail("maximumBytes", maximum.to_string()),
            Self::InvalidLimit { maximum } => failure
                .with_detail("field", limit_field)
                .with_detail("maximumResults", maximum.to_string())
                .with_detail(
                    "expected",
                    format!("An item count between 1 and {maximum}."),
                ),
        }
    }
}

pub const MAX_RESOURCE_ID_BYTES: usize = 1_024;

pub fn validate_resource_id(
    field: &'static str,
    value: &str,
) -> Result<(), CapabilityContractError> {
    if value.trim().is_empty() {
        return Err(CapabilityContractError::InvalidField(field));
    }
    if value.len() > MAX_RESOURCE_ID_BYTES {
        return Err(CapabilityContractError::FieldTooLong {
            field,
            maximum: MAX_RESOURCE_ID_BYTES,
        });
    }
    Ok(())
}
