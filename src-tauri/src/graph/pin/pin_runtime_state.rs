//! pin 运行时的数据
use super::{DataPinState, PinKind, PinState};
use crate::graph::DataValue;
use crate::graph::ExecPinState;
use crate::graph::PinId;
use crate::graph::PinInstance;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinRuntimeState {
    pub id: PinId,
    pub state: PinState,
    pub current_value: Option<DataValue>,
}

impl PinRuntimeState {
    pub fn from_instance(instance: PinInstance) -> Self {
        let state = match instance.definition.kind {
            PinKind::Data => PinState::Data(DataPinState::Uninitialized),
            PinKind::Exec => PinState::Exec(ExecPinState::Idle),
        };
        Self {
            id: PinId::new(),
            state,
            current_value: None,
        }
    }

    pub fn with_current_value(mut self, current_value: Option<DataValue>) -> Self {
        self.current_value = current_value;
        self
    }
}
