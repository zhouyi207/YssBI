use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use calamine::{Data, Reader, Xlsx, XlsxError, open_workbook};

use super::output_parent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExcelIoPhase {
    CreateParent,
    OpenWorkbook,
    ReadSheet,
    CreateCsv,
    WriteCsv,
}

#[derive(Debug, thiserror::Error)]
#[error("Excel workbook I/O failed during {phase:?}")]
pub struct ExcelIoError {
    phase: ExcelIoPhase,
    #[source]
    source: ExcelIoSource,
}

#[derive(Debug, thiserror::Error)]
enum ExcelIoSource {
    #[error("workbook operation failed")]
    Workbook(#[source] XlsxError),
    #[error("filesystem operation failed")]
    Filesystem(#[source] std::io::Error),
}

impl ExcelIoError {
    pub fn phase(&self) -> ExcelIoPhase {
        self.phase
    }
}

fn workbook_error(phase: ExcelIoPhase, source: XlsxError) -> ExcelIoError {
    ExcelIoError {
        phase,
        source: ExcelIoSource::Workbook(source),
    }
}

fn filesystem_error(phase: ExcelIoPhase, source: std::io::Error) -> ExcelIoError {
    ExcelIoError {
        phase,
        source: ExcelIoSource::Filesystem(source),
    }
}

pub fn list_excel_sheets(path: &Path) -> Result<Vec<String>, ExcelIoError> {
    let workbook: Xlsx<_> =
        open_workbook(path).map_err(|error| workbook_error(ExcelIoPhase::OpenWorkbook, error))?;
    Ok(workbook.sheet_names().to_vec())
}

pub fn export_excel_sheet_to_csv(
    workbook_path: &Path,
    sheet_name: &str,
    csv_path: &Path,
) -> Result<(), ExcelIoError> {
    let mut workbook: Xlsx<_> = open_workbook(workbook_path)
        .map_err(|error| workbook_error(ExcelIoPhase::OpenWorkbook, error))?;
    let range = workbook
        .worksheet_range(sheet_name)
        .map_err(|error| workbook_error(ExcelIoPhase::ReadSheet, error))?;

    if let Some(parent) = output_parent(csv_path) {
        fs::create_dir_all(parent)
            .map_err(|error| filesystem_error(ExcelIoPhase::CreateParent, error))?;
    }
    let mut file =
        File::create(csv_path).map_err(|error| filesystem_error(ExcelIoPhase::CreateCsv, error))?;

    for row in range.rows() {
        let line = row.iter().map(csv_cell).collect::<Vec<_>>().join(",");
        writeln!(file, "{line}")
            .map_err(|error| filesystem_error(ExcelIoPhase::WriteCsv, error))?;
    }
    Ok(())
}

fn csv_cell(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(value) => format!("\"{}\"", value.replace('"', "\"\"")),
        Data::Bool(value) => value.to_string(),
        Data::Int(value) => value.to_string(),
        Data::Float(value) => value.to_string(),
        Data::DateTime(value) => format!("\"{value}\""),
        Data::DateTimeIso(value) | Data::DurationIso(value) => {
            format!("\"{}\"", value.replace('"', "\"\""))
        }
        Data::Error(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use calamine::Data;

    use super::{ExcelIoPhase, csv_cell, list_excel_sheets};

    static NEXT_MISSING_PATH: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn missing_workbook_reports_open_phase() {
        let sequence = NEXT_MISSING_PATH.fetch_add(1, Ordering::Relaxed);
        let path = PathBuf::from(format!(
            "missing-yssbi-workbook-{}-{sequence}.xlsx",
            std::process::id()
        ));

        let error = list_excel_sheets(&path).expect_err("missing workbook must fail");

        assert_eq!(error.phase(), ExcelIoPhase::OpenWorkbook);
    }

    #[test]
    fn text_cells_are_quoted_and_escape_quotes() {
        assert_eq!(
            csv_cell(&Data::String("a, \"quoted\" value".to_owned())),
            "\"a, \"\"quoted\"\" value\""
        );
        assert_eq!(csv_cell(&Data::Int(42)), "42");
        assert_eq!(csv_cell(&Data::Empty), "");
    }
}
