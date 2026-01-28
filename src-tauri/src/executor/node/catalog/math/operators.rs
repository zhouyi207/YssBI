use crate::executor::node::definition::NodeDefinition;
use crate::executor::node::data::PinDefinition;
use serde_json::Value;

pub fn get_nodes() -> Vec<NodeDefinition> {
    vec![
        create_add(),
        create_subtract(),
        create_multiply(),
        create_divide(),
        create_equal(),
        create_greater(),
        create_less(),
        create_and(),
        create_or(),
        create_not(),
    ]
}

fn create_and() -> NodeDefinition {
    NodeDefinition {
        node_type: "and".into(),
        category: vec!["Logic".into(), "Operators".into()],
        title: "And (&&)".into(),
        ui_style: "math".into(),
        description: Some("Boolean AND".into()),
        inputs: vec![
            PinDefinition { name: "A".into(), pin_type: "bool".into(), default_value: Some(Value::Bool(true)), is_array: false },
            PinDefinition { name: "B".into(), pin_type: "bool".into(), default_value: Some(Value::Bool(true)), is_array: false },
        ],
        outputs: vec![PinDefinition { name: "Result".into(), pin_type: "bool".into(), default_value: None, is_array: false }],
        data_processor: Some(|ctx, node, _pin_id| {
            let a = ctx.get_pin_value(&node.inputs[0].id).as_bool().unwrap_or(false);
            let b = ctx.get_pin_value(&node.inputs[1].id).as_bool().unwrap_or(false);
            Value::Bool(a && b)
        }),
        flow_processor: None,
    }
}

fn create_or() -> NodeDefinition {
    NodeDefinition {
        node_type: "or".into(),
        category: vec!["Logic".into(), "Operators".into()],
        title: "Or (||)".into(),
        ui_style: "math".into(),
        description: Some("Boolean OR".into()),
        inputs: vec![
            PinDefinition { name: "A".into(), pin_type: "bool".into(), default_value: Some(Value::Bool(false)), is_array: false },
            PinDefinition { name: "B".into(), pin_type: "bool".into(), default_value: Some(Value::Bool(false)), is_array: false },
        ],
        outputs: vec![PinDefinition { name: "Result".into(), pin_type: "bool".into(), default_value: None, is_array: false }],
        data_processor: Some(|ctx, node, _pin_id| {
            let a = ctx.get_pin_value(&node.inputs[0].id).as_bool().unwrap_or(false);
            let b = ctx.get_pin_value(&node.inputs[1].id).as_bool().unwrap_or(false);
            Value::Bool(a || b)
        }),
        flow_processor: None,
    }
}

fn create_not() -> NodeDefinition {
    NodeDefinition {
        node_type: "not".into(),
        category: vec!["Logic".into(), "Operators".into()],
        title: "Not (!)".into(),
        ui_style: "math".into(),
        description: Some("Boolean NOT".into()),
        inputs: vec![
            PinDefinition { name: "In".into(), pin_type: "bool".into(), default_value: Some(Value::Bool(false)), is_array: false },
        ],
        outputs: vec![PinDefinition { name: "Out".into(), pin_type: "bool".into(), default_value: None, is_array: false }],
        data_processor: Some(|ctx, node, _pin_id| {
            let val = ctx.get_pin_value(&node.inputs[0].id).as_bool().unwrap_or(false);
            Value::Bool(!val)
        }),
        flow_processor: None,
    }
}

fn create_add() -> NodeDefinition {
    NodeDefinition {
        node_type: "add".into(),
        category: vec!["Math".into(), "Operators".into()],
        title: "Add (+)".into(),
        ui_style: "math".into(),
        description: Some("Add two numbers".into()),
        inputs: vec![
            PinDefinition { name: "A".into(), pin_type: "float".into(), default_value: Some(Value::from(0.0)), is_array: false },
            PinDefinition { name: "B".into(), pin_type: "float".into(), default_value: Some(Value::from(0.0)), is_array: false },
        ],
        outputs: vec![PinDefinition { name: "Sum".into(), pin_type: "float".into(), default_value: None, is_array: false }],
        data_processor: Some(|ctx, node, _pin_id| {
            let a = ctx.get_pin_value(&node.inputs[0].id).as_f64().unwrap_or(0.0);
            let b = ctx.get_pin_value(&node.inputs[1].id).as_f64().unwrap_or(0.0);
            Value::from(a + b)
        }),
        flow_processor: None,
    }
}

fn create_subtract() -> NodeDefinition {
    NodeDefinition {
        node_type: "subtract".into(),
        category: vec!["Math".into(), "Operators".into()],
        title: "Subtract (-)".into(),
        ui_style: "math".into(),
        description: Some("Subtract two numbers".into()),
        inputs: vec![
            PinDefinition { name: "A".into(), pin_type: "float".into(), default_value: Some(Value::from(0.0)), is_array: false },
            PinDefinition { name: "B".into(), pin_type: "float".into(), default_value: Some(Value::from(0.0)), is_array: false },
        ],
        outputs: vec![PinDefinition { name: "Result".into(), pin_type: "float".into(), default_value: None, is_array: false }],
        data_processor: Some(|ctx, node, _pin_id| {
            let a = ctx.get_pin_value(&node.inputs[0].id).as_f64().unwrap_or(0.0);
            let b = ctx.get_pin_value(&node.inputs[1].id).as_f64().unwrap_or(0.0);
            Value::from(a - b)
        }),
        flow_processor: None,
    }
}

fn create_multiply() -> NodeDefinition {
    NodeDefinition {
        node_type: "multiply".into(),
        category: vec!["Math".into(), "Operators".into()],
        title: "Multiply (*)".into(),
        ui_style: "math".into(),
        description: Some("Multiply two numbers".into()),
        inputs: vec![
            PinDefinition { name: "A".into(), pin_type: "float".into(), default_value: Some(Value::from(0.0)), is_array: false },
            PinDefinition { name: "B".into(), pin_type: "float".into(), default_value: Some(Value::from(0.0)), is_array: false },
        ],
        outputs: vec![PinDefinition { name: "Result".into(), pin_type: "float".into(), default_value: None, is_array: false }],
        data_processor: Some(|ctx, node, _pin_id| {
            let a = ctx.get_pin_value(&node.inputs[0].id).as_f64().unwrap_or(0.0);
            let b = ctx.get_pin_value(&node.inputs[1].id).as_f64().unwrap_or(0.0);
            Value::from(a * b)
        }),
        flow_processor: None,
    }
}

fn create_divide() -> NodeDefinition {
    NodeDefinition {
        node_type: "divide".into(),
        category: vec!["Math".into(), "Operators".into()],
        title: "Divide (/)".into(),
        ui_style: "math".into(),
        description: Some("Divide two numbers".into()),
        inputs: vec![
            PinDefinition { name: "A".into(), pin_type: "float".into(), default_value: Some(Value::from(0.0)), is_array: false },
            PinDefinition { name: "B".into(), pin_type: "float".into(), default_value: Some(Value::from(1.0)), is_array: false },
        ],
        outputs: vec![PinDefinition { name: "Result".into(), pin_type: "float".into(), default_value: None, is_array: false }],
        data_processor: Some(|ctx, node, _pin_id| {
            let a = ctx.get_pin_value(&node.inputs[0].id).as_f64().unwrap_or(0.0);
            let b = ctx.get_pin_value(&node.inputs[1].id).as_f64().unwrap_or(1.0);
            if b == 0.0 { Value::Null } else { Value::from(a / b) }
        }),
        flow_processor: None,
    }
}

fn create_equal() -> NodeDefinition {
    NodeDefinition {
        node_type: "equal".into(),
        category: vec!["Math".into(), "Operators".into()],
        title: "Equal (==)".into(),
        ui_style: "math".into(),
        description: Some("Check if two values are equal".into()),
        inputs: vec![
            PinDefinition { name: "A".into(), pin_type: "any".into(), default_value: None, is_array: false },
            PinDefinition { name: "B".into(), pin_type: "any".into(), default_value: None, is_array: false },
        ],
        outputs: vec![PinDefinition { name: "Result".into(), pin_type: "bool".into(), default_value: None, is_array: false }],
        data_processor: Some(|ctx, node, _pin_id| {
            let a = ctx.get_pin_value(&node.inputs[0].id);
            let b = ctx.get_pin_value(&node.inputs[1].id);
            Value::Bool(a == b)
        }),
        flow_processor: None,
    }
}

fn create_greater() -> NodeDefinition {
    NodeDefinition {
        node_type: "greater".into(),
        category: vec!["Math".into(), "Operators".into()],
        title: "Greater (>)".into(),
        ui_style: "math".into(),
        description: Some("Check if A is greater than B".into()),
        inputs: vec![
            PinDefinition { name: "A".into(), pin_type: "float".into(), default_value: Some(Value::from(0.0)), is_array: false },
            PinDefinition { name: "B".into(), pin_type: "float".into(), default_value: Some(Value::from(0.0)), is_array: false },
        ],
        outputs: vec![PinDefinition { name: "Result".into(), pin_type: "bool".into(), default_value: None, is_array: false }],
        data_processor: Some(|ctx, node, _pin_id| {
            let a = ctx.get_pin_value(&node.inputs[0].id).as_f64().unwrap_or(0.0);
            let b = ctx.get_pin_value(&node.inputs[1].id).as_f64().unwrap_or(0.0);
            Value::Bool(a > b)
        }),
        flow_processor: None,
    }
}

fn create_less() -> NodeDefinition {
    NodeDefinition {
        node_type: "less".into(),
        category: vec!["Math".into(), "Operators".into()],
        title: "Less (<)".into(),
        ui_style: "math".into(),
        description: Some("Check if A is less than B".into()),
        inputs: vec![
            PinDefinition { name: "A".into(), pin_type: "float".into(), default_value: Some(Value::from(0.0)), is_array: false },
            PinDefinition { name: "B".into(), pin_type: "float".into(), default_value: Some(Value::from(0.0)), is_array: false },
        ],
        outputs: vec![PinDefinition { name: "Result".into(), pin_type: "bool".into(), default_value: None, is_array: false }],
        data_processor: Some(|ctx, node, _pin_id| {
            let a = ctx.get_pin_value(&node.inputs[0].id).as_f64().unwrap_or(0.0);
            let b = ctx.get_pin_value(&node.inputs[1].id).as_f64().unwrap_or(0.0);
            Value::Bool(a < b)
        }),
        flow_processor: None,
    }
}
