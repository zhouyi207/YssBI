use super::*;

/// Schema propagation and output resolution
impl GraphInstance {
    pub fn propagate_schemas(&self) {
        let node_order = self.topological_node_order();
        let mut data_state = self.data_state.write().unwrap();

        for pin in data_state.pins.values_mut() {
            pin.resolved_schema = None;
        }

        for node_id in node_order {
            if let Some(schema) = self.compute_output_schema_for_node(&mut data_state, node_id) {
                Self::assign_output_schema(&mut data_state, node_id, &schema);
            }
        }
    }

    /// 收集 seeds 沿 data output 边的下游闭包（含 seeds 自身）
    fn collect_downstream_closure(&self, seeds: &[NodeId]) -> std::collections::HashSet<NodeId> {
        let data_state = self.data_state.read().unwrap();
        let mut visited = std::collections::HashSet::new();
        let mut stack: Vec<NodeId> = seeds.to_vec();
        while let Some(nid) = stack.pop() {
            if !visited.insert(nid) {
                continue;
            }
            if let Some(node) = data_state.nodes.get(&nid) {
                for &pin_id in &node.pin_ids {
                    if let Some(pin) = data_state.pins.get(&pin_id) {
                        if pin.is_output() {
                            for to_pid in data_state.connections.get_downstream(pin_id) {
                                if let Some(p) = data_state.pins.get(&to_pid) {
                                    if !visited.contains(&p.node_id) {
                                        stack.push(p.node_id);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        visited
    }

    /// 增量传播 schema：仅清空并重算受影响节点（seeds 及其下游闭包）的 output
    /// schema，其余节点保留既有 resolved_schema。用于 connect/disconnect 等局部拓扑变更。
    ///
    /// schema 沿 data 边单向向下游流动，故下游闭包即完整受影响集合；上游节点的
    /// schema 不会因本次变更而改变，无需重算。
    pub(crate) fn propagate_schemas_from(&self, seeds: &[NodeId]) {
        if seeds.is_empty() {
            return;
        }
        let affected = self.collect_downstream_closure(seeds);
        if affected.is_empty() {
            return;
        }

        let node_order = self.topological_node_order();
        let mut data_state = self.data_state.write().unwrap();

        for &node_id in &affected {
            let pin_ids: Vec<PinId> = data_state
                .nodes
                .get(&node_id)
                .map(|n| n.pin_ids.clone())
                .unwrap_or_default();
            for pin_id in pin_ids {
                if let Some(pin) = data_state.pins.get_mut(&pin_id) {
                    if pin.is_output() && pin.is_data() {
                        pin.resolved_schema = None;
                    }
                }
            }
        }

        for node_id in node_order {
            if !affected.contains(&node_id) {
                continue;
            }
            if let Some(schema) = self.compute_output_schema_for_node(&mut data_state, node_id) {
                Self::assign_output_schema(&mut data_state, node_id, &schema);
            }
        }
    }

    /// 传播 schema 后，对所有有 pin_resolver 的节点执行 dynamic pin 更新
    ///
    /// 确保 Combine 的 input 部分连接时，下游 Decompose 能正确更新 output pins
    pub fn resolve_all_dynamic_pins(&self) -> Vec<PinChangeSet> {
        self.resolve_all_dynamic_pins_with_mode(PinResolveMode::Interactive)
    }

    pub fn resolve_all_dynamic_pins_with_mode(&self, mode: PinResolveMode) -> Vec<PinChangeSet> {
        let node_ids: Vec<NodeId> = {
            let data_state = self.data_state.read().unwrap();
            data_state.nodes.keys().copied().collect()
        };
        let mut change_sets = Vec::new();
        for node_id in node_ids {
            if let Ok(Some(cs)) = self.resolve_dynamic_pins_with_mode(node_id, mode) {
                change_sets.push(cs);
            }
        }
        change_sets
    }

    /// 打开图 Tab 时：传播 schema、物化 schema 派生 pin、运行类型推断
    pub fn materialize_dynamic_pins(&self) -> (Vec<PinChangeSet>, Vec<(PinId, DataType)>) {
        let result = self.recompile(GraphRecompileScope::Materialize);
        (result.change_sets, result.inferred)
    }

    /// 拓扑序：保证上游节点先于下游
    fn topological_node_order(&self) -> Vec<NodeId> {
        let data_state = self.data_state.read().unwrap();
        let mut in_degree: std::collections::HashMap<NodeId, usize> =
            std::collections::HashMap::new();
        for node_id in data_state.nodes.keys() {
            in_degree.entry(*node_id).or_insert(0);
        }
        for conn in data_state.connections.all_connections() {
            if let (Some(from_p), Some(to_p)) = (
                data_state.pins.get(&conn.from_pin),
                data_state.pins.get(&conn.to_pin),
            ) {
                let from_node = from_p.node_id;
                let to_node = to_p.node_id;
                if from_node != to_node {
                    *in_degree.entry(to_node).or_insert(0) += 1;
                }
            }
        }
        let mut queue: Vec<NodeId> = in_degree
            .iter()
            .filter(|&(_, &d)| d == 0)
            .map(|(id, _)| *id)
            .collect();
        let mut order = Vec::new();
        while let Some(nid) = queue.pop() {
            order.push(nid);
            let node = match data_state.nodes.get(&nid) {
                Some(n) => n,
                None => continue,
            };
            for &pin_id in &node.pin_ids {
                let pin = match data_state.pins.get(&pin_id) {
                    Some(p) => p,
                    None => continue,
                };
                if pin.is_output() {
                    for to_pid in data_state.connections.get_downstream(pin_id) {
                        if let Some(to_pin) = data_state.pins.get(&to_pid) {
                            let to_node = to_pin.node_id;
                            if to_node != nid {
                                if let Some(d) = in_degree.get_mut(&to_node) {
                                    *d = d.saturating_sub(1);
                                    if *d == 0 {
                                        queue.push(to_node);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        order
    }

    /// 构建 OutputSchemaContext（供节点 output_schema_resolver 使用）
    fn build_output_schema_context(
        data_state: &GraphDataState,
        node_id: NodeId,
        schema_provider: Option<SchemaProvider>,
    ) -> Option<OutputSchemaContext> {
        let node = data_state.nodes.get(&node_id)?;
        let mut input_schemas = std::collections::HashMap::new();

        for &pin_id in &node.pin_ids {
            let pin = data_state.pins.get(&pin_id)?;
            if !pin.is_input() || !pin.is_data() {
                continue;
            }
            let upstream_pin_id = match data_state.connections.get_upstream(pin_id) {
                Some(id) => id,
                None => continue, // 跳过未连接的 input，仅用已连接的构建 schema
            };
            let upstream_pin = data_state.pins.get(&upstream_pin_id)?;

            if let Some(ref schema) = upstream_pin.resolved_schema {
                input_schemas.insert(pin.definition.role.clone(), schema.clone());
            } else if let Some(PinDataTypeDefinition::Concrete(DataType::DataSeries(inner))) =
                &upstream_pin.definition.data_type
            {
                let name = {
                    let n = upstream_pin.definition.name.clone();
                    if n.is_empty() || n == "literal" {
                        format!("col_{}", pin.definition.role.index().unwrap_or(0))
                    } else {
                        n
                    }
                };
                input_schemas.insert(
                    pin.definition.role.clone(),
                    DataSchema {
                        columns: vec![ColumnSchema {
                            name,
                            data_type: inner.as_ref().clone(),
                        }],
                    },
                );
            }
        }

        Some(OutputSchemaContext {
            instance_params: node.instance_params.clone(),
            input_schemas,
            schema_provider,
        })
    }

    /// 计算节点的 DataFrame output schema（在已持锁的 data_state 上操作）
    fn compute_output_schema_for_node(
        &self,
        data_state: &mut std::sync::RwLockWriteGuard<'_, GraphDataState>,
        node_id: NodeId,
    ) -> Option<DataSchema> {
        let node = data_state.nodes.get(&node_id)?;

        if let Some(ref resolver) = node.definition.output_schema_resolver {
            if let Some(ctx) = Self::build_output_schema_context(
                &*data_state,
                node_id,
                self.schema_provider.clone(),
            ) {
                return resolver(&ctx);
            }
        }

        Self::compute_output_schema_fallback(&*data_state, node_id)
    }

    /// 默认 fallback：透传上游 Input schema（无 output_schema_resolver 时使用）
    fn compute_output_schema_fallback(
        data_state: &GraphDataState,
        node_id: NodeId,
    ) -> Option<DataSchema> {
        let node = data_state.nodes.get(&node_id)?;
        let input_role = PinRole::Data(DataRole::Input);

        let input_pin = node.pin_ids.iter().find_map(|&pid| {
            let p = data_state.pins.get(&pid)?;
            if p.is_input() && p.definition.role == input_role {
                Some(pid)
            } else {
                None
            }
        })?;
        let upstream_pin_id = data_state.connections.get_upstream(input_pin)?;
        data_state
            .pins
            .get(&upstream_pin_id)?
            .resolved_schema
            .clone()
    }

    /// 节点输入变化时，获取需额外解析的下游节点（消费该节点所有 output 的节点）
    pub(crate) fn get_downstream_resolve_nodes(&self, node_id: NodeId) -> Vec<NodeId> {
        let data_state = self.data_state.read().unwrap();
        let node = match data_state.nodes.get(&node_id) {
            Some(n) => n,
            None => return vec![],
        };

        let mut downstream = Vec::new();
        for &pin_id in &node.pin_ids {
            let pin = match data_state.pins.get(&pin_id) {
                Some(p) => p,
                None => continue,
            };
            if pin.is_output() {
                for to_pid in data_state.connections.get_downstream(pin_id) {
                    if let Some(p) = data_state.pins.get(&to_pid) {
                        downstream.push(p.node_id);
                    }
                }
            }
        }
        downstream
    }

    fn is_model_struct_type_key(type_key: &str) -> bool {
        matches!(type_key, "OLSModel" | "LogitModel" | "ProbitModel")
    }

    fn assign_output_schema(
        data_state: &mut std::sync::RwLockWriteGuard<'_, GraphDataState>,
        node_id: NodeId,
        schema: &DataSchema,
    ) {
        let pin_ids: Vec<PinId> = data_state
            .nodes
            .get(&node_id)
            .map(|n| n.pin_ids.clone())
            .unwrap_or_default();
        let has_output_resolver = data_state
            .nodes
            .get(&node_id)
            .map(|n| n.definition.output_schema_resolver.is_some())
            .unwrap_or(false);
        for pin_id in pin_ids {
            if let Some(pin) = data_state.pins.get_mut(&pin_id) {
                if !pin.is_output() || !pin.is_data() {
                    continue;
                }
                let assign = match pin.definition.data_type.as_ref() {
                    Some(PinDataTypeDefinition::Concrete(DataType::DataFrame)) => true,
                    Some(PinDataTypeDefinition::Concrete(DataType::Struct(type_key)))
                        if has_output_resolver && Self::is_model_struct_type_key(type_key) =>
                    {
                        true
                    }
                    _ if has_output_resolver => true,
                    _ => false,
                };
                if assign {
                    pin.resolved_schema = Some(schema.clone());
                    break;
                }
            }
        }
    }
}
