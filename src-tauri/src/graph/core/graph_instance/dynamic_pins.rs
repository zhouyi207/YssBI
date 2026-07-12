use super::*;

/// 动态 Pin 重建
impl GraphInstance {
    /// 检查节点是否有 `pin_resolver`，若有则重新计算 pins 并应用变更
    ///
    /// 返回 `Some(PinChangeSet)` 表示 pins 有变化，`None` 表示无需变更
    pub fn resolve_dynamic_pins(&self, node_id: NodeId) -> Result<Option<PinChangeSet>, String> {
        self.resolve_dynamic_pins_with_mode(node_id, PinResolveMode::Interactive)
    }

    pub fn resolve_dynamic_pins_with_mode(
        &self,
        node_id: NodeId,
        mode: PinResolveMode,
    ) -> Result<Option<PinChangeSet>, String> {
        let (definition, instance_params, current_pin_ids);
        {
            let data_state = self.data_state.read().unwrap();
            let node = data_state
                .nodes
                .get(&node_id)
                .ok_or_else(|| format!("Node {:?} not found", node_id))?;
            definition = node.definition.clone();
            instance_params = node.instance_params.clone();
            current_pin_ids = node.pin_ids.clone();
        }

        let resolver = match &definition.pin_resolver {
            Some(r) => r.clone(),
            None => return Ok(None),
        };

        // 构建 PinResolverContext
        let ctx = self.build_resolver_context(node_id, &instance_params)?;

        // 调用 resolver 获取新的 pin 定义
        let new_pin_defs = resolver(&ctx)?;

        // 识别哪些是"静态 pins"（不应被替换）和"动态 pins"（应被替换）
        // 静态 pins = 由 pin_slots 中 Fixed/Repeatable 生成的初始 pins
        let static_pin_defs = definition.generate_initial_pins().unwrap_or_default();

        // 从 new_pin_defs 中移除与 static_pin_defs 名称+方向完全匹配的（它们保留不变）
        // 剩余的是动态部分
        let static_keys: std::collections::HashSet<(String, PinDirection)> = static_pin_defs
            .iter()
            .map(|pd| (pd.name.clone(), pd.direction))
            .collect();

        let dynamic_new_defs: Vec<_> = new_pin_defs
            .iter()
            .filter(|pd| !static_keys.contains(&(pd.name.clone(), pd.direction)))
            .collect();

        // 找出当前的动态 pins（不在 static_keys 中的）
        let dynamic_old_pin_ids: Vec<PinId>;
        {
            let data_state = self.data_state.read().unwrap();
            dynamic_old_pin_ids = current_pin_ids
                .iter()
                .filter(|pid| {
                    if let Some(pin) = data_state.pins.get(pid) {
                        !static_keys
                            .contains(&(pin.definition.name.clone(), pin.definition.direction))
                    } else {
                        false
                    }
                })
                .copied()
                .collect();
        }

        let base_order = static_pin_defs.len() as i32;

        // 如果动态部分名称与顺序完全一致，跳过（最常见的稳定情形）
        let old_names: Vec<String>;
        {
            let data_state = self.data_state.read().unwrap();
            old_names = dynamic_old_pin_ids
                .iter()
                .filter_map(|pid| data_state.pins.get(pid).map(|p| p.definition.name.clone()))
                .collect();
        }
        let new_names: Vec<String> = dynamic_new_defs.iter().map(|pd| pd.name.clone()).collect();
        if old_names == new_names {
            return Ok(None);
        }

        // Tab 打开物化：resolver 无输出时保留项目文件中已保存的 pin（常见于 DB schema 尚未 lazy load）
        if mode == PinResolveMode::Materialize && new_names.is_empty() && !old_names.is_empty() {
            return Ok(None);
        }

        // 按身份对齐动态 pin：存活列复用既有 pin id（保留连接），仅增删/重排实际差异
        let target_defs: Vec<PinDefinition> =
            dynamic_new_defs.iter().map(|pd| (*pd).clone()).collect();
        let change_set = {
            let mut data_state = self.data_state.write().unwrap();
            let reconcile = data_state.reconcile_node_pins(
                node_id,
                &dynamic_old_pin_ids,
                &target_defs,
                base_order,
            );
            PinChangeSet {
                node_id,
                removed_pin_ids: reconcile.removed_pin_ids,
                added_pins: reconcile.added_pins,
                updated_pins: reconcile.updated_pins,
                removed_connections: reconcile.removed_connections,
            }
        };

        Ok(Some(change_set))
    }

    /// 构建 PinResolverContext
    ///
    /// 从上游 output pin 的 resolved_schema 获取 input_schemas（连接时已由 propagate_schemas 填充）
    fn build_resolver_context(
        &self,
        node_id: NodeId,
        instance_params: &NodeInstanceParams,
    ) -> Result<PinResolverContext, String> {
        let mut input_schemas = std::collections::HashMap::new();
        let data_state = self.data_state.read().unwrap();

        if let Some(node) = data_state.nodes.get(&node_id) {
            for &pin_id in &node.pin_ids {
                if let Some(pin) = data_state.pins.get(&pin_id) {
                    if !pin.is_input() || !pin.is_data() {
                        continue;
                    }
                    if let Some(upstream_pin_id) = data_state.connections.get_upstream(pin_id) {
                        if let Some(upstream_pin) = data_state.pins.get(&upstream_pin_id) {
                            if let Some(ref schema) = upstream_pin.resolved_schema {
                                input_schemas.insert(pin.definition.role.clone(), schema.clone());
                            }
                        }
                    }
                }
            }
        }

        Ok(PinResolverContext {
            instance_params: instance_params.clone(),
            input_schemas,
        })
    }
}
