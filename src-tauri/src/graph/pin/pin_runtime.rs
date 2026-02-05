//! pin 运行时的数据

use super::{PinDefinition, PinKind};
use crate::graph::pin::PinDataType;
use crate::graph::value::DataValue;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPinRuntime {
    /// 执行期产生的值（来自上游或本节点计算）
    pub current: Option<DataValue>,

    /// 用户在编辑器中手动填写的值
    pub user_override: Option<DataValue>,
}

impl DataPinRuntime {
    pub fn new() -> Self {
        Self {
            current: None,
            user_override: None,
        }
    }

    /// 是否“本 pin 自己有值来源”
    pub fn has_local_value(&self) -> bool {
        self.user_override.is_some() || self.current.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PinRuntime {
    Data(DataPinRuntime),
    Exec,
}

impl PinRuntime {
    pub fn new(kind: PinKind) -> Self {
        match kind {
            PinKind::Data => Self::Data(DataPinRuntime::new()),
            PinKind::Exec => Self::Exec,
        }
    }

    pub fn as_data(&self) -> Option<&DataPinRuntime> {
        match self {
            PinRuntime::Data(d) => Some(d),
            _ => None,
        }
    }

    pub fn as_data_mut(&mut self) -> Option<&mut DataPinRuntime> {
        match self {
            PinRuntime::Data(d) => Some(d),
            _ => None,
        }
    }
}

/// 定义期默认值解析
pub fn resolve_default_value(def: &PinDefinition) -> Option<DataValue> {
    let desc = def.type_desc.as_ref()?;

    if desc.is_optional {
        return None;
    }

    match &desc.data_type {
        PinDataType::Concrete(vt) => vt.default_value(),
        PinDataType::TypeVar(_) => None,
        PinDataType::Unknown => None,
    }
}
