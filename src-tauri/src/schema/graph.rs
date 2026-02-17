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
#[serde(rename_all = "camelCase")]
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
        // 必须按 node.pin_ids 顺序遍历，否则 HashMap 迭代顺序不确定会导致 pin 渲染顺序错乱
        let mut node_pins: HashMap<NodeId, (Vec<String>, Vec<String>)> = HashMap::new();
        for node in data_state.nodes.values() {
            let mut inputs = Vec::new();
            let mut outputs = Vec::new();
            for pin_id in &node.pin_ids {
                if let Some(pin) = data_state.pins.get(pin_id) {
                    match pin.definition.direction {
                        crate::graph::PinDirection::Input => inputs.push(pin_id.to_string()),
                        crate::graph::PinDirection::Output => outputs.push(pin_id.to_string()),
                    }
                }
            }
            node_pins.insert(node.id, (inputs, outputs));
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
            .map(|pin| {
                let resolved_type = data_state.pin_types.get(&pin.id);
                let links = if pin.definition.direction == crate::graph::PinDirection::Output {
                    data_state.connections.get_downstream(pin.id)
                } else {
                    data_state
                        .connections
                        .get_upstream(pin.id)
                        .map(|p| vec![p])
                        .unwrap_or_default()
                };
                PinInstanceDTO::from_pin_with_context(pin, resolved_type, links)
            })
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
#[serde(rename_all = "camelCase")]
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
                .map(|(id, pin)| {
                    let resolved_type = value.pin_types.get(&pin.id);
                    let links = if pin.definition.direction == crate::graph::PinDirection::Output {
                        value.connections.get_downstream(pin.id)
                    } else {
                        value
                            .connections
                            .get_upstream(pin.id)
                            .map(|p| vec![p])
                            .unwrap_or_default()
                    };
                    (
                        *id,
                        PinInstanceDTO::from_pin_with_context(pin, resolved_type, links),
                    )
                })
                .collect(),

            connections: ConnectionDTO::from(&value.connections),
        }
    }
}
