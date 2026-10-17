//! Harness supplies call identity; the workbench owner retains intent validation and execution.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequestUiIntent {
    pub client_key: String,
    pub input: crate::model::RequestUiIntentInput,
}

impl RequestUiIntent {
    pub fn validate(&self) -> Result<(), yss_ui_contract::InvalidUiIntent> {
        yss_ui_contract::RequestUiIntent::from(self.clone()).validate()
    }
}

impl From<RequestUiIntent> for yss_ui_contract::RequestUiIntent {
    fn from(value: RequestUiIntent) -> Self {
        Self {
            client_key: value.client_key,
            intent: value.input.intent.into(),
        }
    }
}
