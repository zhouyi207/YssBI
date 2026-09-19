use super::*;

use crate::session::{ApplicationSession, ApplicationSessionEpoch, ApplicationSessionSlot};
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};
use std::thread;
use yss_database_contract::{
    DatabaseDecl, DatabaseDeclarationObservation, DatabaseDeclarationObservationSet, DatabaseId,
    DatabaseSessionIdentity, DatabaseSessionOpenRequest,
};
use yss_database_runtime::runtime::DatabaseRuntimeRegistry;
use yss_graph_document::GraphResourceKind;
use yss_graph_document::{
    DocumentNode, GraphDocument, GraphResourcePath, NodeId, NodePosition, ParameterValues,
    PortAddress,
};
use yss_graph_execution::identity::{ExecutionSessionId, RuntimeGeneration};
use yss_graph_execution::resource_preparation::ResourceProviderFactory;
use yss_graph_execution::state::ExecutionRuntimeState;
use yss_graph_runtime::{
    GraphRuntimeComponents, GraphRuntimeEpoch, GraphRuntimeState, GraphRuntimeTestControl,
    GraphRuntimeTestEvent,
};
use yss_node_catalog::build_builtin_node_system;
use yss_node_protocol::{NodeTypeId, PortKey};
use yss_project::ProjectState;
use yss_project_identity::ProjectSessionId;
use yss_project_model::{GraphResourceDocument, ProjectData};

struct TestProject {
    root: PathBuf,
    state: Arc<ProjectState>,
}

impl TestProject {
    fn active(label: &str, data: ProjectData) -> Self {
        let root = std::env::temp_dir().join(format!(
            "yssbi-catalog-query-{label}-{}",
            uuid::Uuid::new_v4()
        ));
        yss_project::fixtures::write_project(&data, root.to_string_lossy().as_ref()).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), data);
        Self {
            root,
            state: Arc::new(state),
        }
    }
}

impl Drop for TestProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

struct StagedSession {
    _project: TestProject,
    session: Arc<ApplicationSession>,
    application: ApplicationState,
    slot: Arc<ApplicationSessionSlot>,
    control: GraphRuntimeTestControl,
}

fn staged_session(
    data: ProjectData,
    label: &str,
    control: GraphRuntimeTestControl,
) -> StagedSession {
    let project = TestProject::active(label, data);
    let project_state = Arc::clone(&project.state);
    let project_instance_id = project_state.capture_project_session().unwrap().instance_id;
    let project_session_id =
        ProjectSessionId::new(format!("catalog-query-project-{}", uuid::Uuid::new_v4()));
    let execution_session_id = ExecutionSessionId::new(uuid::Uuid::new_v4());
    let builtin = build_builtin_node_system().unwrap();
    let graph = Arc::new(GraphRuntimeState::new_for_test(
        GraphRuntimeEpoch::from_existing(1),
        GraphRuntimeComponents {
            registry: builtin.registry,
            catalog: builtin.catalog,
        },
        control.clone(),
    ));
    let declarations: Arc<[DatabaseDecl]> = Vec::new().into();
    let observations = DatabaseDeclarationObservationSet::try_from_iter(std::iter::empty::<(
        DatabaseId,
        DatabaseDeclarationObservation,
    )>())
    .unwrap();
    let database = Arc::new(
        DatabaseRuntimeRegistry::new()
            .open_session(DatabaseSessionOpenRequest::new(
                DatabaseSessionIdentity::from_existing(project_session_id.as_str().into()),
                NonZeroU64::new(1).unwrap(),
                None,
                declarations,
                observations,
            ))
            .unwrap(),
    );
    let execution = Arc::new(ExecutionRuntimeState::new(
        execution_session_id,
        RuntimeGeneration::from_existing(1),
        yss_node_kernel::KernelRegistry::default().into(),
    ));
    let session = Arc::new(ApplicationSession::new_for_test(
        ApplicationSessionEpoch::from_existing(1),
        project_instance_id,
        project_session_id.clone(),
        execution_session_id,
        RuntimeGeneration::from_existing(1),
        project_state,
        graph,
        execution,
        database,
        Arc::new(ResourceProviderFactory::new(
            project_session_id.as_str().into(),
        )),
    ));
    let slot = Arc::new(ApplicationSessionSlot::new(
        crate::session::NodeComponents::builtins().unwrap(),
    ));
    slot.publish_for_test(Arc::clone(&session));
    let application = ApplicationState::new(Arc::clone(&slot));
    StagedSession {
        _project: project,
        session,
        application,
        slot,
        control,
    }
}

fn compatible_project(path: &GraphResourcePath) -> ProjectData {
    let mut project = ProjectData::new();
    project.graphs.insert(
        path.clone(),
        GraphResourceDocument::new("Main", GraphResourceKind::Event),
    );
    project
}

fn compatible_draft(source_node: NodeId) -> GraphDocument {
    let mut graph = GraphResourceDocument::new("Main", GraphResourceKind::Event);
    graph.document.nodes.insert(
        source_node,
        DocumentNode {
            id: source_node,
            node_type: NodeTypeId::new("yssbi.constant.get").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: ParameterValues::new(),
            user_label: None,
        },
    );
    set_constant(
        &mut graph.document,
        source_node,
        yss_data_contract::ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
        yss_data_contract::DataValue::Int64(0),
    );
    graph.document
}

#[test]
fn renamed_unloaded_function_caller_keeps_bound_ports_in_semantic_projection() {
    use yss_graph_document::{
        ConnectionId, DocumentConnection, DynamicMemberLocator, DynamicPortBinding,
        FunctionParameterId, InputState, LastKnownPortMetadata, OrderKey, PortInstanceId,
    };
    use yss_project_identity::{OperationId, ResourceRevision};
    let function = GraphResourcePath::new("functions/F.yssbi-function").unwrap();
    let caller = GraphResourcePath::new("events/Caller.yssbi-event").unwrap();
    let source = NodeId::new();
    let call = NodeId::new();
    let mut document = compatible_draft(source);
    document.nodes.insert(
        call,
        DocumentNode {
            id: call,
            node_type: "yssbi.project.function.call".parse().unwrap(),
            position: NodePosition { x: 100., y: 0. },
            user_label: None,
            parameters: [(
                "target".parse().unwrap(),
                serde_json::json!(function.as_str()),
            )]
            .into(),
        },
    );
    let addresses = [0, 1]
        .map(|_| PortAddress::instance(call, "arguments".parse().unwrap(), PortInstanceId::new()));
    for (index, address) in addresses.iter().enumerate() {
        document.port_bindings.insert(
            address.clone(),
            DynamicPortBinding::Resolved {
                origin: DynamicMemberLocator::FunctionParameter {
                    function: function.clone(),
                    parameter: FunctionParameterId::new(format!("p{index}")),
                },
                order: OrderKey::new(index.to_string()),
                last_known: LastKnownPortMetadata::default(),
            },
        );
    }
    let connection = ConnectionId::new();
    document.connections.insert(
        connection,
        DocumentConnection {
            id: connection,
            output: PortAddress::declared(source, "value".parse().unwrap()),
            input: addresses[0].clone(),
            order: None,
        },
    );
    document.input_states.insert(
        addresses[1].clone(),
        InputState {
            literal_override: Some(yss_node_protocol::TypedValue {
                value_type: yss_node_protocol::TypeExpr::Concrete("core.numeric".parse().unwrap()),
                value: yss_node_protocol::Value::Integer(3),
            }),
        },
    );
    let mut definition = GraphResourceDocument::new("F", GraphResourceKind::Function);
    definition.function.as_mut().unwrap().signature.parameters = (0..2)
        .map(|i| yss_project_history::FunctionParameter {
            id: FunctionParameterId::new(format!("p{i}")),
            name: format!("p{i}"),
            type_name: "Numeric".into(),
        })
        .collect();
    let mut resource = GraphResourceDocument::new("Caller", GraphResourceKind::Event);
    resource.document = document.clone();
    let mut project = ProjectData::new();
    project.graphs.insert(function.clone(), definition);
    project.graphs.insert(caller.clone(), resource);
    let session = staged_session(
        project,
        "rename-bound-ports",
        GraphRuntimeTestControl::default(),
    );
    let instance = session.session.project_instance_id().clone();
    session
        .application
        .unload_graph_resource(instance.clone(), caller.clone(), 100, None)
        .unwrap();
    session
        .application
        .rename_graph_resource(
            instance.clone(),
            function,
            ResourceRevision::INITIAL,
            "G".into(),
            200,
            OperationId::new(),
        )
        .unwrap();
    assert!(
        session
            .session
            .project()
            .read_resident_graph(&caller)
            .unwrap()
            .is_none()
    );
    let saved = session
        .session
        .project()
        .read_graph_resource_snapshot(&instance, &caller)
        .unwrap();
    assert_eq!(saved.document.connections, document.connections);
    assert_eq!(saved.document.input_states, document.input_states);
    let projection = session
        .application
        .resolve_graph_document(instance, caller, saved.document, "en-US".into())
        .unwrap();
    let projected = projection
        .nodes
        .iter()
        .find(|node| node.node_id == call)
        .unwrap();
    for address in addresses {
        assert!(projected.ports.iter().any(|port| port.address == address));
    }
    assert!(projected.port_instance_additions.is_empty());
    assert!(
        projected
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code.as_ref() != "graph.port.orphan")
    );
}

#[test]
fn localized_catalog_rejects_stale_project_identity() {
    let session = staged_session(
        ProjectData::new(),
        "localized-stale-project",
        GraphRuntimeTestControl::default(),
    );
    let stale = ProjectInstanceId::from_existing("stale-project-instance".into());

    let error = session
        .application
        .localized_node_catalog(LocalizedCatalogRequest::new(stale, "en-US"))
        .unwrap_err();

    assert!(matches!(
        error,
        CatalogQueryApplicationError::CatalogProjectStale
    ));
    assert!(session.control.events().is_empty());
}

#[test]
fn localized_catalog_returns_resources_from_the_same_coherent_snapshot() {
    let function_path = GraphResourcePath::new("functions/Sales Report.yssbi-function").unwrap();
    let mut project = ProjectData::new();
    project.graphs.insert(
        function_path.clone(),
        GraphResourceDocument::new("Sales Report", GraphResourceKind::Function),
    );
    let session = staged_session(
        project,
        "localized-coherent-resource",
        GraphRuntimeTestControl::default(),
    );
    let project_instance_id = session.session.project_instance_id().clone();

    let catalog = session
        .application
        .localized_node_catalog(LocalizedCatalogRequest::new(
            project_instance_id.clone(),
            "zh-CN",
        ))
        .unwrap();

    assert_eq!(catalog.project_instance_id, project_instance_id);
    assert_eq!(catalog.resource_publication_revision, 0);
    for id in ["length", "count", "sum", "mean"] {
        assert!(
            !catalog
                .catalog
                .items
                .iter()
                .find(|item| item.node_type_id.as_ref() == format!("yssbi.dataframe.series.{id}"))
                .expect("unavailable definitions remain visible")
                .available
        );
    }
    assert!(
        catalog
            .catalog
            .items
            .iter()
            .any(|item| item.node_type_id.as_ref() == "yssbi.numeric.add" && item.available)
    );
    let resource = catalog
        .catalog
        .items
        .iter()
        .find(|item| item.resource_path.is_some())
        .expect("the captured Project index must supply the function resource");
    assert_eq!(resource.title.as_ref(), "Sales Report");
    assert_eq!(
        resource
            .resource_path
            .as_ref()
            .map(yss_node_catalog::CatalogResourcePath::as_str),
        Some(function_path.as_str())
    );
    assert!(matches!(
        resource.creation,
        yss_node_catalog::NodeCreation::ResourceBound { .. }
    ));
    let snapshot = session
        .application
        .query_project_index(project_instance_id.clone(), "zh-CN", true)
        .unwrap();
    assert_eq!(snapshot.activity_panels.len(), 2);
    for id in [
        "statistics",
        "statistics.regression",
        "statistics.panel",
        "statistics.timeseries",
    ] {
        assert!(
            snapshot.activity_panels[1]
                .rows
                .iter()
                .any(|row| row.id == id)
        );
    }
    for (id, expected) in [
        ("yssbi.statistics.linear.fit", true),
        ("yssbi.statistics.logit.fit", false),
    ] {
        assert!(snapshot.activity_panels[1].rows.iter().any(|row| matches!(
            &row.content,
            crate::activity_panel::ActivityRowContent::Item(crate::activity_panel::ActivityItem::Node {
                creation: yss_node_catalog::NodeCreation::Static { node_type_id }, available, ..
            }) if node_type_id.as_str() == id && *available == expected
        )));
    }
    for panel in &snapshot.activity_panels {
        assert_eq!(
            panel.project_instance_id.as_deref(),
            Some(project_instance_id.as_str())
        );
        assert_eq!(
            panel.publication_revision,
            snapshot.index.publication_revision
        );
    }
    assert!(snapshot.activity_panels[0].rows.iter().any(|row| matches!(
        &row.content,
        crate::activity_panel::ActivityRowContent::Item(crate::activity_panel::ActivityItem::Graph { path, name, .. })
        if path == function_path.as_str() && name == "Sales Report"
    )));
    assert!(snapshot.activity_panels[1].rows.iter().any(|row| matches!(
        &row.content,
        crate::activity_panel::ActivityRowContent::Item(crate::activity_panel::ActivityItem::Node {
            creation: yss_node_catalog::NodeCreation::ResourceBound { resource_path, .. }, ..
        }) if resource_path.as_str() == function_path.as_str()
    )));
}

#[test]
fn compatible_catalog_filters_against_unsaved_draft_source() {
    let graph_path = GraphResourcePath::new("events/Main.yssbi-event").unwrap();
    let source_node = NodeId::new();
    let session = staged_session(
        compatible_project(&graph_path),
        "compatible-draft-source",
        GraphRuntimeTestControl::default(),
    );
    let mut document = compatible_draft(source_node);
    document
        .nodes
        .get_mut(&source_node)
        .unwrap()
        .parameters
        .insert("aaa".parse().unwrap(), serde_json::json!("databases/wrong"));
    let request = CompatibleCatalogRequest::new(
        session.session.project_instance_id().clone(),
        graph_path,
        document,
        PortAddress::declared(source_node, PortKey::new("value").unwrap()),
        "en-US",
    );

    let catalog = session
        .application
        .compatible_node_catalog(request)
        .unwrap();
    let ids = catalog
        .catalog
        .items
        .iter()
        .map(|item| item.node_type_id.as_ref())
        .collect::<std::collections::BTreeSet<_>>();

    assert!(ids.contains("yssbi.numeric.add"));
    assert!(!ids.contains("yssbi.logic.not"));
}

#[test]
fn disconnecting_a_decompose_view_preserves_other_consumed_branches() {
    use crate::graph::run::{ExecutionApplicationError, RunGraphRequest, run_graph};
    use yss_data_contract::{DataValue, ValueType};
    use yss_graph_document::{ConnectionId, DocumentConnection};
    use yss_graph_editor::EditorGraphMutation;
    use yss_graph_execution::result::{ConnectionCacheState, ResultCacheState};

    let graph = GraphResourcePath::new("events/New Event.yssbi-event").unwrap();
    let session = staged_session(
        compatible_project(&graph),
        "decompose-view-results",
        GraphRuntimeTestControl::default(),
    );
    let app = &session.application;
    let instance = session.session.project_instance_id().clone();
    let source = NodeId::new();
    let decompose = NodeId::new();
    let viewers = [NodeId::new(), NodeId::new(), NodeId::new()];
    let mut document = compatible_draft(source);
    set_constant(
        &mut document,
        source,
        ValueType::DataFrame,
        DataValue::DataFrame(
            r#"{"species":["setosa","versicolor"],"sepal":[5.1,7.0],"petal":[1.4,4.7]}"#.into(),
        ),
    );
    yss_graph_document::normalize_constant_value(document.constants.values_mut().next().unwrap())
        .unwrap();
    for (id, kind) in std::iter::once((decompose, "yssbi.dataframe.decompose"))
        .chain(viewers.into_iter().map(|id| (id, "yssbi.debug.view")))
    {
        document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: kind.parse().unwrap(),
                position: NodePosition { x: 200., y: 0. },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
    }
    let id = ConnectionId::new();
    document.connections.insert(
        id,
        DocumentConnection {
            id,
            output: PortAddress::declared(source, "value".parse().unwrap()),
            input: PortAddress::declared(decompose, "dataframe".parse().unwrap()),
            order: None,
        },
    );
    let resolve = |document: &GraphDocument| {
        app.resolve_graph_document(
            instance.clone(),
            graph.clone(),
            document.clone(),
            "en-US".into(),
        )
        .unwrap()
    };
    let projection = resolve(&document);
    let columns = projection
        .nodes
        .iter()
        .find(|node| node.node_id == decompose)
        .unwrap();
    let column = |name: &str| {
        columns
            .ports
            .iter()
            .find(|port| port.display.label.as_ref() == name)
            .unwrap()
            .address
            .clone()
    };
    let species = column("species");
    for (viewer, name) in viewers.into_iter().zip(["species", "sepal", "petal"]) {
        document = app
            .transform_graph_document(
                instance.clone(),
                graph.clone(),
                "en-US".into(),
                document,
                EditorGraphMutation::Connect {
                    output: column(name),
                    input: PortAddress::declared(viewer, "data".parse().unwrap()),
                    order: None,
                },
            )
            .unwrap()
            .document;
    }
    let original = document.clone();
    let captured = app.capture_session().unwrap();
    let fixture_capture = captured
        .project()
        .capture_graph_overwrite_operation(
            &instance,
            &graph,
            yss_project_identity::OperationId::new(),
        )
        .unwrap();
    captured
        .project()
        .commit_graph_candidate(fixture_capture.into_authority(), Arc::new(document.clone()))
        .unwrap();
    let ready = app
        .open_graph(crate::graph::open::OpenGraphRequest::new(
            instance.clone(),
            graph.clone(),
            0,
            "en-US",
        ))
        .unwrap()
        .projection()
        .clone();
    let edit_request = || crate::graph::editing::GraphEditRequest {
        project_instance_id: instance.clone(),
        graph_path: graph.clone(),
        locale: "en-US".into(),
        operation_id: yss_project_identity::OperationId::new(),
        version: captured
            .project()
            .read_graph_editing(&instance, &graph)
            .unwrap()
            .state
            .version,
    };
    let semantic_input_hash = ready.basis.semantic_input_hash;
    run_graph(
        app,
        RunGraphRequest::new(
            instance.clone(),
            graph.clone(),
            document.clone(),
            semantic_input_hash,
        ),
    )
    .unwrap();
    let query = |hash| {
        app.query_graph_result_state(graph.clone(), hash)
            .unwrap()
            .expect("current graph cache state")
    };
    let initial = query(semantic_input_hash);
    assert_eq!(initial.connections.len(), 4);
    assert!(
        initial
            .connections
            .iter()
            .all(|edge| edge.state == ConnectionCacheState::Valid)
    );
    assert!(
        initial
            .outputs
            .values()
            .all(|state| matches!(state, ResultCacheState::Valid { .. }))
    );

    let disconnected = app
        .edit_graph(
            edit_request(),
            EditorGraphMutation::DisconnectPort {
                address: species.clone(),
            },
        )
        .unwrap();
    let disconnected = disconnected.update;
    let current = query(
        disconnected
            .projection_replacement
            .projection
            .basis
            .semantic_input_hash,
    );
    assert_eq!(current.outputs, initial.outputs);
    assert_eq!(current.connections.len(), 3);
    assert!(
        current
            .connections
            .iter()
            .all(|edge| edge.state == ConnectionCacheState::Valid)
    );
    assert!(matches!(
        run_graph(
            app,
            RunGraphRequest::new(
                instance.clone(),
                graph.clone(),
                original.clone(),
                semantic_input_hash
            )
        ),
        Err(ExecutionApplicationError::DraftChanged)
    ));
    let after_rejection = query(
        disconnected
            .projection_replacement
            .projection
            .basis
            .semantic_input_hash,
    );
    assert_eq!(after_rejection.outputs, current.outputs);
    assert_eq!(after_rejection.connections.len(), current.connections.len());

    let reconnected = app
        .edit_graph(
            edit_request(),
            EditorGraphMutation::Connect {
                output: column("sepal"),
                input: PortAddress::declared(viewers[0], "data".parse().unwrap()),
                order: None,
            },
        )
        .unwrap();
    let reconnected = reconnected.update;
    let current = query(
        reconnected
            .projection_replacement
            .projection
            .basis
            .semantic_input_hash,
    );
    assert_eq!(current.outputs, initial.outputs);
    for edge in &current.connections {
        let is_new = edge.input.as_str()
            == PortAddress::declared(viewers[0], "data".parse().unwrap()).to_string();
        assert_eq!(
            edge.state,
            if is_new {
                ConnectionCacheState::New
            } else {
                ConnectionCacheState::Valid
            }
        );
    }
    app.change_graph_history(edit_request(), false).unwrap();
    let restored = app.change_graph_history(edit_request(), false).unwrap();
    assert_eq!(restored.update.document, original);
    let current = query(
        restored
            .update
            .projection_replacement
            .projection
            .basis
            .semantic_input_hash,
    );
    assert_eq!(current.outputs, initial.outputs);
    assert!(
        current
            .connections
            .iter()
            .all(|edge| edge.state == ConnectionCacheState::Valid)
    );
}

#[test]
fn compatible_decompose_catalog_uses_column_types_and_claims_only_when_creating_a_connection() {
    use yss_data_contract::{DataValue, ValueType};
    use yss_graph_document::{ConnectionId, DocumentConnection};

    let graph = GraphResourcePath::new("events/Columns.yssbi-event").unwrap();
    let session = staged_session(
        compatible_project(&graph),
        "compatible-derived",
        GraphRuntimeTestControl::default(),
    );
    let instance = session.session.project_instance_id().clone();
    let source = NodeId::new();
    let decompose = NodeId::new();
    let mut document = compatible_draft(source);
    set_constant(
        &mut document,
        source,
        ValueType::DataFrame,
        DataValue::DataFrame(r#"{"amount":[1.5,2.5],"label":["a","b"]}"#.into()),
    );
    yss_graph_document::normalize_constant_value(document.constants.values_mut().next().unwrap())
        .unwrap();
    document.nodes.insert(
        decompose,
        DocumentNode {
            id: decompose,
            node_type: "yssbi.dataframe.decompose".parse().unwrap(),
            position: NodePosition { x: 200., y: 0. },
            parameters: ParameterValues::new(),
            user_label: None,
        },
    );
    let id = ConnectionId::new();
    document.connections.insert(
        id,
        DocumentConnection {
            id,
            output: PortAddress::declared(source, "value".parse().unwrap()),
            input: PortAddress::declared(decompose, "dataframe".parse().unwrap()),
            order: None,
        },
    );
    let projection = session
        .application
        .resolve_graph_document(
            instance.clone(),
            graph.clone(),
            document.clone(),
            "en-US".into(),
        )
        .unwrap();
    let node = projection
        .nodes
        .iter()
        .find(|node| node.node_id == decompose)
        .unwrap();
    let column = |name: &str| {
        node.ports
            .iter()
            .find(|port| port.display.label.as_ref() == name)
            .unwrap()
            .address
            .clone()
    };
    let amount = column("amount");
    let before = document.clone();
    for (output, numeric) in [(amount.clone(), true), (column("label"), false)] {
        let catalog = session
            .application
            .compatible_node_catalog(CompatibleCatalogRequest::new(
                instance.clone(),
                graph.clone(),
                document.clone(),
                output,
                "en-US",
            ))
            .expect("unclaimed column pins must query compatible nodes");
        assert_eq!(
            catalog
                .catalog
                .items
                .iter()
                .any(|item| item.node_type_id.as_ref() == "yssbi.statistics.linear.fit"),
            numeric
        );
        assert!(
            !catalog
                .catalog
                .items
                .iter()
                .any(|item| item.node_type_id.as_ref() == "yssbi.logic.not")
        );
    }
    assert_eq!(document, before);
    assert!(document.port_bindings.is_empty());
    let catalog = session
        .application
        .compatible_node_catalog(CompatibleCatalogRequest::new(
            instance.clone(),
            graph.clone(),
            document.clone(),
            amount.clone(),
            "en-US",
        ))
        .unwrap();
    let descriptor = catalog
        .catalog
        .items
        .iter()
        .find(|item| item.node_type_id.as_ref() == "yssbi.debug.view")
        .unwrap()
        .creation
        .clone();
    let updated = session
        .application
        .transform_graph_document(
            instance.clone(),
            graph.clone(),
            "en-US".into(),
            document,
            yss_graph_editor::EditorGraphMutation::CreateNode {
                descriptor: descriptor.clone(),
                position: NodePosition { x: 400., y: 0. },
                user_label: None,
                connect_from: Some(amount.clone()),
            },
        )
        .unwrap();
    assert_eq!(updated.document.port_bindings.len(), 1);
    assert!(updated.document.port_bindings.contains_key(&amount));
    assert!(
        updated
            .document
            .connections
            .values()
            .any(|connection| connection.output == amount)
    );
    let first_connections = updated.document.connections.clone();
    let updated = session
        .application
        .transform_graph_document(
            instance.clone(),
            graph.clone(),
            "en-US".into(),
            updated.document,
            yss_graph_editor::EditorGraphMutation::CreateNode {
                descriptor,
                position: NodePosition { x: 400., y: 200. },
                user_label: None,
                connect_from: Some(amount.clone()),
            },
        )
        .unwrap();
    assert_eq!(updated.document.port_bindings.len(), 1);
    assert_eq!(
        updated.document.connections.len(),
        first_connections.len() + 1
    );
    for (id, connection) in first_connections {
        assert_eq!(updated.document.connections.get(&id), Some(&connection));
    }
    let port = updated
        .projection_replacement
        .projection
        .nodes
        .iter()
        .find(|node| node.node_id == decompose)
        .unwrap()
        .ports
        .iter()
        .find(|port| port.address == amount)
        .unwrap();
    assert_eq!(port.connections.current, 2);
    assert_eq!(port.connections.maximum, None);
    assert!(port.connections.can_append);
    assert!(!port.connections.can_replace);
    let mut stale = updated.document;
    let constant = stale.constants.values_mut().next().unwrap();
    let yss_data_contract::DataValue::DataFrame(_) = constant.data_value else {
        panic!("dataframe literal");
    };
    constant.tabular = None;
    constant.data_value = DataValue::DataFrame(r#"{"label":["a","b"]}"#.into());
    yss_graph_document::normalize_constant_value(constant).unwrap();
    let error = session
        .application
        .compatible_node_catalog(CompatibleCatalogRequest::new(
            instance, graph, stale, amount, "en-US",
        ))
        .unwrap_err();
    assert!(matches!(
        error,
        CatalogQueryApplicationError::Graph(GraphCatalogQueryError::CompatibleSourceInvalid)
    ));
}

#[test]
fn replacement_after_catalog_compute_returns_stale_and_publishes_nothing() {
    let control = GraphRuntimeTestControl::default();
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    control.pause_after_catalog_compute(Arc::clone(&entered), Arc::clone(&release));
    let session = staged_session(
        ProjectData::new(),
        "catalog-replacement-after-compute",
        control.clone(),
    );
    let replacement = staged_session(
        ProjectData::new(),
        "catalog-replacement-target",
        GraphRuntimeTestControl::default(),
    );
    let worker_application = session.application.clone();
    let request =
        LocalizedCatalogRequest::new(session.session.project_instance_id().clone(), "en-US");
    let worker = thread::spawn(move || worker_application.localized_node_catalog(request));

    entered.wait();
    session
        .slot
        .publish_for_test(Arc::clone(&replacement.session));
    release.wait();
    let result = worker.join().unwrap();

    assert!(matches!(
        result,
        Err(CatalogQueryApplicationError::SessionChanged)
    ));
    assert_eq!(control.events(), [GraphRuntimeTestEvent::CatalogComputed]);
}

#[test]
fn localized_catalog_rejects_a_project_database_schema_mismatch() {
    let database = DatabaseDecl {
        id: DatabaseId::from_existing("sales".into()),
        engine: yss_database_contract::DatabaseEngine::Dataset {},
        schema_version: 1,
        required: true,
        name: "Sales".into(),
    };
    let mut project = ProjectData::new();
    project.databases.insert("sales".into(), database);
    let session = staged_session(
        project,
        "catalog-schema-mismatch",
        GraphRuntimeTestControl::default(),
    );
    let project_instance_id = session.session.project_instance_id().clone();

    let error = session
        .application
        .localized_node_catalog(LocalizedCatalogRequest::new(project_instance_id, "en-US"))
        .unwrap_err();

    assert!(matches!(
        error,
        CatalogQueryApplicationError::Database(error)
            if error.code() == yss_database_runtime::error::DatabaseErrorCode::Conflict
    ));
}

fn set_constant(
    document: &mut GraphDocument,
    node: NodeId,
    data_type: yss_data_contract::ValueType,
    data_value: yss_data_contract::DataValue,
) {
    let id = yss_graph_document::ConstantId::from_uuid(node.as_uuid());
    document.constants.insert(
        id,
        yss_graph_document::GraphConstant {
            id,
            name: id.to_string(),
            data_type,
            data_value,
            tabular: None,
            description: String::new(),
            tags: vec![],
        },
    );
    let node = document.nodes.get_mut(&node).unwrap();
    node.node_type = "yssbi.constant.get".parse().unwrap();
    node.parameters = ParameterValues::from([(
        "constant".parse().unwrap(),
        serde_json::json!(id.to_string()),
    )]);
}
