use super::*;

/// 创建和清理
impl GraphInstance {
    pub fn new(name: impl Into<String>, kind: GraphKind, registry: Arc<NodeRegistry>) -> Self {
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

    pub fn type_system_snapshot(&self) -> crate::graph::value::TypeSystemSnapshot {
        self.registry.type_system_snapshot()
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

    /// Deep-clone `data_state` so an execution run is isolated from live editor mutations.
    pub fn snapshot_for_execution(&self) -> Self {
        let data_state = Arc::new(RwLock::new(self.data_state.read().unwrap().clone()));
        Self {
            resource_path: self.resource_path.clone(),
            name: self.name.clone(),
            kind: self.kind.clone(),
            function_inputs: self.function_inputs.clone(),
            function_outputs: self.function_outputs.clone(),
            runtime_prepared_epoch: self.runtime_prepared_epoch,
            data_state,
            registry: Arc::clone(&self.registry),
            schema_provider: self.schema_provider.clone(),
        }
    }

    /// Unified post-mutation compile entry.
    pub fn recompile(&self, scope: GraphRecompileScope) -> GraphRecompileResult {
        match scope {
            GraphRecompileScope::None => GraphRecompileResult::default(),
            GraphRecompileScope::RuntimePrepare => {
                self.propagate_schemas();
                self.infer_only_result()
            }
            GraphRecompileScope::InferOnly => self.infer_only_result(),
            GraphRecompileScope::Full => {
                self.propagate_schemas();
                let change_sets =
                    self.resolve_all_dynamic_pins_with_mode(PinResolveMode::Interactive);
                let mut result = self.infer_only_result();
                result.change_sets = change_sets;
                result
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
                let mut result = self.infer_only_result();
                result.change_sets = change_sets;
                result
            }
            GraphRecompileScope::TopologyEffects { seeds, mode } => {
                self.recompile_seeded(&seeds, mode)
            }
        }
    }

    fn infer_only_result(&self) -> GraphRecompileResult {
        self.infer_types()
            .map(|report| GraphRecompileResult {
                inferred: report.resolved,
                inference_warnings: report.warnings,
                ..Default::default()
            })
            .unwrap_or_else(|e| {
                crate::log::log_sys::warn!("graph type inference failed: {}", e);
                GraphRecompileResult::default()
            })
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
        let mut result = self.infer_only_result();
        result.change_sets = change_sets;
        result
    }

    /// 全图重编译：schema 传播 + 动态 pin 解析 + 类型推断。
    /// 从种子节点局部重编译（变量变更、局部拓扑变更）。
    /// 获取节点注册表的引用
    pub fn registry(&self) -> &Arc<NodeRegistry> {
        &self.registry
    }
}
