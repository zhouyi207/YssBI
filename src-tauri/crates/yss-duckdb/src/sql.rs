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

pub fn duckdb_table_sql(table: &str) -> String {
    quote_duckdb_identifier(table)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_and_literals_escape_their_own_delimiters() {
        assert_eq!(quote_duckdb_identifier("a\"b"), "\"a\"\"b\"");
        assert_eq!(quote_duckdb_string_literal("a'b"), "'a''b'");
        assert_eq!(duckdb_table_sql("data\"set"), "\"data\"\"set\"");
    }

    #[test]
    fn editable_types_are_allowlisted() {
        assert_eq!(editable_dtype_to_duckdb_sql("UInt64"), Ok("UBIGINT"));
        assert_eq!(editable_dtype_to_duckdb_sql("DateTime"), Ok("TIMESTAMP"));
        assert!(editable_dtype_to_duckdb_sql("STRUCT").is_err());
    }
}
