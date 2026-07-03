//! Graph 实现
//!
//! ❌ 不负责「定义」
//! ❌ 不负责「编译策略」
//! ❌ 不负责「执行调度」
//! ✅ 持有状态
//! ✅ 提供受控 mutation API

use super::{GraphDataState, GraphKind, GraphPosition};
use crate::graph::connection::Connection;
use crate::graph::node::OutputSchemaContext;
pub use crate::graph::node::SchemaProvider;
use crate::graph::node::{
    ColumnSchema, DataSchema, NodeDefinition, NodeId, NodeInstance, NodeInstanceParams,
    NodePosition, PinResolverContext,
};
use crate::graph::pin::{
    DataRole, PinDataTypeDefinition, PinDefinition, PinDirection, PinId, PinInstance, PinKind,
    PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::value::DataType;
use crate::graph::value::DataValue;
use crate::graph::{GraphId, TypeVarDefinition, TypeVarId};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// 动态 pin 解析模式
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PinResolveMode {
    /// 连线 / 断线 / 改参：允许移除过时的 schema 派生 pin
    Interactive,
    /// 打开 Tab 物化：resolver 无结果时保留已持久化的 pin
    Materialize,
}

/// 动态 pin 重建的变更集
#[derive(Debug, Clone, Default)]
pub struct PinChangeSet {
    pub node_id: NodeId,
    pub removed_pin_ids: Vec<PinId>,
    pub added_pins: Vec<PinInstance>,
    /// 同族 repeatable pin 重排索引后需同步到前端的 pin（如 C→B）
    pub updated_pins: Vec<PinInstance>,
    pub removed_connections: Vec<(PinId, PinId)>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FunctionSignaturePin {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub pin_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container_type: Option<String>,
}

/// Graph（运行时世界）
///
/// Graph 是唯一的运行时真实来源，管理：
/// - 所有 Node, Pin 实例 和连接关系
/// - 类型推断上下文
#[derive(Clone)]
pub struct GraphInstance {
    // 图 id
    pub id: GraphId,

    // 图 name
    pub name: String,

    // 类型
    pub kind: GraphKind,

    // 位置
    pub position: GraphPosition,

    // Function graph 对外签名。Event 始终为空。
    pub function_inputs: Vec<FunctionSignaturePin>,
    pub function_outputs: Vec<FunctionSignaturePin>,

    // 数据状态 (node, pin, connection)
    pub data_state: Arc<RwLock<GraphDataState>>,

    // 节点类型注册表（不持久化，需要在加载后重新设置）
    registry: Arc<NodeRegistry>,

    // 模式提供器（不持久化，运行时通过 ProjectState 注入）
    schema_provider: Option<SchemaProvider>,
}

// ============================================================================
// 持久化格式（Phase B）
//
// 磁盘格式与 `GraphRebuildSnapshot` 对齐：扁平的 `nodes[]`（pin 内联）+ 扁平的
// `connections[]`。静态 pin 的完整定义在加载后由 registry 经 `set_registry`
// 重新挂载；动态/可重复 pin 自带完整定义覆盖。运行期缓存
// （`pin_types` / `type_var_bindings` / `resolved_schema`）不落盘。
//
// 该自定义 serde 是 `GraphInstance` 唯一的磁盘序列化路径；前端始终通过
// `GraphInstanceDTO`（`From<&GraphInstance>`）获取数据，不经过此处。
// ============================================================================

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphDocSer<'a> {
    name: &'a str,
    kind: GraphKind,
    position: GraphPosition,
    #[serde(default, skip_serializing_if = "<[_]>::is_empty")]
    function_inputs: &'a [FunctionSignaturePin],
    #[serde(default, skip_serializing_if = "<[_]>::is_empty")]
    function_outputs: &'a [FunctionSignaturePin],
    nodes: Vec<GraphNodeSer<'a>>,
    connections: Vec<Connection>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphNodeSer<'a> {
    id: NodeId,
    node_type: &'a str,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    type_var_map: &'a HashMap<TypeVarId, TypeVarDefinition>,
    position: NodePosition,
    instance_params: &'a NodeInstanceParams,
    pins: Vec<&'a PinInstance>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphDocDe {
    #[serde(default)]
    id: Option<GraphId>,
    name: String,
    kind: GraphKind,
    #[serde(default)]
    position: GraphPosition,
    #[serde(default)]
    function_inputs: Vec<FunctionSignaturePin>,
    #[serde(default)]
    function_outputs: Vec<FunctionSignaturePin>,
    #[serde(default)]
    nodes: Vec<GraphNodeDe>,
    #[serde(default)]
    connections: Vec<Connection>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphNodeDe {
    id: NodeId,
    node_type: String,
    #[serde(default)]
    type_var_map: HashMap<TypeVarId, TypeVarDefinition>,
    #[serde(default)]
    position: NodePosition,
    #[serde(default)]
    instance_params: NodeInstanceParams,
    #[serde(default)]
    pins: Vec<PinInstance>,
}

impl Serialize for GraphInstance {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let data_state = self
            .data_state
            .read()
            .map_err(|_| serde::ser::Error::custom("data_state lock poisoned"))?;

        // 节点按 id 排序，保证落盘 JSON 稳定（避免 HashMap 迭代顺序导致的伪 diff）
        let mut nodes: Vec<&NodeInstance> = data_state.nodes.values().collect();
        nodes.sort_by(|a, b| a.id.to_string().cmp(&b.id.to_string()));

        let node_ser: Vec<GraphNodeSer> = nodes
            .iter()
            .map(|node| {
                let pins: Vec<&PinInstance> = node
                    .pin_ids
                    .iter()
                    .filter_map(|pin_id| data_state.pins.get(pin_id))
                    .collect();
                GraphNodeSer {
                    id: node.id,
                    node_type: &node.definition.node_type,
                    type_var_map: &node.type_var_map,
                    position: node.position.clone(),
                    instance_params: &node.instance_params,
                    pins,
                }
            })
            .collect();

        let mut connections = data_state.connections.all_connections();
        connections.sort_by(|a, b| {
            (a.from_pin.to_string(), a.to_pin.to_string())
                .cmp(&(b.from_pin.to_string(), b.to_pin.to_string()))
        });

        GraphDocSer {
            name: &self.name,
            kind: self.kind.clone(),
            position: self.position.clone(),
            function_inputs: &self.function_inputs,
            function_outputs: &self.function_outputs,
            nodes: node_ser,
            connections,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for GraphInstance {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let doc = GraphDocDe::deserialize(deserializer)?;
        Ok(Self::from_persisted_parts(
            doc.id.unwrap_or_else(GraphId::new),
            doc.name,
            doc.kind,
            doc.position,
            doc.function_inputs,
            doc.function_outputs,
            doc.nodes,
            doc.connections,
        ))
    }
}

impl GraphInstance {
    pub fn resolve_variable_nodes(&self, variables: &HashMap<String, (String, DataType)>) {
        let mut data_state = self.data_state.write().unwrap();
        let variable_nodes: Vec<_> = data_state
            .nodes
            .values()
            .filter_map(|node| {
                node.instance_params
                    .variable_id()
                    .and_then(|variable_id| variables.get(variable_id))
                    .map(|(name, data_type)| (node.id, name.clone(), data_type.clone()))
            })
            .collect();

        for (node_id, variable_name, data_type) in variable_nodes {
            let Some(node) = data_state.nodes.get(&node_id) else {
                continue;
            };
            let pin_ids = node.pin_ids.clone();
            for pin_id in pin_ids {
                let Some(pin) = data_state.pins.get_mut(&pin_id) else {
                    continue;
                };
                if pin.definition.kind != PinKind::Data {
                    continue;
                }
                pin.definition.name = variable_name.clone();
                data_state.pin_types.insert(pin_id, data_type.clone());
            }
        }
    }

    pub fn resolve_dataframe_nodes(&self, dataframes: &HashMap<String, String>) {
        let mut data_state = self.data_state.write().unwrap();
        let dataframe_nodes: Vec<_> = data_state
            .nodes
            .values()
            .filter(|node| node.definition.node_type == "Data:Get DataFrame")
            .filter_map(|node| {
                node.instance_params
                    .dataframe_id()
                    .and_then(|dataframe_id| dataframes.get(dataframe_id))
                    .map(|name| (node.id, name.clone()))
            })
            .collect();

        for (node_id, dataframe_label) in dataframe_nodes {
            let Some(node) = data_state.nodes.get(&node_id) else {
                continue;
            };
            let pin_ids = node.pin_ids.clone();
            for pin_id in pin_ids {
                let Some(pin) = data_state.pins.get_mut(&pin_id) else {
                    continue;
                };
                if pin.definition.kind == PinKind::Data {
                    pin.definition.name = dataframe_label.clone();
                    pin.definition.data_type =
                        Some(PinDataTypeDefinition::concrete(DataType::DataFrame));
                    data_state.pin_types.insert(pin_id, DataType::DataFrame);
                }
            }
        }
    }

    /// 从持久化的扁平节点 + 连接重建 `GraphInstance`（无 registry，
    /// 静态 pin 的完整定义随后由 `set_registry` 重挂）。
    fn from_persisted_parts(
        id: GraphId,
        name: String,
        kind: GraphKind,
        position: GraphPosition,
        function_inputs: Vec<FunctionSignaturePin>,
        function_outputs: Vec<FunctionSignaturePin>,
        nodes: Vec<GraphNodeDe>,
        connections: Vec<Connection>,
    ) -> Self {
        let mut data_state = GraphDataState::default();

        for node in nodes {
            let node_id = node.id;
            let definition = Arc::new(NodeDefinition::placeholder(node.node_type));
            let pin_ids: Vec<PinId> = node.pins.iter().map(|pin| pin.id).collect();

            for pin in node.pins {
                data_state.connections.register_pin(pin.id, node_id);
                data_state.pins.insert(pin.id, pin);
            }

            data_state.add_node(NodeInstance {
                id: node_id,
                definition,
                type_var_map: node.type_var_map,
                position: node.position,
                instance_params: node.instance_params,
                pin_ids,
            });
        }

        for connection in connections {
            data_state
                .connections
                .connect(connection.from_pin, connection.to_pin);
        }

        Self {
            id,
            name,
            kind,
            position,
            function_inputs,
            function_outputs,
            data_state: Arc::new(RwLock::new(data_state)),
            registry: Default::default(),
            schema_provider: None,
        }
    }
}

impl std::fmt::Debug for GraphInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GraphInstance")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("kind", &self.kind)
            .field("position", &self.position)
            .field("function_inputs", &self.function_inputs)
            .field("function_outputs", &self.function_outputs)
            .field("data_state", &self.data_state)
            .finish_non_exhaustive()
    }
}

/// 创建和清理
impl GraphInstance {
    pub fn new(name: impl Into<String>, kind: GraphKind, registry: Arc<NodeRegistry>) -> Self {
        Self {
            id: GraphId::new(),
            name: name.into(),
            position: GraphPosition::default(),
            kind,
            function_inputs: Vec::new(),
            function_outputs: Vec::new(),
            data_state: Default::default(),
            registry,
            schema_provider: None,
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

    /// 全图重编译：schema 传播 + 动态 pin 解析 + 类型推断。
    pub fn compile_graph(&self) {
        self.propagate_schemas();
        let _ = self.resolve_all_dynamic_pins_with_mode(PinResolveMode::Interactive);
        let _ = self
            .infer_types()
            .map_err(|e| crate::log::log_sys::warn!("graph type inference failed: {}", e));
    }

    /// 从种子节点局部重编译（变量变更、局部拓扑变更）。
    pub fn compile_graph_from_seeds(&self, seeds: &[NodeId]) {
        if seeds.is_empty() {
            self.compile_graph();
            return;
        }
        self.propagate_schemas_from(seeds);
        let _ = self
            .infer_types()
            .map_err(|e| crate::log::log_sys::warn!("graph type inference failed: {}", e));

        let mut to_resolve: Vec<NodeId> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for &nid in seeds {
            if seen.insert(nid) {
                to_resolve.push(nid);
            }
            for d in self.get_downstream_resolve_nodes(nid) {
                if seen.insert(d) {
                    to_resolve.push(d);
                }
            }
        }
        for node_id in to_resolve {
            let _ = self.resolve_dynamic_pins(node_id);
        }
    }

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
    pub fn update_instance_params(
        &self,
        node_id: NodeId,
        params: NodeInstanceParams,
    ) -> Result<Vec<PinChangeSet>, String> {
        {
            let mut data_state = self.data_state.write().unwrap();
            let node = data_state
                .nodes
                .get_mut(&node_id)
                .ok_or_else(|| format!("Node {:?} not found", node_id))?;
            node.instance_params = params;
        }
        self.propagate_schemas();
        let mut change_sets = Vec::new();
        if let Some(cs) = self.resolve_dynamic_pins(node_id)? {
            change_sets.push(cs);
        }
        Ok(change_sets)
    }

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

    pub fn remove_node(&self, node_id: NodeId) -> Result<(), String> {
        let neighbors = self.remove_node_raw(node_id)?;
        let _ = self.infer_types();
        for nid in neighbors {
            let _ = self.resolve_dynamic_pins(nid);
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
        let data_state = self.data_state.read().unwrap();
        data_state.nodes.get(&node_id).cloned()
    }

    pub fn get_all_nodes(&self) -> Vec<NodeInstance> {
        let data_state = self.data_state.read().unwrap();
        data_state.nodes.values().cloned().collect()
    }
}

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

/// `connect_topology` 的结果：仅描述拓扑变更，不含 schema/infer/动态 pin 副作用
pub struct ConnectTopology {
    /// 已确定方向的源 pin（output 端）
    pub from_pin: PinId,
    /// 已确定方向的目标 pin（input 端）
    pub to_pin: PinId,
    /// 被自动断开的旧连接（exec 单出边、input 单入边）
    pub auto_disconnected: Vec<(PinId, PinId)>,
    /// 受此次拓扑变更影响的节点（连接两端 + 被自动断开端），作为副作用传播的种子
    pub seed_nodes: Vec<NodeId>,
}

/// 连接管理
impl GraphInstance {
    /// 仅修改连接拓扑（校验 → 自动断开 → 建立连接），不触发 schema/infer/动态 pin。
    ///
    /// 供单次 `connect` 与批量粘贴复用：批量场景下多条连接共享一次副作用收尾
    /// （见 `finish_graph_effects`），避免每条连接都做全图传播。
    pub fn connect_topology(&self, pin_a: PinId, pin_b: PinId) -> Result<ConnectTopology, String> {
        use crate::graph::connection::connection_validator::validate_connection;

        let mut auto_disconnected: Vec<(PinId, PinId)> = Vec::new();
        let mut seed_nodes: Vec<NodeId> = Vec::new();
        let from_pin;
        let to_pin;
        {
            let data_state = self.data_state.write().unwrap();
            let type_system = self.registry.type_system_snapshot();

            let validated = validate_connection(&data_state, pin_a, pin_b, &type_system)?;
            from_pin = validated.from_pin;
            to_pin = validated.to_pin;

            if let Some(p) = data_state.pins.get(&from_pin) {
                seed_nodes.push(p.node_id);
            }
            if let Some(p) = data_state.pins.get(&to_pin) {
                seed_nodes.push(p.node_id);
            }

            // Exec output: enforce single outgoing connection
            let is_exec = data_state
                .pins
                .get(&from_pin)
                .map(|p| p.definition.kind == PinKind::Exec)
                .unwrap_or(false);

            if is_exec {
                let old_targets = data_state.connections.get_downstream(from_pin);
                for old_to in old_targets {
                    if let Some(node_id) = data_state.pins.get(&old_to).map(|p| p.node_id) {
                        seed_nodes.push(node_id);
                    }
                    data_state.connections.disconnect(from_pin, old_to);
                    auto_disconnected.push((from_pin, old_to));
                }
            }

            // Input pin auto-disconnect (existing behavior)
            if let Some(old_from) = data_state.connections.get_upstream(to_pin) {
                if let Some(node_id) = data_state.pins.get(&old_from).map(|p| p.node_id) {
                    seed_nodes.push(node_id);
                }
            }
            if let Some(pair) = data_state.connections.connect(from_pin, to_pin) {
                auto_disconnected.push(pair);
            }
        }

        Ok(ConnectTopology {
            from_pin,
            to_pin,
            auto_disconnected,
            seed_nodes,
        })
    }

    /// 拓扑变更后的统一副作用入口：增量传播 schema → 运行类型推断 →
    /// 重建受影响节点（种子 + 其下游消费者）的动态 pin。
    ///
    /// 所有连接/断开操作（含批量粘贴）都通过此入口收尾，保证「一次拓扑变更
    /// 对应一次副作用」。批量场景累积所有种子后只调用一次。
    pub fn finish_graph_effects(
        &self,
        seed_nodes: &[NodeId],
    ) -> (Vec<PinChangeSet>, Vec<(PinId, DataType)>) {
        self.finish_graph_effects_with_mode(seed_nodes, PinResolveMode::Interactive)
    }

    /// 指定动态 pin 解析模式的后缀（merge undo 使用 Materialize，避免误删已恢复 pin）。
    pub fn finish_graph_effects_with_mode(
        &self,
        seed_nodes: &[NodeId],
        mode: PinResolveMode,
    ) -> (Vec<PinChangeSet>, Vec<(PinId, DataType)>) {
        self.propagate_schemas_from(seed_nodes);
        let inferred = self
            .infer_types()
            .map_err(|e| crate::log::log_sys::warn!("graph type inference failed: {}", e))
            .unwrap_or_default();

        let mut to_resolve: Vec<NodeId> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for &nid in seed_nodes {
            if seen.insert(nid) {
                to_resolve.push(nid);
            }
            for d in self.get_downstream_resolve_nodes(nid) {
                if seen.insert(d) {
                    to_resolve.push(d);
                }
            }
        }

        let mut change_sets = Vec::new();
        for node_id in to_resolve {
            if let Ok(Some(cs)) = self.resolve_dynamic_pins_with_mode(node_id, mode) {
                change_sets.push(cs);
            }
        }

        (change_sets, inferred)
    }

    /// 连接两个 pin（无序），后端自动验证方向、类型兼容性等
    ///
    /// 验证链：存在性 → 同 Pin → 同节点 → 方向推断 → Kind 兼容
    ///        → 重复连接 → 类型兼容 → 环路检测
    ///
    /// Exec output pin 限制为最多 1 条出边（自动断开旧连接），
    /// Data output pin 可以有多条出边。
    ///
    /// 返回 (已确定方向的 from/to, 被自动断开的旧连接列表, 动态 pin 变更集, 推断出的 pin 类型)
    pub fn connect(
        &self,
        pin_a: PinId,
        pin_b: PinId,
    ) -> Result<
        (
            PinId,
            PinId,
            Vec<(PinId, PinId)>,
            Vec<PinChangeSet>,
            Vec<(PinId, DataType)>,
        ),
        String,
    > {
        let topo = self.connect_topology(pin_a, pin_b)?;
        let (change_sets, inferred) = self.finish_graph_effects(&topo.seed_nodes);
        Ok((
            topo.from_pin,
            topo.to_pin,
            topo.auto_disconnected,
            change_sets,
            inferred,
        ))
    }

    /// 返回下游连接的 pin 列表，过滤掉不存在的 pin（孤儿连接，如导入后节点/引脚已删除）
    pub fn get_downstream_by_pin_id(&self, pin_id: PinId) -> Vec<PinId> {
        let data_state = self.data_state.read().unwrap();
        data_state
            .connections
            .get_downstream(pin_id)
            .into_iter()
            .filter(|id| data_state.pins.contains_key(id))
            .collect()
    }

    /// 返回上游连接的 pin，若不存在则返回 None（过滤孤儿连接）
    pub fn get_upstream_by_pin_id(&self, pin_id: PinId) -> Option<PinId> {
        let data_state = self.data_state.read().unwrap();
        data_state
            .connections
            .get_upstream(pin_id)
            .filter(|id| data_state.pins.contains_key(id))
    }

    /// 断开连接，自动运行类型推断和动态 pin 重建
    ///
    /// 返回 (动态 pin 变更集, 推断出的 pin 类型)
    pub fn disconnect(
        &self,
        from_pin: PinId,
        to_pin: PinId,
    ) -> (Vec<PinChangeSet>, Vec<(PinId, DataType)>) {
        let mut seed_nodes: Vec<NodeId> = Vec::new();
        {
            let data_state = self.data_state.write().unwrap();
            if let Some(p) = data_state.pins.get(&from_pin) {
                seed_nodes.push(p.node_id);
            }
            if let Some(p) = data_state.pins.get(&to_pin) {
                seed_nodes.push(p.node_id);
            }
            data_state.connections.disconnect(from_pin, to_pin);
        }

        self.finish_graph_effects(&seed_nodes)
    }

    /// 断开指定 Pin 的所有连接（输入和输出）
    ///
    /// 返回被删除的连接对、disconnect undo patch、动态 pin 变更集、推断类型。
    pub fn disconnect_pin(
        &self,
        pin_id: PinId,
    ) -> (
        Vec<(PinId, PinId)>,
        crate::schema::GraphUndoPatch,
        Vec<PinChangeSet>,
        Vec<(PinId, DataType)>,
    ) {
        let mut removed_connections = Vec::new();
        let mut seed_nodes: Vec<NodeId> = Vec::new();
        {
            let data_state = self.data_state.read().unwrap();
            if let Some(p) = data_state.pins.get(&pin_id) {
                seed_nodes.push(p.node_id);
            }
            for to_pin in data_state.connections.get_downstream(pin_id) {
                removed_connections.push((pin_id, to_pin));
                if let Some(p) = data_state.pins.get(&to_pin) {
                    seed_nodes.push(p.node_id);
                }
            }
            if let Some(from_pin) = data_state.connections.get_upstream(pin_id) {
                removed_connections.push((from_pin, pin_id));
                if let Some(p) = data_state.pins.get(&from_pin) {
                    seed_nodes.push(p.node_id);
                }
            }
        }

        let undo_patch = self.capture_disconnect_undo_patch(&seed_nodes, &removed_connections);

        {
            let data_state = self.data_state.write().unwrap();
            data_state.connections.disconnect_all(pin_id);
        }

        let (change_sets, inferred) = self.finish_graph_effects(&seed_nodes);
        (removed_connections, undo_patch, change_sets, inferred)
    }

    pub fn all_connections(&self) -> Vec<Connection> {
        let data_state = self.data_state.write().unwrap();
        data_state.connections.all_connections()
    }
}

/// 类型推断
impl GraphInstance {
    /// 运行类型推断
    ///
    /// 这个方法会：
    /// 1. 注册所有节点的类型变量
    /// 2. 注册所有 Pin 的类型
    /// 3. 根据连接关系推断类型
    /// 4. 将推断结果写回 GraphDataState
    pub fn infer_types(&self) -> Result<Vec<(PinId, DataType)>, String> {
        crate::graph::infer::infer_graph(self)
    }
}

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

        // 重新运行类型推断
        let _ = self.infer_types();

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

    /// 传播 schema：按拓扑序计算并填充各 output pin 的 resolved_schema
    fn is_model_struct_type_key(type_key: &str) -> bool {
        matches!(type_key, "OLSModel" | "LogitModel" | "ProbitModel")
    }

    /// 将计算出的 schema 写入节点的 output data pin（DataFrame / 模型 Struct）
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
    fn propagate_schemas_from(&self, seeds: &[NodeId]) {
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
        self.propagate_schemas();
        let change_sets = self.resolve_all_dynamic_pins_with_mode(PinResolveMode::Materialize);
        let inferred = self
            .infer_types()
            .map_err(|e| crate::log::log_sys::warn!("graph type inference failed: {}", e))
            .unwrap_or_default();
        (change_sets, inferred)
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
    fn get_downstream_resolve_nodes(&self, node_id: NodeId) -> Vec<NodeId> {
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
}

/// 在 `pin_ids` 中插入新 repeatable 成员的索引：若有同族 pin 则插在最后一个之后；否则插在本槽位在定义中的起始位置（避免 min_count=0 时误插到节点底部）。
fn pin_repeatable_insert_index(
    node: &NodeInstance,
    data_state: &GraphDataState,
    definition: &NodeDefinition,
    slot_index: usize,
    template_role: &PinRole,
) -> usize {
    if let Some(pos) = node.pin_ids.iter().rposition(|pid| {
        data_state
            .pins
            .get(pid)
            .map(|p| p.definition.role.matches_family(template_role))
            .unwrap_or(false)
    }) {
        return pos + 1;
    }

    let mut idx = 0;
    for (si, slot) in definition.pin_slots.iter().enumerate() {
        if si == slot_index {
            return idx;
        }
        match slot {
            PinSlot::Fixed { .. } => {
                if idx < node.pin_ids.len() {
                    idx += 1;
                }
            }
            PinSlot::Repeatable { .. } => {
                let Some(tmpl) = slot.repeatable_template_role() else {
                    continue;
                };
                while idx < node.pin_ids.len() {
                    let pid = node.pin_ids[idx];
                    let m = data_state
                        .pins
                        .get(&pid)
                        .map(|p| p.definition.role.matches_family(tmpl))
                        .unwrap_or(false);
                    if m {
                        idx += 1;
                    } else {
                        break;
                    }
                }
            }
            PinSlot::DerivedFromInput { .. } => {}
        }
    }
    idx
}

/// Repeatable Pin 增删
impl GraphInstance {
    /// 向节点的某个 Repeatable 槽位追加一个新 pin
    ///
    /// `slot_index` 是节点定义 `pin_slots` 数组中的索引，必须指向一个 Repeatable 槽位。
    /// 返回 (新增 pin 的 PinChangeSet, 下游 resolve 产生的 PinChangeSet 列表)
    pub fn add_repeatable_pin(
        &self,
        node_id: NodeId,
        slot_index: usize,
    ) -> Result<(PinChangeSet, Vec<PinChangeSet>), String> {
        let definition;
        {
            let data_state = self.data_state.read().unwrap();
            let node = data_state
                .nodes
                .get(&node_id)
                .ok_or_else(|| format!("Node {:?} not found", node_id))?;
            definition = node.definition.clone();
        }

        let slot = definition
            .pin_slots
            .get(slot_index)
            .ok_or_else(|| format!("Slot index {} out of range", slot_index))?;

        let template_role = match slot {
            PinSlot::Repeatable { template, .. } => &template.role,
            _ => {
                return Err(format!(
                    "Slot index {} is not a Repeatable slot",
                    slot_index
                ));
            }
        };

        let family_pins = self.get_pin_instances_by_pin_role_family(node_id, template_role);
        let current_count = family_pins.len();

        if let PinSlot::Repeatable { max_count, .. } = slot {
            if let Some(max) = max_count {
                if current_count >= *max {
                    return Err(format!("Repeatable slot already at max count ({})", max));
                }
            }
        }

        let new_index = current_count;
        let pin_def = slot
            .generate_pin_at_index(new_index)
            .ok_or_else(|| "Failed to generate pin definition".to_string())?;

        let order = {
            let data_state = self.data_state.read().unwrap();
            let node = data_state.nodes.get(&node_id).unwrap();
            node.pin_ids.len() as i32
        };

        let new_pin = PinInstance::from_definition(&pin_def, node_id, order);
        let new_pin_id = new_pin.id;

        {
            let mut data_state = self.data_state.write().unwrap();
            data_state.connections.register_pin(new_pin_id, node_id);
            data_state.pins.insert(new_pin_id, new_pin.clone());

            let insert_pos = data_state
                .nodes
                .get(&node_id)
                .map(|node| {
                    pin_repeatable_insert_index(
                        node,
                        &data_state,
                        definition.as_ref(),
                        slot_index,
                        template_role,
                    )
                })
                .unwrap_or(0);

            if let Some(node) = data_state.nodes.get_mut(&node_id) {
                let pos = insert_pos.min(node.pin_ids.len());
                node.pin_ids.insert(pos, new_pin_id);
            }
        }

        self.propagate_schemas();
        let resolve_sets = self.resolve_all_dynamic_pins();
        let _ = self.infer_types();

        let main_set = PinChangeSet {
            node_id,
            removed_pin_ids: vec![],
            added_pins: vec![new_pin],
            updated_pins: vec![],
            removed_connections: vec![],
        };
        Ok((main_set, resolve_sets))
    }

    /// 从节点移除一个 Repeatable 槽位的 pin
    ///
    /// 验证 pin 属于某个 Repeatable 槽位且当前数量 > min_count，
    /// 然后断开连接、移除 pin，并重新索引剩余的同族 pin。
    /// 返回 (移除信息的 PinChangeSet, 被移除 pin 在槽位中的索引, 下游 resolve 产生的 PinChangeSet 列表)
    pub fn remove_repeatable_pin(
        &self,
        node_id: NodeId,
        pin_id: PinId,
    ) -> Result<(PinChangeSet, usize, Vec<PinChangeSet>), String> {
        let definition;
        let pin_role;
        {
            let data_state = self.data_state.read().unwrap();
            let node = data_state
                .nodes
                .get(&node_id)
                .ok_or_else(|| format!("Node {:?} not found", node_id))?;
            definition = node.definition.clone();

            let pin = data_state
                .pins
                .get(&pin_id)
                .ok_or_else(|| format!("Pin {:?} not found", pin_id))?;
            if pin.node_id != node_id {
                return Err("Pin does not belong to the specified node".to_string());
            }
            pin_role = pin.definition.role.clone();
        }

        let (slot_index, slot) = definition
            .pin_slots
            .iter()
            .enumerate()
            .find(|(_, s)| {
                s.repeatable_template_role()
                    .map(|tmpl_role| pin_role.matches_family(tmpl_role))
                    .unwrap_or(false)
            })
            .ok_or_else(|| "Pin does not belong to any Repeatable slot".to_string())?;

        let template_role = slot.repeatable_template_role().unwrap();
        let min_count = slot.repeatable_min_count().unwrap_or(0);

        let family_pins = self.get_pin_instances_by_pin_role_family(node_id, template_role);
        let current_count = family_pins.len();

        if current_count <= min_count {
            return Err(format!(
                "Cannot remove pin: already at minimum count ({})",
                min_count
            ));
        }

        let pin_index_in_family = family_pins
            .iter()
            .position(|p| p.id == pin_id)
            .ok_or_else(|| "Pin not found in family".to_string())?;

        // Collect connections that will be removed
        let mut removed_connections = Vec::new();
        {
            let data_state = self.data_state.read().unwrap();
            let downstream = data_state.connections.get_downstream(pin_id);
            for to_pin in &downstream {
                removed_connections.push((pin_id, *to_pin));
            }
            if let Some(from_pin) = data_state.connections.get_upstream(pin_id) {
                removed_connections.push((from_pin, pin_id));
            }
        }

        // Remove the pin
        {
            let mut data_state = self.data_state.write().unwrap();
            data_state.connections.disconnect_all(pin_id);
            data_state.pins.remove(&pin_id);
            data_state.pin_types.remove(&pin_id);
            if let Some(node) = data_state.nodes.get_mut(&node_id) {
                node.pin_ids.retain(|id| *id != pin_id);
            }
        }

        // Re-index remaining pins in the same family
        let updated_pins = self.reindex_repeatable_pins(node_id, slot_index)?;

        self.propagate_schemas();
        let resolve_sets = self.resolve_all_dynamic_pins();
        let _ = self.infer_types();

        let main_set = PinChangeSet {
            node_id,
            removed_pin_ids: vec![pin_id],
            added_pins: vec![],
            updated_pins,
            removed_connections,
        };
        Ok((main_set, pin_index_in_family, resolve_sets))
    }

    /// Re-index all pins belonging to a Repeatable slot so their roles and names
    /// are contiguous (Operands(0), Operands(1), ...; A, B, C, ...).
    /// Returns updated pin instances for frontend sync.
    fn reindex_repeatable_pins(
        &self,
        node_id: NodeId,
        slot_index: usize,
    ) -> Result<Vec<PinInstance>, String> {
        let definition;
        {
            let data_state = self.data_state.read().unwrap();
            let node = data_state
                .nodes
                .get(&node_id)
                .ok_or_else(|| format!("Node {:?} not found", node_id))?;
            definition = node.definition.clone();
        }

        let slot = definition
            .pin_slots
            .get(slot_index)
            .ok_or_else(|| format!("Slot index {} out of range", slot_index))?;

        let template_role = match slot.repeatable_template_role() {
            Some(r) => r.clone(),
            None => return Ok(Vec::new()),
        };

        let family_pins = self.get_pin_instances_by_pin_role_family(node_id, &template_role);

        let mut data_state = self.data_state.write().unwrap();
        for (i, fpin) in family_pins.iter().enumerate() {
            if let Some(pin) = data_state.pins.get_mut(&fpin.id) {
                if let Some(pin_def) = slot.generate_pin_at_index(i) {
                    pin.definition.role = pin_def.role;
                    pin.definition.name = pin_def.name;
                }
            }
        }

        Ok(family_pins
            .iter()
            .filter_map(|fpin| data_state.pins.get(&fpin.id).cloned())
            .collect())
    }
}
