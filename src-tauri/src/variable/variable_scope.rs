use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum VariableScope {
    /// 全局作用域
    Global,
    /// Event 作用域
    Event {
        /// 所属 Event ID（camelCase：eventId）
        event_id: String,
    },
    /// 函数作用域
    Function {
        /// 所属函数 ID（camelCase：functionId）
        function_id: String,
    },
    /// 宏作用域
    Macro {
        /// 所属宏 ID（camelCase：macroId）
        macro_id: String,
    },
}

impl Default for VariableScope {
    fn default() -> Self {
        Self::Global
    }
}
