use crate::node_system::analysis::{CompilationBasis, CorrelationContext, ResourceVersionSet};
use crate::node_system::document::GraphRevision;
use crate::node_system::protocol::Value;
use crate::node_system::runtime::{
    ArtifactSnapshotKind, ResultSourceDescriptor, ResultSourcePage, RunErrorCode, RunEvent,
    RunEventKind,
};
use serde::Serialize;
use std::collections::BTreeMap;

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
            run_id: correlation.run_id.map(|id| id.get().to_string()),
            node_id: correlation.node_id.map(|id| id.to_string()),
            node_type_id: correlation.node_type_id.map(|id| id.as_str().to_owned()),
            parent_call: correlation.parent_call.map(|id| id.get().to_string()),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum RunEventKindDto {
    RunStarted,
    RunCompleted,
    RunErrored {
        code: RunErrorCode,
    },
    RunCancelled,
    OperationStarted {
        #[serde(rename = "operationIndex")]
        operation_index: u32,
        #[serde(rename = "activationId")]
        activation_id: String,
    },
    OperationCompleted {
        #[serde(rename = "operationIndex")]
        operation_index: u32,
        #[serde(rename = "activationId")]
        activation_id: String,
    },
    OperationErrored {
        #[serde(rename = "operationIndex")]
        operation_index: u32,
        #[serde(rename = "activationId")]
        activation_id: String,
        code: RunErrorCode,
    },
    ValueReady {
        #[serde(rename = "valueIndex")]
        value_index: u32,
        #[serde(rename = "sourceId")]
        source_id: String,
    },
    ResultReady {
        name: Box<str>,
        #[serde(rename = "sourceId")]
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
            RunEventKind::ValueReady {
                value_index,
                source_id,
            } => Self::ValueReady {
                value_index,
                source_id: source_id.get().to_string(),
            },
            RunEventKind::ResultReady { name, source_id } => Self::ResultReady {
                name,
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
