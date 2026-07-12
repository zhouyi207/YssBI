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
            id: instance.id,
            state,
            current_value: None,
        }
    }

    pub fn with_current_value(mut self, current_value: Option<DataValue>) -> Self {
        self.current_value = current_value;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::DataType;
    use crate::graph::node::NodeId;
    use crate::graph::pin::{DataRole, PinDataTypeDefinition, PinDefinition};

    #[test]
    fn from_instance_preserves_pin_id() {
        let def = PinDefinition::data_output(
            "out",
            DataRole::Result,
            PinDataTypeDefinition::concrete(DataType::Float64),
        );
        let instance = PinInstance::from_definition(&def, NodeId::new(), 0);
        let expected_id = instance.id;
        let runtime = PinRuntimeState::from_instance(instance);
        assert_eq!(runtime.id, expected_id);
    }
}
