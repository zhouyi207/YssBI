use super::*;

use crate::execution::{ApplicationSession, ApplicationSessionEpoch, ApplicationSessionSlot};
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};
use std::thread;
use yss_database_contract::{
    DatabaseDecl, DatabaseDeclarationObservation, DatabaseDeclarationObservationSet, DatabaseId,
    DatabaseSessionIdentity, DatabaseSessionOpenRequest,
};
use yss_database_runtime::runtime::DatabaseRuntimeRegistry;
use yss_execution::identity::{ExecutionSessionId, RuntimeGeneration};
use yss_execution::resource_preparation::ResourceProviderFactory;
use yss_execution::state::ExecutionRuntimeState;
use yss_graph_catalog::build_builtin_node_system;
use yss_graph_document::GraphResourceKind;
use yss_graph_document::{
    DocumentNode, GraphDocument, GraphResourcePath, NodeId, NodePosition, ParameterValues,
    PortAddress,
};
use yss_graph_protocol::{NodeTypeId, PortKey};
use yss_graph_runtime::{
    GraphRuntimeComponents, GraphRuntimeEpoch, GraphRuntimeState, GraphRuntimeTestControl,
    GraphRuntimeTestEvent,
};
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
    let slot = Arc::new(ApplicationSessionSlot::new());
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
        yss_data_contract::DataType::Int64,
        yss_data_contract::DataValue::Int64(0),
    );
    graph.document
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
            .map(yss_graph_catalog::CatalogResourcePath::as_str),
        Some(function_path.as_str())
    );
    assert!(matches!(
        resource.creation,
        yss_graph_catalog::NodeCreation::ResourceBound { .. }
    ));
    let snapshot = session
        .application
        .query_project_index(project_instance_id.clone(), "zh-CN", true)
        .unwrap();
    assert_eq!(snapshot.activity_panels.len(), 2);
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
            creation: yss_graph_catalog::NodeCreation::ResourceBound { resource_path, .. }, ..
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
fn compatible_decompose_catalog_uses_column_types_and_claims_only_when_creating_a_connection() {
    use yss_data_contract::{DataType, DataValue};
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
        DataType::DataFrame,
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
        .resolve_graph_draft(
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
                .any(|item| item.node_type_id.as_ref()
                    == "yssbi.dataframe.series.inverse_standardize"),
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
        .transform_graph_draft(
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
        .transform_graph_draft(
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
    data_type: yss_data_contract::DataType,
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
