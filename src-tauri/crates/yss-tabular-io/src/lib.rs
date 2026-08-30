//! Polars-backed typed tabular filesystem I/O.

mod excel;

use std::fs::{self, File};
use std::path::Path;

use polars::prelude::{
    CsvWriter, DataFrame, IpcReader, IpcWriter, ParquetWriter, PolarsError, SerReader, SerWriter,
};

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
    Filesystem(#[source] std::io::Error),
    #[error("dataframe operation failed")]
    Dataframe(#[source] PolarsError),
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

fn io_error(
    operation: TabularIoOperation,
    format: TabularIoFormat,
    phase: TabularIoPhase,
    source: std::io::Error,
) -> TabularIoError {
    TabularIoError {
        operation,
        format,
        phase,
        source: TabularIoSource::Filesystem(source),
    }
}

fn dataframe_error(
    operation: TabularIoOperation,
    format: TabularIoFormat,
    phase: TabularIoPhase,
    source: PolarsError,
) -> TabularIoError {
    TabularIoError {
        operation,
        format,
        phase,
        source: TabularIoSource::Dataframe(source),
    }
}

pub fn read_ipc_dataframe(path: &Path) -> Result<DataFrame, TabularIoError> {
    let file = File::open(path).map_err(|error| {
        io_error(
            TabularIoOperation::Read,
            TabularIoFormat::ArrowIpc,
            TabularIoPhase::Open,
            error,
        )
    })?;
    IpcReader::new(file).finish().map_err(|error| {
        dataframe_error(
            TabularIoOperation::Read,
            TabularIoFormat::ArrowIpc,
            TabularIoPhase::Decode,
            error,
        )
    })
}

pub fn write_ipc_dataframe(path: &Path, dataframe: &mut DataFrame) -> Result<(), TabularIoError> {
    create_parent_directory(path, TabularIoFormat::ArrowIpc)?;
    let mut file = File::create(path).map_err(|error| {
        io_error(
            TabularIoOperation::Write,
            TabularIoFormat::ArrowIpc,
            TabularIoPhase::Create,
            error,
        )
    })?;
    IpcWriter::new(&mut file)
        .finish(dataframe)
        .map_err(|error| {
            dataframe_error(
                TabularIoOperation::Write,
                TabularIoFormat::ArrowIpc,
                TabularIoPhase::Encode,
                error,
            )
        })
}

pub fn write_csv_dataframe(path: &Path, dataframe: &mut DataFrame) -> Result<(), TabularIoError> {
    create_parent_directory(path, TabularIoFormat::Csv)?;
    let file = File::create(path).map_err(|error| {
        io_error(
            TabularIoOperation::Write,
            TabularIoFormat::Csv,
            TabularIoPhase::Create,
            error,
        )
    })?;
    CsvWriter::new(file).finish(dataframe).map_err(|error| {
        dataframe_error(
            TabularIoOperation::Write,
            TabularIoFormat::Csv,
            TabularIoPhase::Encode,
            error,
        )
    })
}

pub fn write_parquet_dataframe(
    path: &Path,
    dataframe: &mut DataFrame,
) -> Result<(), TabularIoError> {
    create_parent_directory(path, TabularIoFormat::Parquet)?;
    let file = File::create(path).map_err(|error| {
        io_error(
            TabularIoOperation::Write,
            TabularIoFormat::Parquet,
            TabularIoPhase::Create,
            error,
        )
    })?;
    ParquetWriter::new(file)
        .finish(dataframe)
        .map(|_| ())
        .map_err(|error| {
            dataframe_error(
                TabularIoOperation::Write,
                TabularIoFormat::Parquet,
                TabularIoPhase::Encode,
                error,
            )
        })
}

fn output_parent(path: &Path) -> Option<&Path> {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
}

fn create_parent_directory(path: &Path, format: TabularIoFormat) -> Result<(), TabularIoError> {
    if let Some(parent) = output_parent(path) {
        fs::create_dir_all(parent).map_err(|error| {
            io_error(
                TabularIoOperation::Write,
                format,
                TabularIoPhase::CreateParent,
                error,
            )
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use polars::prelude::{Column, DataFrame};

    use super::{output_parent, read_ipc_dataframe, write_csv_dataframe, write_ipc_dataframe};

    struct TestDirectory(PathBuf);

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    impl TestDirectory {
        fn create() -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default();
            let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            Self(std::env::temp_dir().join(format!(
                "yssbi-tabular-io-{}-{timestamp}-{sequence}",
                std::process::id()
            )))
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn dataframe_round_trips_through_ipc_and_exports_csv() {
        let directory = TestDirectory::create();
        let ipc_path = directory.path().join("nested/data.arrow");
        let csv_path = directory.path().join("nested/data.csv");
        let mut dataframe = DataFrame::new(
            2,
            vec![
                Column::new("name".into(), ["a", "b"]),
                Column::new("value".into(), [1_i64, 2]),
            ],
        )
        .expect("dataframe");

        write_ipc_dataframe(&ipc_path, &mut dataframe).expect("write IPC");
        let mut restored = read_ipc_dataframe(&ipc_path).expect("read IPC");
        write_csv_dataframe(&csv_path, &mut restored).expect("write CSV");
        let csv = std::fs::read_to_string(csv_path).expect("read CSV");

        assert_eq!(restored.height(), 2);
        assert_eq!(csv, "name,value\na,1\nb,2\n");
    }

    #[test]
    fn current_directory_output_has_no_parent_to_create() {
        assert_eq!(output_parent(Path::new("data.csv")), None);
        assert_eq!(
            output_parent(Path::new("nested/data.csv")),
            Some(Path::new("nested"))
        );
    }
}
