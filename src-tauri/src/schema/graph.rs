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

        let nodes: Vec<NodeInstanceDTO> = data_state
            .nodes
            .values()
            .map(NodeInstanceDTO::from)
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
