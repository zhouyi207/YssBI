//! Bounded Arrow IPC/CSV/Parquet I/O and Excel decoding for host data.

mod csv;
mod excel;
pub use csv::{CsvBatchReader, MAX_CSV_BATCH_BYTES};

use std::fs::{self, File};
use std::io::Seek;
use std::path::Path;
use std::sync::Arc;

use arrow::datatypes::{DataType, Schema, SchemaRef};
use arrow::error::ArrowError;
use arrow::ipc::{
    MetadataVersion,
    reader::FileReader,
    writer::{FileWriter, IpcWriteOptions},
};
use arrow::record_batch::{
    RecordBatch, RecordBatchIterator, RecordBatchOptions, RecordBatchReader,
};
use parquet::arrow::{ArrowWriter, arrow_reader::ParquetRecordBatchReaderBuilder};

pub use excel::{ExcelIoError, ExcelIoPhase, export_excel_sheet_to_csv, list_excel_sheets};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabularIoOperation {
    Read,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabularIoFormat {
    ArrowIpc,
    Csv,
    Parquet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabularIoPhase {
    CreateParent,
    Open,
    Create,
    Decode,
    Encode,
    Sync,
}

#[derive(Debug, thiserror::Error)]
#[error("tabular I/O failed")]
pub struct TabularIoError {
    operation: TabularIoOperation,
    format: TabularIoFormat,
    phase: TabularIoPhase,
    #[source]
    source: TabularIoSource,
}

#[derive(Debug, thiserror::Error)]
enum TabularIoSource {
    #[error("filesystem operation failed")]
    Filesystem(#[from] std::io::Error),
    #[error("Arrow operation failed")]
    Arrow(#[from] ArrowError),
    #[error("Parquet operation failed")]
    Parquet(#[from] parquet::errors::ParquetError),
    #[error("literal materialization failed")]
    Literal(#[from] yss_tabular_arrow::TabularArrowError),
}

impl TabularIoError {
    pub fn operation(&self) -> TabularIoOperation {
        self.operation
    }
    pub fn format(&self) -> TabularIoFormat {
        self.format
    }
    pub fn phase(&self) -> TabularIoPhase {
        self.phase
    }
}

fn failure(
    operation: TabularIoOperation,
    format: TabularIoFormat,
    phase: TabularIoPhase,
    source: impl Into<TabularIoSource>,
) -> TabularIoError {
    TabularIoError {
        operation,
        format,
        phase,
        source: source.into(),
    }
}

pub fn read_ipc_batches(
    path: &Path,
    projection: Option<Vec<usize>>,
) -> Result<FileReader<File>, TabularIoError> {
    let file = open(path, TabularIoFormat::ArrowIpc)?;
    FileReader::try_new(file, projection).map_err(|error| {
        failure(
            TabularIoOperation::Read,
            TabularIoFormat::ArrowIpc,
            TabularIoPhase::Decode,
            error,
        )
    })
}

pub fn read_parquet_batches(
    path: &Path,
    batch_size: usize,
    projection: Option<&[usize]>,
) -> Result<impl RecordBatchReader + use<>, TabularIoError> {
    let format = TabularIoFormat::Parquet;
    let error = |source| {
        failure(
            TabularIoOperation::Read,
            format,
            TabularIoPhase::Decode,
            source,
        )
    };
    let mut builder =
        ParquetRecordBatchReaderBuilder::try_new(open(path, format)?).map_err(error)?;
    let mut schema = builder.schema().clone();
    if let Some(projection) = projection {
        if projection
            .iter()
            .any(|index| *index >= builder.schema().fields().len())
        {
            return Err(failure(
                TabularIoOperation::Read,
                format,
                TabularIoPhase::Decode,
                ArrowError::SchemaError("invalid projection".into()),
            ));
        }
        let mask = parquet::arrow::ProjectionMask::roots(
            builder.parquet_schema(),
            projection.iter().copied(),
        );
        let indices = (0..schema.fields().len())
            .filter(|index| projection.contains(index))
            .collect::<Vec<_>>();
        schema = Arc::new(schema.project(&indices).map_err(|source| {
            failure(
                TabularIoOperation::Read,
                format,
                TabularIoPhase::Decode,
                source,
            )
        })?);
        builder = builder.with_projection(mask);
    }
    let reader = builder
        .with_batch_size(batch_size.max(1))
        .build()
        .map_err(error)?;
    // The synchronous Parquet reader builds a field-only Schema. Keep the file's complete
    // projected schema on both the reader and every zero-copy batch we hand to the host.
    let batch_schema = schema.clone();
    Ok(RecordBatchIterator::new(
        reader.map(move |batch| {
            let batch = batch?;
            RecordBatch::try_new_with_options(
                batch_schema.clone(),
                batch.columns().to_vec(),
                &RecordBatchOptions::new().with_row_count(Some(batch.num_rows())),
            )
        }),
        schema,
    ))
}

pub fn read_csv_batches(
    path: &Path,
    delimiter: u8,
    has_header: bool,
    infer_rows: usize,
    batch_size: usize,
) -> Result<CsvBatchReader, TabularIoError> {
    let format = TabularIoFormat::Csv;
    let error = |source| {
        failure(
            TabularIoOperation::Read,
            format,
            TabularIoPhase::Decode,
            source,
        )
    };
    let mut file = open(path, format)?;
    let csv_format = arrow::csv::reader::Format::default()
        .with_header(has_header)
        .with_delimiter(delimiter);
    let (schema, _) = csv_format
        .infer_schema(csv::SchemaSample::new(&mut file), Some(infer_rows.max(1)))
        .map_err(error)?;
    file.rewind().map_err(|source| {
        failure(
            TabularIoOperation::Read,
            format,
            TabularIoPhase::Decode,
            source,
        )
    })?;
    Ok(CsvBatchReader::new(
        file,
        Arc::new(schema),
        csv_format,
        batch_size.max(1),
    ))
}

pub fn write_ipc_batches(
    path: &Path,
    schema: &Schema,
    batches: impl IntoIterator<Item = Result<RecordBatch, ArrowError>>,
) -> Result<(), TabularIoError> {
    let format = TabularIoFormat::ArrowIpc;
    let error = |source| {
        failure(
            TabularIoOperation::Write,
            format,
            TabularIoPhase::Encode,
            source,
        )
    };
    let mut file = create(path, format)?;
    {
        // Julia Arrow 2.x starts IPC messages immediately after the eight-byte magic header.
        // Arrow Rust's default 64-byte alignment otherwise looks like an empty stream to it.
        let options = IpcWriteOptions::try_new(8, false, MetadataVersion::V5).map_err(error)?;
        let mut writer =
            FileWriter::try_new_with_options(&mut file, schema, options).map_err(error)?;
        for batch in batches {
            let batch = yss_tabular_arrow::normalize_batch_categories(&batch.map_err(error)?)
                .map_err(|source| {
                    failure(
                        TabularIoOperation::Write,
                        format,
                        TabularIoPhase::Encode,
                        source,
                    )
                })?;
            writer.write(&batch).map_err(error)?;
        }
        writer.finish().map_err(error)?;
    }
    sync(&file, format)
}

pub fn write_ipc_snapshot(
    path: &Path,
    snapshot: &yss_tabular_contract::TabularSnapshot,
) -> Result<(), TabularIoError> {
    let batch = yss_tabular_arrow::to_record_batch(snapshot).map_err(|source| {
        failure(
            TabularIoOperation::Write,
            TabularIoFormat::ArrowIpc,
            TabularIoPhase::Encode,
            source,
        )
    })?;
    let schema = batch.schema();
    write_ipc_batches(path, &schema, [Ok(batch)])
}

pub fn write_csv_batches(
    path: &Path,
    batches: impl IntoIterator<Item = Result<RecordBatch, ArrowError>>,
) -> Result<(), TabularIoError> {
    let format = TabularIoFormat::Csv;
    let error = |source| {
        failure(
            TabularIoOperation::Write,
            format,
            TabularIoPhase::Encode,
            source,
        )
    };
    let mut file = create(path, format)?;
    {
        let mut writer = arrow::csv::WriterBuilder::new()
            .with_header(true)
            .build(&mut file);
        for batch in batches {
            writer.write(&batch.map_err(error)?).map_err(error)?;
        }
    }
    sync(&file, format)
}

pub fn write_parquet_batches(
    path: &Path,
    schema: SchemaRef,
    batches: impl IntoIterator<Item = Result<RecordBatch, ArrowError>>,
) -> Result<(), TabularIoError> {
    let format = TabularIoFormat::Parquet;
    let mut writer = ParquetBatchWriter::new(create(path, format)?, schema)?;
    for batch in batches {
        let batch = batch.map_err(|source| {
            failure(
                TabularIoOperation::Write,
                format,
                TabularIoPhase::Encode,
                source,
            )
        })?;
        writer.write(&batch)?;
    }
    writer.finish()
}

/// A caller-owned file reservation can be encoded without reopening or truncating its path.
pub struct ParquetBatchWriter {
    writer: ArrowWriter<File>,
}

impl ParquetBatchWriter {
    pub fn new(file: File, schema: SchemaRef) -> Result<Self, TabularIoError> {
        let mut properties = parquet::file::properties::WriterProperties::builder()
            .set_compression(parquet::basic::Compression::ZSTD(Default::default()))
            .set_key_value_metadata(Some(
                schema
                    .metadata()
                    .iter()
                    .map(|(key, value)| {
                        parquet::file::metadata::KeyValue::new(key.clone(), Some(value.clone()))
                    })
                    .collect(),
            ))
            .set_max_row_group_row_count(Some(50_000));
        // Continuous floating-point columns rarely benefit from a dictionary. Small row
        // groups can keep rebuilding one without ever reaching Parquet's fallback threshold.
        for field in schema.fields() {
            if matches!(field.data_type(), DataType::Float32 | DataType::Float64) {
                let path = parquet::schema::types::ColumnPath::new(vec![field.name().clone()]);
                properties = properties
                    .set_column_dictionary_enabled(path.clone(), false)
                    .set_column_encoding(path, parquet::basic::Encoding::BYTE_STREAM_SPLIT);
            }
        }
        let writer = ArrowWriter::try_new(file, schema, Some(properties.build()))
            .map_err(parquet_write_error)?;
        Ok(Self { writer })
    }
    pub fn write(&mut self, batch: &RecordBatch) -> Result<(), TabularIoError> {
        self.writer.write(batch).map_err(parquet_write_error)?;
        if self.writer.memory_size() >= 16 * 1024 * 1024 {
            self.writer.flush().map_err(parquet_write_error)?;
        }
        Ok(())
    }
    pub fn finish(mut self) -> Result<(), TabularIoError> {
        self.writer.finish().map_err(parquet_write_error)?;
        sync(self.writer.inner(), TabularIoFormat::Parquet)
    }
}

fn parquet_write_error(source: parquet::errors::ParquetError) -> TabularIoError {
    failure(
        TabularIoOperation::Write,
        TabularIoFormat::Parquet,
        TabularIoPhase::Encode,
        source,
    )
}

fn open(path: &Path, format: TabularIoFormat) -> Result<File, TabularIoError> {
    File::open(path).map_err(|source| {
        failure(
            TabularIoOperation::Read,
            format,
            TabularIoPhase::Open,
            source,
        )
    })
}

fn output_parent(path: &Path) -> Option<&Path> {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
}

fn create(path: &Path, format: TabularIoFormat) -> Result<File, TabularIoError> {
    if let Some(parent) = output_parent(path) {
        fs::create_dir_all(parent).map_err(|source| {
            failure(
                TabularIoOperation::Write,
                format,
                TabularIoPhase::CreateParent,
                source,
            )
        })?;
    }
    File::create(path).map_err(|source| {
        failure(
            TabularIoOperation::Write,
            format,
            TabularIoPhase::Create,
            source,
        )
    })
}

fn sync(file: &File, format: TabularIoFormat) -> Result<(), TabularIoError> {
    file.sync_all().map_err(|source| {
        failure(
            TabularIoOperation::Write,
            format,
            TabularIoPhase::Sync,
            source,
        )
    })
}

#[cfg(test)]
mod tests;
