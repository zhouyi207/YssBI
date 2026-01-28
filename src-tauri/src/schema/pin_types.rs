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
    /// 是否为执行类型 (exec pin)
    pub is_exec: bool,
    /// 是否支持数组模式 (UI 渲染用)
    #[serde(default)]
    pub supports_array: bool,
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
            is_exec: true,
            supports_array: false,
            implicit_convert_to: vec![],
            explicit_convert_to: vec![],
            default_value: None,
        },
        // 布尔类型
        PinTypeDefinition {
            name: "bool".into(),
            display_name: "Boolean".into(),
            is_exec: false,
            supports_array: true,
            implicit_convert_to: vec!["string".into()],
            explicit_convert_to: vec!["int32".into(), "int64".into(), "float64".into()],
            default_value: Some(serde_json::Value::Bool(false)),
        },
        // 整数类型 (Polars 各个级别)
        PinTypeDefinition {
            name: "int8".into(),
            display_name: "Int8".into(),
            is_exec: false,
            supports_array: true,
            implicit_convert_to: vec!["int16".into(), "int32".into(), "int64".into(), "float64".into(), "string".into()],
            explicit_convert_to: vec!["bool".into()],
            default_value: Some(serde_json::json!(0)),
        },
        PinTypeDefinition {
            name: "int16".into(),
            display_name: "Int16".into(),
            is_exec: false,
            supports_array: true,
            implicit_convert_to: vec!["int32".into(), "int64".into(), "float64".into(), "string".into()],
            explicit_convert_to: vec!["bool".into(), "int8".into()],
            default_value: Some(serde_json::json!(0)),
        },
        PinTypeDefinition {
            name: "int32".into(),
            display_name: "Int32".into(),
            is_exec: false,
            supports_array: true,
            implicit_convert_to: vec!["int64".into(), "float64".into(), "string".into()],
            explicit_convert_to: vec!["bool".into(), "int16".into()],
            default_value: Some(serde_json::json!(0)),
        },
        PinTypeDefinition {
            name: "int64".into(),
            display_name: "Int64".into(),
            is_exec: false,
            supports_array: true,
            implicit_convert_to: vec!["float64".into(), "string".into()],
            explicit_convert_to: vec!["bool".into(), "int32".into()],
            default_value: Some(serde_json::json!(0)),
        },
        PinTypeDefinition {
            name: "uint32".into(),
            display_name: "UInt32".into(),
            is_exec: false,
            supports_array: true,
            implicit_convert_to: vec!["int64".into(), "uint64".into(), "float64".into(), "string".into()],
            explicit_convert_to: vec!["bool".into()],
            default_value: Some(serde_json::json!(0)),
        },
        PinTypeDefinition {
            name: "uint64".into(),
            display_name: "UInt64".into(),
            is_exec: false,
            supports_array: true,
            implicit_convert_to: vec!["float64".into(), "string".into()],
            explicit_convert_to: vec!["bool".into(), "uint32".into()],
            default_value: Some(serde_json::json!(0)),
        },
        // 浮点数类型
        PinTypeDefinition {
            name: "float32".into(),
            display_name: "Float32".into(),
            is_exec: false,
            supports_array: true,
            implicit_convert_to: vec!["float64".into(), "string".into()],
            explicit_convert_to: vec!["int32".into(), "bool".into()],
            default_value: Some(serde_json::json!(0.0)),
        },
        PinTypeDefinition {
            name: "float64".into(),
            display_name: "Float64".into(),
            is_exec: false,
            supports_array: true,
            implicit_convert_to: vec!["string".into()],
            explicit_convert_to: vec!["int64".into(), "bool".into()],
            default_value: Some(serde_json::json!(0.0)),
        },
        // 字符串类型
        PinTypeDefinition {
            name: "string".into(),
            display_name: "String".into(),
            is_exec: false,
            supports_array: true,
            implicit_convert_to: vec![],
            explicit_convert_to: vec!["int32".into(), "float64".into(), "bool".into()],
            default_value: Some(serde_json::Value::String("".into())),
        },
        // 时间日期类型
        PinTypeDefinition {
            name: "date".into(),
            display_name: "Date".into(),
            is_exec: false,
            supports_array: true,
            implicit_convert_to: vec!["string".into()],
            explicit_convert_to: vec![],
            default_value: None,
        },
        PinTypeDefinition {
            name: "datetime".into(),
            display_name: "DateTime".into(),
            is_exec: false,
            supports_array: true,
            implicit_convert_to: vec!["string".into()],
            explicit_convert_to: vec![],
            default_value: None,
        },
        // 数据对象
        PinTypeDefinition {
            name: "dataframe".into(),
            display_name: "DataFrame".into(),
            is_exec: false,
            supports_array: false,
            implicit_convert_to: vec!["object".into()],
            explicit_convert_to: vec![],
            default_value: None,
        },
        // 对象类型 (通配符)
        PinTypeDefinition {
            name: "object".into(),
            display_name: "Object".into(),
            is_exec: false,
            supports_array: true,
            implicit_convert_to: vec![],
            explicit_convert_to: vec![],
            default_value: Some(serde_json::Value::Null),
        },
        // 为了向后兼容，保留一些通用名称
        PinTypeDefinition {
            name: "int".into(),
            display_name: "Integer".into(),
            is_exec: false,
            supports_array: true,
            implicit_convert_to: vec!["int64".into(), "float64".into(), "string".into()],
            explicit_convert_to: vec!["bool".into()],
            default_value: Some(serde_json::json!(0)),
        },
        PinTypeDefinition {
            name: "float".into(),
            display_name: "Float".into(),
            is_exec: false,
            supports_array: true,
            implicit_convert_to: vec!["float64".into(), "string".into()],
            explicit_convert_to: vec!["bool".into()],
            default_value: Some(serde_json::json!(0.0)),
        },
        // 数组类型 (旧版兼容)
        PinTypeDefinition {
            name: "array".into(),
            display_name: "Array".into(),
            is_exec: false,
            supports_array: false,
            implicit_convert_to: vec!["object".into()],
            explicit_convert_to: vec!["string".into()],
            default_value: Some(serde_json::json!([])),
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
        assert_eq!(
            check_type_compatibility("int", "float"),
            TypeConversion::Implicit
        );
        assert_eq!(
            check_type_compatibility("int", "string"),
            TypeConversion::Implicit
        );
    }

    #[test]
    fn test_object_accepts_any() {
        assert_eq!(
            check_type_compatibility("int", "object"),
            TypeConversion::Implicit
        );
        assert_eq!(
            check_type_compatibility("string", "object"),
            TypeConversion::Implicit
        );
        assert_eq!(
            check_type_compatibility("exec", "object"),
            TypeConversion::Incompatible
        );
    }
}
