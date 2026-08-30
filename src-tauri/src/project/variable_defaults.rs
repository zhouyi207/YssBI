use yss_data_contract::{DataType, DataValue};

pub fn default_value_for(data_type: &DataType) -> DataValue {
    match data_type {
        DataType::Boolean => DataValue::Boolean(false),
        DataType::Int64 => DataValue::Int64(0),
        DataType::Float64 => DataValue::Float64(0.0),
        DataType::String
        | DataType::Date
        | DataType::Datetime
        | DataType::Time
        | DataType::Categorical => DataValue::String(String::new()),
        DataType::Array(_) => DataValue::Array(vec![
            DataValue::Int64(1),
            DataValue::Int64(2),
            DataValue::Int64(3),
        ]),
        DataType::Object => {
            let mut values = std::collections::HashMap::new();
            values.insert("key_0".to_owned(), DataValue::Int64(1));
            values.insert("key_1".to_owned(), DataValue::Int64(2));
            DataValue::Object(values)
        }
        DataType::OneOf(types) => types.first().map_or(DataValue::Null, default_value_for),
        DataType::Any | DataType::DataFrame | DataType::DataSeries(_) | DataType::Struct(_) => {
            DataValue::Null
        }
    }
}
