use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum VariableScope {
    /// 全局作用域
    #[serde(rename = "global")]
    Global,
    /// Event 作用域
    #[serde(rename = "event")]
    Event {
        /// 所属 Event ID（前端传 eventId，此处显式 rename 确保兼容）
        #[serde(rename = "eventId")]
        event_id: String,
    },
    /// 函数作用域
    #[serde(rename = "function")]
    Function {
        #[serde(rename = "functionId")]
        function_id: String,
    },
}

impl Default for VariableScope {
    fn default() -> Self {
        Self::Global
    }
}
