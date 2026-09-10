//! Temporary committed dataset fixtures for runtime and consumer contract tests.
use crate::DatabaseInstance;
use arrow::record_batch::RecordBatch;
use std::path::PathBuf;
use yss_database_contract::{DatabaseDecl, DatabaseEngine, DatabaseId};
use yss_dataset_store::DatasetStore;

pub const SALES_ID: &str = "00000000-0000-4000-8000-000000000001";

pub struct DatasetFixture {
    path: PathBuf,
    pub instance: DatabaseInstance,
}
impl DatasetFixture {
    pub fn new(id: &str, batch: RecordBatch) -> Self {
        let path =
            std::env::temp_dir().join(format!("yss-runtime-dataset-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        let store = DatasetStore::create(&path).unwrap();
        let id = DatabaseId::from_existing(id.into());
        let prepared = store
            .prepare_import(
                id.clone(),
                "Sales",
                "fixture-import",
                batch.schema(),
                [Ok(batch)],
            )
            .unwrap();
        let committed = store.commit(prepared).unwrap();
        store
            .acknowledge_publication(&committed.publication)
            .unwrap();
        let decl = DatabaseDecl {
            id,
            engine: DatabaseEngine::Dataset {},
            schema_version: 5,
            required: false,
            name: "Sales".into(),
        };
        Self {
            path,
            instance: crate::bind_dataset_instance(&decl, &store),
        }
    }
    pub fn single_i64(id: &str, column: &str, values: Vec<i64>) -> Self {
        let schema = std::sync::Arc::new(arrow::datatypes::Schema::new(vec![
            arrow::datatypes::Field::new(column, arrow::datatypes::DataType::Int64, true),
        ]));
        Self::new(
            id,
            RecordBatch::try_new(
                schema,
                vec![std::sync::Arc::new(arrow::array::Int64Array::from(values))],
            )
            .unwrap(),
        )
    }
    pub fn single_f64(id: &str, column: &str, values: Vec<f64>) -> Self {
        let schema = std::sync::Arc::new(arrow::datatypes::Schema::new(vec![
            arrow::datatypes::Field::new(column, arrow::datatypes::DataType::Float64, true),
        ]));
        Self::new(
            id,
            RecordBatch::try_new(
                schema,
                vec![std::sync::Arc::new(arrow::array::Float64Array::from(
                    values,
                ))],
            )
            .unwrap(),
        )
    }
}
impl Drop for DatasetFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
