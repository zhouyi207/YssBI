use std::collections::HashMap;

use crate::{
    graph::{GraphDataState, GraphId, GraphInstance, GraphKind, GraphPosition, NodeId, PinId},
    schema::{ConnectionDTO, NodeInstanceDTO, PinInstanceDTO},
};
use serde::{Deserialize, Serialize};

/// Graph 类型 - 对应前端 GraphType
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GraphTypeDTO {
    Event,
    Function,
    Macro,
}

impl From<&GraphKind> for GraphTypeDTO {
    fn from(value: &GraphKind) -> Self {
        match value {
            GraphKind::Event => GraphTypeDTO::Event,
            GraphKind::Function => GraphTypeDTO::Function,
            GraphKind::Macro => GraphTypeDTO::Macro,
        }
    }
}

/// Graph instance DTO - 对应前端 Graph 类型
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphInstanceDTO {
    pub id: GraphId,
    pub name: String,
    #[serde(rename = "type")]
    pub graph_type: GraphTypeDTO,
    pub nodes: Vec<NodeInstanceDTO>,
    pub pins: Vec<PinInstanceDTO>,
    pub connections: ConnectionDTO,
    pub canvas: GraphPosition,
}

impl From<&GraphInstance> for GraphInstanceDTO {
    fn from(value: &GraphInstance) -> Self {
        let data_state = value.data_state.read().unwrap();

        // 构建 node_id -> (inputs, outputs) 的映射
        let mut node_pins: HashMap<NodeId, (Vec<String>, Vec<String>)> = HashMap::new();
        for (pin_id, pin) in &data_state.pins {
            let entry = node_pins.entry(pin.node_id).or_insert((Vec::new(), Vec::new()));
            match pin.definition.direction {
                crate::graph::PinDirection::Input => entry.0.push(pin_id.to_string()),
                crate::graph::PinDirection::Output => entry.1.push(pin_id.to_string()),
            }
        }

        let nodes: Vec<NodeInstanceDTO> = data_state
            .nodes
            .values()
            .map(|node| {
                let mut dto = NodeInstanceDTO::from(node);
                if let Some((inputs, outputs)) = node_pins.get(&node.id) {
                    dto.inputs = inputs.clone();
                    dto.outputs = outputs.clone();
                }
                dto
            })
            .collect();

        let pins: Vec<PinInstanceDTO> = data_state
            .pins
            .values()
            .map(PinInstanceDTO::from)
            .collect();

        Self {
            id: value.id,
            name: value.name.clone(),
            graph_type: GraphTypeDTO::from(&value.kind),
            nodes,
            pins,
            connections: ConnectionDTO::from(&data_state.connections),
            canvas: value.position.clone(),
        }
    }
}

/// Graph data state DTO - 内部使用
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GraphDataStateDTO {
    pub nodes: HashMap<NodeId, NodeInstanceDTO>,
    pub pins: HashMap<PinId, PinInstanceDTO>,
    pub connections: ConnectionDTO,
}

impl From<&GraphDataState> for GraphDataStateDTO {
    fn from(value: &GraphDataState) -> Self {
        Self {
            nodes: value
                .nodes
                .iter()
                .map(|(id, node)| (*id, NodeInstanceDTO::from(node)))
                .collect(),

            pins: value
                .pins
                .iter()
                .map(|(id, pin)| (*id, PinInstanceDTO::from(pin)))
                .collect(),

            connections: ConnectionDTO::from(&value.connections),
        }
    }
}
