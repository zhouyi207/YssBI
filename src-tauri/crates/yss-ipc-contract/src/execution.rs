use crate::graph::PortAddressDto;
use serde::{Deserialize, Serialize};

pub const MAX_SAFE_PREVIEW_GENERATION: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphOutputRefDto {
    pub graph_path: String,
    pub port: PortAddressDto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphResultStateDto {
    pub execution_session_id: String,
    pub semantic_input_hash: String,
    pub outputs: Box<[OutputResultStateDto]>,
    pub connections: Box<[ConnectionResultStateDto]>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionResultStateDto {
    pub output: GraphOutputRefDto,
    pub input: PortAddressDto,
    pub state: ConnectionCacheStateDto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionCacheStateDto {
    New,
    Stale,
    Valid,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputResultStateDto {
    pub output: GraphOutputRefDto,
    pub state: ResultCacheStateDto,
    pub result_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ResultCacheStateDto {
    Missing,
    Stale,
    Valid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ExecutionDemandDto {
    Default,
    Outputs {
        outputs: Box<[GraphOutputRefDto]>,
        include_default_results: bool,
    },
    PinPreview {
        output: GraphOutputRefDto,
        generation: u64,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunErrorOutcomeDto {
    pub code: &'static str,
    pub phase: &'static str,
    pub source: Option<ResultInspectionSourceDto>,
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RunEventKindDto {
    RunStarted {
        outputs: Box<[GraphOutputRefDto]>,
    },
    RunCompleted,
    RunErrored {
        #[serde(flatten)]
        outcome: RunErrorOutcomeDto,
    },
    RunCancelled,
    PinPreviewResultReady {
        output: GraphOutputRefDto,
        generation: u64,
        result_id: String,
    },
    ResultInspectionRequested {
        result_id: String,
        source: ResultInspectionSourceDto,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultInspectionSourceDto {
    pub graph_path: String,
    pub node_id: Option<String>,
    pub port_address: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphRunIdentityDto {
    pub execution_session_id: String,
    pub graph_path: String,
    pub run_id: String,
}

#[derive(Debug, Serialize)]
pub struct RunEventDto {
    pub run: GraphRunIdentityDto,
    pub kind: RunEventKindDto,
}
