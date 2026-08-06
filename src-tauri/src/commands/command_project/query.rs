use crate::application::database_schema::enriched_database_dtos;
use crate::error::AppError;
use crate::event::ProjectActivationResultDto;
use crate::log::LogLevel;
use crate::log_app;
use crate::node_system::analysis::EditorGraphProjectionDto;
use crate::project::{
    ProjectIndex, ProjectState, RevealProjectResourceRequest, format_path_for_user_path,
    normalize_existing_path, resolve_reveal_path,
};
use crate::schema::{DatabasesVariablesDTO, VariableInstanceDTO};
use tauri::State;

/// 分阶段加载第一步：获取 databases + variables（含 schema）
#[tauri::command]
pub fn get_project_databases_variables(
    state: State<ProjectState>,
) -> Result<DatabasesVariablesDTO, AppError> {
    let data = state.get_data().map_err(AppError::from)?;

    log_app!(
        LogLevel::Info,
        "[command.get_project_databases_variables] Loading databases + variables"
    );

    let store = state.project_store.read().unwrap();
    let databases = enriched_database_dtos(&data.databases, &store);
    let variables = data
        .variables
        .iter()
        .map(|(k, v)| (k.to_string(), VariableInstanceDTO::from(v)))
        .collect();

    Ok(DatabasesVariablesDTO {
        databases,
        variables,
    })
}

fn current_project_activation(
    state: &ProjectState,
) -> Result<ProjectActivationResultDto, AppError> {
    let activation_revision = state.activation_revision();
    let session = state.capture_project_session().map_err(AppError::from)?;
    let path = state
        .get_path()
        .ok_or_else(|| AppError::new("stale_project_lifecycle", "No project is active"))?;
    state
        .validate_project_session(&session)
        .map_err(AppError::from)?;
    if state.activation_revision() != activation_revision {
        return Err(AppError::new(
            "stale_project_lifecycle",
            "Project changed during activation capture",
        ));
    }
    Ok(ProjectActivationResultDto {
        path: normalize_existing_path(&path).unwrap_or(path),
        project_instance_id: session.instance_id.to_string(),
        activation_revision,
    })
}

/// 获取当前项目 activation，供项目加载后创建的独立 WebView 建立 lifecycle identity。
#[tauri::command]
pub fn get_current_project_activation(
    state: State<ProjectState>,
) -> Result<ProjectActivationResultDto, AppError> {
    current_project_activation(state.inner())
}

/// 获取当前项目路径
#[tauri::command]
pub fn get_project_path(state: State<ProjectState>) -> Result<Option<String>, AppError> {
    state.ensure_project_operational().map_err(AppError::from)?;
    let path = state.get_path();

    log_app!(
        LogLevel::Info,
        "[command.get_project_path] Path: {:?}",
        path
    );

    Ok(path.map(|path| normalize_existing_path(&path).unwrap_or(path)))
}

#[tauri::command]
pub fn get_project_index(
    state: State<ProjectState>,
    project_instance_id: String,
) -> Result<ProjectIndex, AppError> {
    let project_instance_id = crate::project::ProjectInstanceId::from_existing(project_instance_id);
    state
        .read_project_index(&project_instance_id)
        .map_err(AppError::from)
}

#[tauri::command]
pub fn load_project_graph(
    state: State<ProjectState>,
    project_instance_id: String,
    graph_path: String,
    locale: Option<String>,
    lifecycle_token: u64,
) -> Result<EditorGraphProjectionDto, AppError> {
    let project_instance_id = crate::project::ProjectInstanceId::from_existing(project_instance_id);
    let graph_path = crate::project::GraphResourcePath::new(graph_path).map_err(AppError::from)?;
    state
        .load_graph_projection(
            &project_instance_id,
            &graph_path,
            lifecycle_token,
            locale.as_deref().unwrap_or("en-US"),
        )
        .map_err(AppError::from)
}

/// Resolve the on-disk path for a project resource (graph / database / worksheet).
#[tauri::command]
pub fn get_project_resource_path(
    state: State<ProjectState>,
    kind: String,
    resource_id: String,
) -> Result<String, AppError> {
    let request = RevealProjectResourceRequest::from_parts(&kind, resource_id)?;
    let path = resolve_reveal_path(&state, request).map_err(|e| e.to_string())?;
    if !path.exists() {
        return Err(AppError::new(
            "resource_not_found",
            format!("File not found: {}", path.display()),
        ));
    }
    Ok(format_path_for_user_path(&path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::value::{DataType, DataValue};
    use crate::node_system::document::{
        EditorGraphMutationDto, FunctionDocumentPatch, FunctionResourceKey, FunctionSignature,
        GraphRevision, MutationRequest, OperationId, ResourceKey,
    };
    use crate::node_system::protocol::NodeTypeId;
    use crate::project::{GraphResourcePath, ProjectData};
    use crate::variable::VariableScope;

    fn read_project_index_for_test(state: &ProjectState) -> ProjectIndex {
        let expected = state.capture_project_session().unwrap().instance_id;
        state.read_project_index(&expected).unwrap()
    }

    #[test]
    fn current_project_activation_bootstraps_late_created_webviews() {
        let project = crate::project::fixtures::TempProject::activate(
            "current-activation",
            ProjectData::new(),
        );
        let state = project.state();
        let session = state.capture_project_session().unwrap();

        let activation = current_project_activation(state).unwrap();

        assert_eq!(activation.project_instance_id, session.instance_id.as_str());
        assert_eq!(activation.activation_revision, state.activation_revision());
        assert!(!activation.path.is_empty());
    }

    fn editor_create_node_request(
        graph_path: &GraphResourcePath,
    ) -> MutationRequest<EditorGraphMutationDto> {
        MutationRequest::new(
            ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
                graph_path.as_str().into(),
            )),
            GraphRevision::INITIAL,
            OperationId::new(),
            EditorGraphMutationDto::CreateNode {
                descriptor: crate::node_system::catalog::NodeCreationDescriptor::Static {
                    node_type_id: NodeTypeId::new("yssbi.constant.int64").unwrap(),
                },
                position: crate::node_system::document::NodePosition { x: 10.0, y: 20.0 },
                user_label: None,
            },
        )
    }

    fn function_signature_request(
        function_path: &GraphResourcePath,
    ) -> MutationRequest<FunctionDocumentPatch> {
        MutationRequest::new(
            ResourceKey::Function(FunctionResourceKey(function_path.as_str().into())),
            GraphRevision::INITIAL,
            OperationId::new(),
            FunctionDocumentPatch::new(
                FunctionSignature::default(),
                FunctionSignature {
                    parameters: Vec::new(),
                    return_type: Some("Int64".into()),
                },
            ),
        )
    }

    #[test]
    fn project_index_projects_exact_database_authority_declarations() {
        let state = ProjectState::new();
        let mut data = ProjectData::new();
        let sales_engine = crate::database::DatabaseEngine::DuckDb {
            path: "database/project.duckdb".into(),
            table: "sales_facts".into(),
        };
        data.databases.insert(
            "sales".into(),
            crate::database::DatabaseDecl {
                id: "sales".into(),
                engine: sales_engine.clone(),
                schema_version: 7,
                required: true,
                name: Some("Sales warehouse".into()),
            },
        );
        data.databases.insert(
            "scratch".into(),
            crate::database::DatabaseDecl {
                id: "scratch".into(),
                engine: crate::database::DatabaseEngine::InMemory {
                    name: "scratch".into(),
                },
                schema_version: 2,
                required: false,
                name: None,
            },
        );
        state.activate_project_fixture("database-index-declaration".into(), data);

        let index = read_project_index_for_test(&state);

        assert_eq!(index.databases.len(), 2);
        let sales = index
            .databases
            .iter()
            .find(|database| database.id == "sales")
            .unwrap();
        assert_eq!(sales.resource_path.as_str(), "databases/sales");
        assert_eq!(
            sales.revision,
            crate::node_system::document::ResourceRevision::INITIAL
        );
        assert_eq!(sales.engine, sales_engine);
        assert_eq!(sales.schema_version, 7);
        assert!(sales.required);
        assert_eq!(sales.name.as_deref(), Some("Sales warehouse"));
        let scratch = index
            .databases
            .iter()
            .find(|database| database.id == "scratch")
            .unwrap();
        assert_eq!(scratch.name, None);
    }

    #[test]
    fn project_index_carries_one_coherent_publication_recovery_baseline() {
        let root = std::env::temp_dir().join(format!(
            "yssbi-publication-recovery-index-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        crate::project::fixtures::write_project(
            &ProjectData::new(),
            root.to_string_lossy().as_ref(),
        )
        .unwrap();

        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
        let function_path =
            crate::project::GraphResourcePath::new("functions/Recovery.yssbi-function").unwrap();
        state
            .insert_graph(
                function_path.clone(),
                crate::project::GraphResourceDocument::new(
                    "Recovery",
                    crate::project::GraphDocumentKind::Function,
                ),
            )
            .unwrap();
        let observed_revision = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let observer_revision = std::sync::Arc::clone(&observed_revision);
        let publication = state
            .update_function_signature_observed(
                &function_path,
                "en-US",
                function_signature_request(&function_path),
                move |result| {
                    observer_revision.store(
                        result.publication_revision,
                        std::sync::atomic::Ordering::SeqCst,
                    );
                },
            )
            .unwrap();

        let index = read_project_index_for_test(&state);

        assert_eq!(index.project_instance_id, publication.project_instance_id);
        assert_eq!(
            observed_revision.load(std::sync::atomic::Ordering::SeqCst),
            publication.publication_revision
        );
        assert_eq!(index.publication_revision, publication.publication_revision);
        assert_eq!(index.history, state.history_status());
        assert_eq!(index.history, publication.history);
        assert!(index.history.can_undo);
        assert!(!index.history.can_redo);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resource_publication_api_has_no_delta_only_success_wrappers() {
        let source = include_str!("../../project/project_state.rs");

        for forbidden in [
            "pub fn update_function_signature(",
            "pub fn undo_last_transaction(",
            "pub fn redo_last_transaction(",
        ] {
            assert!(
                !source.contains(forbidden),
                "resource publication API still exposes delta-only wrapper: {forbidden}"
            );
        }
    }

    #[test]
    fn signature_undo_redo_publications_are_contiguous_and_match_project_index() {
        let root = std::env::temp_dir().join(format!(
            "yssbi-signature-history-publication-index-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        crate::project::fixtures::write_project(
            &ProjectData::new(),
            root.to_string_lossy().as_ref(),
        )
        .unwrap();

        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
        let function_path =
            crate::project::GraphResourcePath::new("functions/History.yssbi-function").unwrap();
        state
            .insert_graph(
                function_path.clone(),
                crate::project::GraphResourceDocument::new(
                    "History",
                    crate::project::GraphDocumentKind::Function,
                ),
            )
            .unwrap();
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

        let signature = state
            .update_function_signature_observed(
                &function_path,
                "en-US",
                function_signature_request(&function_path),
                {
                    let observed = std::sync::Arc::clone(&observed);
                    move |result| observed.lock().unwrap().push(result.publication_revision)
                },
            )
            .unwrap();
        let after_signature = read_project_index_for_test(&state);
        let undo = state
            .undo_last_transaction_observed(
                "en-US",
                MutationRequest::new(
                    signature.deltas[0].resource.clone(),
                    signature.deltas[0].to_revision,
                    OperationId::new(),
                    crate::node_system::document::HistoryMutation {},
                ),
                {
                    let observed = std::sync::Arc::clone(&observed);
                    move |result| observed.lock().unwrap().push(result.publication_revision)
                },
            )
            .unwrap();
        let after_undo = read_project_index_for_test(&state);
        let redo = state
            .redo_last_transaction_observed(
                "en-US",
                MutationRequest::new(
                    undo.deltas[0].resource.clone(),
                    undo.deltas[0].to_revision,
                    OperationId::new(),
                    crate::node_system::document::HistoryMutation {},
                ),
                {
                    let observed = std::sync::Arc::clone(&observed);
                    move |result| observed.lock().unwrap().push(result.publication_revision)
                },
            )
            .unwrap();
        let after_redo = read_project_index_for_test(&state);

        assert_eq!(signature.publication_revision, 1);
        assert_eq!(
            after_signature.publication_revision,
            signature.publication_revision
        );
        assert_eq!(undo.publication_revision, 2);
        assert_eq!(after_undo.publication_revision, undo.publication_revision);
        assert_eq!(redo.publication_revision, 3);
        assert_eq!(after_redo.publication_revision, redo.publication_revision);
        assert_eq!(*observed.lock().unwrap(), vec![1, 2, 3]);
        assert_eq!(after_redo.history, state.history_status());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn graph_mutation_does_not_advance_resource_publication_baseline() {
        let root = std::env::temp_dir().join(format!(
            "yssbi-graph-resource-publication-index-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        crate::project::fixtures::write_project(
            &ProjectData::new(),
            root.to_string_lossy().as_ref(),
        )
        .unwrap();

        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
        let graph_path =
            crate::project::GraphResourcePath::new("events/GraphOnly.yssbi-event").unwrap();
        let function_path =
            crate::project::GraphResourcePath::new("functions/Next.yssbi-function").unwrap();
        state
            .insert_graph(
                graph_path.clone(),
                crate::project::GraphResourceDocument::new(
                    "GraphOnly",
                    crate::project::GraphDocumentKind::Event,
                ),
            )
            .unwrap();
        state
            .insert_graph(
                function_path.clone(),
                crate::project::GraphResourceDocument::new(
                    "Next",
                    crate::project::GraphDocumentKind::Function,
                ),
            )
            .unwrap();

        let graph_mutation = state
            .apply_editor_graph_mutation(
                &graph_path,
                "en-US",
                editor_create_node_request(&graph_path),
            )
            .unwrap();
        let after_graph = read_project_index_for_test(&state);
        let resource_publication = state
            .update_function_signature_observed(
                &function_path,
                "en-US",
                function_signature_request(&function_path),
                |_| {},
            )
            .unwrap();

        assert_eq!(graph_mutation.delta.to_revision, GraphRevision::new(1));
        assert!(graph_mutation.history.can_undo);
        assert_eq!(after_graph.publication_revision, 0);
        assert_eq!(after_graph.history, state.history_status());
        assert_eq!(resource_publication.publication_revision, 1);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_index_during_activation_observes_only_the_previous_complete_lifecycle() {
        let project = crate::project::fixtures::TempProject::activate(
            "query-activation-index",
            ProjectData::new(),
        );
        let state = project.state().clone();
        state
            .add_variable(
                "project_a_global",
                DataType::Int64,
                DataValue::Int64(1),
                "",
                VariableScope::Global,
                Vec::new(),
            )
            .unwrap();
        let previous_identity = state.project_instance_id();

        let replacement_project = crate::project::fixtures::TempProject::activate(
            "project-index-replacement",
            ProjectData::new(),
        );
        let replacement_source = replacement_project.state();
        replacement_source
            .add_variable(
                "project_b_global",
                DataType::Int64,
                DataValue::Int64(2),
                "",
                VariableScope::Global,
                Vec::new(),
            )
            .unwrap();
        let replacement_data = replacement_source.get_data().unwrap();

        let (activation_reached_tx, activation_reached_rx) = std::sync::mpsc::channel();
        let (release_activation_tx, release_activation_rx) = std::sync::mpsc::channel();
        let release_activation_rx = std::sync::Mutex::new(release_activation_rx);
        state.set_project_activation_test_hook(std::sync::Arc::new(move || {
            let _ = activation_reached_tx.send(());
            let _ = release_activation_rx
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .recv_timeout(std::time::Duration::from_secs(5));
        }));

        let (activation_done_tx, activation_done_rx) = std::sync::mpsc::channel();
        let activation = std::thread::spawn(move || {
            project
                .state()
                .activate_project_fixture("project-b".into(), replacement_data);
            let _ = activation_done_tx.send(());
        });
        activation_reached_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();

        let index = read_project_index_for_test(&state);
        drop(state);

        release_activation_tx.send(()).unwrap();
        activation_done_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        activation.join().unwrap();

        assert_eq!(index.project_instance_id, previous_identity);
        assert_eq!(index.variables.len(), 1);
        assert_eq!(index.variables[0].name, "project_a_global");
    }
}
