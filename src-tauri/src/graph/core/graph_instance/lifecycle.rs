use super::*;

/// 创建和清理
impl GraphInstance {
    pub fn new(
        name: impl Into<String>,
        kind: GraphKind,
        registry: Arc<NodeRegistry>,
    ) -> Self {
        let (dir, extension) = match kind {
            GraphKind::Event => (crate::project::EVENTS_DIR, crate::project::EVENT_EXTENSION),
            GraphKind::Function => (
                crate::project::FUNCTIONS_DIR,
                crate::project::FUNCTION_EXTENSION,
            ),
        };
        let transient = GraphResourcePath::from_normalized_unchecked(format!(
            "{dir}/transient-{}.{}",
            uuid::Uuid::new_v4().simple(),
            extension
        ));
        Self::new_with_path(name, kind, registry, transient)
    }

    pub fn new_with_path(
        name: impl Into<String>,
        kind: GraphKind,
        registry: Arc<NodeRegistry>,
        resource_path: GraphResourcePath,
    ) -> Self {
        let (function_inputs, function_outputs) = if kind == GraphKind::Function {
            (
                super::types::default_function_exec_inputs(),
                super::types::default_function_exec_outputs(),
            )
        } else {
            (Vec::new(), Vec::new())
        };
        Self {
            resource_path,
            name: name.into(),
            position: GraphPosition::default(),
            kind,
            function_inputs,
            function_outputs,
            data_state: Default::default(),
            registry,
            schema_provider: None,
            runtime_prepared_epoch: 0,
        }
    }

    pub fn clear(&self) {
        *self.data_state.write().unwrap() = GraphDataState::default();
    }

    /// 设置节点注册表（用于反序列化后恢复）
    ///
    /// Re-attaches the full `NodeDefinition` (with function pointers like
    /// `pin_resolver`, `flow_processor`, `data_evaluator`) from the registry
    /// to each existing node, because those fields are `#[serde(skip)]` and
    /// lost during deserialization.
    pub fn set_registry(&mut self, registry: Arc<NodeRegistry>) {
        {
            let mut data_state = self.data_state.write().unwrap();
            for node in data_state.nodes.values_mut() {
                if let Some(full_def) = registry.get(&node.definition.node_type) {
                    node.definition = full_def;
                }
            }
            Self::sync_static_pin_definitions(&mut data_state, &registry);
            data_state.reconcile_connections();
        }
        self.registry = registry;
    }

    pub(crate) fn sync_static_pin_definitions(
        data_state: &mut crate::graph::GraphDataState,
        registry: &NodeRegistry,
    ) {
        for node in data_state.nodes.values() {
            let Some(full_def) = registry.get(&node.definition.node_type) else {
                continue;
            };
            let Ok(expected_pins) = full_def.generate_initial_pins() else {
                continue;
            };

            for pin_id in &node.pin_ids {
                let Some(pin) = data_state.pins.get_mut(pin_id) else {
                    continue;
                };
                if pin.definition.should_persist_full_definition() {
                    continue;
                }
                if let Some(template) = expected_pins.iter().find(|template| {
                    template.role == pin.definition.role && template.name == pin.definition.name
                }) {
                    pin.definition = template.clone();
                }
            }
        }
    }

    /// 设置模式提供器（用于 pin_resolver 查询 DataFrame 列结构）
    pub fn set_schema_provider(&mut self, provider: SchemaProvider) {
        self.schema_provider = Some(provider);
    }

    /// Unified post-mutation compile entry.
    pub fn recompile(&self, scope: GraphRecompileScope) -> GraphRecompileResult {
        match scope {
            GraphRecompileScope::None => GraphRecompileResult::default(),
            GraphRecompileScope::RuntimePrepare => {
                self.propagate_schemas();
                GraphRecompileResult {
                    inferred: self.infer_types_with_warn(),
                    ..Default::default()
                }
            }
            GraphRecompileScope::InferOnly => GraphRecompileResult {
                inferred: self.infer_types_with_warn(),
                ..Default::default()
            },
            GraphRecompileScope::Full => {
                self.propagate_schemas();
                let change_sets =
                    self.resolve_all_dynamic_pins_with_mode(PinResolveMode::Interactive);
                GraphRecompileResult {
                    change_sets,
                    inferred: self.infer_types_with_warn(),
                }
            }
            GraphRecompileScope::FromSeeds(seeds) => {
                if seeds.is_empty() {
                    return self.recompile(GraphRecompileScope::Full);
                }
                self.recompile_seeded(&seeds, PinResolveMode::Interactive)
            }
            GraphRecompileScope::Materialize => {
                self.propagate_schemas();
                let change_sets =
                    self.resolve_all_dynamic_pins_with_mode(PinResolveMode::Materialize);
                GraphRecompileResult {
                    change_sets,
                    inferred: self.infer_types_with_warn(),
                }
            }
            GraphRecompileScope::TopologyEffects { seeds, mode } => {
                self.recompile_seeded(&seeds, mode)
            }
        }
    }

    fn infer_types_with_warn(&self) -> Vec<(PinId, DataType)> {
        self.infer_types()
            .map_err(|e| crate::log::log_sys::warn!("graph type inference failed: {}", e))
            .unwrap_or_default()
    }

    fn collect_downstream_resolve_nodes(&self, seeds: &[NodeId]) -> Vec<NodeId> {
        let mut to_resolve = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for &nid in seeds {
            if seen.insert(nid) {
                to_resolve.push(nid);
            }
            for downstream in self.get_downstream_resolve_nodes(nid) {
                if seen.insert(downstream) {
                    to_resolve.push(downstream);
                }
            }
        }
        to_resolve
    }

    fn recompile_seeded(&self, seeds: &[NodeId], mode: PinResolveMode) -> GraphRecompileResult {
        if seeds.is_empty() {
            return GraphRecompileResult::default();
        }
        self.propagate_schemas_from(seeds);
        let mut change_sets = Vec::new();
        for node_id in self.collect_downstream_resolve_nodes(seeds) {
            if let Ok(Some(cs)) = self.resolve_dynamic_pins_with_mode(node_id, mode) {
                change_sets.push(cs);
            }
        }
        GraphRecompileResult {
            change_sets,
            inferred: self.infer_types_with_warn(),
        }
    }

    /// 全图重编译：schema 传播 + 动态 pin 解析 + 类型推断。
    /// 从种子节点局部重编译（变量变更、局部拓扑变更）。
    /// 获取节点注册表的引用
    pub fn registry(&self) -> &Arc<NodeRegistry> {
        &self.registry
    }

    /// 从前端快照重建后端 Graph 状态（用于 undo/redo 后同步）
    ///
    /// 流程：
    /// 1. 清空当前状态
    /// 2. 按快照重建所有节点（保留原始 ID）
    /// 3. 重建所有连接
    /// 4. 运行类型推断
    pub fn rebuild_from_snapshot(
        &self,
        snapshot: crate::schema::GraphRebuildSnapshot,
    ) -> Result<(), String> {
        use crate::graph::pin::PinId;
        use uuid::Uuid;

        let parse_node_id = |s: &str| -> Result<NodeId, String> {
            Uuid::parse_str(s)
                .map(NodeId::from)
                .map_err(|e| format!("Invalid node id '{}': {}", s, e))
        };
        let parse_pin_id = |s: &str| -> Result<PinId, String> {
            Uuid::parse_str(s)
                .map(PinId::from)
                .map_err(|e| format!("Invalid pin id '{}': {}", s, e))
        };

        let mut data_state = self.data_state.write().unwrap();
        *data_state = GraphDataState::default();

        for node_snap in &snapshot.nodes {
            let definition = self.registry.get(&node_snap.node_type).ok_or_else(|| {
                format!("Node type '{}' not found in registry", node_snap.node_type)
            })?;

            let result = NodeInstance::from_definition(definition.clone())
                .map_err(|e| format!("Failed to create node '{}': {}", node_snap.node_type, e))?;

            let target_node_id = parse_node_id(&node_snap.id)?;

            let mut node = result.node;
            node.id = target_node_id;
            node.position = crate::graph::NodePosition {
                x: node_snap.x,
                y: node_snap.y,
            };
            if let Some(ref params) = node_snap.params {
                node.instance_params = params.clone();
            }

            let mut pins = result.pins;

            for (i, pin) in pins.iter_mut().enumerate() {
                pin.node_id = target_node_id;
                if let Some(snap_pin) = node_snap.pins.get(i) {
                    if let Ok(pid) = parse_pin_id(&snap_pin.id) {
                        pin.id = pid;
                    }
                    if let Some(ref val) = snap_pin.user_value {
                        pin.user_value = Some(val.clone());
                    }
                }
            }

            node.pin_ids = pins.iter().map(|p| p.id).collect();

            for pin in &pins {
                data_state.connections.register_pin(pin.id, target_node_id);
            }

            data_state.add_node(node);
            data_state.add_pins(pins);
        }

        for conn in &snapshot.connections {
            let from_pin = parse_pin_id(&conn.from_pin)?;
            let to_pin = parse_pin_id(&conn.to_pin)?;
            data_state.connections.connect(from_pin, to_pin);
        }

        drop(data_state);

        self.propagate_schemas();
        let _ = self.infer_types();

        Ok(())
    }
}
