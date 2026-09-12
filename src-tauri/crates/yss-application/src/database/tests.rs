use super::*;
use crate::execution::{ApplicationSessionEpoch, ApplicationSessionSlot};
use yss_relational_contract::RelationControl;

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("yss-project-dataset [test]-{}", Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn application(project: Arc<ProjectState>) -> ApplicationState {
    let backend = Arc::new(yss_sci_runtime::SciRuntimeBackend::new());
    let candidate = crate::execution::session_factory::build_current_project_candidate(
        ApplicationSessionEpoch::INITIAL,
        project,
        [],
        backend.clone(),
    )
    .unwrap();
    let app = ApplicationState::from_composition(Arc::new(ApplicationSessionSlot::new()), backend);
    app.install_candidate(candidate).unwrap();
    app
}
fn revision(app: &ApplicationState, id: &str) -> ResourceRevision {
    let captured = app.capture_session().unwrap();
    captured
        .project()
        .read_project_index(captured.project_instance_id())
        .unwrap()
        .databases
        .into_iter()
        .find(|entry| entry.id == id)
        .unwrap()
        .revision
}
fn edit(app: &ApplicationState, id: &str, mutation: DatabaseMutation) -> EditState {
    let instance = app.capture_session().unwrap().project_instance_id().clone();
    app.mutate_database_for_application(
        instance,
        id.into(),
        revision(app, id),
        OperationId::new(),
        mutation,
    )
    .unwrap()
    .data
}
fn rows(app: &ApplicationState, id: &str) -> DatabaseRowsResult {
    let instance = app.capture_session().unwrap().project_instance_id().clone();
    app.query_database_rows_for_application(instance, id.into(), 0, 20)
        .unwrap()
}

fn sample_resources() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/samples")
}

#[test]
fn bundled_samples_import_edit_and_reopen_as_independent_project_datasets() {
    use super::samples::{SAMPLE_ORIGIN_METADATA_KEY, SampleCatalog};
    let directory = Directory::new();
    let project_root = directory.0.join("project");
    let project = Arc::new(ProjectState::new());
    let created = project
        .create_project_transaction("Samples", &project_root, OperationId::new())
        .unwrap();
    project
        .activate_project_from_path(&created.metadata_path)
        .unwrap();
    let app = application(project);
    let instance = app.capture_session().unwrap().project_instance_id().clone();
    let catalog = SampleCatalog::new(sample_resources());
    let samples = catalog.list().unwrap();
    assert_eq!(samples.len(), 5);
    let store = yss_dataset_store::DatasetStore::open(&project_root).unwrap();
    let mut iris_id = String::new();
    let mut iris_operation = OperationId::new();
    for sample in samples {
        let operation = OperationId::new();
        let imported = app
            .import_sample_dataset_for_application(
                &catalog,
                instance.clone(),
                operation,
                &sample.id,
                sample.version,
            )
            .unwrap();
        assert_eq!(
            (imported.data.row_count, imported.data.column_count),
            (sample.row_count, sample.column_count)
        );
        let snapshot = store.snapshot(&database_id(&imported.data.id)).unwrap();
        let origin: serde_json::Value = serde_json::from_str(
            &snapshot.metadata().schema.metadata()[SAMPLE_ORIGIN_METADATA_KEY],
        )
        .unwrap();
        assert_eq!(origin["sampleId"], sample.id);
        assert_eq!(origin["version"], sample.version);
        assert_eq!(rows(&app, &imported.data.id).rows.row_count(), 20);
        if sample.id == "iris" {
            iris_id = imported.data.id;
            iris_operation = operation;
        }
    }
    assert!(
        app.import_sample_dataset_for_application(
            &catalog,
            instance.clone(),
            iris_operation,
            "iris",
            1
        )
        .is_err()
    );
    assert_eq!(store.catalog_metadata().unwrap().len(), 5);
    let second = app
        .import_sample_dataset_for_application(
            &catalog,
            instance.clone(),
            OperationId::new(),
            "iris",
            1,
        )
        .unwrap()
        .data;
    assert_ne!(second.id, iris_id);
    assert_ne!(second.name, "Iris");
    edit(
        &app,
        &iris_id,
        DatabaseMutation::EditCell {
            row: 0,
            column: "sepal_length".into(),
            value: serde_json::json!(99.0),
            row_id: Some(0),
        },
    );
    app.save_database_for_application(
        instance,
        iris_id.clone(),
        revision(&app, &iris_id),
        OperationId::new(),
    )
    .unwrap();
    let reopened = Arc::new(ProjectState::new());
    reopened
        .activate_project_from_path(&created.metadata_path)
        .unwrap();
    let reopened = application(reopened);
    assert_eq!(
        serde_json::to_value(
            rows(&reopened, &iris_id).rows.columns()[0].values()[0].display_value()
        )
        .unwrap(),
        serde_json::json!(99.0)
    );
    assert_eq!(
        serde_json::to_value(
            rows(&reopened, &second.id).rows.columns()[0].values()[0].display_value()
        )
        .unwrap(),
        serde_json::json!(5.1)
    );
    assert!(
        store
            .snapshot(&database_id(&iris_id))
            .unwrap()
            .metadata()
            .schema
            .metadata()
            .contains_key(SAMPLE_ORIGIN_METADATA_KEY)
    );
}

#[test]
fn plugin_data_boundary_enforces_the_granted_snapshot_and_aggregate_result_budget() {
    use yss_plugin_protocol::{CallContext, HostServices, ResourceBudget};
    let directory = Directory::new();
    let project = Arc::new(ProjectState::new());
    let created = project
        .create_project_transaction(
            "Plugin budgets",
            &directory.0.join("project"),
            OperationId::new(),
        )
        .unwrap();
    project
        .activate_project_from_path(&created.metadata_path)
        .unwrap();
    let app = application(project);
    let csv = directory.0.join("input.csv");
    std::fs::write(&csv, "x\n1\n2\n3\n").unwrap();
    let id = app
        .load_database_for_application(
            app.capture_session().unwrap().project_instance_id().clone(),
            OperationId::new(),
            DatabaseImportSource::Csv {
                path: csv.to_string_lossy().into(),
                delimiter: ',',
                has_header: true,
                infer_schema_length: Some(10),
            },
        )
        .unwrap()
        .data
        .id;
    let host = crate::plugins::PluginHostServices::new(app);
    let mut context = CallContext {
        context_id: "budget-context".into(),
        plugin_id: "example.plugin".into(),
        installation_generation: "1".into(),
        instance_id: "instance".into(),
        package_digest: "a".repeat(64),
        project: host.current_project().unwrap(),
        task_id: Some("task".into()),
        operation_id: Some("operation".into()),
        parameters_hash: Some("b".repeat(64)),
        granted_budget: ResourceBudget {
            snapshot_bytes: 256,
            ..ResourceBudget::default()
        },
    };
    let selection = serde_json::json!({"datasetId":id,"columns":["x"]});
    assert_eq!(
        host.invoke(&context, "data.snapshot", selection.clone(), &directory.0)
            .unwrap_err()
            .code,
        "plugin_resource_exhausted"
    );
    assert_eq!(
        std::fs::read_dir(directory.0.join("snapshots"))
            .unwrap()
            .count(),
        0
    );
    context.granted_budget.snapshot_bytes = 4096;
    let snapshot = host
        .invoke(&context, "data.snapshot", selection, &directory.0)
        .unwrap();
    assert!(Path::new(snapshot["path"].as_str().unwrap()).is_file());
    host.release_context(&context.context_id);
    assert!(!Path::new(snapshot["path"].as_str().unwrap()).exists());

    context.granted_budget.snapshot_bytes = 8;
    std::fs::write(directory.0.join("first.bin"), [0; 5]).unwrap();
    std::fs::write(directory.0.join("second.bin"), [0; 5]).unwrap();
    assert_eq!(
        host.invoke(
            &context,
            "results.commit",
            serde_json::json!({"artifacts":[{"path":"first.bin"},{"path":"second.bin"}]}),
            &directory.0
        )
        .unwrap_err()
        .code,
        "plugin_resource_exhausted"
    );
    assert!(!directory.0.join("project/extension-results").exists());
}

#[test]
fn project_import_edit_cast_undo_save_and_reopen_use_committed_dataset_snapshots() {
    let directory = Directory::new();
    let root = directory.0.join("project");
    let project = Arc::new(ProjectState::new());
    let created = project
        .create_project_transaction("Dataset project", &root, OperationId::new())
        .unwrap();
    project
        .activate_project_from_path(&created.metadata_path)
        .unwrap();
    let app = application(project.clone());
    let instance = app.capture_session().unwrap().project_instance_id().clone();
    let csv = directory.0.join("input.csv");
    std::fs::write(&csv, b"x,y\n0,1.5\n1,3.5\n2,5.5\n3,7.5\n").unwrap();
    let loaded = app
        .load_database_for_application(
            instance.clone(),
            OperationId::new(),
            DatabaseImportSource::Csv {
                path: csv.to_string_lossy().into(),
                delimiter: ',',
                has_header: true,
                infer_schema_length: Some(10),
            },
        )
        .unwrap()
        .data;
    let id = loaded.id;
    assert_eq!((loaded.row_count, loaded.column_count), (4, 2));
    assert!(root.join("database/catalog.sqlite").is_file());
    assert!(!root.join("database/project.duckdb").exists());
    let frozen = app
        .capture_session()
        .unwrap()
        .database()
        .capture_relation(&database_id(&id), revision(&app, &id).get())
        .unwrap();
    assert!(
        edit(
            &app,
            &id,
            DatabaseMutation::EditCell {
                row: 1,
                column: "y".into(),
                value: serde_json::json!(9.25),
                row_id: Some(1)
            }
        )
        .can_undo
    );
    assert_eq!(
        serde_json::to_value(rows(&app, &id).rows.columns()[1].values()[1].display_value())
            .unwrap(),
        serde_json::json!(9.25)
    );
    edit(
        &app,
        &id,
        DatabaseMutation::CastColumn {
            column: "y".into(),
            dtype: "Int8".into(),
            force: false,
        },
    );
    assert_eq!(
        serde_json::to_value(&rows(&app, &id).rows.columns()[1].values()[1]).unwrap(),
        serde_json::json!(9)
    );
    assert_eq!(
        app.query_database_meta_for_application(instance.clone(), id.clone())
            .unwrap()
            .columns[1]
            .display_type(),
        "Int8"
    );
    edit(&app, &id, DatabaseMutation::Undo);
    assert_eq!(
        serde_json::to_value(rows(&app, &id).rows.columns()[1].values()[1].display_value())
            .unwrap(),
        serde_json::json!(9.25)
    );
    edit(&app, &id, DatabaseMutation::AddRow { index: Some(1) });
    assert_eq!(rows(&app, &id).row_ids, vec![0, 4, 1, 2, 3]);
    edit(&app, &id, DatabaseMutation::Undo);
    assert_eq!(rows(&app, &id).row_ids, vec![0, 1, 2, 3]);
    let store = yss_dataset_store::DatasetStore::open(&root).unwrap();
    let generation = store
        .snapshot(&database_id(&id))
        .unwrap()
        .metadata()
        .generation_id
        .clone();
    let saved = app
        .save_database_for_application(
            instance.clone(),
            id.clone(),
            revision(&app, &id),
            OperationId::new(),
        )
        .unwrap();
    assert!(!saved.data.can_undo);
    assert_eq!(
        store
            .snapshot(&database_id(&id))
            .unwrap()
            .metadata()
            .generation_id,
        generation
    );
    let control = RelationControl {
        cancellation: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        deadline: std::time::Instant::now() + std::time::Duration::from_secs(10),
        max_input_bytes: 1024 * 1024,
    };
    assert_eq!(
        serde_json::to_value(
            frozen.page(0, 4, &control).unwrap().data.columns()[1].values()[1].display_value()
        )
        .unwrap(),
        serde_json::json!(3.5)
    );
    {
        let reopened = Arc::new(ProjectState::new());
        reopened
            .activate_project_from_path(&created.metadata_path)
            .unwrap();
        let reopened = application(reopened);
        assert_eq!(
            serde_json::to_value(
                rows(&reopened, &id).rows.columns()[1].values()[1].display_value()
            )
            .unwrap(),
            serde_json::json!(9.25)
        );
    }
    assert!(
        app.delete_database_for_application(
            instance.clone(),
            id.clone(),
            ResourceRevision::new(0),
            OperationId::new()
        )
        .is_err()
    );
    assert_eq!(store.catalog_metadata().unwrap().len(), 1);
    app.delete_database_for_application(
        instance,
        id.clone(),
        revision(&app, &id),
        OperationId::new(),
    )
    .unwrap();
    assert!(store.catalog_metadata().unwrap().is_empty());
    assert_eq!(frozen.page(0, 4, &control).unwrap().row_count, 4);
}

#[test]
fn opening_a_version_four_project_does_not_create_a_catalog_or_change_its_files() {
    let directory = Directory::new();
    let manifest = directory.0.join(yss_project_layout::PROJECT_METADATA_FILE);
    let bytes = br#"{"schemaVersion":4,"projectName":"Old","exportTime":""}"#;
    std::fs::write(&manifest, bytes).unwrap();
    std::fs::create_dir(directory.0.join("database")).unwrap();
    let original = directory.0.join("database/project.duckdb");
    std::fs::write(&original, b"preserved old project").unwrap();
    let project = Arc::new(ProjectState::new());
    let app = application(project.clone());
    let before = app.capture_session().unwrap();
    let error = app
        .load_project_for_application(&manifest.to_string_lossy())
        .unwrap_err();
    assert!(matches!(
        error,
        crate::project_lifecycle::ApplicationProjectLifecycleError::Lifecycle(
            crate::project_lifecycle::ProjectLifecycleError::LoadFailed(
                ProjectFilesystemError::TransactionPrepareFailed { .. }
            )
        )
    ));
    let format_error =
        yss_project::load_project_from_file(&manifest.to_string_lossy()).unwrap_err();
    assert!(
        matches!(format_error, yss_project::ProjectError::Deserialize(source) if source.to_string().contains("unsupported schema version 4"))
    );
    assert_eq!(std::fs::read(&manifest).unwrap(), bytes);
    assert_eq!(std::fs::read(&original).unwrap(), b"preserved old project");
    assert!(!directory.0.join("database/catalog.sqlite").exists());
    let after = app
        .capture_session()
        .expect("a rejected project must not close the current application session");
    assert!(Arc::ptr_eq(&before, &after));

    let created = tokio::runtime::Runtime::new().unwrap().block_on(async {
        let store = yss_project_registry_sqlite::SqliteProjectRegistryStore::connect(
            directory.0.join("registry"),
        )
        .await
        .unwrap();
        let registry_path = store.path().to_owned();
        let registry = yss_project_registry::ProjectRegistry::new(Arc::new(store), registry_path);
        app.create_project_for_application(
            &registry,
            "New project after rejected load",
            &directory.0.join("new-project"),
            OperationId::new(),
        )
        .await
        .unwrap()
    });
    app.load_project_for_application(created.path.as_deref().unwrap())
        .unwrap();
    assert!(
        app.capture_session()
            .unwrap()
            .project()
            .get_path()
            .is_some()
    );
}

#[test]
fn failed_final_project_activation_rebuilds_the_previous_database_session() {
    let directory = Directory::new();
    let project = Arc::new(ProjectState::new());
    let current = project
        .create_project_transaction("Current", &directory.0.join("current"), OperationId::new())
        .unwrap();
    project
        .activate_project_from_path(&current.metadata_path)
        .unwrap();
    let app = application(project.clone());
    let csv = directory.0.join("input.csv");
    std::fs::write(&csv, b"x\n1\n2\n").unwrap();
    let before = app.capture_session().unwrap();
    let dataset = app
        .load_database_for_application(
            before.project_instance_id().clone(),
            OperationId::new(),
            DatabaseImportSource::Csv {
                path: csv.to_string_lossy().into(),
                delimiter: ',',
                has_header: true,
                infer_schema_length: Some(10),
            },
        )
        .unwrap()
        .data;
    let expected = rows(&app, &dataset.id);
    let target = project
        .create_project_transaction("Target", &directory.0.join("target"), OperationId::new())
        .unwrap();
    let manifest = target.metadata_path.clone();
    project.set_project_activation_test_hook(Arc::new(move || {
        std::fs::write(&manifest, b"invalidated after preparation").unwrap();
    }));
    assert!(matches!(
        app.load_project_for_application(&target.metadata_path.to_string_lossy()),
        Err(
            crate::project_lifecycle::ApplicationProjectLifecycleError::Lifecycle(
                crate::project_lifecycle::ProjectLifecycleError::LoadFailed(_)
            )
        )
    ));
    let after = app
        .capture_session()
        .expect("failed activation must restore admission");
    assert_eq!(after.project_instance_id(), before.project_instance_id());
    let actual = rows(&app, &dataset.id);
    assert_eq!(actual.rows, expected.rows);
    assert_eq!(actual.row_ids, expected.row_ids);
}
