use std::path::Path;

use duckdb::{Connection, OptionalExt};

use crate::{duckdb_table_sql, quote_duckdb_identifier};
use yss_dataset_profile::{
    CategoryCount, ColumnDistribution, ColumnStats, DEFAULT_HISTOGRAM_BIN_COUNT,
    DEFAULT_TOP_CATEGORY_COUNT, DataCompleteness, DatasetOverview, HistogramBin,
    NumericColumnStats, NumericDistribution, ProfileColumnKind, SchemaOverview, SizeShape,
    StringColumnStats, StringDistribution, format_histogram_bin_label,
    profile_column_kind_from_name,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DatasetProfileColumnRef<'a> {
    name: &'a str,
    data_type: &'a str,
}

impl<'a> DatasetProfileColumnRef<'a> {
    pub const fn new(name: &'a str, data_type: &'a str) -> Self {
        Self { name, data_type }
    }
}

fn open_conn(duckdb_path: &Path) -> Result<Connection, String> {
    Connection::open(duckdb_path).map_err(|e| e.to_string())
}

pub fn compute_all_column_stats(
    duckdb_path: &Path,
    table: &str,
    columns: &[DatasetProfileColumnRef<'_>],
) -> Result<Vec<ColumnStats>, String> {
    let conn = open_conn(duckdb_path)?;
    let table_sql = duckdb_table_sql(table);

    columns
        .iter()
        .map(|col| {
            if profile_column_kind_from_name(col.data_type) == ProfileColumnKind::Numeric {
                numeric_column_stats(&conn, &table_sql, col)
            } else {
                string_column_stats(&conn, &table_sql, col)
            }
        })
        .collect()
}

fn numeric_column_stats(
    conn: &Connection,
    table_sql: &str,
    col: &DatasetProfileColumnRef<'_>,
) -> Result<ColumnStats, String> {
    let col_sql = quote_duckdb_identifier(col.name);
    let sql = format!(
        r#"SELECT
            COUNT(*) AS total,
            COUNT({col}) AS non_null,
            MIN(CASE WHEN isfinite(CAST({col} AS DOUBLE)) THEN CAST({col} AS DOUBLE) END) AS min_val,
            MAX(CASE WHEN isfinite(CAST({col} AS DOUBLE)) THEN CAST({col} AS DOUBLE) END) AS max_val,
            AVG(CASE WHEN isfinite(CAST({col} AS DOUBLE)) THEN CAST({col} AS DOUBLE) END) AS mean_val,
            MEDIAN(CASE WHEN isfinite(CAST({col} AS DOUBLE)) THEN CAST({col} AS DOUBLE) END) AS median_val,
            STDDEV_SAMP(CASE WHEN isfinite(CAST({col} AS DOUBLE)) THEN CAST({col} AS DOUBLE) END) AS std_val
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
            column_name: col.name.to_owned(),
            column_type: col.data_type.to_owned(),
            kind: "numeric",
            count: total.max(0) as usize,
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
    col: &DatasetProfileColumnRef<'_>,
) -> Result<ColumnStats, String> {
    let col_sql = quote_duckdb_identifier(col.name);
    let summary_sql = format!(
        r#"SELECT
            COUNT(*) AS total,
            COUNT({col}) AS non_null,
            COUNT(*) FILTER (WHERE CAST({col} AS VARCHAR) = '') AS empty_count,
            COUNT(DISTINCT CAST({col} AS VARCHAR)) AS unique_count
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
    let count = total.max(0) as usize;
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
        ORDER BY cnt DESC, val ASC
        LIMIT 1"#,
        col = col_sql,
        table = table_sql
    );

    let mode_row = conn
        .query_row(&mode_sql, [], |row| {
            Ok((row.get::<_, Option<String>>(0)?, row.get::<_, i64>(1)?))
        })
        .optional()
        .map_err(|error| format!("Failed to compute mode for '{}': {error}", col.name))?;
    let (mode, mode_count) = mode_row
        .map(|(mode, count)| (mode, count.max(0) as usize))
        .unwrap_or((None, 0));

    Ok(ColumnStats::String(StringColumnStats {
        column_name: col.name.to_owned(),
        column_type: col.data_type.to_owned(),
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

pub fn compute_all_column_distributions(
    duckdb_path: &Path,
    table: &str,
    columns: &[DatasetProfileColumnRef<'_>],
) -> Result<Vec<ColumnDistribution>, String> {
    let conn = open_conn(duckdb_path)?;
    let table_sql = duckdb_table_sql(table);

    columns
        .iter()
        .map(|col| {
            if profile_column_kind_from_name(col.data_type) == ProfileColumnKind::Numeric {
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
    col: &DatasetProfileColumnRef<'_>,
) -> Result<ColumnDistribution, String> {
    let col_sql = quote_duckdb_identifier(col.name);
    let bounds_sql = format!(
        r#"SELECT
            MIN(CAST({col} AS DOUBLE)) AS lo,
            MAX(CAST({col} AS DOUBLE)) AS hi
        FROM {table}
        WHERE {col} IS NOT NULL AND isfinite(CAST({col} AS DOUBLE))"#,
        col = col_sql,
        table = table_sql
    );

    let (lo, hi): (Option<f64>, Option<f64>) = conn
        .query_row(&bounds_sql, [], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| format!("Failed to read bounds for '{}': {e}", col.name))?;

    let (Some(lo), Some(hi)) = (lo, hi) else {
        return Ok(ColumnDistribution::Numeric(NumericDistribution {
            column_name: col.name.to_owned(),
            kind: "numeric",
            bins: vec![],
        }));
    };

    if (hi - lo).abs() < f64::EPSILON {
        let count_sql = format!(
            "SELECT COUNT(*) FROM {table} \
             WHERE {col} IS NOT NULL AND isfinite(CAST({col} AS DOUBLE))",
            table = table_sql,
            col = col_sql
        );
        let count: i64 = conn
            .query_row(&count_sql, [], |row| row.get(0))
            .map_err(|error| {
                format!("Failed to count finite values for '{}': {error}", col.name)
            })?;
        return Ok(ColumnDistribution::Numeric(NumericDistribution {
            column_name: col.name.to_owned(),
            kind: "numeric",
            bins: vec![HistogramBin {
                label: format!("{:.2}", lo),
                count: count.max(0) as usize,
            }],
        }));
    }

    let bins = DEFAULT_HISTOGRAM_BIN_COUNT;
    let width = (hi - lo) / bins as f64;
    let hist_sql = format!(
        r#"SELECT
            CAST(LEAST(
                FLOOR((CAST({col} AS DOUBLE) - {lo}) / {width}),
                {max_bin}
            ) AS BIGINT) AS bin_idx,
            COUNT(*) AS cnt
        FROM {table}
        WHERE {col} IS NOT NULL AND isfinite(CAST({col} AS DOUBLE))
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
            let label = format_histogram_bin_label(
                bin_lo,
                bin_hi,
                precision,
                i + 1 == DEFAULT_HISTOGRAM_BIN_COUNT,
            );
            HistogramBin { label, count }
        })
        .collect();

    Ok(ColumnDistribution::Numeric(NumericDistribution {
        column_name: col.name.to_owned(),
        kind: "numeric",
        bins: histogram,
    }))
}

fn string_column_distribution(
    conn: &Connection,
    table_sql: &str,
    col: &DatasetProfileColumnRef<'_>,
) -> Result<ColumnDistribution, String> {
    let col_sql = quote_duckdb_identifier(col.name);
    let top_sql = format!(
        r#"SELECT CAST({col} AS VARCHAR) AS label, COUNT(*) AS cnt
        FROM {table}
        WHERE {col} IS NOT NULL AND CAST({col} AS VARCHAR) != ''
        GROUP BY label
        ORDER BY cnt DESC, label ASC
        LIMIT {top_n}"#,
        col = col_sql,
        table = table_sql,
        top_n = DEFAULT_TOP_CATEGORY_COUNT
    );

    let total_sql = format!(
        r#"SELECT COUNT(*) FROM {table}
        WHERE {col} IS NOT NULL AND CAST({col} AS VARCHAR) != ''"#,
        table = table_sql,
        col = col_sql
    );

    let total: i64 = conn
        .query_row(&total_sql, [], |row| row.get::<_, i64>(0))
        .map_err(|error| {
            format!(
                "Failed to count categorical values for '{}': {error}",
                col.name
            )
        })?
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
        column_name: col.name.to_owned(),
        kind: "string",
        categories,
        other_count: (total - top_sum).max(0) as usize,
    }))
}

pub fn compute_dataset_overview(
    duckdb_path: &Path,
    table: &str,
    columns: &[DatasetProfileColumnRef<'_>],
    row_count: usize,
) -> Result<DatasetOverview, String> {
    let conn = open_conn(duckdb_path)?;
    let table_sql = duckdb_table_sql(table);
    let n_columns = columns.len();

    let mut numeric_cols = 0usize;
    let mut categorical_cols = 0usize;
    let mut string_cols = 0usize;
    let mut datetime_cols = 0usize;
    let mut bool_cols = 0usize;

    for col in columns {
        match profile_column_kind_from_name(col.data_type) {
            ProfileColumnKind::Numeric => numeric_cols += 1,
            ProfileColumnKind::Categorical => categorical_cols += 1,
            ProfileColumnKind::String => string_cols += 1,
            ProfileColumnKind::Temporal => datetime_cols += 1,
            ProfileColumnKind::Boolean => bool_cols += 1,
        }
    }

    let mut null_parts = Vec::new();
    for col in columns {
        null_parts.push(format!(
            "COUNT(*) - COUNT({})",
            quote_duckdb_identifier(col.name)
        ));
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
    let duplicated_rows = None;
    let rows_with_nulls = if total_nulls == 0 {
        0
    } else {
        let predicates: Vec<String> = columns
            .iter()
            .map(|col| format!("{} IS NULL", quote_duckdb_identifier(col.name)))
            .collect();
        let rows_sql = format!(
            "SELECT COUNT(*) FROM {} WHERE {}",
            table_sql,
            predicates.join(" OR ")
        );
        conn.query_row(&rows_sql, [], |row| row.get::<_, i64>(0))
            .map_err(|error| format!("Failed to count rows with nulls: {error}"))?
            .max(0) as usize
    };

    Ok(DatasetOverview {
        size_shape: SizeShape {
            n_rows: row_count,
            n_columns,
            estimated_dataframe_memory_bytes: None,
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    struct TestDatabase(PathBuf);

    static NEXT_TEST_DATABASE: AtomicU64 = AtomicU64::new(0);

    impl TestDatabase {
        fn create() -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default();
            let sequence = NEXT_TEST_DATABASE.fetch_add(1, Ordering::Relaxed);
            Self(std::env::temp_dir().join(format!(
                "yssbi-duckdb-profile-{}-{timestamp}-{sequence}.duckdb",
                std::process::id()
            )))
        }
    }

    impl Drop for TestDatabase {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn physical_profiles_are_finite_stable_and_empty_table_safe() {
        let database = TestDatabase::create();
        let connection = Connection::open(&database.0).expect("test DuckDB should open");
        connection
            .execute_batch(
                r#"
                CREATE TABLE profile_test (value DOUBLE, label VARCHAR);
                INSERT INTO profile_test VALUES
                    (1.0, 'beta'),
                    (2.0, 'alpha'),
                    (CAST('NaN' AS DOUBLE), 'beta'),
                    (CAST('Infinity' AS DOUBLE), 'alpha'),
                    (NULL, ''),
                    (NULL, NULL);
                CREATE TABLE empty_profile (label VARCHAR);
                "#,
            )
            .expect("test profile tables should be created");
        drop(connection);

        let columns = [
            DatasetProfileColumnRef::new("value", "Float64"),
            DatasetProfileColumnRef::new("label", "String"),
        ];
        let stats = compute_all_column_stats(&database.0, "profile_test", &columns)
            .expect("physical stats should succeed");
        let ColumnStats::Numeric(numeric) = &stats[0] else {
            panic!("numeric metadata must select numeric stats");
        };
        assert_eq!(numeric.count, 6);
        assert_eq!(numeric.null_count, 2);
        assert_eq!(numeric.min, Some(1.0));
        assert_eq!(numeric.max, Some(2.0));
        assert_eq!(numeric.mean, Some(1.5));
        let ColumnStats::String(labels) = &stats[1] else {
            panic!("string metadata must select string stats");
        };
        assert_eq!(labels.mode.as_deref(), Some("alpha"));
        assert_eq!(labels.mode_count, 2);

        let distributions = compute_all_column_distributions(&database.0, "profile_test", &columns)
            .expect("physical distributions should succeed");
        let ColumnDistribution::Numeric(numeric) = &distributions[0] else {
            panic!("numeric metadata must select a numeric distribution");
        };
        assert_eq!(numeric.bins.iter().map(|bin| bin.count).sum::<usize>(), 2);
        assert!(
            numeric
                .bins
                .first()
                .expect("finite values should produce bins")
                .label
                .ends_with(')')
        );
        assert!(
            numeric
                .bins
                .last()
                .expect("finite values should produce bins")
                .label
                .ends_with(']')
        );
        let ColumnDistribution::String(labels) = &distributions[1] else {
            panic!("string metadata must select a string distribution");
        };
        assert_eq!(labels.categories[0].label, "alpha");
        assert_eq!(labels.categories[1].label, "beta");

        let empty_columns = [DatasetProfileColumnRef::new("label", "String")];
        let empty_stats = compute_all_column_stats(&database.0, "empty_profile", &empty_columns)
            .expect("empty physical stats should succeed");
        let ColumnStats::String(empty) = &empty_stats[0] else {
            panic!("empty string metadata must select string stats");
        };
        assert_eq!(empty.count, 0);
        assert_eq!(empty.empty_count, 0);
        assert_eq!(empty.mode, None);
    }
}
