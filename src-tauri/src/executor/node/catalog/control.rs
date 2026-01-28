use crate::executor::node::definition::NodeDefinition;
use crate::executor::node::data::PinDefinition;
use serde_json::Value;

pub fn get_nodes() -> Vec<NodeDefinition> {
    vec![
        create_if_else(),
        create_sequence(),
    ]
}

fn create_if_else() -> NodeDefinition {
    NodeDefinition {
        node_type: "if_else".into(),
        category: vec!["Logic".into(), "Flow Control".into()],
        title: "Branch".into(),
        ui_style: "default".into(),
        description: Some("Branch execution based on condition".into()),
        inputs: vec![
            PinDefinition {
                name: "In".into(),
                pin_type: "exec".into(),
                default_value: None,
                is_array: false,
            },
            PinDefinition {
                name: "Cond".into(),
                pin_type: "bool".into(),
                default_value: Some(Value::Bool(false)),
                is_array: false,
            },
        ],
        outputs: vec![
            PinDefinition {
                name: "True".into(),
                pin_type: "exec".into(),
                default_value: None,
                is_array: false,
            },
            PinDefinition {
                name: "False".into(),
                pin_type: "exec".into(),
                default_value: None,
                is_array: false,
            },
        ],
        data_processor: None,
        flow_processor: Some(|ctx, node| {
            let data_pin = node
                .inputs
                .iter()
                .find(|p| p.pin_type != "exec")
                .ok_or("Branch node missing data input")?;
            let val = ctx.get_pin_value(&data_pin.id);
            let condition = val
                .as_bool()
                .unwrap_or_else(|| val.as_f64().unwrap_or(0.0) != 0.0);
            let next = if condition { "True" } else { "False" };
            ctx.log(format!(
                "  Branch condition is {}, moving to '{}'",
                condition, next
            ));
            Ok(next.to_string())
        }),
    }
}

fn create_sequence() -> NodeDefinition {
    NodeDefinition {
        node_type: "sequence".into(),
        category: vec!["Logic".into(), "Flow Control".into()],
        title: "Sequence".into(),
        ui_style: "default".into(),
        description: Some("Execute multiple pins in order".into()),
        inputs: vec![
            PinDefinition {
                name: "In".into(),
                pin_type: "exec".into(),
                default_value: None,
                is_array: false,
            },
        ],
        outputs: vec![
            PinDefinition {
                name: "Then 0".into(),
                pin_type: "exec".into(),
                default_value: None,
                is_array: false,
            },
            PinDefinition {
                name: "Then 1".into(),
                pin_type: "exec".into(),
                default_value: None,
                is_array: false,
            },
        ],
        data_processor: None,
        flow_processor: Some(|ctx, _node| {
            ctx.run_flow(_node.id.as_str(), "Then 0")?;
            Ok("Then 1".to_string())
        }),
    }
}
