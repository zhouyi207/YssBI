use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum VariableScope {
    /// 全局作用域
    #[default]
    Global,
    /// Event 作用域
    Event {
        #[serde(rename = "eventPath")]
        event_path: String,
    },
    /// 函数作用域
    Function {
        #[serde(rename = "functionPath")]
        function_path: String,
    },
}
