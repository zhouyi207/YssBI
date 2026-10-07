use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandErrorDto {
    pub code: &'static str,
    pub details: Value,
    pub incident_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphMutationErrorDetailsDto {
    pub category: &'static str,
}

impl GraphMutationErrorDetailsDto {
    pub const VALUE: Self = Self {
        category: "graphMutation",
    };
}
