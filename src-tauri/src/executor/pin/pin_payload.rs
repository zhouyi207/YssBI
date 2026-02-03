//! pin 运行时的数据

use super::{PinDefinition, PinKind};
use crate::executor::value::{DataType, DataValue};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PinPayload {
    Data {
        /// 当前计算值（来自上游或默认）
        value: Option<DataValue>,
        /// 用户手动输入值
        user_value: Option<DataValue>,
    },
    Exec,
}

impl PinPayload {
    // 设置默认值
    pub fn new(def: &PinDefinition) -> Self {
        match def.kind {
            PinKind::Data => {
                let value = resolve_default_value(def);
                Self::Data {
                    value,
                    user_value: None,
                }
            }
            PinKind::Exec => Self::Exec,
        }
    }

    pub fn data(value: Option<DataValue>, user_value: Option<DataValue>) -> Self {
        Self::Data { value, user_value }
    }
}

/// 默认值解析逻辑（关键）
fn resolve_default_value(def: &PinDefinition) -> Option<DataValue> {
    let desc = def.type_desc.as_ref()?;

    // optional pin：默认永远是 None
    if desc.is_optional {
        return None;
    }

    match &desc.data_type {
        DataType::Concrete(vt) => vt.default_value(),
        DataType::TypeVar(_) => None,
        DataType::Unknown => None,
    }
}
