use crate::executor::node::definition::NodeDefinition;
use crate::executor::node::data::PinDefinition;
use serde_json::Value;

pub fn get_nodes() -> Vec<NodeDefinition> {
    vec![
        create_get_dataframe(),
        create_get_column(),
    ]
}

fn create_get_dataframe() -> NodeDefinition {
    NodeDefinition {
        node_type: "get_dataframe".into(),
        category: vec!["Data".into()],
        title: "Get DataFrame".into(),
        ui_style: "default".into(),
        description: Some("Get a loaded DataFrame".into()),
        inputs: vec![],
        outputs: vec![PinDefinition {
            name: "DataFrame".into(),
            pin_type: "dataframe".into(),
            default_value: None,
            is_array: false,
        }],
        data_processor: Some(|_ctx, node, _pin_id| {
            if let Some(df_id) = &node.variable_id {
                Value::String(df_id.clone())
            } else {
                Value::Null
            }
        }),
        flow_processor: None,
    }
}

fn create_get_column() -> NodeDefinition {
    NodeDefinition {
        node_type: "get_column".into(),
        category: vec!["Data".into()],
        title: "Get Column".into(),
        ui_style: "default".into(),
        description: Some("Get a column from a DataFrame".into()),
        inputs: vec![PinDefinition {
            name: "DataFrame".into(),
            pin_type: "dataframe".into(),
            default_value: None,
            is_array: false,
        }],
        outputs: vec![PinDefinition {
            name: "Column".into(),
            pin_type: "object".into(),
            default_value: None,
            is_array: true,
        }],
        data_processor: Some(|_ctx, _node, _pin_id| Value::Null),
        flow_processor: None,
    }
}
