/// DuckDB's built-in row identity pseudo-column.
pub const DUCKDB_ROWID_SQL: &str = "rowid";

/// Quotes a DuckDB identifier according to SQL rules.
pub fn quote_duckdb_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

/// Quotes a DuckDB string literal according to SQL rules.
pub fn quote_duckdb_string_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

pub fn editable_dtype_to_duckdb_sql(dtype: &str) -> Result<&'static str, String> {
    let sql_type = match dtype {
        "Boolean" => "BOOLEAN",
        "Int8" => "TINYINT",
        "Int16" => "SMALLINT",
        "Int32" => "INTEGER",
        "Int64" => "BIGINT",
        "UInt8" => "UTINYINT",
        "UInt16" => "USMALLINT",
        "UInt32" => "UINTEGER",
        "UInt64" => "UBIGINT",
        "Float32" => "REAL",
        "Float64" => "DOUBLE",
        "Date" => "DATE",
        "DateTime" => "TIMESTAMP",
        "Categorical" | "String" => "VARCHAR",
        _ => return Err(format!("Unknown database dtype '{dtype}'")),
    };
    Ok(sql_type)
}
