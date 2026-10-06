//! Workbench intentions share the opaque references returned by result tools.
use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequestUiIntentInput {
    pub intent: UiIntentInput,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum UiIntentInput {
    OpenResource {
        resource: ProjectResourceRef,
        node_id: Option<String>,
    },
    OpenResult {
        /// Copy the complete resultRef returned by a result tool.
        result_ref: ResultRef,
    },
    ShowPanel {
        panel: yss_ui_contract::UiPanel,
    },
}

impl From<UiIntentInput> for yss_ui_contract::UiIntent {
    fn from(value: UiIntentInput) -> Self {
        match value {
            UiIntentInput::OpenResource { resource, node_id } => {
                Self::OpenResource { resource, node_id }
            }
            UiIntentInput::OpenResult { result_ref } => Self::OpenResult {
                source: yss_ui_contract::UiSource {
                    execution_session_id: result_ref.execution_session_id().to_owned(),
                    result_id: result_ref.result_id().to_string(),
                },
            },
            UiIntentInput::ShowPanel { panel } => Self::ShowPanel { panel },
        }
    }
}

impl TryFrom<&yss_ui_contract::UiIntent> for UiIntentInput {
    type Error = yss_ui_contract::InvalidUiIntent;

    fn try_from(value: &yss_ui_contract::UiIntent) -> Result<Self, Self::Error> {
        use yss_ui_contract::UiIntent;
        Ok(match value {
            UiIntent::OpenResource { resource, node_id } => Self::OpenResource {
                resource: resource.clone(),
                node_id: node_id.clone(),
            },
            UiIntent::OpenResult { source } => {
                source.validate()?;
                Self::OpenResult {
                    result_ref: ResultRef::new(
                        source.execution_session_id.clone(),
                        source
                            .result_id
                            .parse()
                            .map_err(|_| yss_ui_contract::InvalidUiIntent)?,
                    ),
                }
            }
            UiIntent::ShowPanel { panel } => Self::ShowPanel { panel: *panel },
        })
    }
}
