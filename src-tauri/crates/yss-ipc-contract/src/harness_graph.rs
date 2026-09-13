use serde::Serialize;
use yss_automation_contract::CapabilityId;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessGraphToolRequestDto {
    pub request_id: String,
    pub session_id: String,
    pub project_instance_id: String,
    pub graph_path: String,
    pub capability_id: CapabilityId,
}

#[derive(Serialize)]
#[serde(tag = "type", content = "update", rename_all = "snake_case")]
pub enum HarnessGraphUpdateDto {
    None,
    Execution {
        #[serde(rename = "terminalEventSent")]
        terminal_event_sent: bool,
        status: String,
    },
    Draft(crate::graph_draft::GraphDraftTransformDto),
    Compilation(crate::graph_draft::CompileGraphDraftDto),
    Saved(crate::graph_draft::GraphDraftSaveDto),
}
