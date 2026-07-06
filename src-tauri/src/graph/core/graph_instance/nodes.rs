use super::*;

/// Node 管理
impl GraphInstance {
    pub fn create_node(&self, node_type: &str) -> Result<NodeId, String> {
        self.create_node_with_position(node_type, 0.0, 0.0, None)
    }

    pub fn create_node_with_position(
        &self,
        node_type: &str,
        x: f32,
        y: f32,
        params: Option<NodeInstanceParams>,
    ) -> Result<NodeId, String> {
        // 新建节点尚无任何连接，不会影响已有 pin 的类型；其数据 pin 类型已由
        // pin 定义（`data_type`）或 `create_node_raw` 写入的 `variable_data_type`
        // 覆盖确定。`infer_all` 只处理连接，对孤立节点没有贡献，故此处不做全图
        // 类型推断，避免随图规模线性增长的无谓开销。
        self.create_node_raw(node_type, x, y, params)
    }

    /// 创建节点但不运行类型推断（用于批量创建，外部负责最终调一次 infer_types）
    pub fn create_node_raw(
        &self,
        node_type: &str,
        x: f32,
        y: f32,
        params: Option<NodeInstanceParams>,
    ) -> Result<NodeId, String> {
        let definition = self
            .registry
            .get(node_type)
            .ok_or_else(|| format!("Node type '{}' not found", node_type))?;

        let result = NodeInstance::from_definition(definition.clone())?;
        let node_id = result.node.id;

        let mut node = result.node.with_position(x, y);
        if let Some(ref p) = params {
            node = node.with_instance_params(p.clone());
        }

        {
            let mut data_state = self.data_state.write().unwrap();
            data_state.add_node(node);
            data_state.add_pins(result.pins);
        }

        Ok(node_id)
    }

    /// Create a node with a specific node ID but auto-generated pin IDs.
    /// Handles dynamic-pin nodes where saved pin count may differ from base definition.
    /// Does NOT run type inference — call `infer_types()` after.
    pub fn create_node_raw_with_node_id(
        &self,
        node_type: &str,
        node_id: NodeId,
        x: f32,
        y: f32,
        params: Option<NodeInstanceParams>,
    ) -> Result<NodeId, String> {
        let definition = self
            .registry
            .get(node_type)
            .ok_or_else(|| format!("Node type '{}' not found", node_type))?;

        let result = NodeInstance::from_definition_with_node_id(definition.clone(), node_id)?;

        let mut node = result.node.with_position(x, y);
        if let Some(ref p) = params {
            node = node.with_instance_params(p.clone());
        }

        {
            let mut data_state = self.data_state.write().unwrap();
            data_state.add_node(node);
            data_state.add_pins(result.pins);
        }

        Ok(node_id)
    }

    /// Create a node with specific IDs (for redo — preserves node/pin identity).
    /// Does NOT run type inference — call `infer_types()` after.
    pub fn create_node_raw_with_ids(
        &self,
        node_type: &str,
        node_id: NodeId,
        pin_ids: &[PinId],
        x: f32,
        y: f32,
        params: Option<NodeInstanceParams>,
    ) -> Result<NodeId, String> {
        let definition = self
            .registry
            .get(node_type)
            .ok_or_else(|| format!("Node type '{}' not found", node_type))?;

        let result = NodeInstance::from_definition_with_ids(definition.clone(), node_id, pin_ids)?;

        let mut node = result.node.with_position(x, y);
        if let Some(ref p) = params {
            node = node.with_instance_params(p.clone());
        }

        {
            let mut data_state = self.data_state.write().unwrap();
            data_state.add_node(node);
            data_state.add_pins(result.pins);
        }

        Ok(node_id)
    }

    pub fn get_node_instance(&self, node_id: NodeId) -> Option<NodeInstance> {
        let data_state = self.data_state.read().unwrap();
        data_state.nodes.get(&node_id).cloned()
    }

    /// 更新节点的 instance_params 并触发动态 pin 重建
    /// 批量更新节点位置（拖拽结束时调用，CQRS 模式）
    pub fn set_node_positions(&self, updates: &[(NodeId, f32, f32)]) -> Result<(), String> {
        let mut data_state = self.data_state.write().unwrap();
        for (node_id, x, y) in updates {
            if let Some(node) = data_state.nodes.get_mut(node_id) {
                node.position.x = *x;
                node.position.y = *y;
            }
        }
        Ok(())
    }

    /// 删除节点但不运行类型推断（用于批量删除，外部负责最终调一次 infer_types）
    ///
    /// Returns the set of neighbor node IDs whose connections were affected,
    /// so the caller can run `resolve_dynamic_pins` on them after deletion.
    pub fn remove_node_raw(
        &self,
        node_id: NodeId,
    ) -> Result<std::collections::HashSet<NodeId>, String> {
        let pins = self.get_pin_instances_by_node_id(node_id);

        let mut neighbor_node_ids = std::collections::HashSet::new();
        {
            let mut data_state = self.data_state.write().unwrap();
            if !data_state.nodes.contains_key(&node_id) {
                return Ok(neighbor_node_ids);
            }

            // Collect neighbor nodes before removing connections
            for pin in &pins {
                for downstream_pin_id in data_state.connections.get_downstream(pin.id) {
                    if let Some(p) = data_state.pins.get(&downstream_pin_id) {
                        if p.node_id != node_id {
                            neighbor_node_ids.insert(p.node_id);
                        }
                    }
                }
                if let Some(upstream_pin_id) = data_state.connections.get_upstream(pin.id) {
                    if let Some(p) = data_state.pins.get(&upstream_pin_id) {
                        if p.node_id != node_id {
                            neighbor_node_ids.insert(p.node_id);
                        }
                    }
                }
            }

            for pin in &pins {
                data_state.connections.disconnect_all(pin.id);
            }
            data_state.connections.remove_node(node_id);

            data_state.remove_pins(pins.into_iter().map(|pin| pin.id).collect());
            data_state.nodes.remove(&node_id);
        }

        Ok(neighbor_node_ids)
    }

    pub fn get_node_id_by_pin_id(&self, pin_id: PinId) -> Option<NodeId> {
        let data_state = self.data_state.read().unwrap();
        data_state.pins.get(&pin_id).map(|p| p.node_id)
    }

    pub fn get_node_instance_by_node_id(&self, node_id: NodeId) -> Option<NodeInstance> {
        self.get_node_instance(node_id)
    }

    pub fn get_all_nodes(&self) -> Vec<NodeInstance> {
        let data_state = self.data_state.read().unwrap();
        data_state.nodes.values().cloned().collect()
    }
}

