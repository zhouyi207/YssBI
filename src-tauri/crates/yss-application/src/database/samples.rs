//! Bundled sample discovery and admission into the ordinary project import workflow.

use std::fs::File;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use arrow::datatypes::Schema;
use arrow::record_batch::{RecordBatch, RecordBatchIterator, RecordBatchReader};
use serde::Deserialize;
use yss_canonical_hash::content_sha256_reader;
use yss_project_identity::{OperationId, ProjectInstanceId};

use super::import::{ImportReader, import_in_captured_session};
use super::{ApplicationState, DatabaseMutationResult, DatabaseUseCaseError, LoadDatabaseResult};

const MAX_CATALOG_BYTES: u64 = 256 * 1024;
const MAX_SAMPLE_BYTES: u64 = 64 * 1024 * 1024;
pub const SAMPLE_ORIGIN_METADATA_KEY: &str = "yssbi.sample.origin";

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SampleDataset {
    pub id: String,
    pub name: String,
    pub version: u32,
    pub row_count: usize,
    pub column_count: usize,
    pub byte_size: u64,
    sha256: String,
    source_file: String,
    source_sha256: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CatalogDocument {
    format_version: u32,
    datasets: Vec<SampleDataset>,
}

#[derive(Debug, thiserror::Error)]
pub enum SampleError {
    #[error("sample catalog is invalid")]
    InvalidCatalog,
    #[error("sample was not found")]
    NotFound,
    #[error("sample version does not match the catalog")]
    VersionMismatch,
    #[error("sample resource content does not match the catalog")]
    Integrity,
    #[error("sample resource path is invalid")]
    InvalidPath,
    #[error("sample resource is unavailable")]
    Unavailable(#[from] std::io::Error),
    #[error("sample resource cannot be decoded")]
    Decode(#[from] yss_tabular_io::TabularIoError),
    #[error("sample catalog cannot be decoded")]
    CatalogDecode(#[from] serde_json::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum SampleImportError {
    #[error(transparent)]
    Sample(#[from] SampleError),
    #[error(transparent)]
    Database(#[from] DatabaseUseCaseError),
}

#[derive(Clone)]
pub struct SampleCatalog {
    root: PathBuf,
    entries: Arc<OnceLock<Vec<SampleDataset>>>,
}

impl SampleCatalog {
    /// The desktop composition root supplies the platform's resolved resource directory.
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            entries: Arc::new(OnceLock::new()),
        }
    }

    pub fn list(&self) -> Result<Vec<SampleDataset>, SampleError> {
        if let Some(entries) = self.entries.get() {
            return Ok(entries.clone());
        }
        let mut file = self.open_resource(Path::new("catalog.json"))?;
        let mut bytes = Vec::new();
        file.by_ref()
            .take(MAX_CATALOG_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_CATALOG_BYTES {
            return Err(SampleError::InvalidCatalog);
        }
        let document: CatalogDocument = serde_json::from_slice(&bytes)?;
        if document.format_version != 1 || document.datasets.len() > 64 {
            return Err(SampleError::InvalidCatalog);
        }
        let mut ids = std::collections::HashSet::new();
        for entry in &document.datasets {
            if entry.id.is_empty()
                || entry.id.len() > 64
                || !entry
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                || !ids.insert(&entry.id)
                || entry.version == 0
                || entry.name.trim().is_empty()
                || entry.name.len() > 128
                || entry.row_count > 5_000_000
                || entry.column_count == 0
                || entry.column_count > 512
                || entry.byte_size == 0
                || entry.byte_size > MAX_SAMPLE_BYTES
                || !valid_hash(&entry.sha256)
                || !valid_hash(&entry.source_sha256)
                || entry.source_file.len() > 128
                || Path::new(&entry.source_file)
                    .file_name()
                    .and_then(|name| name.to_str())
                    != Some(entry.source_file.as_str())
            {
                return Err(SampleError::InvalidCatalog);
            }
        }
        // Cache only successful metadata reads. A repaired installation can retry failures.
        let _ = self.entries.set(document.datasets.clone());
        Ok(document.datasets)
    }

    fn open_resource(&self, relative: &Path) -> Result<File, SampleError> {
        let mut path = self.root.clone();
        reject_redirect(&path)?;
        for component in relative.components() {
            let std::path::Component::Normal(part) = component else {
                return Err(SampleError::InvalidPath);
            };
            path.push(part);
            reject_redirect(&path)?;
        }
        let file = File::open(path)?;
        if !file.metadata()?.is_file() {
            return Err(SampleError::InvalidPath);
        }
        Ok(file)
    }

    fn open_sample(&self, id: &str, version: u32) -> Result<ImportReader, SampleError> {
        let entry = self
            .list()?
            .into_iter()
            .find(|entry| entry.id == id)
            .ok_or(SampleError::NotFound)?;
        if entry.version != version {
            return Err(SampleError::VersionMismatch);
        }
        let relative = Path::new(&entry.id)
            .join(format!("v{}", entry.version))
            .join("data.parquet");
        let mut file = self.open_resource(&relative)?;
        if file.metadata()?.len() != entry.byte_size
            || content_sha256_reader(file.by_ref().take(entry.byte_size + 1))? != entry.sha256
        {
            return Err(SampleError::Integrity);
        }
        file.rewind()?;
        let mut reader = yss_tabular_io::read_parquet_file_batches(file, 10_000, None)?;
        let original_schema = reader.schema();
        if original_schema.fields().len() != entry.column_count {
            return Err(SampleError::Integrity);
        }
        let mut metadata = original_schema.metadata().clone();
        metadata.insert(
            SAMPLE_ORIGIN_METADATA_KEY.into(),
            serde_json::json!({
                "sampleId": entry.id, "version": entry.version, "sha256": entry.sha256,
                "sourceFile": entry.source_file, "sourceSha256": entry.source_sha256,
            })
            .to_string(),
        );
        let schema = Arc::new(Schema::new_with_metadata(
            original_schema.fields().clone(),
            metadata,
        ));
        let batch_schema = schema.clone();
        let mut rows = 0usize;
        let mut finished = false;
        let batches = std::iter::from_fn(move || {
            if finished {
                return None;
            }
            let batch = match reader.next() {
                Some(batch) => batch,
                None => {
                    finished = true;
                    return (rows != entry.row_count).then(|| {
                        Err(arrow::error::ArrowError::ExternalError(Box::new(
                            SampleError::Integrity,
                        )))
                    });
                }
            };
            Some(batch.and_then(|batch| {
                rows += batch.num_rows();
                if rows > entry.row_count || batch.get_array_memory_size() > 16 * 1024 * 1024 {
                    finished = true;
                    return Err(arrow::error::ArrowError::ExternalError(Box::new(
                        SampleError::Integrity,
                    )));
                }
                RecordBatch::try_new(batch_schema.clone(), batch.columns().to_vec())
            }))
        });
        Ok(ImportReader::new(
            entry.name,
            Box::new(RecordBatchIterator::new(batches, schema)),
        ))
    }
}

fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn reject_redirect(path: &Path) -> Result<(), SampleError> {
    let metadata = std::fs::symlink_metadata(path)?;
    if yss_project_filesystem::metadata_is_redirect(&metadata) {
        return Err(SampleError::InvalidPath);
    }
    Ok(())
}

impl ApplicationState {
    pub fn import_sample_dataset_for_application(
        &self,
        catalog: &SampleCatalog,
        project_instance_id: ProjectInstanceId,
        operation_id: OperationId,
        sample_id: &str,
        version: u32,
    ) -> Result<DatabaseMutationResult<LoadDatabaseResult>, SampleImportError> {
        let captured = self.capture_database_session(&project_instance_id)?;
        let reader = catalog.open_sample(sample_id, version)?;
        let result = import_in_captured_session(&captured, operation_id, |_| Ok(reader))?;
        self.refresh_database_session()?;
        Ok(result)
    }
}
