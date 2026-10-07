//! Read presentation only; the original graph editor normalizes and validates writes.
use yss_data_contract::{DataValue, SemanticType, ValueType};
use yss_graph_document::GraphConstant;

pub(super) fn type_options() -> Vec<(String, String)> {
    let base = SemanticType::ALL
        .into_iter()
        .map(ValueType::Scalar)
        .chain([ValueType::Object, ValueType::DataFrame])
        .collect::<Vec<_>>();
    base.iter()
        .cloned()
        .chain(
            base.iter()
                .cloned()
                .map(|value| ValueType::Array(Box::new(value))),
        )
        .chain(
            SemanticType::ALL
                .into_iter()
                .map(|value| ValueType::DataSeries(Box::new(ValueType::Scalar(value)))),
        )
        .map(|value| {
            let name = value.to_string();
            (name.clone(), name)
        })
        .collect()
}

pub(super) fn scalar_text(value: &DataValue) -> Option<String> {
    match value {
        DataValue::Null => Some(String::new()),
        DataValue::Bool(value) => Some(value.to_string()),
        DataValue::Integer(value) => Some(value.to_string()),
        DataValue::Unsigned(value) => Some(value.to_string()),
        DataValue::Decimal(value) => Some(value.as_str().to_owned()),
        DataValue::String(value) if value.len() <= 4096 => Some(value.to_string()),
        _ => None,
    }
}

pub(super) fn initial_value(data_type: &ValueType) -> &'static str {
    match data_type {
        ValueType::Scalar(SemanticType::Numeric) => "0",
        ValueType::Scalar(SemanticType::Binary) => "false",
        ValueType::Scalar(_) => "",
        ValueType::Array(_) => "{\"List\":[]}",
        ValueType::DataSeries(_) => "{\"value\":[]}",
        ValueType::DataFrame => "{}",
        _ => "{\"Object\":{}}",
    }
}

pub(super) fn structured_text(constant: &GraphConstant) -> anyhow::Result<String> {
    if let Some(snapshot) = &constant.tabular {
        let columns = snapshot
            .columns()
            .iter()
            .map(|column| {
                Ok(format!(
                    "{}: {}",
                    serde_json::to_string(column.name().as_str())?,
                    serde_json::to_string(column.values())?
                ))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        return Ok(format!("{{\n{}\n}}", columns.join(",\n")));
    }
    if let DataValue::String(value) = &constant.data_value {
        return Ok(value.to_string());
    }
    if matches!(
        constant.data_type,
        ValueType::DataFrame | ValueType::DataSeries(_)
    ) {
        return Ok(initial_value(&constant.data_type).into());
    }
    Ok(serde_json::to_string_pretty(&constant.data_value)?)
}
