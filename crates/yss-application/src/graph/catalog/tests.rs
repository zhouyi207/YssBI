use super::*;
mod function_calls;
mod schema_feedback;

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
                declarations,
                observations,
            ))
            .unwrap(),
    );
    let execution = Arc::new(ExecutionRuntimeState::new(
        execution_session_id,
        RuntimeGeneration::from_existing(1),
        yss_node_kernel::KernelRegistry::default().into(),
        yss_database_runtime::dataset_query_engine().unwrap(),
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
        GraphResourceDocument::new("Main", GraphResourceKind::EventGraph),
    );
    project
}

fn compatible_draft(source_node: NodeId) -> GraphDocument {
    let mut graph = GraphResourceDocument::new("Main", GraphResourceKind::EventGraph);
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
        yss_data_contract::DataValue::Integer(0),
    );
    graph.document
}

#[test]
fn registered_calls_capture_transitive_bodies_and_invalidate_signature_consumers() {
    use crate::session::{NodeComponents, build_current_project_candidate};
    use std::collections::BTreeSet;
    use yss_graph_resource_contract::{
        FunctionCatalogEntry, FunctionSignature, ResourceCatalogSnapshot,
    };
    use yss_node_registry::{
        NodeRegistryBuilder, ProviderRegistration, RegisteredNode, StructuralNodeRole,
    };
    use yss_project_history::{
        FunctionDocumentPatch, FunctionResourceKey, MutationRequest, ResourceKey,
    };
    use yss_project_identity::OperationId;

    let function = GraphResourcePath::new("functions/Custom.yssbi-function").unwrap();
    let nested = GraphResourcePath::new("functions/Nested.yssbi-function").unwrap();
    let caller = GraphResourcePath::new("events/Caller.yssbi-event").unwrap();
    let group_apply = GraphResourcePath::new("events/Apply.yssbi-event").unwrap();
    let group_transform = GraphResourcePath::new("events/Transform.yssbi-event").unwrap();
    let signature_reader = GraphResourcePath::new("events/Signature.yssbi-event").unwrap();
    let outer_signature_reader =
        GraphResourcePath::new("events/Outer Signature.yssbi-event").unwrap();
    let unused = GraphResourcePath::new("functions/Unused.yssbi-function").unwrap();
    let builtin = build_builtin_node_system().unwrap();
    let mut protocol = builtin
        .registry
        .protocol(&"yssbi.project.function.call".parse().unwrap())
        .unwrap()
        .clone();
    protocol.type_id = "tests.function.call".parse().unwrap();
    let reference = &mut protocol.parameters.groups[0].parameters[0];
    reference.default_value = Some(yss_node_protocol::TypedValue {
        value_type: reference.value_type.clone(),
        value: yss_data_contract::DataValue::String(function.as_str().into()),
    });
    let mut signature_protocol = protocol.clone();
    signature_protocol.type_id = "tests.function.signature".parse().unwrap();
    signature_protocol.instance_display = yss_node_protocol::NodeInstanceDisplaySpec::Static;
    signature_protocol.parameters.groups[0].parameters[0].editor =
        yss_node_protocol::ParameterEditorSpec::Text { multiline: false };
    let mut builder = NodeRegistryBuilder::new();
    let catalog = yss_node_catalog::register_builtin_nodes(&mut builder).unwrap();
    let mut provider = ProviderRegistration::new("tests.functions".parse().unwrap());
    provider.nodes = vec![
        RegisteredNode::structural(Arc::new(protocol), StructuralNodeRole::Call),
        RegisteredNode::leaf(
            Arc::new(signature_protocol),
            yss_node_registry::LeafImplementation::new("tests.signature"),
        ),
    ]
    .into();
    builder.register_provider(provider).unwrap();
    let components = NodeComponents::new(
        Arc::new(builder.freeze().unwrap()),
        Arc::new(catalog),
        yss_node_kernel::KernelRegistryBuilder::new(),
    )
    .unwrap();

    let mut data = ProjectData::new();
    for path in [&function, &nested, &unused] {
        let mut resource =
            GraphResourceDocument::new(path.display_name(), GraphResourceKind::FunctionGraph);
        let id = NodeId::new();
        resource.document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: "yssbi.project.function.entry".parse().unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: [(
                    "function".parse().unwrap(),
                    serde_json::json!(path.as_str()),
                )]
                .into(),
                user_label: None,
            },
        );
        data.graphs.insert(path.clone(), resource);
    }
    for path in [
        &caller,
        &group_apply,
        &group_transform,
        &signature_reader,
        &outer_signature_reader,
    ] {
        data.graphs.insert(
            path.clone(),
            GraphResourceDocument::new(path.display_name(), GraphResourceKind::EventGraph),
        );
    }
    for (owner, target, kind) in [
        (&caller, None, "tests.function.call"),
        (
            &group_apply,
            Some(&function),
            "yssbi.dataframe.groupby.apply",
        ),
        (
            &group_transform,
            Some(&function),
            "yssbi.dataframe.groupby.transform",
        ),
        (&function, Some(&nested), "tests.function.call"),
        (&nested, Some(&function), "tests.function.call"),
        (&signature_reader, Some(&nested), "tests.function.signature"),
        (&outer_signature_reader, None, "tests.function.signature"),
    ] {
        let id = NodeId::new();
        data.graphs.get_mut(owner).unwrap().document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: kind.parse().unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: target
                    .map(|target| {
                        [(
                            "target".parse().unwrap(),
                            serde_json::json!(target.as_str()),
                        )]
                        .into()
                    })
                    .unwrap_or_default(),
                user_label: None,
            },
        );
    }
    let root_document = data.graphs[&caller].document.clone();
    let project = TestProject::active("registered-function-capture", data);
    let candidate = build_current_project_candidate(
        ApplicationSessionEpoch::INITIAL,
        Arc::clone(&project.state),
        std::iter::empty(),
        &components,
    )
    .unwrap();
    let application = ApplicationState::new(Arc::new(ApplicationSessionSlot::new(components)));
    application.install_candidate(candidate).unwrap();
    let session = application.capture_session().unwrap();
    let resources = ResourceCatalogSnapshot::new(
        [&function, &nested]
            .into_iter()
            .map(|path| {
                (
                    path.clone(),
                    FunctionCatalogEntry::new(FunctionSignature::new(vec![], None)),
                )
            })
            .collect(),
        BTreeMap::new(),
    );
    let captured =
        crate::graph::inputs::capture_function_dependencies(&session, &root_document, resources)
            .unwrap();
    assert!(captured.function_document(&function).is_some());
    assert!(captured.function_document(&nested).is_some());
    session.project().unload_graph_resource(&function).unwrap();
    assert!(
        session
            .project()
            .read_resident_graph(&function)
            .unwrap()
            .is_none()
    );
    session.project().unload_graph_resource(&unused).unwrap();
    // An unrelated, unloaded body is outside the dependency capture's read set.
    std::fs::write(
        project.root.join(unused.as_str()),
        b"unreadable function body",
    )
    .unwrap();
    let activities = Arc::new(std::sync::Mutex::new(BTreeSet::new()));
    let observed = Arc::clone(&activities);
    let _subscription = application
        .subscribe_graph_activity(
            session.project_instance_id(),
            Arc::new(move |activity| {
                if let crate::graph::editing::GraphActivity::Changed { graph_path, .. } = activity {
                    observed.lock().unwrap().insert(graph_path);
                }
            }),
        )
        .unwrap();

    let before = session
        .project()
        .read_graph_resource_snapshot(session.project_instance_id(), &nested)
        .unwrap()
        .function
        .unwrap();
    let mut after = before.signature.clone();
    after
        .parameters
        .push(yss_project_history::FunctionParameter {
            id: yss_graph_document::FunctionParameterId::new("value"),
            name: "Value".into(),
            type_name: "Numeric".into(),
        });
    let committed = application
        .update_function_signature(
            session.project_instance_id().clone(),
            nested.clone(),
            MutationRequest::new(
                ResourceKey::Function(FunctionResourceKey(nested.as_str().into())),
                before.revision,
                OperationId::new(),
                FunctionDocumentPatch::new(before.signature, after),
            ),
        )
        .unwrap();
    assert!(
        committed
            .projection_status
            .affected_graph_paths()
            .contains(&caller)
    );
    assert!(
        committed
            .projection_status
            .affected_graph_paths()
            .contains(&function)
    );
    assert_eq!(
        committed
            .projection_status
            .affected_graph_paths()
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>(),
        [
            nested.clone(),
            function.clone(),
            caller.clone(),
            group_apply.clone(),
            group_transform.clone(),
            signature_reader.clone()
        ]
        .into()
    );
    assert_eq!(
        *activities.lock().unwrap(),
        [
            nested.as_str().into(),
            caller.as_str().into(),
            group_apply.as_str().into(),
            group_transform.as_str().into(),
            signature_reader.as_str().into()
        ]
        .into()
    );
    assert!(
        session
            .project()
            .read_resident_graph(&function)
            .unwrap()
            .is_none()
    );
}

#[test]
fn connection_candidates_are_read_only_and_reject_an_obsolete_graph_version() {
    use yss_graph_editor::projection::{ConnectionDecision, ConnectionIntent};
    use yss_graph_editor::{EditorGraphMutation, NodePositionMutation};
    let graph = GraphResourcePath::new("events/Candidates.yssbi-event").unwrap();
    let source = NodeId::new();
    let target = NodeId::new();
    let mut project = compatible_project(&graph);
    let mut document = compatible_draft(source);
    document.nodes.insert(
        target,
        DocumentNode {
            id: target,
            node_type: "yssbi.numeric.subtract".parse().unwrap(),
            position: NodePosition { x: 100., y: 0. },
            parameters: ParameterValues::new(),
            user_label: None,
        },
    );
    project.graphs.get_mut(&graph).unwrap().document = document;
    let staged = staged_session(
        project,
        "connection-candidates",
        GraphRuntimeTestControl::default(),
    );
    let app = &staged.application;
    let instance = staged.session.project_instance_id().clone();
    app.open_graph(crate::graph::open::OpenGraphRequest::new(
        instance.clone(),
        graph.clone(),
        0,
        "en-US",
    ))
    .unwrap();
    let before = staged
        .session
        .project()
        .read_graph_editing(&instance, &graph)
        .unwrap();
    let source_port = PortAddress::declared(source, "value".parse().unwrap());
    let candidates = app
        .graph_connection_candidates(
            &instance,
            &graph,
            before.state.version,
            &source_port,
            ConnectionIntent::Connect,
        )
        .unwrap();
    assert!(candidates.candidates.iter().any(|candidate| candidate.port
        == PortAddress::declared(target, "left".parse().unwrap())
        && candidate.decision == ConnectionDecision::Append));
    let after = staged
        .session
        .project()
        .read_graph_editing(&instance, &graph)
        .unwrap();
    assert_eq!(after.state, before.state);
    assert_eq!(after.document, before.document);
    app.edit_graph(
        crate::graph::editing::GraphEditRequest {
            project_instance_id: instance.clone(),
            graph_path: graph.clone(),
            locale: "en-US".into(),
            operation_id: yss_project_identity::OperationId::new(),
            version: before.state.version,
        },
        EditorGraphMutation::MoveNodes {
            positions: vec![NodePositionMutation {
                node_id: target,
                position: NodePosition { x: 200., y: 0. },
            }],
        },
    )
    .unwrap();
    assert!(matches!(
        app.graph_connection_candidates(
            &instance,
            &graph,
            before.state.version,
            &source_port,
            ConnectionIntent::Connect
        ),
        Err(
            crate::graph::resources::ResourceMutationApplicationError::GraphOperation(
                yss_project::ProjectGraphOperationError::RevisionConflict { .. }
            )
        )
    ));
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
                value: yss_data_contract::DataValue::Integer(3),
            }),
        },
    );
    let mut definition = GraphResourceDocument::new("F", GraphResourceKind::FunctionGraph);
    definition.function.as_mut().unwrap().signature.parameters = (0..2)
        .map(|i| yss_project_history::FunctionParameter {
            id: FunctionParameterId::new(format!("p{i}")),
            name: format!("p{i}"),
            type_name: "Numeric".into(),
        })
        .collect();
    let mut resource = GraphResourceDocument::new("Caller", GraphResourceKind::EventGraph);
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
fn node_creation_form_is_read_only_and_preserves_protocol_defaults_and_conditions() {
    let path = GraphResourcePath::new("events/Draft.yssbi-event").unwrap();
    let mut project = ProjectData::new();
    project.graphs.insert(
        path.clone(),
        GraphResourceDocument::new("Draft", GraphResourceKind::EventGraph),
    );
    let session = staged_session(project, "creation-form", GraphRuntimeTestControl::default());
    let instance = session.session.project_instance_id();
    session
        .application
        .unload_graph_resource(instance.clone(), path.clone(), 100, None)
        .unwrap();
    assert!(!session.session.project().has_resident_graph(&path).unwrap());
    let before = session
        .session
        .project()
        .read_project_index(instance)
        .unwrap();
    let file = session._project.root.join(path.as_str());
    let saved = std::fs::read(&file).unwrap();
    let curve = "yssbi.statistics.regression.curve".parse().unwrap();
    let initial = session
        .application
        .node_creation_form(
            instance,
            &curve,
            Default::default(),
            Default::default(),
            "en-US",
        )
        .unwrap();
    assert!(initial.values.is_empty());
    assert_eq!(
        initial
            .groups
            .iter()
            .flat_map(|group| group.parameters.iter())
            .find(|parameter| parameter.key.as_str() == "degree")
            .unwrap()
            .value,
        Some(serde_json::json!(2))
    );
    let conditional = session
        .application
        .node_creation_form(
            instance,
            &curve,
            [
                (
                    "curve_family".parse().unwrap(),
                    serde_json::json!("exponential"),
                ),
                ("degree".parse().unwrap(), serde_json::json!(4)),
            ]
            .into(),
            Default::default(),
            "en-US",
        )
        .unwrap();
    assert!(!conditional.values.contains_key(&"degree".parse().unwrap()));
    assert!(
        conditional
            .groups
            .iter()
            .flat_map(|group| group.parameters.iter())
            .all(|parameter| parameter.key.as_str() != "degree")
    );
    assert!(
        session
            .application
            .node_creation_form(
                instance,
                &curve,
                [("degree".parse().unwrap(), serde_json::json!(0))].into(),
                Default::default(),
                "en-US"
            )
            .is_err()
    );
    let form = session
        .application
        .node_creation_form(
            instance,
            &"yssbi.statistics.linear.fit".parse().unwrap(),
            Default::default(),
            [("x".parse().unwrap(), 4)].into(),
            "zh-CN",
        )
        .unwrap();
    assert_eq!(form.port_counts[&"x".parse().unwrap()], 4);
    assert!(matches!(
        form.ports
            .iter()
            .find(|port| port.key.as_str() == "y")
            .unwrap()
            .count,
        yss_node_catalog::PortCountPolicy::Fixed
    ));
    assert!(
        session
            .application
            .node_creation_form(
                &ProjectInstanceId::from_existing("stale".into()),
                &curve,
                Default::default(),
                Default::default(),
                "en-US"
            )
            .is_err()
    );
    let after = session
        .session
        .project()
        .read_project_index(instance)
        .unwrap();
    assert_eq!(before.authority_generation(), after.authority_generation());
    assert_eq!(before.publication_revision, after.publication_revision);
    assert!(!session.session.project().has_resident_graph(&path).unwrap());
    assert_eq!(std::fs::read(file).unwrap(), saved);
}

#[test]
fn localized_catalog_returns_resources_from_the_same_coherent_snapshot() {
    let function_path = GraphResourcePath::new("functions/Sales Report.yssbi-function").unwrap();
    let mut project = ProjectData::new();
    project.graphs.insert(
        function_path.clone(),
        GraphResourceDocument::new("Sales Report", GraphResourceKind::FunctionGraph),
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
            catalog
                .catalog
                .items
                .iter()
                .find(|item| item.node_type_id.as_ref() == format!("yssbi.dataframe.series.{id}"))
                .expect("registered series kernels remain discoverable")
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
        ("yssbi.statistics.logit.fit", true),
        ("yssbi.statistics.inequality.gini", true),
        ("yssbi.statistics.postestimation.adjusted_predictions", true),
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
        crate::activity_panel::ActivityRowContent::Item(crate::activity_panel::ActivityItem::FunctionGraph { path, name })
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
fn all_non_deferred_builtin_nodes_are_available_in_catalog_and_sidebar() {
    let session = staged_session(
        ProjectData::new(),
        "all-builtin-nodes-availability",
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
    let snapshot = session
        .application
        .query_project_index(project_instance_id, "zh-CN", true)
        .unwrap();
    // Scope exceptions are reviewed in the catalog owner's document, not inferred
    // from missing kernels. Implementing an exception must also remove its record.
    let deferred = include_str!("../../../../yss-node-catalog/DEFERRED_NODES.md")
        .lines()
        .filter(|line| line.starts_with("| yssbi."))
        .map(|line| line.split('|').nth(1).unwrap().trim())
        .collect::<std::collections::BTreeSet<_>>();
    for id in &deferred {
        let item = catalog
            .catalog
            .items
            .iter()
            .find(|item| item.node_type_id.as_ref() == *id)
            .unwrap_or_else(|| panic!("unknown deferred node {id}"));
        assert!(
            !item.available,
            "remove implemented {id} from the deferred document"
        );
    }
    let unavailable = catalog
        .catalog
        .items
        .iter()
        .filter(|item| !item.available && !deferred.contains(item.node_type_id.as_ref()))
        .map(|item| {
            format!(
                "{} | {} | {}",
                item.category_id, item.node_type_id, item.title
            )
        })
        .collect::<Vec<_>>();
    assert!(
        unavailable.is_empty(),
        "{} non-deferred unavailable nodes:\n{}",
        unavailable.len(),
        unavailable.join("\n")
    );
    let categories = catalog
        .catalog
        .items
        .iter()
        .map(|item| item.category_id.as_ref())
        .collect::<std::collections::BTreeSet<_>>();
    for category in categories {
        let items = catalog
            .catalog
            .items
            .iter()
            .filter(|item| item.category_id.as_ref() == category)
            .collect::<Vec<_>>();
        assert!(!items.is_empty(), "missing category {category}");
        for item in &items {
            let expected_available = !deferred.contains(item.node_type_id.as_ref());
            assert_eq!(
                item.available, expected_available,
                "{} ({})",
                item.title, item.node_type_id
            );
            assert!(snapshot.activity_panels.iter().flat_map(|panel| &panel.rows).any(
                |row| matches!(
                    &row.content,
                    crate::activity_panel::ActivityRowContent::Item(crate::activity_panel::ActivityItem::Node {
                        creation, available, ..
                    }) if creation == &item.creation && *available == expected_available
                )
            ), "sidebar availability must match catalog for {}", item.node_type_id);
        }
        println!(
            "{category}: {} available, {} deferred",
            items.iter().filter(|item| item.available).count(),
            items.iter().filter(|item| !item.available).count()
        );
    }
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
fn compatible_catalog_excludes_disjoint_numeric_and_model_result_types_in_both_directions() {
    let graph = GraphResourcePath::new("events/Types.yssbi-event").unwrap();
    let session = staged_session(
        compatible_project(&graph),
        "compatible-model-result",
        GraphRuntimeTestControl::default(),
    );
    let mut document = GraphDocument::default();
    let summary = NodeId::new();
    let multiply = NodeId::new();
    for (id, kind) in [
        (summary, "yssbi.statistics.linear.summary"),
        (multiply, "yssbi.numeric.multiply"),
    ] {
        document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: kind.parse().unwrap(),
                position: NodePosition { x: 0., y: 0. },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
    }
    for (source, excluded, included) in [
        (
            PortAddress::declared(summary, "result".parse().unwrap()),
            "yssbi.numeric.multiply",
            "yssbi.debug.view",
        ),
        (
            PortAddress::declared(multiply, "left".parse().unwrap()),
            "yssbi.statistics.linear.summary",
            "yssbi.constant.pi",
        ),
    ] {
        let catalog = session
            .application
            .compatible_node_catalog(CompatibleCatalogRequest::new(
                session.session.project_instance_id().clone(),
                graph.clone(),
                document.clone(),
                source,
                "en-US",
            ))
            .unwrap();
        let contains = |id: &str| {
            catalog
                .catalog
                .items
                .iter()
                .any(|item| item.node_type_id.as_ref() == id)
        };
        assert!(
            !contains(excluded),
            "incompatible node {excluded} must be filtered out"
        );
        assert!(
            contains(included),
            "compatible node {included} must remain discoverable"
        );
    }
}

#[test]
fn connection_mutations_reject_model_results_even_through_resolved_generic_outputs() {
    use crate::graph::resources::ResourceMutationApplicationError;
    use yss_graph_document::{ConnectionId, DocumentConnection};
    use yss_graph_editor::{EditorGraphMutation, EditorMutationErrorCode, MutationConflict};

    let graph = GraphResourcePath::new("events/Types.yssbi-event").unwrap();
    let session = staged_session(
        compatible_project(&graph),
        "connect-model-result",
        GraphRuntimeTestControl::default(),
    );
    let builtin = build_builtin_node_system().unwrap();
    let mut document = GraphDocument::default();
    let summary = NodeId::new();
    let multiply = NodeId::new();
    let reroute = NodeId::new();
    let numeric = NodeId::new();
    for (id, kind) in [
        (summary, "yssbi.statistics.linear.summary"),
        (multiply, "yssbi.numeric.multiply"),
        (reroute, "yssbi.core.reroute"),
        (numeric, "yssbi.constant.pi"),
    ] {
        document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: kind.parse().unwrap(),
                position: NodePosition { x: 0., y: 0. },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
    }
    let result = PortAddress::declared(summary, "result".parse().unwrap());
    let input = PortAddress::declared(multiply, "left".parse().unwrap());
    let routed = PortAddress::declared(reroute, "output".parse().unwrap());
    let id = ConnectionId::new();
    document.connections.insert(
        id,
        DocumentConnection {
            id,
            output: result.clone(),
            input: PortAddress::declared(reroute, "input".parse().unwrap()),
            order: None,
        },
    );
    let create = |kind: &str, source: PortAddress| EditorGraphMutation::CreateNode {
        port_counts: Default::default(),
        parameters: Default::default(),
        descriptor: yss_node_catalog::authoritative_static_descriptor(
            builtin.registry.protocol(&kind.parse().unwrap()).unwrap(),
        )
        .unwrap(),
        position: NodePosition { x: 200., y: 0. },
        user_label: None,
        connect_from: Some(source),
    };
    let transform = |mutation| {
        session.application.transform_graph_document(
            session.session.project_instance_id().clone(),
            graph.clone(),
            "en-US".into(),
            document.clone(),
            mutation,
        )
    };
    for mutation in [
        EditorGraphMutation::Connect {
            output: result.clone(),
            input: input.clone(),
            order: None,
        },
        create("yssbi.numeric.multiply", result.clone()),
        create("yssbi.statistics.linear.summary", input.clone()),
        EditorGraphMutation::Connect {
            output: routed.clone(),
            input,
            order: None,
        },
        create("yssbi.numeric.multiply", routed),
    ] {
        let error = transform(mutation)
            .expect_err("incompatible connections must reject the entire mutation");
        assert!(
            matches!(error,
                ResourceMutationApplicationError::Mutation(MutationConflict::Editor(ref failure))
                    if failure.code == EditorMutationErrorCode::GraphConnectionTypeMismatch
            ),
            "unexpected rejection: {error:?}"
        );
    }
    for mutation in [
        create("yssbi.debug.view", result),
        create(
            "yssbi.numeric.multiply",
            PortAddress::declared(numeric, "value".parse().unwrap()),
        ),
    ] {
        let updated = transform(mutation)
            .expect("valid generic and numeric connections must remain possible");
        assert_eq!(updated.document.nodes.len(), document.nodes.len() + 1);
        assert_eq!(
            updated.document.connections.len(),
            document.connections.len() + 1
        );
        let mut restored = updated.document;
        yss_graph_document_edit::apply_graph_document_patch(
            &mut restored,
            &updated.patch.inverse(),
        )
        .unwrap();
        assert_eq!(
            restored, document,
            "creation and connection must undo together"
        );
    }
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
        DataValue::String(
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
    let source_output = PortAddress::declared(source, "value".parse().unwrap()).to_string();
    for (output, state) in &initial.outputs {
        if output.port().as_str() == source_output {
            assert_eq!(
                *state,
                ResultCacheState::Missing,
                "internal query plans are not materialized results"
            );
        } else {
            assert!(matches!(state, ResultCacheState::Valid { .. }));
        }
    }

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
        DataValue::String(r#"{"amount":[1.5,2.5],"label":["a","b"]}"#.into()),
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
                port_counts: Default::default(),
                parameters: Default::default(),
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
                port_counts: Default::default(),
                parameters: Default::default(),
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
    assert_eq!(constant.data_value, DataValue::Null);
    assert!(constant.tabular.is_some());
    constant.tabular = None;
    constant.data_value = DataValue::String(r#"{"label":["a","b"]}"#.into());
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
fn clipboard_export_uses_project_declarations_without_database_schema_capture() {
    let graph = GraphResourcePath::new("events/Clipboard.yssbi-event").unwrap();
    let function = GraphResourcePath::new("functions/Measure.yssbi-function").unwrap();
    let mut project = compatible_project(&graph);
    let mut definition = GraphResourceDocument::new("Measure", GraphResourceKind::FunctionGraph);
    definition.function.as_mut().unwrap().signature.return_type = Some("Numeric".into());
    project.graphs.insert(function.clone(), definition);
    project.databases.insert(
        "sales".into(),
        DatabaseDecl {
            id: DatabaseId::from_existing("sales".into()),
            engine: yss_database_contract::DatabaseEngine::Dataset {},
            schema_version: 1,
            required: true,
            name: "Sales".into(),
        },
    );
    let mut document = GraphDocument::default();
    for (node_type, parameter, path) in [
        ("yssbi.project.function.call", "target", function.as_str()),
        ("yssbi.dataframe.source.get", "dataframe", "databases/sales"),
    ] {
        let id = NodeId::new();
        document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: node_type.parse().unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: [(parameter.parse().unwrap(), serde_json::json!(path))].into(),
                user_label: None,
            },
        );
    }
    project.graphs.get_mut(&graph).unwrap().document = document.clone();
    let staged = staged_session(
        project,
        "clipboard-declarations",
        GraphRuntimeTestControl::default(),
    );
    let instance = staged.session.project_instance_id().clone();
    // This session has no database runtime declaration or schema for Sales.
    assert!(matches!(
        staged.application.localized_node_catalog(LocalizedCatalogRequest::new(instance.clone(), "en-US")),
        Err(CatalogQueryApplicationError::Database(error))
            if error.code() == yss_database_runtime::error::DatabaseErrorCode::Conflict
    ));
    let exported = staged
        .application
        .export_graph_subgraph(
            instance,
            graph,
            document.clone(),
            document.nodes.keys().copied().collect(),
        )
        .unwrap();
    let paths = exported
        .nodes
        .iter()
        .map(|node| {
            let yss_graph_editor::ClipboardNodeCreation::ResourceBound { resource_path, .. } =
                &node.creation
            else {
                panic!("resource nodes must keep authoritative creation descriptors")
            };
            resource_path.as_str()
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(paths, [function.as_str(), "databases/sales"].into());
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
