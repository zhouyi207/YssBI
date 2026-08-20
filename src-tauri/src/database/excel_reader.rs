//! Excel 读取：列出 Sheet，并将 Sheet 流式转换为 CSV（使用 calamine）。

use calamine::{Data, Reader, Xlsx, open_workbook};
use std::path::Path;

/// 列出 Excel 文件中的 Sheet 名称
pub fn list_sheets(file_path: &str) -> Result<Vec<String>, String> {
    let path = Path::new(file_path);
    let workbook: Xlsx<_> =
        open_workbook(path).map_err(|e| format!("Failed to open Excel: {}", e))?;
    let names = workbook.sheet_names().to_vec();
    Ok(names)
}

/// 将 Excel Sheet 流式写入 CSV，避免先进 Polars 再 ingest。
pub fn export_sheet_to_csv(
    file_path: &str,
    sheet_name: &str,
    csv_path: &Path,
) -> Result<(), String> {
    use std::io::Write;

    let path = Path::new(file_path);
    let mut workbook: Xlsx<_> =
        open_workbook(path).map_err(|e| format!("Failed to open Excel: {}", e))?;

    let range = workbook
        .worksheet_range(sheet_name)
        .map_err(|e| format!("Failed to read sheet '{}': {}", sheet_name, e))?;

    let mut file =
        std::fs::File::create(csv_path).map_err(|e| format!("Failed to create CSV: {e}"))?;

    for row in range.rows() {
        let line = row
            .iter()
            .map(|cell| csv_cell(cell))
            .collect::<Vec<_>>()
            .join(",");
        writeln!(file, "{line}").map_err(|e| format!("Failed to write CSV row: {e}"))?;
    }

    Ok(())
}

fn csv_cell(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => format!("\"{}\"", s.replace('"', "\"\"")),
        Data::Bool(b) => b.to_string(),
        Data::Int(i) => i.to_string(),
        Data::Float(f) => f.to_string(),
        Data::DateTime(dt) => format!("\"{}\"", dt),
        Data::DateTimeIso(s) | Data::DurationIso(s) => {
            format!("\"{}\"", s.replace('"', "\"\""))
        }
        Data::Error(_) => String::new(),
    }
}
