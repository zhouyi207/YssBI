//! Execution lifecycle/result encoding and channel delivery.
use crate::graph::run::{RunApplicationEvent, RunApplicationEventKind};
use tauri::ipc::Channel;
use yss_graph_execution::plan::{PlanOutputRef, PlanPortAddress};
use yss_ipc_contract::execution::*;
use yss_ipc_contract::graph::PortAddressDto;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunEventDtoError {
    UnsafePreviewGeneration,
    InvalidOutput,
}

pub fn output_dto(value: &PlanOutputRef) -> Result<GraphOutputRefDto, RunEventDtoError> {
    Ok(GraphOutputRefDto {
        graph_path: value.graph().as_str().to_owned(),
        port: port_address_dto(value.port())?,
    })
}

pub fn port_address_dto(value: &PlanPortAddress) -> Result<PortAddressDto, RunEventDtoError> {
    let parts = value.as_str().split(':').collect::<Vec<_>>();
    let port = match parts.as_slice() {
        [node_id, port_key] if uuid::Uuid::parse_str(node_id).is_ok() => PortAddressDto::Declared {
            node_id: (*node_id).into(),
            port_key: (*port_key).into(),
        },
        [node_id, template_key, instance_id]
            if uuid::Uuid::parse_str(node_id).is_ok()
                && uuid::Uuid::parse_str(instance_id).is_ok() =>
        {
            PortAddressDto::Instance {
                node_id: (*node_id).into(),
                template_key: (*template_key).into(),
                instance_id: (*instance_id).into(),
            }
        }
        _ => return Err(RunEventDtoError::InvalidOutput),
    };
    Ok(port)
}

fn run_failure_to_transport(
    failure: &yss_graph_execution::error::RunFailure,
) -> RunErrorOutcomeDto {
    use yss_graph_execution::error::{RunFailureCode, RunPhase};
    RunErrorOutcomeDto {
        code: match failure.code {
            RunFailureCode::KernelFailed => "kernelFailed",
            RunFailureCode::KernelNotFound => "kernelNotFound",
            RunFailureCode::InvalidNumericInput => "invalidNumericInput",
            RunFailureCode::DivisionByZero => "divisionByZero",
            RunFailureCode::NonFiniteResult => "nonFiniteResult",
            RunFailureCode::DeadlineExceeded => "deadlineExceeded",
            RunFailureCode::ResourceUnavailable => "resourceUnavailable",
            RunFailureCode::FinalizationFailed => "finalizationFailed",
        },
        phase: match failure.phase {
            RunPhase::Admission => "admission",
            RunPhase::PlanValidation => "planValidation",
            RunPhase::ResourcePreparation => "resourcePreparation",
            RunPhase::Execution => "execution",
            RunPhase::Finalization => "finalization",
        },
        source: failure
            .source
            .as_ref()
            .map(|source| ResultInspectionSourceDto {
                graph_path: source.graph().as_str().to_owned(),
                node_id: source.node().map(|node| node.as_str().to_owned()),
                port_address: source.port().map(|port| port.as_str().to_owned()),
            }),
    }
}

pub fn execution_event_to_transport(
    event: RunApplicationEvent,
) -> Result<RunEventDto, RunEventDtoError> {
    let identity = event.identity();
    let run = GraphRunIdentityDto {
        execution_session_id: identity.execution_session_id().as_uuid().to_string(),
        graph_path: identity.graph_path().as_str().to_owned(),
        run_id: identity.run_id().get().to_string(),
    };
    let kind = match event.kind() {
        RunApplicationEventKind::RunStarted { outputs } => RunEventKindDto::RunStarted {
            outputs: outputs.iter().map(output_dto).collect::<Result<_, _>>()?,
        },
        RunApplicationEventKind::RunCompleted => RunEventKindDto::RunCompleted,
        RunApplicationEventKind::RunCancelled => RunEventKindDto::RunCancelled,
        RunApplicationEventKind::RunErrored { failure } => RunEventKindDto::RunErrored {
            outcome: run_failure_to_transport(failure),
        },
        RunApplicationEventKind::PinPreviewResultReady {
            output,
            generation,
            result_id,
        } => {
            if *generation > MAX_SAFE_PREVIEW_GENERATION {
                return Err(RunEventDtoError::UnsafePreviewGeneration);
            }
            RunEventKindDto::PinPreviewResultReady {
                output: output_dto(output)?,
                generation: *generation,
                result_id: result_id.get().to_string(),
            }
        }
        RunApplicationEventKind::ResultInspectionRequested { result_id, source } => {
            RunEventKindDto::ResultInspectionRequested {
                result_id: result_id.get().to_string(),
                source: ResultInspectionSourceDto {
                    graph_path: source.graph().as_str().to_owned(),
                    node_id: source.node().map(|node| node.as_str().to_owned()),
                    port_address: source.port().map(|port| port.as_str().to_owned()),
                },
            }
        }
    };
    Ok(RunEventDto { run, kind })
}

pub struct TauriExecutionChannelAdapter {
    channel: Channel<RunEventDto>,
}

impl TauriExecutionChannelAdapter {
    pub fn new(channel: Channel<RunEventDto>) -> Self {
        Self { channel }
    }
    pub fn deliver(&self, event: RunApplicationEvent) -> bool {
        execution_event_to_transport(event).is_ok_and(|event| self.channel.send(event).is_ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_graph_execution::plan::{PlanGraphId, PlanNodeId, PlanSourceIdentity};

    #[test]
    fn run_failure_wire_preserves_the_cause_phase_and_node() {
        let failure = yss_graph_execution::error::RunFailure {
            code: yss_graph_execution::error::RunFailureCode::DivisionByZero,
            phase: yss_graph_execution::error::RunPhase::Execution,
            source: Some(PlanSourceIdentity::new(
                PlanGraphId::from_existing("events/contract.yssbi-event".into()),
                Some(PlanNodeId::from_existing(
                    "00000000-0000-0000-0000-000000000002".into(),
                )),
                None,
            )),
        };
        let actual = serde_json::to_value(RunEventKindDto::RunErrored {
            outcome: run_failure_to_transport(&failure),
        })
        .unwrap();
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../../src/tests/fixtures/node-system-contracts/execution-wire.json"
        ))
        .unwrap();
        let expected = fixture["runEvents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|event| event["kind"]["code"] == "divisionByZero")
            .unwrap();
        assert_eq!(actual, expected["kind"]);
    }
}
