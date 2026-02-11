use crate::graph::PinInstance;
use crate::graph::{NodeId, PinDirection, PinId, PinKind};
use serde::{Deserialize, Serialize};
use crate::graph::DataValue;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PinInstanceDTO {
    pub id: PinId,
    pub node_id: NodeId,
    pub name: String,
    pub direction: PinDirection,
    pub kind: PinKind,
    pub value: Option<DataValue>,
}

// impl From<&PinInstance> for PinInstanceDTO {
//     fn from(value: &PinInstance) -> Self {
//         Self {
//             id: value.id,
//             node_id: value.node_id,
//             name: value.definition.name.clone(),
//             direction: value.definition.direction,
//             kind: value.definition.kind,
//             value: value.().cloned(),
//         }
//     }
// }
