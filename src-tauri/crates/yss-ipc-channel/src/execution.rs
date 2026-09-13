//! Execution and Run Output encoding and channel delivery.
use tauri::ipc::Channel;
use yss_application::execution::run_graph::{RunApplicationEvent, RunApplicationEventKind};
use yss_execution::plan::{PlanOutputRef, PlanPortAddress};
use yss_execution::run_output::{RunOutputMessage, RunOutputStatus, RunOutputStream};
use yss_graph_document::GraphResourcePath;
use yss_ipc_contract::execution::*;
use yss_ipc_contract::graph::PortAddressDto;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunEventDtoError {
    UnsafePreviewGeneration,
    InvalidOutput,
    UnexpectedRunOutput,
}

pub fn output_dto(value: &PlanOutputRef) -> Result<GraphOutputRefDto, RunEventDtoError> {
    Ok(GraphOutputRefDto {
        graph_path: value.graph().as_str().to_owned(),
        port: port_address_dto(value.port())?,
    })
}

fn port_address_dto(value: &PlanPortAddress) -> Result<PortAddressDto, RunEventDtoError> {
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

fn run_output_source(
    source: &yss_execution::plan::PlanSourceIdentity,
) -> Result<(String, String, PortAddressDto), RunEventDtoError> {
    GraphResourcePath::new(source.graph().as_str()).map_err(|_| RunEventDtoError::InvalidOutput)?;
    let node = source.node().ok_or(RunEventDtoError::InvalidOutput)?;
    uuid::Uuid::parse_str(node.as_str()).map_err(|_| RunEventDtoError::InvalidOutput)?;
    let port = source.port().ok_or(RunEventDtoError::InvalidOutput)?;
    if port.as_str().split(':').next() != Some(node.as_str()) {
        return Err(RunEventDtoError::InvalidOutput);
    }
    Ok((
        source.graph().as_str().to_owned(),
        node.as_str().to_owned(),
        port_address_dto(port)?,
    ))
}

fn run_output_dto(
    message: &RunOutputMessage,
) -> Result<ExecutionChannelEventDto, RunEventDtoError> {
    match message {
        RunOutputMessage::Output(event) => {
            let (source_graph_path, source_node_id, source_port) =
                run_output_source(event.source())?;
            Ok(ExecutionChannelEventDto::Output(RunOutputEventDto {
                run_id: event.run_id().get().to_string(),
                sequence: event.sequence(),
                stream: run_output_stream_to_transport(event.stream()),
                text: event.text().into(),
                source_graph_path,
                source_node_id,
                source_port,
            }))
        }
        RunOutputMessage::Status(event) => {
            let (source_graph_path, source_node_id, source_port) =
                run_output_source(event.source())?;
            Ok(ExecutionChannelEventDto::OutputStatus(
                RunOutputStatusEventDto {
                    run_id: event.run_id().get().to_string(),
                    sequence: event.sequence(),
                    stream: run_output_stream_to_transport(event.stream()),
                    status: run_output_status_to_transport(event.status()),
                    source_graph_path,
                    source_node_id,
                    source_port,
                },
            ))
        }
    }
}

fn run_failure_to_transport(failure: &yss_execution::error::RunFailure) -> RunErrorOutcomeDto {
    use yss_execution::error::{RunFailureCode, RunPhase};
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

fn run_event_to_transport(event: RunApplicationEvent) -> Result<RunEventDto, RunEventDtoError> {
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
        RunApplicationEventKind::RunOutput(_) => {
            return Err(RunEventDtoError::UnexpectedRunOutput);
        }
    };
    Ok(RunEventDto { run, kind })
}

fn run_output_stream_to_transport(value: RunOutputStream) -> RunOutputStreamDto {
    match value {
        RunOutputStream::Stdout => RunOutputStreamDto::Stdout,
        RunOutputStream::Stderr => RunOutputStreamDto::Stderr,
    }
}

fn run_output_status_to_transport(value: RunOutputStatus) -> RunOutputStatusDto {
    match value {
        RunOutputStatus::Truncated => RunOutputStatusDto::Truncated,
        RunOutputStatus::Dropped => RunOutputStatusDto::Dropped,
    }
}

pub fn execution_event_to_transport(
    event: RunApplicationEvent,
) -> Result<ExecutionChannelEventDto, RunEventDtoError> {
    if let RunApplicationEventKind::RunOutput(message) = event.kind() {
        return run_output_dto(message);
    }
    Ok(ExecutionChannelEventDto::Event(run_event_to_transport(
        event,
    )?))
}

pub struct TauriExecutionChannelAdapter {
    channel: Channel<ExecutionChannelEventDto>,
}

impl TauriExecutionChannelAdapter {
    pub fn new(channel: Channel<ExecutionChannelEventDto>) -> Self {
        Self { channel }
    }
    pub fn deliver(&self, event: RunApplicationEvent) -> bool {
        execution_event_to_transport(event).is_ok_and(|event| self.channel.send(event).is_ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_execution::plan::{PlanGraphId, PlanNodeId, PlanSourceIdentity};
    use yss_execution::run_output::test_support;
    use yss_execution::run_registry::RunId;

    #[test]
    fn run_failure_wire_preserves_the_cause_phase_and_node() {
        let failure = yss_execution::error::RunFailure {
            code: yss_execution::error::RunFailureCode::DivisionByZero,
            phase: yss_execution::error::RunPhase::Execution,
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
            "../../../../src/tests/fixtures/node-system-contracts/execution-wire.json"
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

    #[test]
    fn run_output_uses_the_existing_flat_channel_wire_with_source_port() {
        let node_id = "00000000-0000-0000-0000-000000000002";
        let message = test_support::output(
            RunId::from_existing(1),
            1,
            RunOutputStream::Stdout,
            "Hello, World!",
            PlanSourceIdentity::new(
                PlanGraphId::from_existing("events/Output.yssbi-event".into()),
                Some(PlanNodeId::from_existing(node_id.into())),
                Some(PlanPortAddress::from_existing(
                    format!("{node_id}:message").into_boxed_str(),
                )),
            ),
        );

        let dto = run_output_dto(&message).expect("the runtime output source is valid");

        assert_eq!(
            serde_json::to_value(dto).expect("run output serializes"),
            serde_json::json!({
                "runId": "1",
                "sequence": 1,
                "stream": "stdout",
                "text": "Hello, World!",
                "sourceGraphPath": "events/Output.yssbi-event",
                "sourceNodeId": node_id,
                "sourcePort": {
                    "kind": "declared",
                    "nodeId": node_id,
                    "portKey": "message"
                }
            })
        );
    }
}
