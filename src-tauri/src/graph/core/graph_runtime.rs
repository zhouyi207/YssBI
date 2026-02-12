use crate::graph::NodeDefinition;
use crate::graph::{DataType, DataValue, GraphInstance, PinInstance, PinRole};
use crate::graph::{NodeId, NodeRuntimeState, PinId, PinRuntimeState};
use std::collections::HashMap;
use std::sync::Arc;

pub struct GraphRuntime {
    graph_instance: Arc<GraphInstance>,

    // 运行期 pin 状态
    pins_runtime_state: HashMap<PinId, PinRuntimeState>,

    // 运行期 node 状态
    nodes_runtime_state: HashMap<NodeId, NodeRuntimeState>,
}

/// output data 是无法设置值的
impl GraphRuntime {
    pub fn new(graph_instance: Arc<GraphInstance>) -> Self {
        // 在这里需要解析获取 pins_runtime_state 和 nodes_runtime_state
        Self {
            graph_instance,
            pins_runtime_state: HashMap::new(),
            nodes_runtime_state: HashMap::new(),
        }
    }

    pub fn set_pin_current_value(&mut self, pin_id: PinId, value: DataValue) {
        let pin_instance = self.get_pin_instance_by_pin_id(pin_id).unwrap();
        let pin_runtime_state = self.pins_runtime_state.get_mut(&pin_id);
        if let Some(pin_runtime_state) = pin_runtime_state {
            pin_runtime_state.current_value = Some(value);
        } else {
            let pin_runtime_state = PinRuntimeState::from_instance(pin_instance).with_current_value(Some(value));
            self.pins_runtime_state.insert(pin_id, pin_runtime_state);
        }
    }

    pub fn get_pin_instance_by_pin_id(&self, pin_id: PinId) -> Option<PinInstance> {
        self.graph_instance.get_pin_instance_by_pin_id(pin_id)
    }

    pub fn get_pin_instance_by_pin_role(
        &self,
        node_id: NodeId,
        role: &PinRole,
    ) -> Option<PinInstance> {
        self.graph_instance
            .get_pin_instance_by_pin_role(node_id, role)
    }

    pub fn get_pin_instances_by_pin_role(
        &self,
        node_id: NodeId,
        role: &PinRole,
    ) -> Vec<PinInstance> {
        self.graph_instance
            .get_pin_instances_by_pin_role(node_id, role)
    }

    pub fn get_pin_instances_by_node_id(&self, node_id: NodeId) -> Vec<PinInstance> {
        self.graph_instance.get_pin_instances_by_node_id(node_id)
    }

    pub fn get_pin_data_value_by_pin_role(&self, node_id: NodeId, role: &PinRole) -> DataValue {
        let pin_instance = self
            .graph_instance
            .get_pin_instance_by_pin_role(node_id, role)
            .unwrap();
        assert_eq!(pin_instance.is_data(), true);

        let id = pin_instance.id;
        if let Some(pin_runtime_state) = self.pins_runtime_state.get(&id) {
            return pin_runtime_state.current_value.clone().unwrap();
        }

        // Try user_value first, then fall back to default value
        if let Some(user_value) = pin_instance.user_value {
            return user_value;
        }

        // Use default value from data type definition
        if let Some(pin_data_type_def) = &pin_instance.definition.data_type {
            if let crate::graph::pin::PinDataTypeDefinition::Concrete(data_type) = pin_data_type_def {
                if let Some(default_value) = data_type.default_value() {
                    return default_value;
                }
            }
        }

        panic!("No value available for pin {:?}", pin_instance.id);
    }

    pub fn get_pin_data_value_by_pin_id(&self, pin_id: PinId) -> DataValue {
        let pin_instance = self.get_pin_instance_by_pin_id(pin_id).unwrap();
        if let Some(pin_runtime_state) = self.pins_runtime_state.get(&pin_id) {
            return pin_runtime_state.current_value.clone().unwrap();
        }

        // Try user_value first, then fall back to default value
        if let Some(user_value) = pin_instance.user_value {
            return user_value;
        }

        // Use default value from data type definition
        if let Some(pin_data_type_def) = &pin_instance.definition.data_type {
            if let crate::graph::pin::PinDataTypeDefinition::Concrete(data_type) = pin_data_type_def {
                if let Some(default_value) = data_type.default_value() {
                    return default_value;
                }
            }
        }

        panic!("No value available for pin {:?}", pin_id);
    }

    pub fn get_pin_datas_value_by_pin_role(
        &self,
        node_id: NodeId,
        role: &PinRole,
    ) -> Vec<DataValue> {
        let pin_instances = self
            .graph_instance
            .get_pin_instances_by_pin_role(node_id, role);

        let mut user_values = vec![];
        for pin_instance in pin_instances {
            assert_eq!(pin_instance.is_data(), true);
            let id = pin_instance.id;
            if let Some(pin_runtime_state) = self.pins_runtime_state.get(&id) {
                user_values.push(pin_runtime_state.current_value.clone().unwrap());
            } else {
                user_values.push(pin_instance.user_value.unwrap());
            }
        }

        user_values
    }

    pub fn get_pin_data_type_by_pin_role(&self, pin_id: PinId) -> Option<DataType> {
        self.graph_instance.get_pin_data_type_by_pin_id(pin_id)
    }

    pub fn get_node_definition_by_node_id(&self, node_id: NodeId) -> Arc<NodeDefinition> {
        let node_instance = self
            .graph_instance
            .get_node_instance_by_node_id(node_id)
            .unwrap();
        node_instance.definition
    }

    pub fn get_node_pins(&self, node_id: NodeId) -> Vec<PinInstance> {
        self.graph_instance.get_pin_instances_by_node_id(node_id)
    }

    pub fn get_downstream_by_pin_id(&self, pin_id: PinId) -> Vec<PinId> {
        self.graph_instance.get_downstream_by_pin_id(pin_id)
    }

    pub fn get_upstream_by_pin_id(&self, pin_id: PinId) -> Option<PinId> {
        self.graph_instance.get_upstream_by_pin_id(pin_id)
    }

    pub fn get_node_id_by_pin_id(&self, pin_id: PinId) -> NodeId {
        self.graph_instance.get_node_id_by_pin_id(pin_id)
    }

    pub fn get_pin() {}

    pub fn get_node() {}

    pub fn get_pin_by_role() {}
}
