//! pin 运行时的数据
use super::{DataPinState, PinKind, PinState};
use crate::graph::DataValue;
use crate::graph::ExecPinState;
use crate::graph::PinId;
use crate::graph::PinInstance;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinRuntime {
    pub id: PinId,
    pub state: PinState,
    pub instance: PinInstance,
    pub current_value: Option<DataValue>,
}

impl PinRuntime {
    pub fn from_instance(&self, instance: PinInstance) -> Self {
        let state = match instance.definition.kind {
            PinKind::Data => PinState::Data(DataPinState::Uninitialized),
            PinKind::Exec => PinState::Exec(ExecPinState::Idle),
        };
        Self {
            id: PinId::new(),
            state,
            instance,
            current_value: None,
        }
    }
}
