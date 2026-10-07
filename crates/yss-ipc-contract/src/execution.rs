use crate::graph::PortAddressDto;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphOutputRefDto {
    pub graph_path: String,
    pub port: PortAddressDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphResultStateDto {
    pub revision: String,
    pub execution_session_id: String,
    pub semantic_input_hash: String,
    pub outputs: Box<[OutputResultStateDto]>,
    pub connections: Box<[ConnectionResultStateDto]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionResultStateDto {
    pub output: GraphOutputRefDto,
    pub input: PortAddressDto,
    pub state: ConnectionCacheStateDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionCacheStateDto {
    New,
    Stale,
    Valid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputResultStateDto {
    pub output: GraphOutputRefDto,
    pub state: ResultCacheStateDto,
    pub result_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    Node {
        node_id: String,
        mode: NodeExecutionModeDto,
    },
    Outputs {
        outputs: Box<[GraphOutputRefDto]>,
        include_default_results: bool,
        reuse_inputs: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NodeExecutionModeDto {
    CurrentInputs,
    Dependencies,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunErrorOutcomeDto {
    pub code: &'static str,
    pub phase: &'static str,
    pub source: Option<ResultInspectionSourceDto>,
    pub groups: Box<[GroupFailureContextDto]>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupFailureContextDto {
    pub caller: ResultInspectionSourceDto,
    pub function: String,
    pub ordinal: Option<String>,
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
    pub semantic_input_hash: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunEventDto {
    pub run: GraphRunIdentityDto,
    pub result_revision: String,
    pub kind: RunEventKindDto,
}
