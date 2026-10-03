//! Workbench intent protocol shared by desktop clients and automation.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiSource {
    pub execution_session_id: String,
    pub result_id: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum UiPanel {
    Project,
    Nodes,
    Commands,
    Details,
    Assistant,
    Problems,
    Output,
    Logs,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum UiIntent {
    OpenResource {
        resource: yss_project_identity::ProjectResourceRef,
        node_id: Option<String>,
    },
    OpenResult {
        source: UiSource,
    },
    ShowPanel {
        panel: UiPanel,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("invalid UI intent")]
pub struct InvalidUiIntent;

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        && !matches!(id, "__proto__" | "prototype" | "constructor")
}

fn valid_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

impl UiSource {
    pub fn validate(&self) -> Result<(), InvalidUiIntent> {
        if !valid_uuid(&self.execution_session_id)
            || self.result_id.is_empty()
            || self.result_id.len() > 20
            || self.result_id.starts_with('0')
            || !self.result_id.bytes().all(|b| b.is_ascii_digit())
            || self.result_id.parse::<u64>().is_err()
        {
            return Err(InvalidUiIntent);
        }
        Ok(())
    }
}

impl UiIntent {
    pub fn validate(&self) -> Result<(), InvalidUiIntent> {
        match self {
            Self::OpenResource { resource, node_id } => {
                if resource.id.is_empty()
                    || resource.id.len() > 4096
                    || node_id.as_ref().is_some_and(|id| {
                        !valid_uuid(id)
                            || !matches!(
                                resource.kind,
                                yss_project_identity::ProjectResourceKind::EventGraph
                                    | yss_project_identity::ProjectResourceKind::FunctionGraph
                            )
                    })
                {
                    return Err(InvalidUiIntent);
                }
            }
            Self::OpenResult { source } => source.validate()?,
            Self::ShowPanel { .. } => {}
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequestUiIntent {
    pub client_key: String,
    pub intent: UiIntent,
}

impl RequestUiIntent {
    pub fn validate(&self) -> Result<(), InvalidUiIntent> {
        if !valid_id(&self.client_key) {
            return Err(InvalidUiIntent);
        }
        self.intent.validate()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum UiIntentStatus {
    Pending,
    Claimed,
    Applied,
    Failed,
    Expired,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiIntentReceipt {
    pub id: String,
    pub intent: UiIntent,
    pub status: UiIntentStatus,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectUiIntentRequest {
    pub id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum UiEvent {
    Intent { receipt: UiIntentReceipt },
    Resync,
    SessionChanged,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intents_reject_invalid_result_and_node_references() {
        assert!(
            UiIntent::OpenResource {
                resource: yss_project_identity::ProjectResourceRef {
                    kind: yss_project_identity::ProjectResourceKind::EventGraph,
                    id: "events/test.yssbi-event".into()
                },
                node_id: Some("x".repeat(36))
            }
            .validate()
            .is_err()
        );
        assert!(
            UiSource {
                execution_session_id: "x".repeat(36),
                result_id: "1".into()
            }
            .validate()
            .is_err()
        );
    }
}
