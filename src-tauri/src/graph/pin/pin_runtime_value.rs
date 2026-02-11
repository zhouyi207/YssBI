use crate::graph::value::DataValue;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinRuntimeValue {
    /// 执行期产生的值（来自上游或本节点计算）
    current_value: Option<DataValue>,

    /// 用户在编辑器中手动填写的值
    user_value: Option<DataValue>,
    // todo 有必要添加默认值吗
}

impl PinRuntimeValue {
    pub fn current_value(&self) -> Option<&DataValue> {
        self.current_value.as_ref()
    }

    /// 用户填写的值（仅在未连接时由 Context 使用）
    pub fn user_value(&self) -> Option<&DataValue> {
        self.user_value.as_ref()
    }

    /// 设置运行时值（来自连接 / 节点计算）
    pub fn set_current_value(&mut self, value: Option<DataValue>) {
        self.current_value = value;
    }

    /// 设置用户值（编辑器行为）
    pub fn set_user_value(&mut self, value: Option<DataValue>) {
        self.user_value = value;
    }
}
