use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;

use arrow::array::{Int64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use arrow::error::ArrowError;
use arrow::record_batch::RecordBatch;
use uuid::Uuid;
use yss_database_contract::DatabaseId;
use yss_relational_contract::DatasetOverlay;
use yss_tabular_io::ParquetBatchWriter;

use crate::{DatasetFile, DatasetMetadata, DatasetSnapshot, DatasetStore, DatasetStoreError};

const MAX_PART_ROWS: usize = 1_000_000;
const WRITE_BATCH_ROWS: usize = 50_000;

#[derive(Default)]
struct InferredDomain {
    labels: Vec<String>,
    seen: std::collections::HashSet<String>,
}

pub struct PreparedDataset {
    pub(crate) store: Arc<DatasetStore>,
    pub(crate) operation_id: Box<str>,
    pub(crate) expected_snapshot: Option<Box<str>>,
    pub(crate) metadata: DatasetMetadata,
    pub(crate) files: Box<[DatasetFile]>,
    pub(crate) next_row_id: i64,
    pub(crate) retain_files: bool,
    pub(crate) base_schema: SchemaRef,
    pub(crate) overlay: DatasetOverlay,
    pub(crate) new_generation: bool,
    pub(crate) directory: Option<PathBuf>,
    pub(crate) snapshot_leases: Box<[Arc<DatasetSnapshot>]>,
    pub(crate) _generation_lease: Option<Arc<()>>,
}

impl PreparedDataset {
    pub fn publication(&self) -> crate::DatasetPublication {
        crate::DatasetPublication {
            operation_id: self.operation_id.clone(),
            dataset: self.metadata.id.clone(),
            before_snapshot: self.expected_snapshot.clone(),
            after_snapshot: self.metadata.snapshot_id.clone(),
        }
    }
    pub fn metadata(&self) -> &DatasetMetadata {
        &self.metadata
    }

    pub fn preserve_display_name(
        &mut self,
        snapshot: &DatasetSnapshot,
    ) -> Result<(), DatasetStoreError> {
        if self.metadata.id != snapshot.metadata().id {
            return Err(DatasetStoreError::InvalidIdentity);
        }
        self.metadata.name = snapshot.metadata().name.clone();
        Ok(())
    }
}

impl DatasetStore {
    pub(crate) fn prepare_generation(
        self: &Arc<Self>,
        metadata: DatasetMetadata,
        operation: &str,
        expected_snapshot: Option<Box<str>>,
    ) -> Result<PreparedDataset, DatasetStoreError> {
        let dataset = Uuid::parse_str(metadata.id.as_str())
            .map_err(|_| DatasetStoreError::InvalidIdentity)?;
        let generation = Uuid::parse_str(&metadata.generation_id)
            .map_err(|_| DatasetStoreError::InvalidIdentity)?;
        if operation.trim().is_empty() || metadata.name.trim().is_empty() {
            return Err(DatasetStoreError::InvalidIdentity);
        }
        yss_tabular_arrow::validate_storage_schema(&metadata.schema)
            .map_err(|_| DatasetStoreError::InvalidSchema)?;
        let directory = self
            .root
            .join("datasets")
            .join(dataset.to_string())
            .join(generation.to_string());
        let mut leases = self
            .leases
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        crate::paths::validate(&self.root, &directory)?;
        std::fs::create_dir_all(
            directory
                .parent()
                .ok_or(DatasetStoreError::InvalidIdentity)?,
        )?;
        std::fs::create_dir(&directory)?;
        crate::paths::validate(&self.root, &directory)?;
        let generation_lease = leases.prepare(directory.clone());
        Ok(PreparedDataset {
            store: self.clone(),
            operation_id: operation.into(),
            expected_snapshot,
            base_schema: metadata.schema.clone(),
            metadata,
            files: Box::new([]),
            next_row_id: 0,
            retain_files: false,
            overlay: DatasetOverlay::default(),
            new_generation: true,
            directory: Some(directory),
            snapshot_leases: Box::new([]),
            _generation_lease: Some(generation_lease),
        })
    }

    pub fn prepare_import(
        self: &Arc<Self>,
        id: DatabaseId,
        name: &str,
        operation: &str,
        source_schema: SchemaRef,
        batches: impl IntoIterator<Item = Result<RecordBatch, ArrowError>>,
    ) -> Result<PreparedDataset, DatasetStoreError> {
        let schema = import_schema(&source_schema)?;
        let metadata = DatasetMetadata {
            id,
            name: name.into(),
            deleted: false,
            snapshot_id: Uuid::new_v4().to_string().into(),
            generation_id: Uuid::new_v4().to_string().into(),
            data_revision: 1,
            schema_revision: 1,
            row_count: 0,
            schema: schema.clone(),
        };
        let mut prepared = self.prepare_generation(metadata, operation, None)?;
        let mut writer = GenerationWriter::new(&prepared)?;
        let mut domains = std::collections::BTreeMap::<usize, InferredDomain>::new();
        let mut domain_bytes = 0usize;
        for (index, field) in source_schema.fields().iter().enumerate() {
            if matches!(field.data_type(), DataType::Dictionary(..))
                && yss_tabular_arrow::CategoryDomain::from_field(field)
                    .map_err(|_| DatasetStoreError::InvalidSchema)?
                    .is_none()
            {
                domains.insert(index, InferredDomain::default());
            }
        }
        for batch in batches {
            let batch = batch?;
            if batch.schema() != source_schema {
                return Err(DatasetStoreError::InvalidSchema);
            }
            for (index, domain) in &mut domains {
                let data = batch.column(*index).to_data();
                let dictionary = data
                    .child_data()
                    .first()
                    .ok_or(DatasetStoreError::InvalidSchema)?;
                let values = arrow::array::make_array(dictionary.clone());
                let ordered = source_schema
                    .field(*index)
                    .dict_is_ordered()
                    .unwrap_or(false);
                let strings = arrow::compute::cast(values.as_ref(), &DataType::Utf8)?;
                let strings = strings
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .ok_or(DatasetStoreError::InvalidSchema)?;
                if ordered
                    && !domain.labels.is_empty()
                    && !strings
                        .iter()
                        .flatten()
                        .eq(domain.labels.iter().map(String::as_str))
                {
                    return Err(DatasetStoreError::InvalidSchema);
                }
                for label in strings.iter().flatten() {
                    if !domain.seen.contains(label) {
                        let cost = label
                            .len()
                            .checked_mul(2)
                            .and_then(|bytes| bytes.checked_add(64))
                            .ok_or(DatasetStoreError::DeltaLimit)?;
                        domain_bytes = domain_bytes
                            .checked_add(cost)
                            .ok_or(DatasetStoreError::DeltaLimit)?;
                        if domain_bytes > crate::codec::MAX_DELTA_BYTES {
                            return Err(DatasetStoreError::DeltaLimit);
                        }
                        domain.seen.insert(label.to_owned());
                        domain.labels.push(label.to_owned());
                    }
                }
            }
            for offset in (0..batch.num_rows()).step_by(WRITE_BATCH_ROWS) {
                let count = WRITE_BATCH_ROWS.min(batch.num_rows() - offset);
                let end = prepared
                    .next_row_id
                    .checked_add(
                        i64::try_from(count).map_err(|_| DatasetStoreError::InvalidSchema)?,
                    )
                    .ok_or(DatasetStoreError::InvalidSchema)?;
                let slice = batch.slice(offset, count);
                let mut columns = slice
                    .columns()
                    .iter()
                    .zip(schema.fields())
                    .map(|(array, field)| {
                        if array.data_type() == field.data_type() {
                            Ok(array.clone())
                        } else {
                            arrow::compute::cast(array.as_ref(), field.data_type())
                        }
                    })
                    .collect::<Result<Vec<_>, ArrowError>>()?;
                columns.push(Arc::new(Int64Array::from_iter_values(
                    prepared.next_row_id..end,
                )));
                columns.push(Arc::new(StringArray::from_iter_values(
                    (prepared.next_row_id..end).map(|id| format!("{id:020}")),
                )));
                writer.write(&RecordBatch::try_new(schema.clone(), columns)?)?;
                prepared.next_row_id = end;
            }
        }
        writer.finish(&mut prepared)?;
        if !domains.is_empty() {
            let mut fields = prepared.metadata.schema.fields().to_vec();
            for (index, inferred) in domains {
                let field = &fields[index];
                let id = yss_tabular_arrow::column_identity(field)
                    .map_err(|_| DatasetStoreError::InvalidSchema)?;
                let domain = yss_tabular_arrow::CategoryDomain {
                    labels: inferred.labels,
                    ordered: source_schema
                        .field(index)
                        .dict_is_ordered()
                        .unwrap_or(false),
                };
                fields[index] = Arc::new(
                    yss_tabular_arrow::with_column_metadata(
                        field.as_ref().clone(),
                        id,
                        Some(&domain),
                    )
                    .map_err(|_| DatasetStoreError::InvalidSchema)?,
                );
            }
            prepared.metadata.schema = Arc::new(Schema::new_with_metadata(
                fields,
                prepared.metadata.schema.metadata().clone(),
            ));
            prepared.base_schema = prepared.metadata.schema.clone();
        }
        Ok(prepared)
    }
}

pub(crate) struct GenerationWriter {
    root: PathBuf,
    relative_directory: PathBuf,
    schema: SchemaRef,
    writer: Option<ParquetBatchWriter>,
    part_path: PathBuf,
    part_rows: usize,
    total_rows: usize,
    files: Vec<DatasetFile>,
}

impl GenerationWriter {
    pub fn new(prepared: &PreparedDataset) -> Result<Self, DatasetStoreError> {
        let directory = prepared
            .directory
            .as_ref()
            .ok_or(DatasetStoreError::InvalidIdentity)?;
        let relative_directory = directory
            .strip_prefix(&prepared.store.root)
            .map_err(|_| DatasetStoreError::InvalidIdentity)?
            .to_owned();
        Ok(Self {
            root: prepared.store.root.clone(),
            relative_directory,
            schema: prepared.metadata.schema.clone(),
            writer: None,
            part_path: PathBuf::new(),
            part_rows: 0,
            total_rows: 0,
            files: Vec::new(),
        })
    }
    fn open_part(&mut self) -> Result<(), DatasetStoreError> {
        self.part_path = self
            .relative_directory
            .join(format!("part-{:06}.parquet", self.files.len()));
        crate::paths::validate(&self.root, &self.root.join(&self.part_path))?;
        self.writer = Some(ParquetBatchWriter::new(
            File::options()
                .write(true)
                .create_new(true)
                .open(self.root.join(&self.part_path))?,
            self.schema.clone(),
        )?);
        Ok(())
    }
    fn close_part(&mut self) -> Result<(), DatasetStoreError> {
        self.writer
            .take()
            .ok_or(DatasetStoreError::InvalidSchema)?
            .finish()?;
        self.files.push(DatasetFile {
            size_bytes: std::fs::metadata(self.root.join(&self.part_path))?.len(),
            relative_path: self.part_path.clone(),
            row_count: self.part_rows,
        });
        self.part_rows = 0;
        Ok(())
    }
    pub fn write(&mut self, batch: &RecordBatch) -> Result<(), DatasetStoreError> {
        if batch.schema() != self.schema {
            return Err(DatasetStoreError::InvalidSchema);
        }
        let mut offset = 0;
        while offset < batch.num_rows() {
            if self.writer.is_none() {
                self.open_part()?;
            }
            let count = WRITE_BATCH_ROWS
                .min(MAX_PART_ROWS - self.part_rows)
                .min(batch.num_rows() - offset);
            self.writer
                .as_mut()
                .ok_or(DatasetStoreError::InvalidSchema)?
                .write(&batch.slice(offset, count))?;
            self.part_rows += count;
            self.total_rows = self
                .total_rows
                .checked_add(count)
                .ok_or(DatasetStoreError::InvalidSchema)?;
            offset += count;
            if self.part_rows == MAX_PART_ROWS {
                self.close_part()?;
            }
        }
        Ok(())
    }
    pub fn finish(mut self, prepared: &mut PreparedDataset) -> Result<(), DatasetStoreError> {
        if self.files.is_empty() && self.writer.is_none() {
            self.open_part()?;
        }
        if self.writer.is_some() {
            self.close_part()?;
        }
        prepared.files = self.files.into_boxed_slice();
        prepared.metadata.row_count = self.total_rows;
        Ok(())
    }
}

fn import_schema(source: &Schema) -> Result<SchemaRef, DatasetStoreError> {
    let unique = |base: &str| {
        let mut name = base.to_owned();
        while source.index_of(&name).is_ok() {
            name.push('_');
        }
        name
    };
    let row_id = unique("__yssbi_row_id");
    let order = unique("__yssbi_display_order");
    let mut fields = source
        .fields()
        .iter()
        .map(|field| {
            // Dictionary indices are encoding details, not category identities. A managed
            // domain can span input batches whose individual dictionaries use smaller keys.
            let data_type = match field.data_type() {
                DataType::Dictionary(_, values)
                    if matches!(
                        values.as_ref(),
                        DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View
                    ) =>
                {
                    DataType::Dictionary(Box::new(DataType::Int32), Box::new(DataType::Utf8))
                }
                DataType::Dictionary(..) => return Err(DatasetStoreError::InvalidSchema),
                data_type => data_type.clone(),
            };
            let categories = yss_tabular_arrow::CategoryDomain::from_field(field)
                .map_err(|_| DatasetStoreError::InvalidSchema)?;
            yss_tabular_arrow::with_column_metadata(
                field
                    .as_ref()
                    .clone()
                    .with_nullable(true)
                    .with_data_type(data_type),
                &Uuid::new_v4().to_string(),
                categories.as_ref(),
            )
            .map_err(|_| DatasetStoreError::InvalidSchema)
        })
        .collect::<Result<Vec<_>, _>>()?;
    for field in [
        Field::new(&row_id, DataType::Int64, false),
        Field::new(&order, DataType::Utf8, false),
    ] {
        fields.push(
            yss_tabular_arrow::with_column_metadata(field, &Uuid::new_v4().to_string(), None)
                .map_err(|_| DatasetStoreError::InvalidSchema)?,
        );
    }
    let schema = yss_tabular_arrow::with_row_columns(
        Schema::new_with_metadata(fields, source.metadata().clone()),
        &row_id,
        &order,
    )
    .map_err(|_| DatasetStoreError::InvalidSchema)?;
    yss_tabular_arrow::validate_storage_schema(&schema)
        .map_err(|_| DatasetStoreError::InvalidSchema)?;
    Ok(Arc::new(schema))
}

impl Drop for PreparedDataset {
    fn drop(&mut self) {
        let Some(directory) = &self.directory else {
            return;
        };
        if !self.retain_files
            && directory.starts_with(self.store.root.join("datasets"))
            && crate::paths::validate(&self.store.root, directory).is_ok()
            && directory
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| Uuid::parse_str(name).is_ok())
        {
            let _ = std::fs::remove_dir_all(directory);
        }
    }
}
