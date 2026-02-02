use std::sync::Arc;
use crate::executor::node::registry::NodeRegistry;
use crate::executor::node::implementation::GenericNode;
use crate::executor::pin::{GenericOutDataPin, GenericInDataPin};
use serde_json::Value;
use crate::executor::value::{PinTypeDesc, TypeVarId, TypeConstraint};

pub fn register(registry: &NodeRegistry) {
    macro_rules! reg_binary {
        ($type:expr, $title:expr, $cat:expr, $p1:expr, $p2:expr, $constraints:expr, $logic:expr) => {
            let node = GenericNode::new_prototype($type, $title);
            
            // 🔑 创建类型变量，A、B、Result 共享同一个类型变量
            let type_var = TypeVarId::new();
            
            node.add_in_data_pin(GenericInDataPin::new(
                uuid::Uuid::nil(),
                $p1,
                PinTypeDesc::type_var_with_constraints(type_var, $constraints.clone())
            ));
            
            node.add_in_data_pin(GenericInDataPin::new(
                uuid::Uuid::nil(),
                $p2,
                PinTypeDesc::type_var(type_var)
            ));
            
            node.add_out_data_pin(GenericOutDataPin::new(
                uuid::Uuid::nil(),
                "Result",
                PinTypeDesc::type_var(type_var)
            ));
            
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
    
    // 🔑 数学运算节点：使用 Numeric 约束
    let numeric_constraints = vec![TypeConstraint::Numeric];

    reg_binary!("add", "Add (+)", math_cat.clone(), "A", "B", numeric_constraints, |a: Value, b: Value| {
        let va = a.as_f64().unwrap_or(0.0);
        let vb = b.as_f64().unwrap_or(0.0);
        Value::from(va + vb)
    });
    
    reg_binary!("subtract", "Subtract (-)", math_cat.clone(), "A", "B", numeric_constraints, |a: Value, b: Value| {
        let va = a.as_f64().unwrap_or(0.0);
        let vb = b.as_f64().unwrap_or(0.0);
        Value::from(va - vb)
    });
    
    reg_binary!("multiply", "Multiply (*)", math_cat.clone(), "A", "B", numeric_constraints, |a: Value, b: Value| {
        let va = a.as_f64().unwrap_or(0.0);
        let vb = b.as_f64().unwrap_or(0.0);
        Value::from(va * vb)
    });
    
    reg_binary!("divide", "Divide (/)", math_cat.clone(), "A", "B", numeric_constraints, |a: Value, b: Value| {
        let va = a.as_f64().unwrap_or(0.0);
        let vb = b.as_f64().unwrap_or(1.0);
        Value::from(va / vb)
    });
    
    // 🔑 比较运算节点：使用 Comparable 约束
    let comparable_constraints = vec![TypeConstraint::Comparable];
    
    reg_binary!("greater", "Greater (>)", math_cat.clone(), "A", "B", comparable_constraints, |a: Value, b: Value| {
        let va = a.as_f64().unwrap_or(0.0);
        let vb = b.as_f64().unwrap_or(0.0);
        Value::from(va > vb)
    });
    
    reg_binary!("less", "Less (<)", math_cat.clone(), "A", "B", comparable_constraints, |a: Value, b: Value| {
        let va = a.as_f64().unwrap_or(0.0);
        let vb = b.as_f64().unwrap_or(0.0);
        Value::from(va < vb)
    });
    
    // 🔑 相等比较：不需要约束（任意类型都可以比较相等）
    reg_binary!("equal", "Equal (==)", math_cat.clone(), "A", "B", vec![], |a: Value, b: Value| {
        Value::from(a == b)
    });

    // 🔑 逻辑运算节点：Boolean 类型
    let _boolean_type_var = TypeVarId::new();
    
    reg_binary!("and", "And (&&)", logic_cat.clone(), "A", "B", vec![], |a: Value, b: Value| {
        Value::from(a.as_bool().unwrap_or(false) && b.as_bool().unwrap_or(false))
    });
    
    reg_binary!("or", "Or (||)", logic_cat.clone(), "A", "B", vec![], |a: Value, b: Value| {
        Value::from(a.as_bool().unwrap_or(false) || b.as_bool().unwrap_or(false))
    });

    // Not 节点
    let not_node = GenericNode::new_prototype("not", "Not (!)");
    
    not_node.add_in_data_pin(GenericInDataPin::new(
        uuid::Uuid::nil(),
        "In",
        PinTypeDesc::concrete(crate::executor::value::ValueType::Boolean)
    ));
    
    not_node.add_out_data_pin(GenericOutDataPin::new(
        uuid::Uuid::nil(),
        "Out",
        PinTypeDesc::concrete(crate::executor::value::ValueType::Boolean)
    ));
    
    not_node.set_data_processor(Box::new(|ctx, node, _pin_id| {
        let val = ctx.get_pin_value(&node.inputs[0].id);
        Value::from(!val.as_bool().unwrap_or(false))
    }));
    
    let mut not_node = not_node;
    not_node.set_metadata(logic_cat, "math".into(), None);
    registry.register("not".into(), Arc::new(not_node));
}
