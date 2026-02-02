use std::sync::Arc;
use crate::executor::node::registry::NodeRegistry;
use crate::executor::node::implementation::GenericNode;
use crate::executor::pin::{GenericOutDataPin, GenericInDataPin};
use crate::executor::value::{ValueType, PinTypeDesc};

pub fn register(registry: &NodeRegistry) {
    // 1. Get DataFrame
    let get_df = GenericNode::new_prototype("get_dataframe", "Get DataFrame");
    get_df.add_out_data_pin(GenericOutDataPin::new(uuid::Uuid::nil(), "DataFrame", PinTypeDesc::concrete(ValueType::DataFrame)));
    
    get_df.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "Name", PinTypeDesc::concrete(ValueType::String)));
    
    get_df.set_data_processor(Box::new(|ctx, node, _output_pin_id| {
        use serde_json::Value as JsonValue;
        
        // 获取 DataFrame 名称（如果提供）
        let df_name = if !node.inputs.is_empty() {
            let name_value = ctx.get_pin_value(&node.inputs[0].id);
            name_value.as_str().unwrap_or("iris").to_string()
        } else {
            // 从节点标题中提取名称
            let title = &node.title;
            if title.starts_with("Get ") {
                title[4..].to_string()
            } else {
                "iris".to_string()
            }
        };
        
        ctx.log(format!("[Get DataFrame] Loading DataFrame: {}", df_name));
        
        // 返回示例 Iris 数据集（简化版）
        // 在实际应用中，这里应该从数据存储中加载
        let iris_data = serde_json::json!([
            {"sepal_length": 5.1, "sepal_width": 3.5, "petal_length": 1.4, "petal_width": 0.2, "species": "setosa"},
            {"sepal_length": 4.9, "sepal_width": 3.0, "petal_length": 1.4, "petal_width": 0.2, "species": "setosa"},
            {"sepal_length": 4.7, "sepal_width": 3.2, "petal_length": 1.3, "petal_width": 0.2, "species": "setosa"},
            {"sepal_length": 4.6, "sepal_width": 3.1, "petal_length": 1.5, "petal_width": 0.2, "species": "setosa"},
            {"sepal_length": 5.0, "sepal_width": 3.6, "petal_length": 1.4, "petal_width": 0.2, "species": "setosa"},
            {"sepal_length": 7.0, "sepal_width": 3.2, "petal_length": 4.7, "petal_width": 1.4, "species": "versicolor"},
            {"sepal_length": 6.4, "sepal_width": 3.2, "petal_length": 4.5, "petal_width": 1.5, "species": "versicolor"},
            {"sepal_length": 6.9, "sepal_width": 3.1, "petal_length": 4.9, "petal_width": 1.5, "species": "versicolor"},
            {"sepal_length": 5.5, "sepal_width": 2.3, "petal_length": 4.0, "petal_width": 1.3, "species": "versicolor"},
            {"sepal_length": 6.5, "sepal_width": 2.8, "petal_length": 4.6, "petal_width": 1.5, "species": "versicolor"},
            {"sepal_length": 6.3, "sepal_width": 3.3, "petal_length": 6.0, "petal_width": 2.5, "species": "virginica"},
            {"sepal_length": 5.8, "sepal_width": 2.7, "petal_length": 5.1, "petal_width": 1.9, "species": "virginica"},
            {"sepal_length": 7.1, "sepal_width": 3.0, "petal_length": 5.9, "petal_width": 2.1, "species": "virginica"},
            {"sepal_length": 6.3, "sepal_width": 2.9, "petal_length": 5.6, "petal_width": 1.8, "species": "virginica"},
            {"sepal_length": 6.5, "sepal_width": 3.0, "petal_length": 5.8, "petal_width": 2.2, "species": "virginica"},
        ]);
        
        let row_count = iris_data.as_array().map(|a| a.len()).unwrap_or(0);
        ctx.log(format!("[Get DataFrame] Loaded {} rows", row_count));
        ctx.log(format!("[Get DataFrame] Returning DataFrame as JSON: {} bytes", serde_json::to_string(&iris_data).unwrap_or_default().len()));
        
        iris_data
    }));
    
    let mut get_df = get_df;
    get_df.set_metadata(vec!["Data".into()], "default".into(), Some("Get a loaded DataFrame (currently returns Iris dataset)".into()));
    registry.register("get_dataframe".into(), Arc::new(get_df));

    // 2. Get Column
    let get_col = GenericNode::new_prototype("get_column", "Get Column");
    get_col.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "DataFrame", PinTypeDesc::concrete(ValueType::DataFrame)));
    get_col.add_out_data_pin(GenericOutDataPin::new(uuid::Uuid::nil(), "Column", PinTypeDesc::concrete(ValueType::Series)));
    
    get_col.set_data_processor(Box::new(|ctx, node, _output_pin_id| {
        use crate::executor::value::conversions::json_to_dataframe;
        use serde_json::Value as JsonValue;
        
        // 获取 DataFrame
        if node.inputs.is_empty() {
            ctx.log("[Get Column] Error: Missing DataFrame input".to_string());
            return JsonValue::Null;
        }
        
        ctx.log(format!("[Get Column] Getting DataFrame from pin: {}", node.inputs[0].id));
        let df_value = ctx.get_pin_value(&node.inputs[0].id);
        ctx.log(format!("[Get Column] DataFrame value type: {}", 
            if df_value.is_array() { "Array" } 
            else if df_value.is_object() { "Object" } 
            else if df_value.is_null() { "Null" } 
            else { "Other" }
        ));
        
        // 从节点标题中提取列名
        // 标题格式: "Get column_name"
        let title = &node.title;
        let column_name = if title.starts_with("Get ") {
            title[4..].to_string()
        } else {
            ctx.log(format!("[Get Column] Error: Cannot extract column name from title '{}'", title));
            return JsonValue::Null;
        };
        
        ctx.log(format!("[Get Column] Extracting column '{}' from DataFrame", column_name));
        
        // 将 JSON Value 转换为 DataFrame
        let dataframe = match json_to_dataframe(&df_value) {
            Ok(df) => {
                ctx.log(format!("[Get Column] Successfully converted to DataFrame: {} rows × {} cols", df.height(), df.width()));
                df
            },
            Err(e) => {
                ctx.log(format!("[Get Column] Error converting to DataFrame: {}", e));
                return JsonValue::Null;
            }
        };
        
        // 提取指定列
        let column = match dataframe.column(&column_name) {
            Ok(c) => {
                ctx.log(format!("[Get Column] Successfully extracted column '{}': {} values", column_name, c.len()));
                c
            },
            Err(e) => {
                ctx.log(format!("[Get Column] Error: Column '{}' not found: {}", column_name, e));
                ctx.log(format!("[Get Column] Available columns: {:?}", dataframe.get_column_names()));
                return JsonValue::Null;
            }
        };
        
        // 将 Column 转换为 JSON 数组
        let series_json: Vec<JsonValue> = (0..column.len())
            .map(|i| {
                let av = column.get(i).unwrap_or(polars::prelude::AnyValue::Null);
                match av {
                    polars::prelude::AnyValue::Null => JsonValue::Null,
                    polars::prelude::AnyValue::Boolean(b) => JsonValue::Bool(b),
                    polars::prelude::AnyValue::Int8(i) => serde_json::json!(i),
                    polars::prelude::AnyValue::Int16(i) => serde_json::json!(i),
                    polars::prelude::AnyValue::Int32(i) => serde_json::json!(i),
                    polars::prelude::AnyValue::Int64(i) => serde_json::json!(i),
                    polars::prelude::AnyValue::UInt8(u) => serde_json::json!(u),
                    polars::prelude::AnyValue::UInt16(u) => serde_json::json!(u),
                    polars::prelude::AnyValue::UInt32(u) => serde_json::json!(u),
                    polars::prelude::AnyValue::UInt64(u) => serde_json::json!(u),
                    polars::prelude::AnyValue::Float32(f) => serde_json::json!(f),
                    polars::prelude::AnyValue::Float64(f) => serde_json::json!(f),
                    polars::prelude::AnyValue::String(s) => JsonValue::String(s.to_string()),
                    _ => JsonValue::Null,
                }
            })
            .collect();
        
        ctx.log(format!("[Get Column] Extracted {} values from column '{}', first 3: {:?}", 
            series_json.len(), 
            column_name,
            series_json.iter().take(3).collect::<Vec<_>>()
        ));
        
        JsonValue::Array(series_json)
    }));
    
    let mut get_col = get_col;
    get_col.set_metadata(vec!["Data".into()], "default".into(), Some("Get a column from a DataFrame as a Series".into()));
    registry.register("get_column".into(), Arc::new(get_col));
}
