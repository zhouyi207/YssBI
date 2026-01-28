use crate::nodes::definition::NodeDefinition;
use crate::nodes::types::PinDefinition;
use serde_json::Value;

pub fn get_nodes() -> Vec<NodeDefinition> {
    vec![
        create_get_variable(),
        create_set_variable(),
    ]
}

fn create_get_variable() -> NodeDefinition {
    NodeDefinition {
        node_type: "get_variable".into(),
        category: vec!["Variable".into()],
        title: "Get Variable".into(),
        ui_style: "default".into(),
        description: Some("Get variable value".into()),
        inputs: vec![],
        outputs: vec![PinDefinition {
            name: "Value".into(),
            pin_type: "object".into(),
            default_value: None,
        }],
        data_processor: Some(|ctx, node, _pin_id| {
            if let Some(var_id) = &node.variable_id {
                match ctx.get_variable(var_id) {
                    Some(val) => val.clone(),
                    None => {
                        ctx.log(format!(
                            "[Error] Variable ID '{}' not found in context.",
                            var_id
                        ));
                        Value::Null
                    }
                }
            } else {
                ctx.log(format!(
                    "[Error] Get Variable node '{}' has no variable assigned.",
                    node.id
                ));
                Value::Null
            }
        }),
        flow_processor: None,
    }
}

fn create_set_variable() -> NodeDefinition {
    NodeDefinition {
        node_type: "set_variable".into(),
        category: vec!["Variable".into()],
        title: "Set Variable".into(),
        ui_style: "default".into(),
        description: Some("Set variable value".into()),
        inputs: vec![
            PinDefinition {
                name: "In".into(),
                pin_type: "exec".into(),
                default_value: None,
            },
            PinDefinition {
                name: "Value".into(),
                pin_type: "object".into(),
                default_value: None,
            },
        ],
        outputs: vec![
            PinDefinition {
                name: "Out".into(),
                pin_type: "exec".into(),
                default_value: None,
            },
            PinDefinition {
                name: "Value".into(),
                pin_type: "object".into(),
                default_value: None,
            },
        ],
        data_processor: Some(|ctx, node, _pin_id| {
            if let Some(var_id) = &node.variable_id {
                ctx.get_variable(var_id).cloned().unwrap_or(Value::Null)
            } else {
                Value::Null
            }
        }),
        flow_processor: Some(|ctx, node| {
            let var_id = node.variable_id.as_ref().ok_or_else(|| {
                format!(
                    "[Error] Set Variable node '{}' has no variable assigned.",
                    node.id
                )
            })?;

            let data_pin = node
                .inputs
                .iter()
                .find(|p| p.name == "Value")
                .ok_or("Set Variable missing 'Value' input")?;
            let val = ctx.get_pin_value(&data_pin.id);

            if ctx.set_variable(var_id, val) {
                Ok("Out".to_string())
            } else {
                Err(format!(
                    "[Error] Cannot set unknown variable ID '{}'.",
                    var_id
                ))
            }
        }),
    }
}
