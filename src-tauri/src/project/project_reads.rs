use crate::project::{
    ProjectData, ProjectFilesystemError, ProjectIndex, ProjectInstanceId, ProjectSession,
    ProjectState, WorksheetDocument,
};

impl ProjectState {
    pub fn read_project_index(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
    ) -> Result<ProjectIndex, ProjectFilesystemError> {
        read_project_index_with(self, expected_project_instance_id, |root| {
            crate::project::project_io::read_project_index_from_root(root).map_err(read_error)
        })
    }

    pub fn load_worksheet_document(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        worksheet_id: &str,
    ) -> Result<WorksheetDocument, ProjectFilesystemError> {
        load_worksheet_document_with_reader(
            self,
            expected_project_instance_id,
            worksheet_id,
            |root, worksheet_id| {
                crate::project::worksheet_io::load_worksheet_from_root_readonly(root, worksheet_id)
                    .map_err(read_error)
            },
        )
    }
}

fn expected_session(
    state: &ProjectState,
    expected_project_instance_id: &ProjectInstanceId,
) -> Result<ProjectSession, ProjectFilesystemError> {
    let session = state.capture_project_session()?;
    if &session.instance_id != expected_project_instance_id {
        return Err(ProjectFilesystemError::StaleProjectLifecycle {
            message: format!(
                "requested project instance '{}' is no longer active",
                expected_project_instance_id
            ),
        });
    }
    Ok(session)
}

fn read_project_index_with(
    state: &ProjectState,
    expected_project_instance_id: &ProjectInstanceId,
    read: impl FnOnce(&std::path::Path) -> Result<ProjectIndex, ProjectFilesystemError>,
) -> Result<ProjectIndex, ProjectFilesystemError> {
    let session = expected_session(state, expected_project_instance_id)?;
    let _lease = state.filesystem().acquire(session.root.clone())?;
    state.validate_project_session(&session)?;
    let disk_result = read(session.root.as_path());
    state.validate_project_session(&session)?;
    let mut index = disk_result?;
    let (project_instance_id, publication_revision, history, data) =
        state.coherent_project_read_snapshot(&session)?;
    let variable_revisions = state.global_variable_revision_snapshot();
    overlay_authoritative_project_index(&data, &variable_revisions, &mut index);
    index.project_instance_id = project_instance_id;
    index.publication_revision = publication_revision;
    index.history = history;
    state.validate_project_session(&session)?;
    Ok(index)
}

fn load_worksheet_document_with_reader(
    state: &ProjectState,
    expected_project_instance_id: &ProjectInstanceId,
    worksheet_id: &str,
    read: impl FnOnce(&std::path::Path, &str) -> Result<WorksheetDocument, ProjectFilesystemError>,
) -> Result<WorksheetDocument, ProjectFilesystemError> {
    let session = expected_session(state, expected_project_instance_id)?;
    let _lease = state.filesystem().acquire(session.root.clone())?;
    state.validate_project_session(&session)?;
    let disk_result = read(session.root.as_path(), worksheet_id);
    state.validate_project_session(&session)?;
    let disk_document = disk_result?;
    let (_, _, _, data) = state.coherent_project_read_snapshot(&session)?;
    let document = data
        .worksheets
        .get(worksheet_id)
        .cloned()
        .unwrap_or(disk_document);
    state.validate_project_session(&session)?;
    Ok(document)
}

fn overlay_authoritative_project_index(
    data: &ProjectData,
    variable_revisions: &std::collections::BTreeMap<
        crate::node_system::document::ResourceKey,
        crate::node_system::document::ResourceRevision,
    >,
    index: &mut ProjectIndex,
) {
    index
        .variables
        .retain(|variable| !matches!(variable.scope, crate::variable::VariableScope::Global));
    index.variables.extend(
        data.variables
            .values()
            .filter(|variable| matches!(variable.scope, crate::variable::VariableScope::Global))
            .cloned()
            .map(|variable| {
                let key = crate::node_system::document::ResourceKey::Variable(
                    crate::node_system::document::VariableResourceKey(
                        format!("variables/{}", variable.id).into(),
                    ),
                );
                let mut entry = crate::project::ProjectVariableIndexEntry::from(variable);
                entry.revision = variable_revisions
                    .get(&key)
                    .copied()
                    .expect("authoritative global variable has a revision");
                entry
            }),
    );
    for entry in &mut index.graphs {
        let Ok(path) = crate::project::GraphResourcePath::new(&entry.path) else {
            continue;
        };
        let Some(function) = data
            .graphs
            .get(&path)
            .and_then(|resource| resource.function.as_ref())
        else {
            continue;
        };
        entry.function_revision = Some(function.revision);
        entry.function_signature = Some(function.signature.clone());
    }
}

fn read_error(error: crate::project::ProjectError) -> ProjectFilesystemError {
    ProjectFilesystemError::TransactionPrepareFailed {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use crate::graph::value::{DataType, DataValue};
    use crate::node_system::document::FunctionSignature;
    use crate::project::{
        GraphDocumentKind, GraphResourceDocument, GraphResourcePath, ProjectData,
        ProjectFilesystemError, ProjectState, WorksheetDocument, fixtures,
        read_project_index as read_project_index_from_disk,
    };
    use crate::variable::VariableScope;

    use std::time::Duration;

    fn project_root(label: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "yssbi-project-reads-{label}-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn delayed_project_index_read_has_zero_effects_after_project_replacement() {
        let root = project_root("delayed-index");
        let graph_path = GraphResourcePath::new("events/Legacy.yssbi-event").unwrap();
        let mut project = ProjectData::new();
        project.graphs.insert(
            graph_path.clone(),
            GraphResourceDocument::new("Legacy", GraphDocumentKind::Event),
        );
        fixtures::write_project(&project, root.to_string_lossy().as_ref()).unwrap();
        fixtures::write_graph(&project, root.to_string_lossy().as_ref(), &graph_path).unwrap();
        let nested_dir = root.join("events/Nested");
        std::fs::create_dir_all(&nested_dir).unwrap();
        let nested_path = nested_dir.join("Legacy.yssbi-event");
        let flattened_path = root.join(graph_path.as_str());
        std::fs::rename(&flattened_path, &nested_path).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
        let expected = state.capture_project_session().unwrap().instance_id;
        let replacement_state = state.clone();

        let result = super::read_project_index_with(&state, &expected, move |root| {
            replacement_state.activate_project_fixture("project-b".into(), ProjectData::new());
            replacement_state
                .add_variable(
                    "project_b_global",
                    DataType::Int64,
                    DataValue::Int64(42),
                    "",
                    VariableScope::Global,
                    Vec::new(),
                )
                .unwrap();
            read_project_index_from_disk(root.to_string_lossy().as_ref()).map_err(|error| {
                ProjectFilesystemError::TransactionPrepareFailed {
                    message: error.to_string(),
                }
            })
        });

        assert_eq!(result.unwrap_err().code(), "stale_project_lifecycle");
        assert_eq!(state.get_path().as_deref(), Some("project-b"));
        assert!(
            nested_path.is_file(),
            "stale index read moved the nested graph"
        );
        assert!(
            nested_dir.is_dir(),
            "stale index read removed the nested directory"
        );
        assert!(
            !flattened_path.exists(),
            "stale index read created a flattened graph"
        );
        let data = state.get_data().unwrap();
        assert_eq!(data.variables.len(), 1);
        assert_eq!(
            data.variables
                .values()
                .next()
                .map(|value| value.name.as_str()),
            Some("project_b_global")
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn compatibility_project_index_reader_is_not_exposed() {
        let reads = include_str!("project_reads.rs");
        let query = include_str!("../commands/command_project/query.rs");

        let forbidden = concat!("pub(crate) fn read_project_index_", "with_reader");
        let compatibility_name = concat!("read_project_index_", "with_reader");
        assert!(!reads.contains(forbidden));
        assert!(!query.contains(compatibility_name));
    }

    #[test]
    fn project_index_overlays_functions_and_globals_from_one_authoritative_snapshot() {
        let root = project_root("coherent-index");
        let function_path = GraphResourcePath::new("functions/Shared.yssbi-function").unwrap();
        let mut disk = ProjectData::new();
        let disk_global = crate::variable::VariableInstance {
            id: crate::variable::VariableId::new(),
            name: "stale_disk_global".into(),
            data_type: DataType::Int64,
            data_value: DataValue::Int64(1),
            tabular: None,
            description: String::new(),
            scope: VariableScope::Global,
            tags: Vec::new(),
        };
        disk.variables.insert(disk_global.id, disk_global);
        disk.graphs.insert(
            function_path.clone(),
            GraphResourceDocument::new("Shared", GraphDocumentKind::Function),
        );
        fixtures::write_project(&disk, root.to_string_lossy().as_ref()).unwrap();
        fixtures::write_graph(&disk, root.to_string_lossy().as_ref(), &function_path).unwrap();

        let mut authoritative = disk;
        let global = authoritative.variables.values_mut().next().unwrap();
        global.name = "authoritative_global".into();
        let function = authoritative.graphs.get_mut(&function_path).unwrap();
        function.function = Some(crate::node_system::document::FunctionDocument {
            revision: crate::node_system::document::GraphRevision::new(7),
            signature: FunctionSignature {
                parameters: Vec::new(),
                return_type: Some("Int64".into()),
            },
        });
        let before = serde_json::to_value(&authoritative).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), authoritative);
        let expected = state.capture_project_session().unwrap().instance_id;

        let index = state.read_project_index(&expected).unwrap();

        assert_eq!(index.variables.len(), 1);
        assert_eq!(index.variables[0].name, "authoritative_global");
        let function = index
            .graphs
            .iter()
            .find(|entry| entry.path == function_path.as_str())
            .unwrap();
        assert_eq!(function.function_revision.unwrap().get(), 7);
        assert_eq!(
            function
                .function_signature
                .as_ref()
                .unwrap()
                .return_type
                .as_deref(),
            Some("Int64")
        );
        assert_eq!(
            serde_json::to_value(state.get_data().unwrap()).unwrap(),
            before
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_index_waits_for_resource_writer_and_returns_committed_layout() {
        let root = project_root("writer-index");
        fixtures::write_project(&ProjectData::new(), root.to_string_lossy().as_ref()).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
        let session = state.capture_project_session().unwrap();
        let lease = state.filesystem().acquire(session.root.clone()).unwrap();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let reader_state = state.clone();
        let expected = session.instance_id.clone();
        let reader = std::thread::spawn(move || {
            let result = reader_state.read_project_index(&expected);
            done_tx.send(()).unwrap();
            result
        });

        assert!(done_rx.recv_timeout(Duration::from_millis(100)).is_err());
        let graph_path = GraphResourcePath::new("events/Committed.yssbi-event").unwrap();
        let mut committed = ProjectData::new();
        committed.graphs.insert(
            graph_path.clone(),
            GraphResourceDocument::new("Committed", GraphDocumentKind::Event),
        );
        fixtures::write_graph(
            &committed,
            session.root.as_path().to_string_lossy().as_ref(),
            &graph_path,
        )
        .unwrap();
        drop(lease);

        let index = reader.join().unwrap().unwrap();
        assert_eq!(index.graphs.len(), 1);
        assert_eq!(index.graphs[0].path, graph_path.as_str());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn worksheet_load_rejects_replaced_project_before_returning_document() {
        let root = project_root("worksheet-replacement");
        let worksheet = WorksheetDocument::new("Old worksheet", "db-1");
        let mut project = ProjectData::new();
        project
            .worksheets
            .insert(worksheet.id.clone(), worksheet.clone());
        fixtures::write_project(&project, root.to_string_lossy().as_ref()).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), project);
        let expected = state.capture_project_session().unwrap().instance_id;
        let replacement_state = state.clone();

        let worksheet_id = worksheet.id.clone();
        let result = super::load_worksheet_document_with_reader(
            &state,
            &expected,
            &worksheet_id,
            move |_, _| {
                replacement_state.activate_project_fixture("project-b".into(), ProjectData::new());
                Ok(worksheet)
            },
        );

        assert_eq!(result.unwrap_err().code(), "stale_project_lifecycle");
        assert!(state.get_data().unwrap().worksheets.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
