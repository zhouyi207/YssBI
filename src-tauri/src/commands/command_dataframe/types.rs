use chrono::{DateTime, Datelike, NaiveDate};
use polars::prelude::{AnyValue, DataFrame, TimeUnit};

pub(super) fn polars_value_to_json(v: AnyValue<'_>) -> serde_json::Value {
    match v {
        AnyValue::Null => serde_json::Value::Null,
        AnyValue::Boolean(b) => serde_json::Value::Bool(b),
        AnyValue::String(s) => serde_json::Value::String(s.to_string()),
        AnyValue::Int64(i) => serde_json::Number::from_f64(i as f64)
            .map(serde_json::Value::Number)
            .unwrap_or_else(|| serde_json::Value::String(i.to_string())),
        AnyValue::UInt64(u) => serde_json::Number::from_f64(u as f64)
            .map(serde_json::Value::Number)
            .unwrap_or_else(|| serde_json::Value::String(u.to_string())),
        AnyValue::Float64(f) => serde_json::Number::from_f64(f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        AnyValue::Date(days) => {
            let epoch = NaiveDate::from_ymd_opt(1970, 1, 1)
                .unwrap()
                .num_days_from_ce();
            NaiveDate::from_num_days_from_ce_opt(epoch + days as i32)
                .map(|d| serde_json::Value::String(d.format("%Y-%m-%d").to_string()))
                .unwrap_or_else(|| serde_json::Value::String(days.to_string()))
        }
        AnyValue::Datetime(ts, unit, _) | AnyValue::DatetimeOwned(ts, unit, _) => {
            let (secs, nsecs) = match unit {
                TimeUnit::Nanoseconds => ((ts / 1_000_000_000) as i64, (ts % 1_000_000_000) as u32),
                TimeUnit::Microseconds => {
                    ((ts / 1_000_000) as i64, ((ts % 1_000_000) * 1000) as u32)
                }
                TimeUnit::Milliseconds => ((ts / 1000) as i64, ((ts % 1000) * 1_000_000) as u32),
            };
            DateTime::from_timestamp(secs, nsecs)
                .map(|dt| {
                    let s = dt.format("%Y-%m-%d %H:%M:%S%.3f").to_string();
                    let s = s.trim_end_matches('0').trim_end_matches('.');
                    serde_json::Value::String(s.to_string())
                })
                .unwrap_or_else(|| serde_json::Value::String(ts.to_string()))
        }
        AnyValue::Time(ns) => {
            let secs = (ns / 1_000_000_000) as u32;
            let h = secs / 3600;
            let m = (secs % 3600) / 60;
            let s = secs % 60;
            serde_json::Value::String(format!("{:02}:{:02}:{:02}", h, m, s))
        }
        AnyValue::Duration(ns, _) => serde_json::Value::String(ns.to_string()),
        AnyValue::Categorical(_, _)
        | AnyValue::CategoricalOwned(_, _)
        | AnyValue::Enum(_, _)
        | AnyValue::EnumOwned(_, _) => serde_json::Value::String(format!("{}", v)),
        _ => serde_json::Value::String(format!("{}", v)),
    }
}

pub(super) fn dataframe_to_row_matrix(df: &DataFrame) -> Vec<Vec<serde_json::Value>> {
    (0..df.height())
        .map(|row_idx| {
            df.columns()
                .iter()
                .map(|s| match s.get(row_idx) {
                    Ok(v) => polars_value_to_json(v),
                    Err(_) => serde_json::Value::Null,
                })
                .collect()
        })
        .collect()
}
