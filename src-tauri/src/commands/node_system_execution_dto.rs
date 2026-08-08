use crate::node_system::analysis::{CompilationBasis, CorrelationContext, ResourceVersionSet};
use crate::node_system::document::{GraphResourcePath, GraphRevision, PortAddressDto};
use crate::node_system::plan::{ExecutionDemand, GraphOutputRef, MAX_SAFE_PREVIEW_GENERATION};
use crate::node_system::protocol::Value;
use crate::node_system::runtime::{
    ArtifactSnapshotKind, ResultSourceDescriptor, ResultSourcePage, RunErrorCode, RunEvent,
    RunEventKind,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphOutputRefDto {
    graph_path: String,
    port: PortAddressDto,
}

impl From<GraphOutputRef> for GraphOutputRefDto {
    fn from(output: GraphOutputRef) -> Self {
        Self {
            graph_path: output.graph_path.0.into(),
            port: output.port.into(),
        }
    }
}

impl TryFrom<GraphOutputRefDto> for GraphOutputRef {
    type Error = String;

    fn try_from(output: GraphOutputRefDto) -> Result<Self, Self::Error> {
        Ok(Self {
            graph_path: GraphResourcePath(output.graph_path.into()),
            port: output.port.try_into()?,
        })
    }
}

macro_rules! define_execution_demand_dto {
    ($($variant:ident => $wire_type:literal $({ $($field:ident: $field_type:ty),* $(,)? })?),* $(,)?) => {
        #[cfg(test)]
        pub(crate) const EXECUTION_DEMAND_DTO_WIRE_TYPES: [&str;
            [$(stringify!($variant)),*].len()] = [$($wire_type),*];

        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(
            tag = "type",
            rename_all = "camelCase",
            rename_all_fields = "camelCase",
            deny_unknown_fields
        )]
        pub enum ExecutionDemandDto {
            $($variant $({ $($field: $field_type),* })?),*
        }
    };
}

define_execution_demand_dto! {
    Default => "default" {},
    Outputs => "outputs" {
        outputs: Box<[GraphOutputRefDto]>,
        include_default_results: bool,
    },
    PinPreview => "pinPreview" {
        output: GraphOutputRefDto,
        generation: u64,
    },
}

impl TryFrom<ExecutionDemandDto> for ExecutionDemand {
    type Error = String;

    fn try_from(demand: ExecutionDemandDto) -> Result<Self, Self::Error> {
        match demand {
            ExecutionDemandDto::Default {} => Ok(Self::Default),
            ExecutionDemandDto::Outputs {
                outputs,
                include_default_results,
            } => Ok(Self::Outputs {
                outputs: outputs
                    .into_vec()
                    .into_iter()
                    .map(GraphOutputRef::try_from)
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
                include_default_results,
            }),
            ExecutionDemandDto::PinPreview { output, generation } => {
                if generation > MAX_SAFE_PREVIEW_GENERATION {
                    return Err(
                        "pin preview generation exceeds JavaScript safe integer range".into(),
                    );
                }
                Ok(Self::PinPreview {
                    output: output.try_into()?,
                    generation,
                })
            }
        }
    }
}

impl From<ExecutionDemand> for ExecutionDemandDto {
    fn from(demand: ExecutionDemand) -> Self {
        match demand {
            ExecutionDemand::Default => Self::Default {},
            ExecutionDemand::Outputs {
                outputs,
                include_default_results,
            } => Self::Outputs {
                outputs: outputs
                    .into_vec()
                    .into_iter()
                    .map(GraphOutputRefDto::from)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                include_default_results,
            },
            ExecutionDemand::PinPreview { output, generation } => Self::PinPreview {
                output: output.into(),
                generation,
            },
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CompilationBasisDto {
    graph_revision: String,
    registry_fingerprint: String,
    resource_versions: BTreeMap<String, String>,
}

impl From<CompilationBasis<GraphRevision>> for CompilationBasisDto {
    fn from(basis: CompilationBasis<GraphRevision>) -> Self {
        Self {
            graph_revision: basis.graph_revision.get().to_string(),
            registry_fingerprint: basis.registry_fingerprint.to_hex(),
            resource_versions: resource_versions(basis.resource_versions),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunCorrelationDto {
    project_session_id: String,
    graph_path: String,
    graph_revision: String,
    registry_fingerprint: String,
    resource_versions: BTreeMap<String, String>,
    compile_id: String,
    selection_digest: Option<Box<str>>,
    run_id: Option<String>,
    node_id: Option<String>,
    node_type_id: Option<String>,
    parent_call: Option<String>,
}

impl From<CorrelationContext> for RunCorrelationDto {
    fn from(correlation: CorrelationContext) -> Self {
        Self {
            project_session_id: correlation.project_session_id.as_str().to_owned(),
            graph_path: String::from(correlation.graph_path.0),
            graph_revision: correlation.graph_revision.get().to_string(),
            registry_fingerprint: correlation.registry_fingerprint.to_hex(),
            resource_versions: resource_versions(correlation.resource_versions),
            compile_id: correlation.compile_id.get().to_string(),
            selection_digest: correlation.selection_digest,
            run_id: correlation.run_id.map(|id| id.get().to_string()),
            node_id: correlation.node_id.map(|id| id.to_string()),
            node_type_id: correlation.node_type_id.map(|id| id.as_str().to_owned()),
            parent_call: correlation.parent_call.map(|id| id.get().to_string()),
        }
    }
}

macro_rules! define_run_event_kind_dto {
    ($($variant:ident => $wire_type:literal $({ $($field:ident: $field_type:ty),* $(,)? })?),* $(,)?) => {
        #[cfg(test)]
        pub(crate) const RUN_EVENT_KIND_DTO_WIRE_TYPES: [&str;
            [$(stringify!($variant)),*].len()] = [$($wire_type),*];

        #[derive(Debug, Serialize)]
        #[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
        pub(crate) enum RunEventKindDto {
            $($variant $({ $($field: $field_type),* })?),*
        }
    };
}

define_run_event_kind_dto! {
    RunStarted => "runStarted",
    RunCompleted => "runCompleted",
    RunErrored => "runErrored" { code: RunErrorCode },
    RunCancelled => "runCancelled",
    OperationStarted => "operationStarted" {
        operation_index: u32,
        activation_id: String,
    },
    OperationCompleted => "operationCompleted" {
        operation_index: u32,
        activation_id: String,
    },
    OperationErrored => "operationErrored" {
        operation_index: u32,
        activation_id: String,
        code: RunErrorCode,
    },
    ResultReady => "resultReady" { name: Box<str>, source_id: String },
    OutputReady => "outputReady" {
        output: GraphOutputRefDto,
        generation: Option<u64>,
        source_id: String,
    },
}

impl From<RunEventKind> for RunEventKindDto {
    fn from(kind: RunEventKind) -> Self {
        match kind {
            RunEventKind::RunStarted => Self::RunStarted,
            RunEventKind::RunCompleted => Self::RunCompleted,
            RunEventKind::RunErrored { code } => Self::RunErrored { code },
            RunEventKind::RunCancelled => Self::RunCancelled,
            RunEventKind::OperationStarted {
                operation_index,
                activation_id,
            } => Self::OperationStarted {
                operation_index,
                activation_id: activation_id.to_string(),
            },
            RunEventKind::OperationCompleted {
                operation_index,
                activation_id,
            } => Self::OperationCompleted {
                operation_index,
                activation_id: activation_id.to_string(),
            },
            RunEventKind::OperationErrored {
                operation_index,
                activation_id,
                code,
            } => Self::OperationErrored {
                operation_index,
                activation_id: activation_id.to_string(),
                code,
            },
            RunEventKind::ResultReady { name, source_id } => Self::ResultReady {
                name,
                source_id: source_id.get().to_string(),
            },
            RunEventKind::OutputReady {
                output,
                generation,
                source_id,
            } => Self::OutputReady {
                output: output.into(),
                generation,
                source_id: source_id.get().to_string(),
            },
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunEventDto {
    correlation: RunCorrelationDto,
    basis: CompilationBasisDto,
    kind: RunEventKindDto,
}

impl From<RunEvent> for RunEventDto {
    fn from(event: RunEvent) -> Self {
        Self {
            correlation: event.correlation.into(),
            basis: event.basis.into(),
            kind: event.kind.into(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultSourceDescriptorDto {
    source_id: String,
    artifact_id: String,
    name: Box<str>,
    kind: ArtifactSnapshotKind,
    total_count: usize,
    correlation: RunCorrelationDto,
    basis: CompilationBasisDto,
}

impl From<ResultSourceDescriptor> for ResultSourceDescriptorDto {
    fn from(descriptor: ResultSourceDescriptor) -> Self {
        Self {
            source_id: descriptor.source_id.get().to_string(),
            artifact_id: descriptor.artifact_id.get().to_string(),
            name: descriptor.name,
            kind: descriptor.kind,
            total_count: descriptor.total_count,
            correlation: descriptor.correlation.into(),
            basis: descriptor.basis.into(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultSourcePageDto {
    source_id: String,
    offset: usize,
    limit: usize,
    total_count: usize,
    values: Box<[Value]>,
}

impl From<ResultSourcePage> for ResultSourcePageDto {
    fn from(page: ResultSourcePage) -> Self {
        Self {
            source_id: page.source_id.get().to_string(),
            offset: page.offset,
            limit: page.limit,
            total_count: page.total_count,
            values: page.values,
        }
    }
}

fn resource_versions(versions: ResourceVersionSet) -> BTreeMap<String, String> {
    versions
        .into_iter()
        .map(|(key, version)| (key.as_str().to_owned(), version.as_str().to_owned()))
        .collect()
}

#[cfg(test)]
mod execution_demand_tests {
    use super::*;
    use crate::node_system::document::{PortAddress, PortAddressDto, PortRef};
    use crate::node_system::plan::ExecutionDemand;
    use serde_json::{Value, json};

    const NODE_ID: &str = "00000000-0000-0000-0000-000000000001";
    const INSTANCE_ID: &str = "00000000-0000-0000-0000-000000000002";

    fn decode(value: Value) -> ExecutionDemand {
        serde_json::from_value::<ExecutionDemandDto>(value)
            .unwrap()
            .try_into()
            .unwrap()
    }

    #[test]
    fn execution_demand_default_is_strict_and_has_no_compiler_local_identity() {
        assert_eq!(
            decode(json!({ "type": "default" })),
            ExecutionDemand::Default
        );

        for invalid in [
            json!({}),
            json!({ "type": "unknown" }),
            json!({ "type": "default", "outputs": [] }),
            json!({ "type": "default", "valueIndex": 0 }),
            json!({ "type": "default", "operationIndex": 0 }),
        ] {
            assert!(serde_json::from_value::<ExecutionDemandDto>(invalid).is_err());
        }
    }

    #[test]
    fn execution_demand_outputs_round_trip_declared_instance_empty_and_duplicate_order() {
        let declared = json!({
            "graphPath": "events/Main.yssbi-event",
            "port": { "kind": "declared", "nodeId": NODE_ID, "portKey": "result" }
        });
        let instance = json!({
            "graphPath": "events/Main.yssbi-event",
            "port": {
                "kind": "instance",
                "nodeId": NODE_ID,
                "templateKey": "results",
                "instanceId": INSTANCE_ID
            }
        });
        let wire = json!({
            "type": "outputs",
            "outputs": [declared.clone(), instance.clone(), declared],
            "includeDefaultResults": true
        });

        let demand = decode(wire.clone());
        let ExecutionDemand::Outputs {
            outputs,
            include_default_results,
        } = &demand
        else {
            panic!("expected output demand");
        };
        assert!(include_default_results);
        assert_eq!(outputs.len(), 3);
        assert_eq!(outputs[0], outputs[2]);
        assert!(matches!(outputs[0].port.port, PortRef::Declared { .. }));
        assert!(matches!(outputs[1].port.port, PortRef::Instance { .. }));

        let encoded = ExecutionDemandDto::from(demand);
        assert_eq!(serde_json::to_value(encoded).unwrap(), wire);
        assert_eq!(
            decode(json!({
                "type": "outputs",
                "outputs": [],
                "includeDefaultResults": false
            })),
            ExecutionDemand::Outputs {
                outputs: Box::new([]),
                include_default_results: false,
            }
        );
    }

    #[test]
    fn execution_demand_outputs_reject_missing_extra_and_compiler_local_fields() {
        for invalid in [
            json!({ "type": "outputs", "includeDefaultResults": false }),
            json!({ "type": "outputs", "outputs": [] }),
            json!({
                "type": "outputs",
                "outputs": [],
                "includeDefaultResults": false,
                "extra": true
            }),
            json!({
                "type": "outputs",
                "outputs": [{
                    "graphPath": "events/Main.yssbi-event",
                    "port": { "kind": "declared", "nodeId": NODE_ID, "portKey": "result" },
                    "valueIndex": 1
                }],
                "includeDefaultResults": false
            }),
            json!({
                "type": "outputs",
                "outputs": [{
                    "graphPath": "events/Main.yssbi-event",
                    "port": { "kind": "declared", "nodeId": NODE_ID, "portKey": "result" },
                    "operationIndex": 1
                }],
                "includeDefaultResults": false
            }),
        ] {
            assert!(serde_json::from_value::<ExecutionDemandDto>(invalid).is_err());
        }
    }

    #[test]
    fn pin_preview_demand_rejects_generation_above_javascript_safe_integer() {
        let output = json!({
            "graphPath": "events/Main.yssbi-event",
            "port": { "kind": "declared", "nodeId": NODE_ID, "portKey": "result" }
        });
        let maximum = serde_json::from_value::<ExecutionDemandDto>(json!({
            "type": "pinPreview",
            "output": output.clone(),
            "generation": 9_007_199_254_740_991_u64
        }))
        .unwrap();
        assert!(ExecutionDemand::try_from(maximum).is_ok());

        let unsafe_generation = serde_json::from_value::<ExecutionDemandDto>(json!({
            "type": "pinPreview",
            "output": output,
            "generation": 9_007_199_254_740_992_u64
        }))
        .unwrap();
        assert!(ExecutionDemand::try_from(unsafe_generation).is_err());
    }

    #[test]
    fn output_ready_serializes_only_stable_output_and_source_id() {
        let output = GraphOutputRefDto {
            graph_path: "events/Main.yssbi-event".into(),
            port: PortAddressDto::from(PortAddress::declared(
                crate::node_system::document::NodeId::from_uuid(
                    uuid::Uuid::parse_str(NODE_ID).unwrap(),
                ),
                crate::node_system::protocol::PortKey::new("result").unwrap(),
            )),
        };
        let wire = serde_json::to_value(RunEventKindDto::OutputReady {
            output,
            generation: None,
            source_id: "42".into(),
        })
        .unwrap();

        assert_eq!(
            wire,
            json!({
                "type": "outputReady",
                "output": {
                    "graphPath": "events/Main.yssbi-event",
                    "port": { "kind": "declared", "nodeId": NODE_ID, "portKey": "result" }
                },
                "generation": null,
                "sourceId": "42"
            })
        );
        assert!(wire.get("valueIndex").is_none());
        assert!(wire.get("operationIndex").is_none());
        assert!(wire.get("name").is_none());
    }
}
