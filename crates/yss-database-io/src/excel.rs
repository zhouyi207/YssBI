use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use calamine::{Data, ExcelDateTime, ExcelDateTimeType, Reader, Xlsx, XlsxError, open_workbook};
use chrono::Datelike;

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
        Data::DateTime(value) => {
            // Calamine's Display prints the serial, not the calendar. Bound its component
            // conversion to Excel's calendar range; elapsed durations remain serial days.
            if value.is_datetime() && (0.0..2_958_466.0).contains(&value.as_f64()) {
                let (year, month, day, hour, minute, second, milli) = value.to_ymd_hms_milli();
                let date = match (year, hour) {
                    (1900, 24) => {
                        // Nonnegative 1900 dates cannot use the 1904 epoch. Let Calamine
                        // carry through Excel's fictitious leap day without normalizing it.
                        let (year, month, day, ..) = ExcelDateTime::new(
                            value.as_f64().ceil(),
                            ExcelDateTimeType::DateTime,
                            false,
                        )
                        .to_ymd_hms_milli();
                        Some((year, month, day))
                    }
                    (_, 24) => chrono::NaiveDate::from_ymd_opt(
                        i32::from(year),
                        u32::from(month),
                        u32::from(day),
                    )
                    .and_then(|date| date.succ_opt())
                    .map(|date| (date.year() as u16, date.month() as u8, date.day() as u8)),
                    _ => Some((year, month, day)),
                };
                if let Some((year, month, day)) = date
                    && year <= 9999
                {
                    let hour = if hour == 24 { 0 } else { hour };
                    return format!(
                        "\"{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{milli:03}\""
                    );
                }
            }
            format!("\"{value}\"")
        }
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

    use calamine::{Data, ExcelDateTime, ExcelDateTimeType};

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
        for (serial, is_1904, expected) in [
            (45943.541, false, "2025-10-13T12:59:02.400"),
            (44481.541, true, "2025-10-13T12:59:02.400"),
            (0.5, false, "1899-12-31T12:00:00.000"),
            (0.5, true, "1904-01-01T12:00:00.000"),
            (60.0, false, "1900-02-29T00:00:00.000"),
            (45943.999999999, false, "2025-10-14T00:00:00.000"),
            (44481.999999999, true, "2025-10-14T00:00:00.000"),
            (59.999999999, false, "1900-02-29T00:00:00.000"),
            (60.999999999, false, "1900-03-01T00:00:00.000"),
            (366.999999999, false, "1901-01-01T00:00:00.000"),
            (365.999999999, true, "1905-01-01T00:00:00.000"),
        ] {
            assert_eq!(
                csv_cell(&Data::DateTime(ExcelDateTime::new(
                    serial,
                    ExcelDateTimeType::DateTime,
                    is_1904,
                ))),
                format!("\"{expected}\""),
            );
        }
        assert_eq!(
            csv_cell(&Data::DateTime(ExcelDateTime::new(
                1.5,
                ExcelDateTimeType::TimeDelta,
                false,
            ))),
            "\"1.5\"",
        );
        for serial in [-1.0, f64::MAX, 2_958_465.999999999] {
            assert_eq!(
                csv_cell(&Data::DateTime(ExcelDateTime::new(
                    serial,
                    ExcelDateTimeType::DateTime,
                    false,
                ))),
                format!("\"{serial}\""),
            );
        }
        for value in [
            Data::DateTimeIso("2025-10-13T12:00:00".into()),
            Data::DurationIso("PT36H".into()),
        ] {
            assert_eq!(csv_cell(&value), format!("\"{value}\""));
        }
    }

    #[test]
    fn excel_calendar_cells_reach_csv_reader_as_timestamps_with_nulls() {
        use arrow::datatypes::DataType;
        use arrow::record_batch::RecordBatchReader;
        use yss_database_arrow::array_to_json;

        struct CsvFile(PathBuf);
        impl Drop for CsvFile {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let path = CsvFile(std::env::temp_dir().join(format!(
            "yss-excel-calendar-{}-{}.csv",
            std::process::id(),
            NEXT_MISSING_PATH.fetch_add(1, Ordering::Relaxed),
        )));
        let calendar = csv_cell(&Data::DateTime(ExcelDateTime::new(
            45943.541,
            ExcelDateTimeType::DateTime,
            false,
        )));
        let duration = csv_cell(&Data::DateTime(ExcelDateTime::new(
            1.5,
            ExcelDateTimeType::TimeDelta,
            false,
        )));
        let midnight = csv_cell(&Data::DateTime(ExcelDateTime::new(
            45943.999999999,
            ExcelDateTimeType::DateTime,
            false,
        )));
        std::fs::write(
            &path.0,
            format!(
                "at,duration,label\n{calendar},{duration},first\n{midnight},,midnight\n,,blank\n"
            ),
        )
        .unwrap();
        let reader = crate::read_csv_batches(&path.0, b',', true, 10, 10).unwrap();
        assert!(matches!(
            reader.schema().field(0).data_type(),
            DataType::Timestamp(_, None)
        ));
        assert_eq!(reader.schema().field(1).data_type(), &DataType::Float64);
        let batch = reader.collect::<Result<Vec<_>, _>>().unwrap().remove(0);
        assert_eq!(
            array_to_json(batch.column(0).as_ref()).unwrap(),
            vec![
                serde_json::json!("2025-10-13T12:59:02.400"),
                serde_json::json!("2025-10-14T00:00:00"),
                serde_json::Value::Null,
            ]
        );
        assert_eq!(
            array_to_json(batch.column(1).as_ref()).unwrap(),
            vec![
                serde_json::json!(1.5),
                serde_json::Value::Null,
                serde_json::Value::Null,
            ]
        );
    }
}
