#[path = "tests/errors.rs"]
mod errors;
#[path = "tests/execution.rs"]
mod execution;
#[path = "tests/graph_lifecycle.rs"]
mod graph_lifecycle;
#[path = "tests/mutation.rs"]
mod mutation;
#[path = "tests/projection.rs"]
mod projection;
#[path = "tests/resources.rs"]
mod resources;
#[path = "tests/results.rs"]
mod results;

use super::*;
use crate::application::graph_execution::{DeliveryDisposition, TerminalRunEventDelivery};
use crate::application::graph_execution::{
    GraphExecutionDeliveryReport, GraphExecutionStreamEvent, TerminalRunEventKind,
};
use crate::commands::node_system_execution_dto::ResultStateKindDto;
use crate::commands::node_system_execution_dto::ResultValueDto;
use crate::event::{Event, EventProject};
use crate::node_system::ProjectSessionId;

use crate::node_system::catalog::NodeCreationDescriptor;
use crate::node_system::document::{
    FunctionDocumentPatch, FunctionResourceKey, ResourceKey, ResourceRevision,
};
use crate::node_system::document::{
    HistoryMutation, MutationRequest, NodeId, OperationId, PortAddressDto,
};

use crate::node_system::runtime::{
    ActivationId, ActivationProvenance, GraphRunIdentity, PendingOutputDescriptor, ResultId,
    ResultStore, ResultUsage, RunEvent, RunEventKind, RunId, RunOutputMessage, StoredValue,
};
use crate::project::{
    GraphDocumentKind, GraphResourceDocument, GraphResourcePath, ProjectData, fixtures,
};
use crate::project::{ProjectInstanceId, ProjectState};
use std::sync::Arc;

fn graph_mutation_request(graph_path: &GraphResourcePath) -> serde_json::Value {
    serde_json::json!({
        "resource": { "kind": "graph", "key": graph_path.as_str() },
        "baseRevision": 0,
        "operationId": "00000000-0000-0000-0000-000000000806",
        "payload": {
            "type": "createNode",
            "payload": {
                "descriptor": {
                    "kind": "static",
                    "nodeTypeId": "yssbi.constant.int64"
                },
                "position": { "x": 1.0, "y": 2.0 },
                "userLabel": null
            }
        }
    })
}

fn resource_bound_graph_mutation_request(graph_path: &GraphResourcePath) -> serde_json::Value {
    serde_json::json!({
        "resource": { "kind": "graph", "key": graph_path.as_str() },
        "baseRevision": 0,
        "operationId": "00000000-0000-0000-0000-000000000807",
        "payload": {
            "type": "createNode",
            "payload": {
                "descriptor": {
                    "kind": "resourceBound",
                    "nodeTypeId": "yssbi.project.function.call",
                    "resourcePath": "functions/Helper.yssbi-function",
                    "resourceRevision": 0,
                    "createArgs": { "kind": "function" }
                },
                "position": { "x": 1.0, "y": 2.0 },
                "userLabel": null
            }
        }
    })
}

fn graph_project(graph_path: &GraphResourcePath) -> ProjectData {
    let mut project = ProjectData::new();
    project.graphs.insert(
        graph_path.clone(),
        GraphResourceDocument::new("Main", GraphDocumentKind::Event),
    );
    project
}

fn function_project(graph_path: &GraphResourcePath) -> ProjectData {
    let mut project = ProjectData::new();
    project.graphs.insert(
        graph_path.clone(),
        GraphResourceDocument::new("Compute", GraphDocumentKind::Function),
    );
    project
}

fn history_request(graph_path: &GraphResourcePath) -> MutationRequest<HistoryMutation> {
    MutationRequest::new(
        ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
            graph_path.as_str().into(),
        )),
        ResourceRevision::INITIAL,
        OperationId::new(),
        HistoryMutation {},
    )
}

fn function_signature_request(
    graph_path: &GraphResourcePath,
) -> MutationRequest<FunctionDocumentPatch> {
    MutationRequest::new(
        ResourceKey::Function(FunctionResourceKey(graph_path.as_str().into())),
        ResourceRevision::INITIAL,
        OperationId::new(),
        FunctionDocumentPatch::new(Default::default(), Default::default()),
    )
}

fn terminal_delivery_report(
    kind: TerminalRunEventKind,
    disposition: DeliveryDisposition,
) -> GraphExecutionDeliveryReport {
    GraphExecutionDeliveryReport {
        delivered_event_count: usize::from(disposition == DeliveryDisposition::Delivered),
        rejected_event_count: usize::from(disposition == DeliveryDisposition::Rejected),
        terminal: Some(TerminalRunEventDelivery { kind, disposition }),
    }
}

fn insert_ready_result(
    store: &ResultStore,
    run_id: RunId,
    activation_id: ActivationId,
    output: crate::node_system::plan::GraphOutputRef,
) -> ResultId {
    let group = store
        .create_pending_group(
            ActivationProvenance {
                run_id,
                activation_id,
                graph_path: output.graph_path.clone(),
                graph_revision: crate::node_system::document::GraphRevision::new(1),
                node_id: output.port.node_id,
                created_at_ms: activation_id.get(),
                usage: ResultUsage::Produced,
            },
            &[PendingOutputDescriptor {
                value: crate::node_system::plan::ValueRef::new(activation_id.get() as u32),
                output: Some(output),
                presentation: crate::node_system::plan::ResultPresentation::Inspector,
                contract: crate::node_system::plan::PlannedValueContract::opaque(),
            }],
        )
        .unwrap();
    let result_id = group.output_result_ids[0];
    store
        .complete_group(
            &group,
            vec![StoredValue::scalar(
                crate::node_system::protocol::Value::Integer(activation_id.get() as i64),
            )]
            .into_boxed_slice(),
        )
        .unwrap();
    result_id
}

fn test_output(graph_path: &str) -> crate::node_system::plan::GraphOutputRef {
    crate::node_system::plan::GraphOutputRef {
        graph_path: crate::node_system::document::GraphResourcePath(graph_path.into()),
        port: crate::node_system::document::PortAddress::declared(
            crate::node_system::document::NodeId::from_uuid(uuid::Uuid::nil()),
            crate::node_system::protocol::PortKey::new("result").unwrap(),
        ),
    }
}
