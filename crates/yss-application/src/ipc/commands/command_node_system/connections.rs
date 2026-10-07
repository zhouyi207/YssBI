use serde::{Deserialize, Serialize};
use tauri::State;
use yss_graph_editor::projection::{ConnectionDecision, ConnectionIntent};
use yss_ipc_contract::{graph::PortAddressDto, graph_editing::GraphEditVersionDto};
use yss_project_identity::ProjectInstanceId;

use crate::{ipc::error::CommandError, session::ApplicationState};

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionIntentDto {
    Connect,
    MoveConnections,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConnectionCandidatesRequestDto {
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    version: GraphEditVersionDto,
    source_port: PortAddressDto,
    intent: ConnectionIntentDto,
}

#[derive(Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ConnectionDecisionDto {
    Append,
    Replace {
        displaced_connection_ids: Vec<String>,
    },
    Invalid {
        reason: &'static str,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionCandidateDto {
    port: PortAddressDto,
    decision: ConnectionDecisionDto,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionCandidatesDto {
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    version: GraphEditVersionDto,
    semantic_input_hash: String,
    source_port: PortAddressDto,
    intent: ConnectionIntentDto,
    candidates: Vec<ConnectionCandidateDto>,
}

#[tauri::command]
pub async fn get_connection_candidates(
    state: State<'_, ApplicationState>,
    request: ConnectionCandidatesRequestDto,
) -> Result<ConnectionCandidatesDto, CommandError> {
    let application = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let graph = super::common::parse_graph_path(request.graph_path.clone())?;
        let source = request
            .source_port
            .clone()
            .try_into()
            .map_err(|_| CommandError::expected("invalid_output"))?;
        let version = crate::ipc::schema::graph_editing::graph_edit_version_from_transport(
            request.version.clone(),
        )?;
        let intent = match request.intent {
            ConnectionIntentDto::Connect => ConnectionIntent::Connect,
            ConnectionIntentDto::MoveConnections => ConnectionIntent::MoveConnections,
        };
        let projection = application
            .graph_connection_candidates(
                &request.project_instance_id,
                &graph,
                version,
                &source,
                intent,
            )
            .map_err(|error| {
                super::common::resource_mutation_to_command_error(error, "graph_edit_changed")
            })?;
        Ok(ConnectionCandidatesDto {
            project_instance_id: request.project_instance_id,
            graph_path: request.graph_path,
            version: request.version,
            semantic_input_hash: projection
                .semantic_input_hash
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
            source_port: request.source_port,
            intent: request.intent,
            candidates: projection
                .candidates
                .into_iter()
                .map(|candidate| ConnectionCandidateDto {
                    port: candidate.port.into(),
                    decision: match candidate.decision {
                        ConnectionDecision::Append => ConnectionDecisionDto::Append,
                        ConnectionDecision::Replace {
                            displaced_connection_ids,
                        } => ConnectionDecisionDto::Replace {
                            displaced_connection_ids: displaced_connection_ids
                                .into_iter()
                                .map(|id| id.to_string())
                                .collect(),
                        },
                        ConnectionDecision::Invalid { reason } => {
                            ConnectionDecisionDto::Invalid { reason }
                        }
                    },
                })
                .collect(),
        })
    })
    .await
    .map_err(CommandError::internal)?
}
