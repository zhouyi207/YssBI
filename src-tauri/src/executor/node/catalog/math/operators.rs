use std::sync::Arc;
use crate::executor::node::registry::NodeRegistry;
use crate::executor::node::implementation::GenericNode;
use crate::executor::pin::{GenericOutDataPin, GenericInDataPin};
use serde_json::Value;

pub fn register(registry: &NodeRegistry) {
    macro_rules! reg_binary {
        ($type:expr, $title:expr, $cat:expr, $p1:expr, $p2:expr, $pt:expr, $logic:expr) => {
            let node = GenericNode::new_prototype($type, $title);
            node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), $p1, $pt));
            node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), $p2, $pt));
            node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Result", $pt));
            
            node.set_data_processor(Box::new(|ctx, node, _pin_id| {
                let a = ctx.get_pin_value(&node.inputs[0].id);
                let b = ctx.get_pin_value(&node.inputs[1].id);
                $logic(a, b)
            }));

            let mut node = node;
            node.set_metadata($cat, "math".into(), None);
            registry.register($type.into(), Arc::new(node));
        };
    }

    let math_cat = vec!["Math".into(), "Operators".into()];
    let logic_cat = vec!["Logic".into(), "Operators".into()];

    reg_binary!("add", "Add (+)", math_cat.clone(), "A", "B", "float", |a: Value, b: Value| {
        let va = a.as_f64().unwrap_or(0.0);
        let vb = b.as_f64().unwrap_or(0.0);
        Value::from(va + vb)
    });
    reg_binary!("subtract", "Subtract (-)", math_cat.clone(), "A", "B", "float", |a: Value, b: Value| {
        let va = a.as_f64().unwrap_or(0.0);
        let vb = b.as_f64().unwrap_or(0.0);
        Value::from(va - vb)
    });
    reg_binary!("multiply", "Multiply (*)", math_cat.clone(), "A", "B", "float", |a: Value, b: Value| {
        let va = a.as_f64().unwrap_or(0.0);
        let vb = b.as_f64().unwrap_or(0.0);
        Value::from(va * vb)
    });
    reg_binary!("divide", "Divide (/)", math_cat.clone(), "A", "B", "float", |a: Value, b: Value| {
        let va = a.as_f64().unwrap_or(0.0);
        let vb = b.as_f64().unwrap_or(1.0);
        Value::from(va / vb)
    });
    
    reg_binary!("greater", "Greater (>)", math_cat.clone(), "A", "B", "float", |a: Value, b: Value| {
        let va = a.as_f64().unwrap_or(0.0);
        let vb = b.as_f64().unwrap_or(0.0);
        Value::from(va > vb)
    });
    reg_binary!("less", "Less (<)", math_cat.clone(), "A", "B", "float", |a: Value, b: Value| {
        let va = a.as_f64().unwrap_or(0.0);
        let vb = b.as_f64().unwrap_or(0.0);
        Value::from(va < vb)
    });
    reg_binary!("equal", "Equal (==)", math_cat.clone(), "A", "B", "any", |a: Value, b: Value| {
        Value::from(a == b)
    });

    reg_binary!("and", "And (&&)", logic_cat.clone(), "A", "B", "bool", |a: Value, b: Value| {
        Value::from(a.as_bool().unwrap_or(false) && b.as_bool().unwrap_or(false))
    });
    reg_binary!("or", "Or (||)", logic_cat.clone(), "A", "B", "bool", |a: Value, b: Value| {
        Value::from(a.as_bool().unwrap_or(false) || b.as_bool().unwrap_or(false))
    });

    // Not
    let not_node = GenericNode::new_prototype("not", "Not (!)");
    not_node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "In", "bool"));
    not_node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Out", "bool"));
    not_node.set_data_processor(Box::new(|ctx, node, _pin_id| {
        let val = ctx.get_pin_value(&node.inputs[0].id);
        Value::from(!val.as_bool().unwrap_or(false))
    }));
    let mut not_node = not_node;
    not_node.set_metadata(logic_cat, "math".into(), None);
    registry.register("not".into(), Arc::new(not_node));
}
