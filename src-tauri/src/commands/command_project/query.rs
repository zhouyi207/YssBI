use crate::application::execution::{ApplicationState, SessionCaptureError};
use crate::application::graph_open::{OpenGraphApplicationError, OpenGraphRequest};
use crate::error::CommandError;
#[cfg(test)]
use crate::project::ProjectState;
use crate::project::{ProjectIndex, normalize_existing_path};
use crate::schema::application_event::ProjectActivationResultDto;
use crate::schema::editor_projection_types::EditorGraphProjectionDto;
use crate::schema::{DatabaseDeclDTO, DatabasesVariablesDTO, VariableInstanceDTO};
use serde::Serialize;
use tauri::State;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryRequiredDetails {
    recovery_required: bool,
}

/// 分阶段加载第一步：获取 databases + variables（含 schema）
#[tauri::command]
pub fn get_project_databases_variables(
    application: State<ApplicationState>,
) -> Result<DatabasesVariablesDTO, CommandError> {
    tracing::info!(
        target: "yssbi::commands::project",
        diagnostic_domain = "data",
        diagnostic_event = "getProjectDataResources",
        "Loading project databases and variables"
    );

    application
        .query_project_databases_variables()
        .map_err(map_project_query_error)
        .and_then(project_databases_variables_to_transport)
}

fn project_databases_variables_to_transport(
    snapshot: crate::application::project_query::ProjectDatabasesVariablesSnapshot,
) -> Result<DatabasesVariablesDTO, CommandError> {
    let databases = snapshot
        .databases()
        .iter()
        .map(|database| {
            let mut dto = DatabaseDeclDTO::from(&database.declaration);
            let columns = crate::schema::column_info_from_schema(database.schema.columns());
            dto.column_count = Some(columns.len());
            dto.columns = Some(columns);
            (database.declaration.id.as_str().to_owned(), dto)
        })
        .collect();
    let variables = snapshot
        .variables()
        .iter()
        .map(|variable| {
            VariableInstanceDTO::try_from(variable)
                .map(|dto| (variable.id.to_string(), dto))
                .map_err(|error| CommandError::diagnosed("project_variable_mapping_failed", error))
        })
        .collect::<Result<_, _>>()?;
    Ok(DatabasesVariablesDTO {
        databases,
        variables,
    })
}

#[cfg(test)]
fn current_project_activation(
    state: &ProjectState,
) -> Result<ProjectActivationResultDto, CommandError> {
    let activation_revision = state.activation_revision();
    let session = state
        .capture_project_session()
        .map_err(CommandError::from)?;
    let path = state
        .get_path()
        .ok_or_else(|| CommandError::expected("stale_project_lifecycle"))?;
    state
        .validate_project_session(&session)
        .map_err(CommandError::from)?;
    if state.activation_revision() != activation_revision {
        return Err(CommandError::expected("stale_project_lifecycle"));
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
    application: State<ApplicationState>,
) -> Result<ProjectActivationResultDto, CommandError> {
    application
        .query_current_project_activation()
        .map_err(map_project_query_error)
        .map(project_activation_to_transport)
}

fn project_activation_to_transport(
    activation: crate::application::project_query::ProjectActivation,
) -> ProjectActivationResultDto {
    ProjectActivationResultDto {
        path: activation.path,
        project_instance_id: activation.project_instance_id.to_string(),
        activation_revision: activation.activation_revision,
    }
}

/// 获取当前项目路径
#[tauri::command]
pub fn get_project_path(
    application: State<ApplicationState>,
) -> Result<Option<String>, CommandError> {
    let path = application
        .query_project_path()
        .map_err(map_project_query_error)?;

    tracing::info!(
        target: "yssbi::commands::project",
        diagnostic_domain = "application",
        diagnostic_event = "getProjectPath",
        path = ?path,
        "Read project path"
    );

    Ok(path.map(|path| normalize_existing_path(&path).unwrap_or(path)))
}

#[tauri::command]
pub fn get_project_index(
    application: State<ApplicationState>,
    project_instance_id: String,
) -> Result<ProjectIndex, CommandError> {
    let project_instance_id =
        yss_project_identity::ProjectInstanceId::from_existing(project_instance_id);
    application
        .query_project_index(project_instance_id)
        .map_err(map_project_query_error)
}

#[tauri::command]
pub fn load_project_graph(
    state: State<'_, ApplicationState>,
    project_instance_id: String,
    graph_path: String,
    locale: Option<String>,
    lifecycle_token: u64,
) -> Result<EditorGraphProjectionDto, CommandError> {
    let project_instance_id =
        yss_project_identity::ProjectInstanceId::from_existing(project_instance_id);
    let graph_path = yss_graph_document::GraphResourcePath::new(graph_path)
        .map_err(|_| CommandError::expected("invalid_project_format"))?;
    let receipt = state
        .open_graph(OpenGraphRequest::new(
            project_instance_id,
            graph_path,
            lifecycle_token,
            locale.as_deref().unwrap_or("en-US"),
        ))
        .map_err(open_graph_command_error)?;
    crate::schema::editor_projection::map_editor_projection(receipt.projection())
        .map_err(|error| CommandError::diagnosed("editor_projection_mapping_failed", error))
}

fn open_graph_command_error(error: OpenGraphApplicationError) -> CommandError {
    match error {
        OpenGraphApplicationError::SessionCapture(error) => match error {
            SessionCaptureError::Inactive => CommandError::expected("stale_project_lifecycle"),
            SessionCaptureError::Replacing => {
                CommandError::expected("project_lifecycle_admission_closed")
            }
            SessionCaptureError::Recovering => CommandError::expected("project_recovery_required")
                .with_details(RecoveryRequiredDetails {
                    recovery_required: true,
                }),
        },
        OpenGraphApplicationError::SessionChanged => {
            CommandError::expected("stale_project_lifecycle")
        }
        OpenGraphApplicationError::Project(error) => {
            CommandError::diagnosed("graph_open_failed", error)
        }
        OpenGraphApplicationError::Database(error) => {
            CommandError::diagnosed("database_catalog_failed", error)
        }
        OpenGraphApplicationError::Contract(error) => {
            CommandError::diagnosed("graph_contract_failed", error)
        }
        OpenGraphApplicationError::Materialization(error) => {
            CommandError::diagnosed("graph_materialization_failed", error)
        }
        OpenGraphApplicationError::Projection(error) => {
            CommandError::diagnosed("editor_projection_failed", error)
        }
    }
}

/// Resolve the on-disk path for a project resource (graph / database / worksheet).
#[tauri::command]
pub fn get_project_resource_path(
    application: State<ApplicationState>,
    kind: String,
    resource_id: String,
) -> Result<String, CommandError> {
    application
        .reveal_project_resource(kind, resource_id)
        .map_err(map_project_query_error)
}

fn map_project_query_error(
    error: crate::application::project_query::ProjectQueryApplicationError,
) -> CommandError {
    use crate::application::project_query::ProjectQueryApplicationError;
    match error {
        ProjectQueryApplicationError::SessionCapture(error) => match error {
            SessionCaptureError::Inactive => CommandError::expected("stale_project_lifecycle"),
            SessionCaptureError::Replacing => {
                CommandError::expected("project_lifecycle_admission_closed")
            }
            SessionCaptureError::Recovering => CommandError::expected("project_recovery_required")
                .with_details(RecoveryRequiredDetails {
                    recovery_required: true,
                }),
        },
        ProjectQueryApplicationError::ProjectIdentityMismatch { .. } => {
            CommandError::expected("stale_project_lifecycle")
        }
        ProjectQueryApplicationError::Project(error) => CommandError::from(error),
        ProjectQueryApplicationError::ProjectRead(error) => {
            CommandError::diagnosed("project_query_failed", error)
        }
        ProjectQueryApplicationError::Database(error) => {
            CommandError::diagnosed("database_catalog_failed", error)
        }
        ProjectQueryApplicationError::InvalidResourceReference => {
            CommandError::expected("invalid_resource_reference")
        }
        ProjectQueryApplicationError::ResourceNotFound => {
            CommandError::expected("resource_not_found")
        }
        ProjectQueryApplicationError::SessionChanged(error) => {
            CommandError::diagnosed("project_query_session_changed", error)
        }
    }
}

#[cfg(all(test, any()))]
mod tests {
    use super::*;
    use yss_data_contract::{DataType, DataValue};
    use yss_graph_document::GraphResourcePath;
    use yss_graph_document::GraphRevision;
    use yss_graph_editor::EditorGraphMutation;
    use yss_graph_protocol::NodeTypeId;
    use yss_project_history::{
        FunctionDocumentPatch, FunctionResourceKey, FunctionSignature, MutationRequest, ResourceKey,
    };
    use yss_project_identity::OperationId;
    use yss_project_identity::ProjectInstanceId;
    use yss_project_model::ProjectData;
    use yss_variable_contract::VariableScope;

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
    ) -> MutationRequest<EditorGraphMutation> {
        MutationRequest::new(
            ResourceKey::Graph(graph_path.clone()),
            yss_project_identity::ResourceRevision::from_graph_revision(GraphRevision::INITIAL),
            OperationId::new(),
            EditorGraphMutation::CreateNode {
                descriptor: yss_graph_catalog::NodeCreation::Static {
                    node_type_id: NodeTypeId::new("yssbi.constant.int64").unwrap(),
                },
                position: yss_graph_document::NodePosition { x: 10.0, y: 20.0 },
                user_label: None,
                connect_from: None,
            },
        )
    }

    fn function_signature_request(
        function_path: &GraphResourcePath,
    ) -> MutationRequest<FunctionDocumentPatch> {
        MutationRequest::new(
            ResourceKey::Function(FunctionResourceKey(function_path.as_str().into())),
            yss_project_identity::ResourceRevision::from_graph_revision(GraphRevision::INITIAL),
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
            yss_graph_document::GraphResourcePath::new("functions/Recovery.yssbi-function")
                .unwrap();
        state
            .insert_graph(
                function_path.clone(),
                yss_project_model::GraphResourceDocument::new(
                    "Recovery",
                    yss_graph_document::GraphResourceKind::Function,
                ),
            )
            .unwrap();
        let observed_revision = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let observer_revision = std::sync::Arc::clone(&observed_revision);
        let publication = state
            .update_function_signature_observed(
                &ProjectInstanceId::from_existing(state.project_instance_id()),
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
            yss_graph_document::GraphResourcePath::new("functions/History.yssbi-function").unwrap();
        state
            .insert_graph(
                function_path.clone(),
                yss_project_model::GraphResourceDocument::new(
                    "History",
                    yss_graph_document::GraphResourceKind::Function,
                ),
            )
            .unwrap();
        let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

        let signature = state
            .update_function_signature_observed(
                &ProjectInstanceId::from_existing(state.project_instance_id()),
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
        let project_instance_id = state.capture_project_session().unwrap().instance_id;
        let undo = state
            .undo_last_transaction_observed(
                &project_instance_id,
                "en-US",
                MutationRequest::new(
                    signature.deltas[0].resource.clone(),
                    signature.deltas[0].to_revision,
                    OperationId::new(),
                    yss_project_history::HistoryMutation {},
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
                &project_instance_id,
                "en-US",
                MutationRequest::new(
                    undo.deltas[0].resource.clone(),
                    undo.deltas[0].to_revision,
                    OperationId::new(),
                    yss_project_history::HistoryMutation {},
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
            yss_graph_document::GraphResourcePath::new("events/GraphOnly.yssbi-event").unwrap();
        let function_path =
            yss_graph_document::GraphResourcePath::new("functions/Next.yssbi-function").unwrap();
        state
            .insert_graph(
                graph_path.clone(),
                yss_project_model::GraphResourceDocument::new(
                    "GraphOnly",
                    yss_graph_document::GraphResourceKind::Event,
                ),
            )
            .unwrap();
        state
            .insert_graph(
                function_path.clone(),
                yss_project_model::GraphResourceDocument::new(
                    "Next",
                    yss_graph_document::GraphResourceKind::Function,
                ),
            )
            .unwrap();

        let project_instance_id = state.capture_project_session().unwrap().instance_id;
        let graph_mutation = state
            .apply_editor_graph_mutation(
                &project_instance_id,
                &graph_path,
                "en-US",
                editor_create_node_request(&graph_path),
            )
            .unwrap();
        let after_graph = read_project_index_for_test(&state);
        let resource_publication = state
            .update_function_signature_observed(
                &ProjectInstanceId::from_existing(state.project_instance_id()),
                &function_path,
                "en-US",
                function_signature_request(&function_path),
                |_| {},
            )
            .unwrap();

        assert_eq!(
            graph_mutation.delta.to_revision,
            yss_project_identity::ResourceRevision::from_graph_revision(GraphRevision::new(1))
        );
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

    fn project_resources(label: &str) -> ProjectData {
        let mut data = ProjectData::new();
        data.databases.insert(
            "shared".into(),
            yss_database_contract::DatabaseDecl {
                id: yss_database_contract::DatabaseId::from_existing("shared".into()),
                engine: yss_database_contract::DatabaseEngine::InMemory {
                    name: format!("{label} engine"),
                },
                schema_version: 1,
                required: false,
                name: format!("{label} database").into(),
            },
        );
        let variable = yss_variable_contract::VariableInstance {
            id: yss_variable_contract::VariableId::new(),
            name: format!("{label} variable"),
            data_type: DataType::Int64,
            data_value: DataValue::Int64(1),
            tabular: None,
            description: String::new(),
            scope: VariableScope::Global,
            tags: Vec::new(),
        };
        data.variables.insert(variable.id, variable);
        data
    }

    #[test]
    fn project_resource_query_during_activation_never_mixes_authority_and_runtime() {
        let project = crate::project::fixtures::TempProject::activate(
            "query-resource-snapshot-old",
            project_resources("old"),
        );
        let state = project.state().clone();
        let old_snapshot = state.project_resource_snapshot().unwrap();
        assert_eq!(
            old_snapshot.project_instance_id.as_str(),
            state.project_instance_id()
        );
        assert_eq!(
            old_snapshot.authority_generation,
            state.authority_generation_for_test()
        );

        let (store_replaced_tx, store_replaced_rx) = std::sync::mpsc::channel();
        let (release_activation_tx, release_activation_rx) = std::sync::mpsc::channel();
        let release_activation_rx = std::sync::Mutex::new(release_activation_rx);
        state.set_activation_store_replaced_test_hook(std::sync::Arc::new(move || {
            store_replaced_tx.send(()).unwrap();
            release_activation_rx
                .lock()
                .unwrap()
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
        }));

        let activation_state = state.clone();
        let activation = std::thread::spawn(move || {
            activation_state.activate_project_fixture(
                "query-resource-snapshot-new".into(),
                project_resources("new"),
            );
        });
        store_replaced_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();

        let old_resources =
            crate::application::database_schema::databases_variables_from_snapshot(old_snapshot)
                .unwrap();
        assert_eq!(
            old_resources.databases["shared"].name.as_deref(),
            Some("old database")
        );
        assert_eq!(
            old_resources.variables.values().next().unwrap().name,
            "old variable"
        );

        let query_state = state.clone();
        let (query_started_tx, query_started_rx) = std::sync::mpsc::channel();
        let query = std::thread::spawn(move || {
            query_started_tx.send(()).unwrap();
            crate::application::database_schema::project_databases_variables(&query_state)
        });
        query_started_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        release_activation_tx.send(()).unwrap();

        activation.join().unwrap();
        let resources = query.join().unwrap().unwrap();
        let database = &resources.databases["shared"];
        let variable = resources.variables.values().next().unwrap();
        assert_eq!(database.name.as_deref(), Some("new database"));
        assert_eq!(variable.name, "new variable");
    }
}
