use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VariableScope {
    /// 全局作用域
    Global,
    /// 函数作用域
    Function {
        /// 所属函数 ID
        id: String,
    },
    /// 宏作用域
    Macro {
        /// 所属宏 ID
        id: String,
    },
}

impl Default for VariableScope {
    fn default() -> Self {
        Self::Global
    }
}
