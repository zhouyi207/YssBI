//! Excel 读取：列出 Sheet、读取 Sheet 数据转 Polars DataFrame（使用 calamine）

use calamine::{open_workbook, Data, DataType, Reader, Xlsx};
use polars::prelude::*;
use std::path::Path;

/// 列出 Excel 文件中的 Sheet 名称
pub fn list_sheets(file_path: &str) -> Result<Vec<String>, String> {
    let path = Path::new(file_path);
    let workbook: Xlsx<_> = open_workbook(path).map_err(|e| format!("Failed to open Excel: {}", e))?;
    let names = workbook.sheet_names().to_vec();
    Ok(names)
}

/// calamine Data 转 Polars AnyValue
fn calamine_data_to_anyvalue(d: &Data) -> AnyValue<'static> {
    use calamine::Data;
    match d {
        Data::Empty => AnyValue::Null,
        Data::Int(i) => AnyValue::Int64(*i),
        Data::Float(f) => AnyValue::Float64(*f),
        Data::String(s) => AnyValue::StringOwned(s.clone().into()),
        Data::Bool(b) => AnyValue::Boolean(*b),
        Data::DateTime(dt) => AnyValue::StringOwned(dt.to_string().into()),
        Data::DateTimeIso(s) | Data::DurationIso(s) => AnyValue::StringOwned(s.clone().into()),
        Data::Error(_) => AnyValue::Null,
    }
}

/// 从 Excel 指定 Sheet 读取数据并构建 Polars DataFrame
pub fn read_sheet_to_dataframe(file_path: &str, sheet_name: &str) -> Result<DataFrame, String> {
    let path = Path::new(file_path);
    let mut workbook: Xlsx<_> = open_workbook(path).map_err(|e| format!("Failed to open Excel: {}", e))?;

    let range = workbook
        .worksheet_range(sheet_name)
        .map_err(|e| format!("Failed to read sheet '{}': {}", sheet_name, e))?;

    let (height, width) = range.get_size();
    if height == 0 || width == 0 {
        let columns: Vec<polars::prelude::Column> = Vec::new();
        return DataFrame::new(columns).map_err(|e| format!("Failed to build DataFrame: {}", e));
    }

    let mut rows = range.rows();
    let header_row = rows.next().ok_or("Empty sheet")?;
    let column_names: Vec<String> = header_row
        .iter()
        .enumerate()
        .map(|(i, cell)| {
            cell.as_string()
                .unwrap_or_else(|| format!("Column_{}", i + 1))
        })
        .collect();

    let column_count = column_names.len();
    let mut columns_data: Vec<Vec<AnyValue<'static>>> = (0..column_count).map(|_| Vec::new()).collect();

    for row in rows {
        for (i, col_data) in columns_data.iter_mut().enumerate() {
            let cell = row.get(i).unwrap_or(&Data::Empty);
            col_data.push(calamine_data_to_anyvalue(cell));
        }
    }

    let series: Vec<Series> = column_names
        .iter()
        .zip(columns_data.iter())
        .map(|(name, data): (&String, &Vec<AnyValue<'static>>)| {
            let name_ss: PlSmallStr = name.as_str().into();
            Series::from_any_values(name_ss.clone(), data, false)
                .unwrap_or_else(|_| Series::new_null(name_ss, data.len()))
        })
        .collect();

    let columns: Vec<polars::prelude::Column> = series.into_iter().map(polars::prelude::Column::from).collect();
    DataFrame::new(columns).map_err(|e| format!("Failed to build DataFrame: {}", e))
}
