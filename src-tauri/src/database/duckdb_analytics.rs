use std::path::Path;

use duckdb::Connection;

use super::{
    CategoryCount, ColumnDistribution, ColumnStats, DataCompleteness, DatasetOverview,
    DuckDbColumnMeta, HistogramBin, NumericColumnStats, NumericDistribution, SchemaOverview,
    SizeShape, StringColumnStats, StringDistribution, duckdb_table_sql,
};

const DEFAULT_BINS: usize = 20;
const DEFAULT_TOP_N: usize = 15;

fn is_numeric_dtype_str(dtype: &str) -> bool {
    matches!(
        dtype,
        "Int8"
            | "Int16"
            | "Int32"
            | "Int64"
            | "UInt8"
            | "UInt16"
            | "UInt32"
            | "UInt64"
            | "Float32"
            | "Float64"
    )
}

fn is_bool_dtype_str(dtype: &str) -> bool {
    dtype == "Boolean"
}

fn is_datetime_dtype_str(dtype: &str) -> bool {
    dtype.starts_with("Datetime") || dtype == "Date" || dtype == "Time"
}

fn quote_column(name: &str) -> String {
    format!(r#""{}""#, super::sql_escape_literal(name))
}

fn open_conn(duckdb_path: &Path) -> Result<Connection, String> {
    Connection::open(duckdb_path).map_err(|e| e.to_string())
}

pub fn compute_all_column_stats_duckdb(
    duckdb_path: &Path,
    table: &str,
    columns: &[DuckDbColumnMeta],
    row_count: usize,
) -> Result<Vec<ColumnStats>, String> {
    let conn = open_conn(duckdb_path)?;
    let table_sql = duckdb_table_sql(table);

    columns
        .iter()
        .map(|col| {
            if is_numeric_dtype_str(&col.dtype) {
                numeric_column_stats(&conn, &table_sql, col, row_count)
            } else {
                string_column_stats(&conn, &table_sql, col, row_count)
            }
        })
        .collect()
}

fn numeric_column_stats(
    conn: &Connection,
    table_sql: &str,
    col: &DuckDbColumnMeta,
    row_count: usize,
) -> Result<ColumnStats, String> {
    let col_sql = quote_column(&col.name);
    let sql = format!(
        r#"SELECT
            COUNT(*) AS total,
            COUNT({col}) AS non_null,
            MIN(CAST({col} AS DOUBLE)) AS min_val,
            MAX(CAST({col} AS DOUBLE)) AS max_val,
            AVG(CAST({col} AS DOUBLE)) AS mean_val,
            MEDIAN(CAST({col} AS DOUBLE)) AS median_val,
            STDDEV_SAMP(CAST({col} AS DOUBLE)) AS std_val
        FROM {table}"#,
        col = col_sql,
        table = table_sql
    );

    conn.query_row(&sql, [], |row| {
        let total: i64 = row.get(0)?;
        let non_null: i64 = row.get(1)?;
        let min_val: Option<f64> = row.get(2)?;
        let max_val: Option<f64> = row.get(3)?;
        let mean_val: Option<f64> = row.get(4)?;
        let median_val: Option<f64> = row.get(5)?;
        let std_val: Option<f64> = row.get(6)?;
        let null_count = (total - non_null).max(0) as usize;
        let variance = std_val.map(|s| s * s);

        Ok(ColumnStats::Numeric(NumericColumnStats {
            column_name: col.name.clone(),
            column_type: col.dtype.clone(),
            kind: "numeric",
            count: row_count.max(total.max(0) as usize),
            null_count,
            min: min_val,
            max: max_val,
            mean: mean_val,
            median: median_val,
            std: std_val,
            variance,
        }))
    })
    .map_err(|e| format!("Failed to compute numeric stats for '{}': {e}", col.name))
}

fn string_column_stats(
    conn: &Connection,
    table_sql: &str,
    col: &DuckDbColumnMeta,
    row_count: usize,
) -> Result<ColumnStats, String> {
    let col_sql = quote_column(&col.name);
    let summary_sql = format!(
        r#"SELECT
            COUNT(*) AS total,
            COUNT({col}) AS non_null,
            SUM(CASE WHEN CAST({col} AS VARCHAR) = '' THEN 1 ELSE 0 END) AS empty_count,
            COUNT(DISTINCT {col}) AS unique_count
        FROM {table}"#,
        col = col_sql,
        table = table_sql
    );

    let (total, non_null, empty_count, unique) = conn
        .query_row(&summary_sql, [], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .map_err(|e| format!("Failed to compute string stats for '{}': {e}", col.name))?;

    let null_count = (total - non_null).max(0) as usize;
    let empty_count = empty_count.max(0) as usize;
    let count = row_count.max(total.max(0) as usize);
    let valid_count = count.saturating_sub(null_count).saturating_sub(empty_count);
    let valid_ratio = if count > 0 {
        valid_count as f64 / count as f64
    } else {
        0.0
    };

    let mode_sql = format!(
        r#"SELECT CAST({col} AS VARCHAR) AS val, COUNT(*) AS cnt
        FROM {table}
        WHERE {col} IS NOT NULL AND CAST({col} AS VARCHAR) != ''
        GROUP BY val
        ORDER BY cnt DESC
        LIMIT 1"#,
        col = col_sql,
        table = table_sql
    );

    let (mode, mode_count) = conn
        .query_row(&mode_sql, [], |row| {
            Ok((row.get::<_, Option<String>>(0)?, row.get::<_, i64>(1)?))
        })
        .map(|(mode, cnt)| (mode, cnt.max(0) as usize))
        .unwrap_or((None, 0));

    Ok(ColumnStats::String(StringColumnStats {
        column_name: col.name.clone(),
        column_type: col.dtype.clone(),
        kind: "string",
        count,
        null_count,
        empty_count,
        valid_ratio,
        unique: unique.max(0) as usize,
        mode,
        mode_count,
    }))
}

pub fn compute_all_column_distributions_duckdb(
    duckdb_path: &Path,
    table: &str,
    columns: &[DuckDbColumnMeta],
) -> Result<Vec<ColumnDistribution>, String> {
    let conn = open_conn(duckdb_path)?;
    let table_sql = duckdb_table_sql(table);

    columns
        .iter()
        .map(|col| {
            if is_numeric_dtype_str(&col.dtype) {
                numeric_column_distribution(&conn, &table_sql, col)
            } else {
                string_column_distribution(&conn, &table_sql, col)
            }
        })
        .collect()
}

fn numeric_column_distribution(
    conn: &Connection,
    table_sql: &str,
    col: &DuckDbColumnMeta,
) -> Result<ColumnDistribution, String> {
    let col_sql = quote_column(&col.name);
    let bounds_sql = format!(
        r#"SELECT
            MIN(CAST({col} AS DOUBLE)) AS lo,
            MAX(CAST({col} AS DOUBLE)) AS hi
        FROM {table}
        WHERE {col} IS NOT NULL"#,
        col = col_sql,
        table = table_sql
    );

    let (lo, hi): (Option<f64>, Option<f64>) = conn
        .query_row(&bounds_sql, [], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| format!("Failed to read bounds for '{}': {e}", col.name))?;

    let (Some(lo), Some(hi)) = (lo, hi) else {
        return Ok(ColumnDistribution::Numeric(NumericDistribution {
            column_name: col.name.clone(),
            kind: "numeric",
            bins: vec![],
        }));
    };

    if (hi - lo).abs() < f64::EPSILON {
        let count_sql = format!(
            "SELECT COUNT(*) FROM {table} WHERE {col} IS NOT NULL",
            table = table_sql,
            col = col_sql
        );
        let count: i64 = conn
            .query_row(&count_sql, [], |row| row.get(0))
            .unwrap_or(0);
        return Ok(ColumnDistribution::Numeric(NumericDistribution {
            column_name: col.name.clone(),
            kind: "numeric",
            bins: vec![HistogramBin {
                label: format!("{:.2}", lo),
                count: count.max(0) as usize,
            }],
        }));
    }

    let bins = DEFAULT_BINS;
    let width = (hi - lo) / bins as f64;
    let hist_sql = format!(
        r#"SELECT
            LEAST(
                FLOOR((CAST({col} AS DOUBLE) - {lo}) / {width}),
                {max_bin}
            ) AS bin_idx,
            COUNT(*) AS cnt
        FROM {table}
        WHERE {col} IS NOT NULL
        GROUP BY bin_idx
        ORDER BY bin_idx"#,
        col = col_sql,
        table = table_sql,
        lo = lo,
        width = width,
        max_bin = bins - 1
    );

    let mut counts = vec![0usize; bins];
    let mut stmt = conn
        .prepare(&hist_sql)
        .map_err(|e| format!("Failed to prepare histogram for '{}': {e}", col.name))?;
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
        .map_err(|e| e.to_string())?;
    for row in rows {
        let (idx, cnt) = row.map_err(|e| e.to_string())?;
        if idx >= 0 && (idx as usize) < bins {
            counts[idx as usize] = cnt.max(0) as usize;
        }
    }

    let precision = if width >= 1.0 { 1usize } else { 2usize };
    let histogram = counts
        .into_iter()
        .enumerate()
        .map(|(i, count)| {
            let bin_lo = lo + i as f64 * width;
            let bin_hi = bin_lo + width;
            let label = if precision == 1 {
                format!("[{:.1}, {:.1}]", bin_lo, bin_hi)
            } else {
                format!("[{:.2}, {:.2}]", bin_lo, bin_hi)
            };
            HistogramBin { label, count }
        })
        .collect();

    Ok(ColumnDistribution::Numeric(NumericDistribution {
        column_name: col.name.clone(),
        kind: "numeric",
        bins: histogram,
    }))
}

fn string_column_distribution(
    conn: &Connection,
    table_sql: &str,
    col: &DuckDbColumnMeta,
) -> Result<ColumnDistribution, String> {
    let col_sql = quote_column(&col.name);
    let top_sql = format!(
        r#"SELECT CAST({col} AS VARCHAR) AS label, COUNT(*) AS cnt
        FROM {table}
        WHERE {col} IS NOT NULL AND CAST({col} AS VARCHAR) != ''
        GROUP BY label
        ORDER BY cnt DESC
        LIMIT {top_n}"#,
        col = col_sql,
        table = table_sql,
        top_n = DEFAULT_TOP_N
    );

    let total_sql = format!(
        r#"SELECT COUNT(*) FROM {table}
        WHERE {col} IS NOT NULL AND CAST({col} AS VARCHAR) != ''"#,
        table = table_sql,
        col = col_sql
    );

    let total: i64 = conn
        .query_row(&total_sql, [], |row| row.get(0))
        .unwrap_or(0)
        .max(0);

    let mut stmt = conn
        .prepare(&top_sql)
        .map_err(|e| format!("Failed to prepare distribution for '{}': {e}", col.name))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|e| e.to_string())?;

    let mut categories = Vec::new();
    let mut top_sum = 0i64;
    for row in rows {
        let (label, value) = row.map_err(|e| e.to_string())?;
        top_sum += value;
        categories.push(CategoryCount {
            label,
            value: value.max(0) as usize,
        });
    }

    Ok(ColumnDistribution::String(StringDistribution {
        column_name: col.name.clone(),
        kind: "string",
        categories,
        other_count: (total - top_sum).max(0) as usize,
    }))
}

pub fn compute_dataset_overview_duckdb(
    duckdb_path: &Path,
    table: &str,
    columns: &[DuckDbColumnMeta],
    row_count: usize,
) -> Result<DatasetOverview, String> {
    let conn = open_conn(duckdb_path)?;
    let table_sql = duckdb_table_sql(table);
    let n_columns = columns.len();

    let memory_size = std::fs::metadata(duckdb_path)
        .map(|m| m.len() as usize)
        .unwrap_or(0);

    let mut numeric_cols = 0usize;
    let mut categorical_cols = 0usize;
    let mut string_cols = 0usize;
    let mut datetime_cols = 0usize;
    let mut bool_cols = 0usize;

    for col in columns {
        if is_numeric_dtype_str(&col.dtype) {
            numeric_cols += 1;
        } else if is_bool_dtype_str(&col.dtype) {
            bool_cols += 1;
        } else if is_datetime_dtype_str(&col.dtype) {
            datetime_cols += 1;
        } else if col.dtype.starts_with("Categorical") || col.dtype.starts_with("Enum") {
            categorical_cols += 1;
        } else {
            string_cols += 1;
        }
    }

    let mut null_parts = Vec::new();
    for col in columns {
        null_parts.push(format!("COUNT(*) - COUNT({})", quote_column(&col.name)));
    }
    let null_sql = format!("SELECT {} FROM {}", null_parts.join(", "), table_sql);

    let mut total_nulls = 0usize;
    let mut cols_with_nulls = 0usize;
    if !null_parts.is_empty() {
        conn.query_row(&null_sql, [], |row| {
            for i in 0..columns.len() {
                let null_count: i64 = row.get(i)?;
                let null_count = null_count.max(0) as usize;
                total_nulls += null_count;
                if null_count > 0 {
                    cols_with_nulls += 1;
                }
            }
            Ok(())
        })
        .map_err(|e| format!("Failed to compute null overview: {e}"))?;
    }

    let total_cells = row_count.saturating_mul(n_columns);
    let null_ratio = if total_cells > 0 {
        total_nulls as f64 / total_cells as f64
    } else {
        0.0
    };

    // Full-row duplicate detection is intentionally skipped for DuckDB-backed tables.
    let duplicated_rows = 0usize;
    let rows_with_nulls = if total_nulls == 0 {
        0
    } else {
        let predicates: Vec<String> = columns
            .iter()
            .map(|col| format!("{} IS NULL", quote_column(&col.name)))
            .collect();
        let rows_sql = format!(
            "SELECT COUNT(*) FROM {} WHERE {}",
            table_sql,
            predicates.join(" OR ")
        );
        conn.query_row(&rows_sql, [], |row| row.get::<_, i64>(0))
            .unwrap_or(0)
            .max(0) as usize
    };

    Ok(DatasetOverview {
        size_shape: SizeShape {
            n_rows: row_count,
            n_columns,
            memory_size,
            duplicated_rows,
        },
        schema_overview: SchemaOverview {
            numeric_cols,
            categorical_cols,
            string_cols,
            datetime_cols,
            bool_cols,
        },
        data_completeness: DataCompleteness {
            total_nulls,
            null_ratio,
            cols_with_nulls,
            rows_with_nulls,
        },
    })
}
