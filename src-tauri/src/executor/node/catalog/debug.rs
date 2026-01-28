use crate::executor::node::definition::NodeDefinition;
use crate::executor::node::data::PinDefinition;
use serde_json::Value;

pub fn get_nodes() -> Vec<NodeDefinition> {
    vec![
        create_print(),
    ]
}

fn create_print() -> NodeDefinition {
    NodeDefinition {
        node_type: "print".into(),
        category: vec!["Debug".into()],
        title: "Print".into(),
        ui_style: "default".into(),
        description: Some("Print a value to the log".into()),
        inputs: vec![
            PinDefinition {
                name: "In".into(),
                pin_type: "exec".into(),
                default_value: None,
                is_array: false,
            },
            PinDefinition {
                name: "Value".into(),
                pin_type: "string".into(),
                default_value: Some(Value::String("".into())),
                is_array: false,
            },
        ],
        outputs: vec![PinDefinition {
            name: "Out".into(),
            pin_type: "exec".into(),
            default_value: None,
            is_array: false,
        }],
        data_processor: None,
        flow_processor: Some(|ctx, node| {
            let data_pin = node
                .inputs
                .iter()
                .find(|p| p.name == "Value")
                .ok_or("Print node missing 'Value' input")?;
            let val = ctx.get_pin_value(&data_pin.id);
            let output = if let Value::String(s) = &val {
                s.clone()
            } else {
                val.to_string()
            };
            ctx.log(format!("[NODE PRINT]: {}", output));
            println!("[NODE PRINT]: {}", output);
            Ok("Out".to_string())
        }),
    }
}
