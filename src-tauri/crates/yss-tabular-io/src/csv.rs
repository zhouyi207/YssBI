use arrow::datatypes::{DataType, Schema, SchemaRef};
use arrow::error::ArrowError;
use arrow::record_batch::{RecordBatch, RecordBatchReader};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};

pub const MAX_CSV_BATCH_BYTES: usize = 16 * 1024 * 1024;
const MAX_SCHEMA_SAMPLE_BYTES: usize = 64 * 1024 * 1024;

pub(crate) struct SchemaSample<'a> {
    file: &'a mut File,
    remaining: usize,
}
impl<'a> SchemaSample<'a> {
    pub fn new(file: &'a mut File) -> Self {
        Self {
            file,
            remaining: MAX_SCHEMA_SAMPLE_BYTES,
        }
    }
}
impl Read for SchemaSample<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            let mut probe = [0u8];
            if self.file.read(&mut probe)? == 0 {
                return Ok(0);
            }
            return Err(std::io::Error::other(
                "CSV schema sample exceeds byte budget",
            ));
        }
        let count = bytes.len().min(self.remaining);
        let count = self.file.read(&mut bytes[..count])?;
        self.remaining -= count;
        Ok(count)
    }
}

pub struct CsvBatchReader {
    schema: SchemaRef,
    reader: BufReader<File>,
    decoder: arrow::csv::reader::Decoder,
    batch_rows: usize,
    consumed: usize,
    finished: bool,
}
impl CsvBatchReader {
    pub(crate) fn new(
        file: File,
        schema: SchemaRef,
        format: arrow::csv::reader::Format,
        batch_rows: usize,
    ) -> Self {
        let schema = std::sync::Arc::new(yss_tabular_arrow::timezone_free_schema(&schema));
        // Decode datetime cells as text first, before a parser can apply their input offsets.
        let decoder_schema = Schema::new_with_metadata(
            schema
                .fields()
                .iter()
                .map(|field| {
                    if matches!(field.data_type(), DataType::Timestamp(..)) {
                        field.as_ref().clone().with_data_type(DataType::Utf8)
                    } else {
                        field.as_ref().clone()
                    }
                })
                .collect::<Vec<_>>(),
            schema.metadata().clone(),
        );
        let decoder = arrow::csv::ReaderBuilder::new(std::sync::Arc::new(decoder_schema))
            .with_format(format)
            .with_batch_size(batch_rows)
            .build_decoder();
        Self {
            schema,
            reader: BufReader::new(file),
            decoder,
            batch_rows,
            consumed: 0,
            finished: false,
        }
    }
    fn flush(&mut self) -> Result<Option<RecordBatch>, ArrowError> {
        let batch = self.decoder.flush()?;
        self.consumed = 0;
        if batch
            .as_ref()
            .is_some_and(|batch| batch.get_array_memory_size() > MAX_CSV_BATCH_BYTES)
        {
            return Err(budget_error());
        }
        batch
            .map(|batch| {
                let arrays = batch
                    .columns()
                    .iter()
                    .zip(self.schema.fields())
                    .map(|(array, field)| {
                        if matches!(field.data_type(), DataType::Timestamp(..)) {
                            let strings = array
                                .as_any()
                                .downcast_ref::<arrow::array::StringArray>()
                                .ok_or_else(|| {
                                    ArrowError::SchemaError("invalid CSV datetime column".into())
                                })?;
                            yss_tabular_arrow::datetime_strings_without_timezone(
                                strings,
                                field.data_type(),
                            )
                            .map_err(|error| ArrowError::ExternalError(Box::new(error)))
                        } else {
                            Ok(array.clone())
                        }
                    })
                    .collect::<Result<Vec<_>, ArrowError>>()?;
                RecordBatch::try_new_with_options(
                    self.schema.clone(),
                    arrays,
                    &arrow::record_batch::RecordBatchOptions::new()
                        .with_row_count(Some(batch.num_rows())),
                )
            })
            .transpose()
    }
    fn read_batch(&mut self) -> Result<Option<RecordBatch>, ArrowError> {
        loop {
            let bytes = self.reader.fill_buf()?;
            if bytes.is_empty() {
                self.decoder.decode(&[])?;
                self.finished = true;
                return self.flush();
            }
            // Physical line boundaries plus the native decoder's completed-record count also
            // handle quoted multiline fields; never flush while a CSV record is incomplete.
            let end = bytes
                .iter()
                .position(|byte| matches!(byte, b'\n' | b'\r'))
                .map_or(bytes.len(), |index| index + 1);
            if self
                .consumed
                .checked_add(end)
                .is_none_or(|bytes| bytes > MAX_CSV_BATCH_BYTES)
            {
                return Err(budget_error());
            }
            let boundary = matches!(bytes[end - 1], b'\n' | b'\r');
            let before = self.decoder.capacity();
            let consumed = self.decoder.decode(&bytes[..end])?;
            self.reader.consume(consumed);
            self.consumed += consumed;
            let rows = self.batch_rows - self.decoder.capacity();
            let bytes = rows
                .checked_mul(self.schema.fields().len())
                .and_then(|cells| cells.checked_mul(16))
                .and_then(|overhead| overhead.checked_add(self.consumed))
                .ok_or_else(budget_error)?;
            if bytes > MAX_CSV_BATCH_BYTES {
                return Err(budget_error());
            }
            let should_flush = consumed == 0
                || self.decoder.capacity() == 0
                || (boundary
                    && before > self.decoder.capacity()
                    && bytes >= MAX_CSV_BATCH_BYTES / 2);
            if should_flush && let Some(batch) = self.flush()? {
                return Ok(Some(batch));
            }
        }
    }
}
fn budget_error() -> ArrowError {
    ArrowError::InvalidArgumentError("CSV record or batch exceeds byte budget".into())
}
impl Iterator for CsvBatchReader {
    type Item = Result<RecordBatch, ArrowError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        match self.read_batch() {
            Ok(batch) => batch.map(Ok),
            Err(error) => {
                self.finished = true;
                Some(Err(error))
            }
        }
    }
}
impl RecordBatchReader for CsvBatchReader {
    fn schema(&self) -> SchemaRef {
        self.schema.clone()
    }
}
