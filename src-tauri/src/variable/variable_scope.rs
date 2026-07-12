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
        #[serde(rename = "eventPath")]
        event_path: String,
    },
    /// 函数作用域
    #[serde(rename = "function")]
    Function {
        #[serde(rename = "functionPath")]
        function_path: String,
    },
}

impl Default for VariableScope {
    fn default() -> Self {
        Self::Global
    }
}
