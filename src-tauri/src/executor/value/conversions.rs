//! 值类型转换模块
//!
//! 提供 Value 与 serde_json::Value 和 Polars AnyValue 之间的转换

use super::types::Value;
use polars::prelude::*;
use serde_json;

/// 从 serde_json::Value 转换为 Value
/// 
/// 用于从前端接收数据或从旧代码迁移
pub fn from_json(json: &serde_json::Value) -> Value {
    match json {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Boolean(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int64(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float64(f)
            } else {
                Value::Null
            }
        }
        serde_json::Value::String(s) => Value::String(s.clone()),
        serde_json::Value::Array(arr) => {
            let items = arr.iter().map(from_json).collect();
            Value::List(items)
        }
        serde_json::Value::Object(obj) => {
            let fields = obj
                .iter()
                .map(|(k, v)| (k.clone(), from_json(v)))
                .collect();
            Value::Struct(fields)
        }
    }
}

/// 从 Value 转换为 serde_json::Value
/// 
/// 用于向前端发送数据或与旧代码兼容
pub fn to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Null => serde_json::Value::Null,
        Value::Boolean(b) => serde_json::Value::Bool(*b),
        Value::Int64(i) => serde_json::json!(*i),
        Value::Float64(f) => serde_json::json!(*f),
        Value::String(s) => serde_json::Value::String(s.clone()),
        Value::Date(d) => serde_json::json!(*d),
        Value::Datetime(dt) => serde_json::json!(*dt),
        Value::Duration(dur) => serde_json::json!(*dur),
        Value::List(items) => {
            let json_items: Vec<serde_json::Value> = items.iter().map(to_json).collect();
            serde_json::Value::Array(json_items)
        }
        Value::Struct(fields) => {
            let mut map = serde_json::Map::new();
            for (name, val) in fields {
                map.insert(name.clone(), to_json(val));
            }
            serde_json::Value::Object(map)
        }
        Value::DataFrame(df) => {
            // 将 DataFrame 转换为 JSON 数组
            // 每一行是一个对象
            dataframe_to_json(df)
        }
        Value::Series(s) => {
            // 将 Series 转换为 JSON 数组
            series_to_json(s)
        }
    }
}

/// 从 Polars AnyValue 转换为 Value
pub fn from_polars(any_value: AnyValue) -> Value {
    match any_value {
        AnyValue::Null => Value::Null,
        AnyValue::Boolean(b) => Value::Boolean(b),
        AnyValue::Int8(i) => Value::Int64(i as i64),
        AnyValue::Int16(i) => Value::Int64(i as i64),
        AnyValue::Int32(i) => Value::Int64(i as i64),
        AnyValue::Int64(i) => Value::Int64(i),
        AnyValue::UInt8(u) => Value::Int64(u as i64),
        AnyValue::UInt16(u) => Value::Int64(u as i64),
        AnyValue::UInt32(u) => Value::Int64(u as i64),
        AnyValue::UInt64(u) => Value::Int64(u as i64),
        AnyValue::Float32(f) => Value::Float64(f as f64),
        AnyValue::Float64(f) => Value::Float64(f),
        AnyValue::String(s) => Value::String(s.to_string()),
        AnyValue::Date(d) => Value::Date(d),
        AnyValue::Datetime(dt, _, _) => Value::Datetime(dt),
        AnyValue::Duration(dur, _) => Value::Duration(dur),
        AnyValue::List(series) => {
            // 将 Series 转换为 Value 列表
            let items: Vec<Value> = series
                .iter()
                .map(from_polars)
                .collect();
            Value::List(items)
        }
        // Struct 类型暂时不支持，返回 Null
        AnyValue::Struct(_, _, _) => Value::Null,
        _ => Value::Null,
    }
}

/// 从 Value 转换为 Polars AnyValue
/// 
/// 注意：某些类型（如 DataFrame）无法直接转换为 AnyValue
pub fn to_polars(value: &Value) -> AnyValue<'static> {
    match value {
        Value::Null => AnyValue::Null,
        Value::Boolean(b) => AnyValue::Boolean(*b),
        Value::Int64(i) => AnyValue::Int64(*i),
        Value::Float64(f) => AnyValue::Float64(*f),
        Value::String(s) => AnyValue::StringOwned(s.clone().into()),
        Value::Date(d) => AnyValue::Date(*d),
        Value::Datetime(dt) => AnyValue::Datetime(*dt, TimeUnit::Microseconds, None),
        Value::Duration(dur) => AnyValue::Duration(*dur, TimeUnit::Nanoseconds),
        // List 和 Struct 需要更复杂的转换，暂时返回 Null
        Value::List(_) => AnyValue::Null,
        Value::Struct(_) => AnyValue::Null,
        Value::DataFrame(_) => AnyValue::Null,
        Value::Series(_) => AnyValue::Null,
    }
}

/// 将 DataFrame 转换为 JSON
fn dataframe_to_json(df: &DataFrame) -> serde_json::Value {
    // 简化版本：将 DataFrame 转换为行数组
    let mut rows = Vec::new();
    
    for i in 0..df.height() {
        let mut row = serde_json::Map::new();
        for col in df.get_columns() {
            let col_name = col.name().to_string();
            let value = col.get(i).unwrap_or(AnyValue::Null);
            row.insert(col_name, anyvalue_to_json(&value));
        }
        rows.push(serde_json::Value::Object(row));
    }
    
    serde_json::Value::Array(rows)
}

/// 将 Series 转换为 JSON
fn series_to_json(series: &Series) -> serde_json::Value {
    let items: Vec<serde_json::Value> = series
        .iter()
        .map(|av| anyvalue_to_json(&av))
        .collect();
    serde_json::Value::Array(items)
}

/// 将 Polars AnyValue 转换为 serde_json::Value
fn anyvalue_to_json(av: &AnyValue) -> serde_json::Value {
    match av {
        AnyValue::Null => serde_json::Value::Null,
        AnyValue::Boolean(b) => serde_json::json!(*b),
        AnyValue::Int8(i) => serde_json::json!(*i),
        AnyValue::Int16(i) => serde_json::json!(*i),
        AnyValue::Int32(i) => serde_json::json!(*i),
        AnyValue::Int64(i) => serde_json::json!(*i),
        AnyValue::UInt8(u) => serde_json::json!(*u),
        AnyValue::UInt16(u) => serde_json::json!(*u),
        AnyValue::UInt32(u) => serde_json::json!(*u),
        AnyValue::UInt64(u) => serde_json::json!(*u),
        AnyValue::Float32(f) => serde_json::json!(*f),
        AnyValue::Float64(f) => serde_json::json!(*f),
        AnyValue::String(s) => serde_json::json!(s),
        AnyValue::Date(d) => serde_json::json!(*d),
        AnyValue::Datetime(dt, _, _) => serde_json::json!(*dt),
        AnyValue::Duration(dur, _) => serde_json::json!(*dur),
        _ => serde_json::Value::Null,
    }
}

/// 从 JSON 创建 DataFrame
/// 
/// 期望 JSON 格式为对象数组：[{col1: val1, col2: val2}, ...]
pub fn json_to_dataframe(json: &serde_json::Value) -> Result<DataFrame, String> {
    if let Some(arr) = json.as_array() {
        if arr.is_empty() {
            return Ok(DataFrame::empty());
        }
        
        // 从第一行推断列名和类型
        if let Some(first_row) = arr[0].as_object() {
            let mut columns: Vec<Column> = Vec::new();
            
            for (col_name, _) in first_row {
                // 收集该列的所有值
                let values: Vec<&serde_json::Value> = arr
                    .iter()
                    .filter_map(|row| row.get(col_name))
                    .collect();
                
                // 根据第一个非空值推断类型
                let series = if let Some(first_val) = values.first() {
                    match first_val {
                        serde_json::Value::Bool(_) => {
                            let bools: Vec<Option<bool>> = values
                                .iter()
                                .map(|v| v.as_bool())
                                .collect();
                            Series::new(col_name.into(), bools)
                        }
                        serde_json::Value::Number(n) if n.is_i64() => {
                            let ints: Vec<Option<i64>> = values
                                .iter()
                                .map(|v| v.as_i64())
                                .collect();
                            Series::new(col_name.into(), ints)
                        }
                        serde_json::Value::Number(_) => {
                            let floats: Vec<Option<f64>> = values
                                .iter()
                                .map(|v| v.as_f64())
                                .collect();
                            Series::new(col_name.into(), floats)
                        }
                        serde_json::Value::String(_) => {
                            let strings: Vec<Option<&str>> = values
                                .iter()
                                .map(|v| v.as_str())
                                .collect();
                            Series::new(col_name.into(), strings)
                        }
                        _ => Series::new(col_name.into(), vec![AnyValue::Null; values.len()]),
                    }
                } else {
                    Series::new(col_name.into(), vec![AnyValue::Null; arr.len()])
                };
                
                columns.push(series.into());
            }
            
            DataFrame::new(columns).map_err(|e| format!("Failed to create DataFrame: {}", e))
        } else {
            Err("First element is not an object".to_string())
        }
    } else {
        Err("JSON is not an array".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_json_to_value() {
        let json = json!({
            "name": "Alice",
            "age": 30,
            "active": true
        });
        
        let value = from_json(&json);
        
        if let Value::Struct(fields) = value {
            assert_eq!(fields.len(), 3);
        } else {
            panic!("Expected Struct");
        }
    }

    #[test]
    fn test_value_to_json() {
        let value = Value::Struct(vec![
            ("name".to_string(), Value::String("Bob".to_string())),
            ("age".to_string(), Value::Int64(25)),
        ]);
        
        let json = to_json(&value);
        
        assert_eq!(json["name"], "Bob");
        assert_eq!(json["age"], 25);
    }

    #[test]
    fn test_value_type_conversion() {
        let int_val = Value::Int64(42);
        assert_eq!(int_val.as_f64(), Some(42.0));
        
        let float_val = Value::Float64(3.14);
        assert_eq!(float_val.as_i64(), Some(3));
        
        let bool_val = Value::Boolean(true);
        assert_eq!(bool_val.as_i64(), Some(1));
    }
}
