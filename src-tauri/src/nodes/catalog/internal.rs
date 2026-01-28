use crate::nodes::definition::NodeDefinition;
use crate::nodes::types::PinDefinition;

pub fn get_nodes() -> Vec<NodeDefinition> {
    vec![
        create_event_on_run(),
        create_function_entry(),
        create_function_return(),
        create_macro_inputs(),
        create_macro_outputs(),
    ]
}

fn create_event_on_run() -> NodeDefinition {
    NodeDefinition {
        node_type: "event_on_run".into(),
        category: vec!["Internal".into()],
        title: "On Run".into(),
        ui_style: "event".into(),
        description: Some("Project or Event execution entry point".into()),
        inputs: vec![],
        outputs: vec![PinDefinition {
            name: "Exec".into(),
            pin_type: "exec".into(),
            default_value: None,
        }],
        data_processor: None,
        flow_processor: Some(|_ctx, _node| Ok("Exec".to_string())),
    }
}

fn create_function_entry() -> NodeDefinition {
    NodeDefinition {
        node_type: "function_entry".into(),
        category: vec!["Internal".into()],
        title: "Entry".into(),
        ui_style: "event".into(),
        description: Some("Function execution entry point".into()),
        inputs: vec![],
        outputs: vec![PinDefinition {
            name: "Then".into(),
            pin_type: "exec".into(),
            default_value: None,
        }],
        data_processor: None,
        flow_processor: Some(|_ctx, _node| Ok("Then".to_string())),
    }
}

fn create_function_return() -> NodeDefinition {
    NodeDefinition {
        node_type: "function_return".into(),
        category: vec!["Internal".into()],
        title: "Return".into(),
        ui_style: "event".into(),
        description: Some("Function execution exit point".into()),
        inputs: vec![PinDefinition {
            name: "In".into(),
            pin_type: "exec".into(),
            default_value: None,
        }],
        outputs: vec![],
        data_processor: None,
        flow_processor: Some(|_ctx, _node| Ok("__RETURN__".to_string())),
    }
}

fn create_macro_inputs() -> NodeDefinition {
    NodeDefinition {
        node_type: "macro_inputs".into(),
        category: vec!["Internal".into()],
        title: "Inputs".into(),
        ui_style: "event".into(),
        description: Some("Macro inputs container".into()),
        inputs: vec![],
        outputs: vec![PinDefinition {
            name: "In".into(),
            pin_type: "exec".into(),
            default_value: None,
        }],
        data_processor: None,
        flow_processor: Some(|_ctx, _node| Ok("In".to_string())),
    }
}

fn create_macro_outputs() -> NodeDefinition {
    NodeDefinition {
        node_type: "macro_outputs".into(),
        category: vec!["Internal".into()],
        title: "Outputs".into(),
        ui_style: "event".into(),
        description: Some("Macro outputs container".into()),
        inputs: vec![PinDefinition {
            name: "Out".into(),
            pin_type: "exec".into(),
            default_value: None,
        }],
        outputs: vec![],
        data_processor: None,
        flow_processor: Some(|_ctx, _node| Ok("__RETURN__".to_string())),
    }
}
