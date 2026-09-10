//! Committed dataset catalog, immutable Parquet generations, and storage publication handoffs.
//! Project session gates and graph/document authority remain with their existing owners.

mod catalog;
mod codec;
mod edits;
mod gc;
mod leases;
mod paths;
mod prepare;

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use arrow::datatypes::SchemaRef;
use sqlx::SqlitePool;
use yss_database_contract::DatabaseId;
use yss_relational_contract::{
    DatasetOverlay, DatasetRelationInput, RelationBinding, RelationError,
};

pub use edits::{DatasetCellEdit, DatasetColumnCast};
pub use prepare::PreparedDataset;

#[derive(Debug, thiserror::Error)]
pub enum DatasetStoreError {
    #[error("dataset filesystem operation failed")]
    Io(#[from] std::io::Error),
    #[error("dataset catalog operation failed")]
    Catalog(#[from] sqlx::Error),
    #[error("dataset commit outcome requires catalog recovery")]
    CommitUncertain(#[source] sqlx::Error),
    #[error("dataset batch encoding failed")]
    Encoding(#[from] yss_tabular_io::TabularIoError),
    #[error("dataset batch could not be read")]
    Batch(#[from] arrow::error::ArrowError),
    #[error("dataset schema is invalid")]
    InvalidSchema,
    #[error("dataset identity or operation is invalid")]
    InvalidIdentity,
    #[error("dataset snapshot is unavailable")]
    NotFound,
    #[error("dataset catalog has an unsupported format")]
    UnsupportedFormat,
    #[error("dataset catalog changed before commit")]
    Conflict,
    #[error("dataset catalog record is invalid")]
    CorruptCatalog,
    #[error("dataset edit value is invalid")]
    InvalidValue,
    #[error("dataset row is unavailable")]
    RowNotFound,
    #[error("dataset sparse edits require compaction")]
    DeltaLimit,
    #[error("dataset query failed")]
    Query(#[from] RelationError),
}

#[derive(Clone, Debug)]
pub struct DatasetFile {
    relative_path: PathBuf,
    pub row_count: usize,
    pub size_bytes: u64,
}

impl DatasetFile {
    pub fn relative_path(&self) -> &Path {
        &self.relative_path
    }
}

#[derive(Clone, Debug)]
pub struct DatasetMetadata {
    pub id: DatabaseId,
    pub name: Box<str>,
    pub deleted: bool,
    pub snapshot_id: Box<str>,
    pub generation_id: Box<str>,
    pub data_revision: u64,
    pub schema_revision: u64,
    pub row_count: usize,
    pub schema: SchemaRef,
}

/// A fixed committed version. The owner stays alive while an execution/result holds this lease.
pub struct DatasetSnapshot {
    metadata: DatasetMetadata,
    files: Box<[DatasetFile]>,
    base_schema: SchemaRef,
    overlay: DatasetOverlay,
    next_row_id: i64,
    store: Arc<DatasetStore>,
}

impl DatasetSnapshot {
    pub fn store(&self) -> &Arc<DatasetStore> {
        &self.store
    }
    pub fn metadata(&self) -> &DatasetMetadata {
        &self.metadata
    }
    pub fn files(&self) -> &[DatasetFile] {
        &self.files
    }
    pub fn file_paths(&self) -> Vec<PathBuf> {
        self.files
            .iter()
            .map(|file| self.store.root.join(&file.relative_path))
            .collect()
    }
    pub fn relation_input(&self) -> DatasetRelationInput {
        DatasetRelationInput {
            base_schema: self.base_schema.clone(),
            schema: self.metadata.schema.clone(),
            files: self.file_paths().into_boxed_slice(),
            overlay: self.overlay.clone(),
        }
    }
    pub fn query(
        self: &Arc<Self>,
        engine: &Arc<yss_datafusion::DataFusionRuntime>,
        project_session: &str,
    ) -> Result<yss_datafusion::DatasetQuery, DatasetStoreError> {
        Ok(engine.dataset_query(
            RelationBinding {
                project_session: project_session.into(),
                dataset: self.metadata.id.clone(),
                snapshot: self.metadata.snapshot_id.clone(),
                revision: self.metadata.data_revision,
            },
            self.relation_input(),
            self.clone(),
        )?)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatasetPublication {
    pub operation_id: Box<str>,
    pub dataset: DatabaseId,
    pub before_snapshot: Option<Box<str>>,
    pub after_snapshot: Box<str>,
}

pub struct CommittedDataset {
    pub snapshot: Arc<DatasetSnapshot>,
    pub publication: DatasetPublication,
}

pub struct DatasetStore {
    root: PathBuf,
    pool: SqlitePool,
    runtime: Option<tokio::runtime::Runtime>,
    catalog_gate: Mutex<()>,
    leases: Arc<Mutex<leases::SnapshotLeases>>,
}

impl DatasetStore {
    /// Create a new catalog; never replace an existing project catalog.
    pub fn create(project_root: &Path) -> Result<Arc<Self>, DatasetStoreError> {
        let project_root = std::fs::canonicalize(project_root)?;
        let root = project_root.join(yss_project_layout::DATABASE_DIR);
        paths::validate(&project_root, &root)?;
        std::fs::create_dir_all(&root)?;
        paths::validate_catalog(&root)?;
        std::fs::File::options()
            .write(true)
            .create_new(true)
            .open(root.join(yss_project_layout::PROJECT_DATASET_CATALOG_FILE))?;
        Self::connect(root, true)
    }

    /// Open only committed catalog state. Unlisted files are never discovered as datasets.
    pub fn open(project_root: &Path) -> Result<Arc<Self>, DatasetStoreError> {
        let project_root = std::fs::canonicalize(project_root)?;
        let root = project_root.join(yss_project_layout::DATABASE_DIR);
        paths::validate(&project_root, &root)?;
        Self::connect(root, false)
    }

    fn connect(root: PathBuf, initialize: bool) -> Result<Arc<Self>, DatasetStoreError> {
        outside_runtime(|| Self::connect_blocking(root, initialize))
    }

    fn connect_blocking(root: PathBuf, initialize: bool) -> Result<Arc<Self>, DatasetStoreError> {
        paths::validate_catalog(&root)?;
        let root = std::fs::canonicalize(root)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        if !initialize {
            runtime.block_on(catalog::validate_existing(
                &root.join(yss_project_layout::PROJECT_DATASET_CATALOG_FILE),
            ))?;
        }
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(root.join(yss_project_layout::PROJECT_DATASET_CATALOG_FILE))
            .create_if_missing(false)
            .foreign_keys(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Full);
        let pool = runtime.block_on(
            sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options),
        )?;
        if initialize {
            runtime.block_on(catalog::initialize(&pool))?;
        }
        let version: i64 =
            runtime.block_on(sqlx::query_scalar("PRAGMA user_version").fetch_one(&pool))?;
        if version != catalog::CATALOG_VERSION {
            return Err(DatasetStoreError::UnsupportedFormat);
        }
        runtime.block_on(async { pool.acquire().await?.close().await })?;
        Ok(Arc::new(Self {
            leases: leases::for_root(&root),
            root,
            pool,
            runtime: Some(runtime),
            catalog_gate: Mutex::new(()),
        }))
    }

    fn catalog<T: Send>(
        &self,
        operation: impl Future<Output = Result<T, DatasetStoreError>> + Send,
    ) -> Result<T, DatasetStoreError> {
        outside_runtime(|| {
            let _gate = self
                .catalog_gate
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            paths::validate_catalog(&self.root)?;
            let runtime = self.runtime.as_ref().ok_or(DatasetStoreError::Conflict)?;
            let result = runtime.block_on(operation);
            // Keep leases independent of SQLite handles so drained projects can move on Windows.
            // A close error must not recast a successful commit.
            let _ = runtime.block_on(async { self.pool.acquire().await?.close().await });
            result
        })
    }

    pub fn catalog_metadata(&self) -> Result<Vec<DatasetMetadata>, DatasetStoreError> {
        self.catalog(catalog::list(&self.pool))
    }

    pub fn catalog_snapshot(&self) -> Result<Vec<u8>, DatasetStoreError> {
        struct TemporaryCatalog(PathBuf);
        impl Drop for TemporaryCatalog {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let temporary = TemporaryCatalog(
            std::env::temp_dir().join(format!("yss-catalog-copy-{}.sqlite", uuid::Uuid::new_v4())),
        );
        let file = std::fs::File::options()
            .write(true)
            .create_new(true)
            .open(&temporary.0)?;
        self.catalog(async {
            sqlx::query("VACUUM INTO ?")
                .bind(
                    temporary
                        .0
                        .to_str()
                        .ok_or(DatasetStoreError::InvalidIdentity)?,
                )
                .execute(&self.pool)
                .await?;
            Ok(())
        })?;
        file.sync_all()?;
        Ok(std::fs::read(&temporary.0)?)
    }

    pub fn snapshot(
        self: &Arc<Self>,
        database: &DatabaseId,
    ) -> Result<Arc<DatasetSnapshot>, DatasetStoreError> {
        let mut leases = self.leases.lock().unwrap_or_else(PoisonError::into_inner);
        let catalog::LoadedSnapshot {
            metadata,
            files,
            base_schema,
            overlay,
            next_row_id,
        } = self.catalog(catalog::load_current(&self.pool, database))?;
        for file in &files {
            paths::validate(&self.root, &self.root.join(&file.relative_path))?;
        }
        let snapshot = Arc::new(DatasetSnapshot {
            metadata,
            files,
            base_schema,
            overlay,
            next_row_id,
            store: self.clone(),
        });
        leases.retain(&snapshot);
        Ok(snapshot)
    }

    pub fn commit(
        self: &Arc<Self>,
        mut prepared: PreparedDataset,
    ) -> Result<CommittedDataset, DatasetStoreError> {
        if !Arc::ptr_eq(self, &prepared.store) {
            return Err(DatasetStoreError::InvalidIdentity);
        }
        for file in &prepared.files {
            paths::validate(&self.root, &self.root.join(&file.relative_path))?;
        }
        let publication = prepared.publication();
        let encoded = codec::encode_overlay(&prepared.metadata.schema, &prepared.overlay)?;
        self.catalog(catalog::commit(&self.pool, &mut prepared, &encoded))?;
        let snapshot = Arc::new(DatasetSnapshot {
            metadata: prepared.metadata.clone(),
            files: prepared.files.clone(),
            base_schema: prepared.base_schema.clone(),
            overlay: prepared.overlay.clone(),
            next_row_id: prepared.next_row_id,
            store: self.clone(),
        });
        self.leases
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(&snapshot);
        Ok(CommittedDataset {
            snapshot,
            publication,
        })
    }

    pub fn pending_publications(&self) -> Result<Vec<DatasetPublication>, DatasetStoreError> {
        self.catalog(catalog::pending_publications(&self.pool))
    }

    pub fn publication_committed(
        &self,
        publication: &DatasetPublication,
    ) -> Result<bool, DatasetStoreError> {
        self.catalog(async {
            let found: Option<(String, Option<String>, String)> = sqlx::query_as("SELECT dataset_id,before_snapshot,after_snapshot FROM dataset_operations WHERE id=?").bind(publication.operation_id.as_ref()).fetch_optional(&self.pool).await?;
            match found {
                None => Ok(false),
                Some((dataset, before, after)) if dataset == publication.dataset.as_str() && before.as_deref() == publication.before_snapshot.as_deref() && after == publication.after_snapshot.as_ref() => Ok(true),
                Some(_) => Err(DatasetStoreError::Conflict),
            }
        })
    }

    pub fn acknowledge_publication(
        &self,
        publication: &DatasetPublication,
    ) -> Result<(), DatasetStoreError> {
        self.catalog(catalog::acknowledge(&self.pool, publication))
    }
}

impl Drop for DatasetStore {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

// Project lifecycle entry points can also call this synchronous adapter while driving Tokio.
// Isolate only those calls; normal database blocking workers incur no thread handoff.
fn outside_runtime<T: Send>(
    operation: impl FnOnce() -> Result<T, DatasetStoreError> + Send,
) -> Result<T, DatasetStoreError> {
    if tokio::runtime::Handle::try_current().is_err() {
        return operation();
    }
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name("dataset-catalog".into())
            .spawn_scoped(scope, operation)?
            .join()
            .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    })
}

#[cfg(test)]
mod tests;
