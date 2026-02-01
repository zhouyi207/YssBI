//! 类型推断 API 测试

use yssbi_lib::executor::value::{PinTypeDesc, TypeInferenceContext};
use uuid::Uuid;

#[test]
fn test_pin_type_desc_from_string() {
    // 测试 Unknown 类型
    let any_pin = PinTypeDesc::from_string("any");
    assert!(any_pin.data_type.is_unknown());
    
    let object_pin = PinTypeDesc::from_string("object");
    assert!(object_pin.data_type.is_unknown());
    
    // 测试具体类型
    let float_pin = PinTypeDesc::from_string("float64");
    assert!(float_pin.data_type.is_concrete());
    assert_eq!(float_pin.data_type.to_string(), "float64");
    
    let string_pin = PinTypeDesc::from_string("string");
    assert!(string_pin.data_type.is_concrete());
    assert_eq!(string_pin.data_type.to_string(), "string");
    
    // 测试未知类型默认为 Unknown
    let unknown_pin = PinTypeDesc::from_string("some_unknown_type");
    assert!(unknown_pin.data_type.is_unknown());
}

#[test]
fn test_type_inference_with_unknown_types() {
    let mut ctx = TypeInferenceContext::new();
    
    // Print 节点的 Value input (any -> Unknown)
    let print_pin_id = Uuid::new_v4();
    let print_pin = PinTypeDesc::from_string("any");
    ctx.register_pin(print_pin_id, print_pin);
    
    // Constant 节点的 output (float64 -> Concrete)
    let const_pin_id = Uuid::new_v4();
    let const_pin = PinTypeDesc::from_string("float64");
    ctx.register_pin(const_pin_id, const_pin);
    
    // Unknown 类型应该能接受任何具体类型
    let result = ctx.infer_connection(const_pin_id, print_pin_id);
    assert!(result.is_ok(), "Unknown type should accept concrete type");
}

#[test]
fn test_type_inference_concrete_to_concrete() {
    let mut ctx = TypeInferenceContext::new();
    
    // 两个 float64 类型
    let pin1_id = Uuid::new_v4();
    let pin2_id = Uuid::new_v4();
    
    ctx.register_pin(pin1_id, PinTypeDesc::from_string("float64"));
    ctx.register_pin(pin2_id, PinTypeDesc::from_string("float64"));
    
    // 相同具体类型应该兼容
    let result = ctx.infer_connection(pin1_id, pin2_id);
    assert!(result.is_ok(), "Same concrete types should be compatible");
}

#[test]
fn test_type_inference_incompatible_concrete_types() {
    let mut ctx = TypeInferenceContext::new();
    
    // string 和 float64 类型
    let string_pin_id = Uuid::new_v4();
    let float_pin_id = Uuid::new_v4();
    
    ctx.register_pin(string_pin_id, PinTypeDesc::from_string("string"));
    ctx.register_pin(float_pin_id, PinTypeDesc::from_string("float64"));
    
    // 不兼容的具体类型应该失败
    let result = ctx.infer_connection(string_pin_id, float_pin_id);
    assert!(result.is_err(), "Incompatible concrete types should fail");
}

#[test]
fn test_legacy_type_compatibility() {
    use yssbi_lib::schema::can_connect;
    
    // 测试旧的类型检查系统仍然工作
    assert!(can_connect("int", "object"));
    assert!(can_connect("float64", "object"));
    assert!(!can_connect("exec", "object"));
    
    // 测试具体类型
    assert!(can_connect("int", "int"));
    // 注意：根据实际的类型定义，int 可能可以隐式转换为 string
    // 这取决于 schema/pin_types.rs 中的定义
}

#[test]
fn test_pin_type_desc_display() {
    let concrete_pin = PinTypeDesc::concrete(yssbi_lib::executor::value::ValueType::Float64);
    assert!(concrete_pin.type_string().contains("float64"));
    
    let unknown_pin = PinTypeDesc::unknown();
    assert_eq!(unknown_pin.type_string(), "?");
    
    let array_pin = PinTypeDesc::concrete(yssbi_lib::executor::value::ValueType::String).array();
    assert!(array_pin.type_string().contains("[]"));
    
    let optional_pin = PinTypeDesc::concrete(yssbi_lib::executor::value::ValueType::Boolean).optional();
    assert!(optional_pin.type_string().contains("?"));
}

#[test]
fn test_type_conversion_scenarios() {
    // 模拟文档中提到的场景
    
    // 场景1: Print 节点的 Value pin (any) 接受 Constant 的输出 (float64)
    let mut ctx = TypeInferenceContext::new();
    
    let print_value_id = Uuid::new_v4();
    let constant_output_id = Uuid::new_v4();
    
    ctx.register_pin(print_value_id, PinTypeDesc::from_string("any"));
    ctx.register_pin(constant_output_id, PinTypeDesc::from_string("float64"));
    
    let result = ctx.infer_connection(constant_output_id, print_value_id);
    assert!(result.is_ok(), "Print should accept any type from Constant");
    
    // 场景2: 两个具体类型的连接
    let mut ctx2 = TypeInferenceContext::new();
    
    let add_a_id = Uuid::new_v4();
    let add_b_id = Uuid::new_v4();
    
    ctx2.register_pin(add_a_id, PinTypeDesc::from_string("float64"));
    ctx2.register_pin(add_b_id, PinTypeDesc::from_string("float64"));
    
    let result2 = ctx2.infer_connection(add_a_id, add_b_id);
    assert!(result2.is_ok(), "Same types should be compatible");
}