//! Pin 类型定义模块
//! 
//! 定义所有可用的 Pin 类型及其属性，包括颜色、兼容性规则等。

use serde::{Deserialize, Serialize};

/// Pin 类型定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinTypeDefinition {
    /// 类型标识符 (如 "exec", "float", "int", "string", "bool")
    pub name: String,
    /// 显示名称
    pub display_name: String,
    /// UI 颜色 (十六进制)
    pub color: String,
    /// 是否为执行类型 (exec pin)
    pub is_exec: bool,
    /// 可以隐式转换到的类型列表
    pub implicit_convert_to: Vec<String>,
    /// 可以显式转换到的类型列表
    pub explicit_convert_to: Vec<String>,
    /// 默认值的 JSON 表示
    pub default_value: Option<serde_json::Value>,
}

/// 类型兼容性检查结果
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TypeConversion {
    /// 相同类型，直接兼容
    Same,
    /// 可以隐式转换
    Implicit,
    /// 需要显式转换
    Explicit,
    /// 不兼容
    Incompatible,
}

/// 获取所有 Pin 类型定义
pub fn get_pin_type_definitions() -> Vec<PinTypeDefinition> {
    vec![
        // 执行类型
        PinTypeDefinition {
            name: "exec".into(),
            display_name: "Exec".into(),
            color: "#FFFFFF".into(),
            is_exec: true,
            implicit_convert_to: vec![],
            explicit_convert_to: vec![],
            default_value: None,
        },
        // 布尔类型
        PinTypeDefinition {
            name: "bool".into(),
            display_name: "Boolean".into(),
            color: "#9D0006".into(),
            is_exec: false,
            implicit_convert_to: vec!["string".into()],
            explicit_convert_to: vec!["int".into(), "float".into()],
            default_value: Some(serde_json::Value::Bool(false)),
        },
        // 整数类型
        PinTypeDefinition {
            name: "int".into(),
            display_name: "Integer".into(),
            color: "#1C9898".into(),
            is_exec: false,
            implicit_convert_to: vec!["float".into(), "string".into()],
            explicit_convert_to: vec!["bool".into()],
            default_value: Some(serde_json::Value::Number(0.into())),
        },
        // 浮点数类型
        PinTypeDefinition {
            name: "float".into(),
            display_name: "Float".into(),
            color: "#9ECD4D".into(),
            is_exec: false,
            implicit_convert_to: vec!["string".into()],
            explicit_convert_to: vec!["int".into(), "bool".into()],
            default_value: Some(serde_json::json!(0.0)),
        },
        // 字符串类型
        PinTypeDefinition {
            name: "string".into(),
            display_name: "String".into(),
            color: "#FF00FF".into(),
            is_exec: false,
            implicit_convert_to: vec![],
            explicit_convert_to: vec!["int".into(), "float".into(), "bool".into()],
            default_value: Some(serde_json::Value::String("".into())),
        },
        // 对象类型 (通配符，可以接受任何类型)
        PinTypeDefinition {
            name: "object".into(),
            display_name: "Object".into(),
            color: "#0D7EA6".into(),
            is_exec: false,
            implicit_convert_to: vec![],
            explicit_convert_to: vec![],
            default_value: Some(serde_json::Value::Null),
        },
        // 数组类型
        PinTypeDefinition {
            name: "array".into(),
            display_name: "Array".into(),
            color: "#FF7F00".into(),
            is_exec: false,
            implicit_convert_to: vec!["object".into()],
            explicit_convert_to: vec!["string".into()],
            default_value: Some(serde_json::json!([])),
        },
        // 结构体类型
        PinTypeDefinition {
            name: "struct".into(),
            display_name: "Struct".into(),
            color: "#0055FF".into(),
            is_exec: false,
            implicit_convert_to: vec!["object".into()],
            explicit_convert_to: vec!["string".into()],
            default_value: Some(serde_json::json!({})),
        },
        // 委托/事件类型
        PinTypeDefinition {
            name: "delegate".into(),
            display_name: "Delegate".into(),
            color: "#FF3333".into(),
            is_exec: false,
            implicit_convert_to: vec![],
            explicit_convert_to: vec![],
            default_value: None,
        },
    ]
}

/// 检查两个类型之间的兼容性
pub fn check_type_compatibility(from_type: &str, to_type: &str) -> TypeConversion {
    // 相同类型
    if from_type == to_type {
        return TypeConversion::Same;
    }

    // object 类型可以接受任何非 exec 类型
    if to_type == "object" && from_type != "exec" {
        return TypeConversion::Implicit;
    }

    // 查找源类型定义
    let definitions = get_pin_type_definitions();
    let from_def = definitions.iter().find(|d| d.name == from_type);

    if let Some(def) = from_def {
        // 检查隐式转换
        if def.implicit_convert_to.contains(&to_type.to_string()) {
            return TypeConversion::Implicit;
        }
        // 检查显式转换
        if def.explicit_convert_to.contains(&to_type.to_string()) {
            return TypeConversion::Explicit;
        }
    }

    TypeConversion::Incompatible
}

/// 判断是否可以连接（隐式或相同）
pub fn can_connect(from_type: &str, to_type: &str) -> bool {
    matches!(
        check_type_compatibility(from_type, to_type),
        TypeConversion::Same | TypeConversion::Implicit
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_type() {
        assert_eq!(check_type_compatibility("int", "int"), TypeConversion::Same);
    }

    #[test]
    fn test_implicit_conversion() {
        assert_eq!(check_type_compatibility("int", "float"), TypeConversion::Implicit);
        assert_eq!(check_type_compatibility("int", "string"), TypeConversion::Implicit);
    }

    #[test]
    fn test_object_accepts_any() {
        assert_eq!(check_type_compatibility("int", "object"), TypeConversion::Implicit);
        assert_eq!(check_type_compatibility("string", "object"), TypeConversion::Implicit);
        assert_eq!(check_type_compatibility("exec", "object"), TypeConversion::Incompatible);
    }
}
