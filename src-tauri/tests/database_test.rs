//! Production database workflows over the committed catalog and Arrow file boundary.
use arrow::array::{
    Int8Array, Int32Array, Int64Array, StringArray,
    UInt8Array,
};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::{RecordBatch, RecordBatchReader};
use std::path::PathBuf;
use std::sync::Arc;
use yss_application::database::{DatabaseMutation, DatabaseRowsResult};
use yss_application::execution::{
    ApplicationSessionEpoch, ApplicationSessionSlot, ApplicationState,
};
use yss_database_contract::DatabaseImportSource;

use yss_project::ProjectState;
use yss_project_identity::{OperationId, ResourceRevision};

struct Native {
    schema: arrow::datatypes::SchemaRef,
    batches: Vec<RecordBatch>,
}

struct Project {
    app: ApplicationState,
    state: Arc<ProjectState>,
    directory: PathBuf,
    metadata: PathBuf,
}
impl Project {
    fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("yss-database ['test']-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let project = Arc::new(ProjectState::new());
        let metadata = project
            .create_project_transaction("Data", &directory.join("project"), OperationId::new())
            .unwrap()
            .metadata_path;
        project.activate_project_from_path(&metadata).unwrap();
        Self {
            app: application(project.clone()),
            state: project,
            directory,
            metadata,
        }
    }
    fn import(&self, source: DatabaseImportSource) -> String {
        self.app
            .load_database_for_application(
                self.app
                    .capture_session()
                    .unwrap()
                    .project_instance_id()
                    .clone(),
                OperationId::new(),
                source,
            )
            .unwrap()
            .data
            .id
    }
    fn batch(&self, batch: RecordBatch) -> String {
        let path = self
            .directory
            .join(format!("input-{}.parquet", uuid::Uuid::new_v4()));
        yss_tabular_io::write_parquet_batches(&path, batch.schema(), [Ok(batch.clone())]).unwrap();
        self.import(DatabaseImportSource::Parquet {
            path: path.to_string_lossy().into(),
            columns: None,
        })
    }
    fn revision(&self, id: &str) -> ResourceRevision {
        let capture = self.app.capture_session().unwrap();
        self.state
            .read_project_index(capture.project_instance_id())
            .unwrap()
            .databases
            .into_iter()
            .find(|entry| entry.id == id)
            .unwrap()
            .revision
    }
    fn edit(
        &self,
        id: &str,
        operation: DatabaseMutation,
    ) -> Result<yss_database_edit::EditState, yss_application::database::DatabaseUseCaseError> {
        self.app
            .mutate_database_for_application(
                self.app
                    .capture_session()
                    .unwrap()
                    .project_instance_id()
                    .clone(),
                id.into(),
                self.revision(id),
                OperationId::new(),
                operation,
            )
            .map(|result| result.data)
    }
    fn rows(&self, id: &str, offset: usize, limit: usize) -> DatabaseRowsResult {
        self.app
            .query_database_rows_for_application(
                self.app
                    .capture_session()
                    .unwrap()
                    .project_instance_id()
                    .clone(),
                id.into(),
                offset,
                limit,
            )
            .unwrap()
    }
    fn native(&self, id: &str, columns: &[&str], offset: usize, limit: usize) -> Native {
        let store = yss_dataset_store::DatasetStore::open(self.metadata.parent().unwrap()).unwrap();
        let decl = self.state.get_data().unwrap().databases[id].clone();
        let instance = yss_database_runtime::bind_dataset_instance(&decl, &store);
        let control = yss_relational_contract::RelationControl {
            cancellation: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(20),
            max_input_bytes: 16 * 1024 * 1024,
        };
        let batches = instance
            .read_arrow_columns(columns, offset, limit, &control)
            .unwrap();
        Native {
            schema: batches[0].schema(),
            batches,
        }
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
fn application(project: Arc<ProjectState>) -> ApplicationState {
    let backend = Arc::new(yss_sci_runtime::SciRuntimeBackend::new());
    let candidate = yss_application::execution::session_factory::build_current_project_candidate(
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
fn value(rows: &DatabaseRowsResult, column: usize, row: usize) -> serde_json::Value {
    serde_json::to_value(&rows.rows.columns()[column].values()[row]).unwrap()
}

#[test]
fn invalid_edit_targets_and_integer_overflow_leave_data_and_history_unchanged() {
    let project = Project::new();
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("signed", DataType::Int8, false),
            Field::new("unsigned", DataType::UInt8, false),
        ])),
        vec![
            Arc::new(Int8Array::from(vec![7])),
            Arc::new(UInt8Array::from(vec![9])),
        ],
    )
    .unwrap();
    let id = project.batch(batch);
    let revision = project.revision(&id);
    assert!(
        project
            .edit(
                &id,
                DatabaseMutation::AddColumn {
                    name: "bad".into(),
                    dtype: "Mystery".into()
                }
            )
            .is_err()
    );
    for (column, value) in [
        ("signed", serde_json::json!(128)),
        ("unsigned", serde_json::json!(-1)),
    ] {
        assert!(
            project
                .edit(
                    &id,
                    DatabaseMutation::EditCell {
                        row: 0,
                        column: column.into(),
                        value,
                        row_id: Some(0)
                    }
                )
                .is_err()
        );
    }
    let native = project.native(&id, &["signed", "unsigned"], 0, 1);
    assert_eq!(native.schema.field(0).data_type(), &DataType::Int8);
    assert_eq!(
        native.batches[0]
            .column(0)
            .as_any()
            .downcast_ref::<Int8Array>()
            .unwrap()
            .value(0),
        7
    );
    assert_eq!(
        native.batches[0]
            .column(1)
            .as_any()
            .downcast_ref::<UInt8Array>()
            .unwrap()
            .value(0),
        9
    );
    assert_eq!(project.revision(&id), revision);
    assert!(
        !project
            .app
            .query_database_edit_state_for_application(
                project
                    .app
                    .capture_session()
                    .unwrap()
                    .project_instance_id()
                    .clone(),
                id
            )
            .unwrap()
            .can_undo
    );
}

#[test]
fn deleting_a_large_column_and_undo_preserve_exact_type_nulls_and_stable_identity() {
    let project = Project::new();
    let count = 50_001_i32;
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![
            Field::new("keep", DataType::Int64, false),
            Field::new("removed", DataType::Int32, true),
        ])),
        vec![
            Arc::new(Int64Array::from_iter_values(0..i64::from(count))),
            Arc::new(Int32Array::from_iter(
                (0..count).map(|value| (value % 5 != 0).then_some(-value)),
            )),
        ],
    )
    .unwrap();
    let id = project.batch(batch);
    let original = project.native(&id, &["removed"], 49_998, 3);
    project
        .edit(
            &id,
            DatabaseMutation::DeleteColumn {
                name: "removed".into(),
            },
        )
        .unwrap();
    let stale = project.revision(&id);
    project.edit(&id, DatabaseMutation::Undo).unwrap();
    let restored = project.native(&id, &["removed"], 49_998, 3);
    assert_eq!(restored.schema, original.schema);
    assert_eq!(restored.batches[0], original.batches[0]);
    assert_eq!(
        project.rows(&id, 49_998, 3).row_ids,
        vec![49_998, 49_999, 50_000]
    );
    assert!(
        project
            .app
            .mutate_database_for_application(
                project
                    .app
                    .capture_session()
                    .unwrap()
                    .project_instance_id()
                    .clone(),
                id.clone(),
                stale,
                OperationId::new(),
                DatabaseMutation::Undo
            )
            .is_err()
    );
    assert_eq!(
        project.native(&id, &["removed"], 49_998, 3).batches,
        restored.batches
    );
}

#[test]
fn literal_names_and_force_cast_have_reversible_values() {
    let project = Project::new();
    let column = "value.a\"'[]";
    let batch = RecordBatch::try_new(
        Arc::new(Schema::new(vec![Field::new(column, DataType::Utf8, true)])),
        vec![Arc::new(StringArray::from(vec!["1", "bad", ""]))],
    )
    .unwrap();
    let id = project.batch(batch);
    project
        .edit(
            &id,
            DatabaseMutation::EditCell {
                row: 0,
                column: column.into(),
                value: serde_json::json!("O'Reilly\\path"),
                row_id: Some(0),
            },
        )
        .unwrap();
    assert_eq!(
        value(&project.rows(&id, 0, 3), 0, 0),
        serde_json::json!("O'Reilly\\path")
    );
    project.edit(&id, DatabaseMutation::Undo).unwrap();
    assert!(
        project
            .edit(
                &id,
                DatabaseMutation::CastColumn {
                    column: column.into(),
                    dtype: "Int64".into(),
                    force: false
                }
            )
            .is_err()
    );
    project
        .edit(
            &id,
            DatabaseMutation::CastColumn {
                column: column.into(),
                dtype: "Int64".into(),
                force: true,
            },
        )
        .unwrap();
    let page = project.rows(&id, 0, 3);
    assert_eq!(value(&page, 0, 0), serde_json::json!(1));
    assert_eq!(value(&page, 0, 1), serde_json::Value::Null);
    project.edit(&id, DatabaseMutation::Undo).unwrap();
    assert_eq!(
        value(&project.rows(&id, 0, 3), 0, 1),
        serde_json::json!("bad")
    );
}

#[test]
fn row_history_save_and_multiple_dataset_reopen_keep_names_and_contents() {
    let project = Project::new();
    let csv = project.directory.join("input.csv");
    std::fs::write(&csv, b"value,label\n1,alpha\n2,beta\n3,gamma\n").unwrap();
    let source = DatabaseImportSource::Csv {
        path: csv.to_string_lossy().into(),
        delimiter: ',',
        has_header: true,
        infer_schema_length: Some(10),
    };
    let id = project.import(source.clone());
    let second = project.import(source);
    project
        .edit(&id, DatabaseMutation::AddRow { index: None })
        .unwrap();
    assert_eq!(project.rows(&id, 0, 10).row_ids, vec![0, 1, 2, 3]);
    project
        .edit(
            &id,
            DatabaseMutation::DeleteRows {
                indices: vec![2, 0],
                row_ids: Some(vec![2, 0]),
            },
        )
        .unwrap();
    assert_eq!(project.rows(&id, 0, 10).row_ids, vec![1, 3]);
    project.edit(&id, DatabaseMutation::Undo).unwrap();
    project.edit(&id, DatabaseMutation::Redo).unwrap();
    let instance = project
        .app
        .capture_session()
        .unwrap()
        .project_instance_id()
        .clone();
    project
        .app
        .rename_database_for_application(
            instance.clone(),
            id.clone(),
            project.revision(&id),
            "Renamed".into(),
            OperationId::new(),
        )
        .unwrap();
    let saved = project
        .app
        .save_database_for_application(
            instance,
            id.clone(),
            project.revision(&id),
            OperationId::new(),
        )
        .unwrap()
        .data;
    assert!(!saved.can_undo && !saved.can_redo && !saved.is_modified);
    let reopened = Arc::new(ProjectState::new());
    reopened
        .activate_project_from_path(&project.metadata)
        .unwrap();
    let app = application(reopened.clone());
    let capture = app.capture_session().unwrap();
    let index = reopened
        .read_project_index(capture.project_instance_id())
        .unwrap();
    assert_eq!(index.databases.len(), 2);
    assert_eq!(
        index
            .databases
            .iter()
            .find(|entry| entry.id == id)
            .unwrap()
            .name
            .as_deref(),
        Some("Renamed")
    );
    let rows = app
        .query_database_rows_for_application(capture.project_instance_id().clone(), id, 0, 10)
        .unwrap();
    assert_eq!(rows.row_ids, vec![1, 3]);
    assert_eq!(value(&rows, 0, 0), serde_json::json!(2));
    assert_eq!(project.rows(&second, 0, 10).row_ids, vec![0, 1, 2]);
}
