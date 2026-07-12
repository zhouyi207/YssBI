//! Value / conversion node documentation.

use crate::graph::node::NodeDefinition;

pub fn apply_docs(mut def: NodeDefinition, name: &str) -> NodeDefinition {
    if let Some((zh, en)) = documentation(name) {
        def = def.with_documentation(zh, en);
    }
    def
}

pub fn documentation(node_name: &str) -> Option<(&'static str, &'static str)> {
    Some(match node_name {
        "Convert" => (CONVERT_ZH, CONVERT_EN),
        "String to Categorical" => (STR_CAT_ZH, STR_CAT_EN),
        "String to Float64" => (STR_F64_ZH, STR_F64_EN),
        "String to Int64" => (STR_I64_ZH, STR_I64_EN),
        "Int64 to String" => (I64_STR_ZH, I64_STR_EN),
        "Float64 to String" => (F64_STR_ZH, F64_STR_EN),
        "Int64 to Float64" => (I64_F64_ZH, I64_F64_EN),
        "Float64 to Int64" => (F64_I64_ZH, F64_I64_EN),
        "Int64 to Boolean" => (I64_BOOL_ZH, I64_BOOL_EN),
        "Float64 to Boolean" => (F64_BOOL_ZH, F64_BOOL_EN),
        "Categorical to String" => (CAT_STR_ZH, CAT_STR_EN),
        "Int64 to Categorical" => (I64_CAT_ZH, I64_CAT_EN),
        "Categorical to Int64" => (CAT_I64_ZH, CAT_I64_EN),
        "Float64 to Categorical" => (F64_CAT_ZH, F64_CAT_EN),
        "Categorical to Float64" => (CAT_F64_ZH, CAT_F64_EN),
        "Boolean" => (BOOL_CONST_ZH, BOOL_CONST_EN),
        "Int64" => (INT64_CONST_ZH, INT64_CONST_EN),
        "Float64" => (FLOAT64_CONST_ZH, FLOAT64_CONST_EN),
        "String" => (STRING_CONST_ZH, STRING_CONST_EN),
        "Get Variable" => (GET_VAR_ZH, GET_VAR_EN),
        "Set Variable" => (SET_VAR_ZH, SET_VAR_EN),
        "Call Function" => (CALL_ZH, CALL_EN),
        _ => return None,
    })
}

pub const CONVERT_ZH: &str = include_str!("zh/convert.md");
pub const CONVERT_EN: &str = include_str!("en/convert.md");
pub const CALL_ZH: &str = include_str!("zh/call_function.md");
pub const CALL_EN: &str = include_str!("en/call_function.md");
pub const GET_VAR_ZH: &str = include_str!("zh/get_variable.md");
pub const GET_VAR_EN: &str = include_str!("en/get_variable.md");
pub const SET_VAR_ZH: &str = include_str!("zh/set_variable.md");
pub const SET_VAR_EN: &str = include_str!("en/set_variable.md");
pub const BOOL_CONST_ZH: &str = include_str!("zh/boolean_const.md");
pub const BOOL_CONST_EN: &str = include_str!("en/boolean_const.md");
pub const INT64_CONST_ZH: &str = include_str!("zh/int64_const.md");
pub const INT64_CONST_EN: &str = include_str!("en/int64_const.md");
pub const FLOAT64_CONST_ZH: &str = include_str!("zh/float64_const.md");
pub const FLOAT64_CONST_EN: &str = include_str!("en/float64_const.md");
pub const STRING_CONST_ZH: &str = include_str!("zh/string_const.md");
pub const STRING_CONST_EN: &str = include_str!("en/string_const.md");
pub const STR_CAT_ZH: &str = include_str!("zh/string_to_categorical.md");
pub const STR_CAT_EN: &str = include_str!("en/string_to_categorical.md");
pub const STR_F64_ZH: &str = include_str!("zh/string_to_float64.md");
pub const STR_F64_EN: &str = include_str!("en/string_to_float64.md");
pub const STR_I64_ZH: &str = include_str!("zh/string_to_int64.md");
pub const STR_I64_EN: &str = include_str!("en/string_to_int64.md");
pub const I64_STR_ZH: &str = include_str!("zh/int64_to_string.md");
pub const I64_STR_EN: &str = include_str!("en/int64_to_string.md");
pub const F64_STR_ZH: &str = include_str!("zh/float64_to_string.md");
pub const F64_STR_EN: &str = include_str!("en/float64_to_string.md");
pub const I64_F64_ZH: &str = include_str!("zh/int64_to_float64.md");
pub const I64_F64_EN: &str = include_str!("en/int64_to_float64.md");
pub const F64_I64_ZH: &str = include_str!("zh/float64_to_int64.md");
pub const F64_I64_EN: &str = include_str!("en/float64_to_int64.md");
pub const I64_BOOL_ZH: &str = include_str!("zh/int64_to_boolean.md");
pub const I64_BOOL_EN: &str = include_str!("en/int64_to_boolean.md");
pub const F64_BOOL_ZH: &str = include_str!("zh/float64_to_boolean.md");
pub const F64_BOOL_EN: &str = include_str!("en/float64_to_boolean.md");
pub const CAT_STR_ZH: &str = include_str!("zh/categorical_to_string.md");
pub const CAT_STR_EN: &str = include_str!("en/categorical_to_string.md");
pub const I64_CAT_ZH: &str = include_str!("zh/int64_to_categorical.md");
pub const I64_CAT_EN: &str = include_str!("en/int64_to_categorical.md");
pub const CAT_I64_ZH: &str = include_str!("zh/categorical_to_int64.md");
pub const CAT_I64_EN: &str = include_str!("en/categorical_to_int64.md");
pub const F64_CAT_ZH: &str = include_str!("zh/float64_to_categorical.md");
pub const F64_CAT_EN: &str = include_str!("en/float64_to_categorical.md");
pub const CAT_F64_ZH: &str = include_str!("zh/categorical_to_float64.md");
pub const CAT_F64_EN: &str = include_str!("en/categorical_to_float64.md");
