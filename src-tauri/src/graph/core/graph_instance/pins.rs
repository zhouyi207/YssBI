use super::*;

/// Pin 管理
impl GraphInstance {
    pub fn get_pin_instances_by_node_id(&self, node_id: NodeId) -> Vec<PinInstance> {
        let data_state = self.data_state.read().unwrap();
        let pin_ids = match data_state.nodes.get(&node_id) {
            Some(node) => node.pin_ids.clone(),
            None => return Vec::new(),
        };

        pin_ids
            .into_iter()
            .filter_map(|id| data_state.pins.get(&id).cloned())
            .collect()
    }

    pub fn get_pin_instance_by_pin_id(&self, pin_id: PinId) -> Option<PinInstance> {
        let data_state = self.data_state.read().unwrap();
        data_state.pins.get(&pin_id).cloned()
    }

    pub fn get_pin_data_type_by_pin_id(&self, pin_id: PinId) -> Option<DataType> {
        self.data_state
            .read()
            .unwrap()
            .pin_types
            .get(&pin_id)
            .cloned()
    }

    pub fn get_pin_user_value_by_pin_id(&self, pin_id: PinId) -> Option<DataValue> {
        let data_state = self.data_state.read().unwrap();
        if let Some(pin) = data_state.pins.get(&pin_id) {
            return pin.user_value.clone();
        }
        None
    }

    pub fn set_pin_user_value_by_pin_id(
        &self,
        pin_id: PinId,
        value: DataValue,
    ) -> Result<(), String> {
        {
            let mut data_state = self.data_state.write().unwrap();

            if let Some(pin) = data_state.pins.get_mut(&pin_id) {
                pin.user_value = Some(value);
            } else {
                return Err(format!("Pin {:?} not found", pin_id));
            }
        }

        let _ = self.infer_types();
        Ok(())
    }

    /// 清除 Pin 的用户值（恢复为 None，使用默认值或连接值）
    pub fn clear_pin_user_value_by_pin_id(&self, pin_id: PinId) -> Result<(), String> {
        {
            let mut data_state = self.data_state.write().unwrap();
            if let Some(pin) = data_state.pins.get_mut(&pin_id) {
                pin.user_value = None;
            } else {
                return Err(format!("Pin {:?} not found", pin_id));
            }
        }
        let _ = self.infer_types();
        Ok(())
    }

    pub fn get_pin_instance_by_pin_role(
        &self,
        node_id: NodeId,
        role: &PinRole,
    ) -> Option<PinInstance> {
        self.get_pin_instances_by_node_id(node_id)
            .into_iter()
            .find(|p| &p.definition.role == role)
    }

    /// 通过 Role 获取多个 Pin（用于动态 Pin 组）
    pub fn get_pin_instances_by_pin_role(
        &self,
        node_id: NodeId,
        role: &PinRole,
    ) -> Vec<PinInstance> {
        self.get_pin_instances_by_node_id(node_id)
            .into_iter()
            .filter(|p| &p.definition.role == role)
            .collect()
    }

    pub fn get_pin_instances_by_pin_role_family(
        &self,
        node_id: NodeId,
        pattern: &PinRole,
    ) -> Vec<PinInstance> {
        self.get_pin_instances_by_node_id(node_id)
            .into_iter()
            .filter(|p| p.definition.role.matches_family(pattern))
            .collect()
    }
}
