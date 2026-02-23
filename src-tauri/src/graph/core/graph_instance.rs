//! Graph 实现
//!
//! ❌ 不负责「定义」
//! ❌ 不负责「编译策略」
//! ❌ 不负责「执行调度」
//! ✅ 持有状态
//! ✅ 提供受控 mutation API

use super::{GraphDataState, GraphKind, GraphPosition};
use crate::graph::connection::Connection;
use crate::graph::node::{DataSchema, NodeId, NodeInstance, NodeInstanceParams, PinResolverContext};
use crate::graph::pin::{PinId, PinInstance, PinKind, PinRole, PinDirection, PinSlot};
use crate::graph::register::NodeRegistry;
use crate::graph::value::DataValue;
use crate::graph::{DataType, GraphId};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

/// 模式提供器：通过 dataframe_id 查询 DataFrame 的列结构
pub type SchemaProvider = Arc<dyn Fn(&str) -> Option<DataSchema> + Send + Sync>;

/// 动态 pin 重建的变更集
#[derive(Debug, Clone, Default)]
pub struct PinChangeSet {
    pub node_id: NodeId,
    pub removed_pin_ids: Vec<PinId>,
    pub added_pins: Vec<PinInstance>,
    pub removed_connections: Vec<(PinId, PinId)>,
}

/// Graph（运行时世界）
///
/// Graph 是唯一的运行时真实来源，管理：
/// - 所有 Node, Pin 实例 和连接关系
/// - 类型推断上下文
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphInstance {
    // 图 id
    pub id: GraphId,

    // 图 name
    pub name: String,

    // 类型
    pub kind: GraphKind,

    // 位置
    pub position: GraphPosition,


    // 数据状态 (node, pin, connection)
    pub data_state: Arc<RwLock<GraphDataState>>,

    // 节点类型注册表（序列化时跳过，需要在加载后重新设置）
    #[serde(skip)]
    registry: Arc<NodeRegistry>,

    // 模式提供器（序列化时跳过，运行时通过 ProjectState 注入）
    #[serde(skip)]
    schema_provider: Option<SchemaProvider>,
}

impl std::fmt::Debug for GraphInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GraphInstance")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("kind", &self.kind)
            .field("position", &self.position)
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
        }
        self.registry = registry;
    }

    /// 设置模式提供器（用于 pin_resolver 查询 DataFrame 列结构）
    pub fn set_schema_provider(&mut self, provider: SchemaProvider) {
        self.schema_provider = Some(provider);
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
            let definition = self
                .registry
                .get(&node_snap.node_type)
                .ok_or_else(|| format!("Node type '{}' not found in registry", node_snap.node_type))?;

            let result = NodeInstance::from_definition(definition.clone())
                .map_err(|e| format!("Failed to create node '{}': {}", node_snap.node_type, e))?;

            let target_node_id = parse_node_id(&node_snap.id)?;

            let mut node = result.node;
            node.id = target_node_id;
            node.position = crate::graph::NodePosition { x: node_snap.x, y: node_snap.y };
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

            if let Some(dt) = node_snap.params.as_ref()
                .and_then(|p| p.variable_type())
                .and_then(|vt| vt.parse::<DataType>().ok())
            {
                for pin in &pins {
                    if pin.definition.kind == PinKind::Data {
                        data_state.pin_types.insert(pin.id, dt.clone());
                    }
                }
            }

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
        let node_id = self.create_node_raw(node_type, x, y, params)?;
        let _ = self.infer_types();
        Ok(node_id)
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

        // 根据 instance_params 中的类型信息设置数据 pin 的具体类型
        let variable_data_type = params
            .as_ref()
            .and_then(|p| p.variable_type())
            .and_then(|vt| vt.parse::<DataType>().ok());

        {
            let mut data_state = self.data_state.write().unwrap();
            if let Some(ref dt) = variable_data_type {
                for pin in &result.pins {
                    if pin.definition.kind == PinKind::Data {
                        data_state.pin_types.insert(pin.id, dt.clone());
                    }
                }
            }
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

        let variable_data_type = params
            .as_ref()
            .and_then(|p| p.variable_type())
            .and_then(|vt| vt.parse::<DataType>().ok());

        {
            let mut data_state = self.data_state.write().unwrap();
            if let Some(ref dt) = variable_data_type {
                for pin in &result.pins {
                    if pin.definition.kind == PinKind::Data {
                        data_state.pin_types.insert(pin.id, dt.clone());
                    }
                }
            }
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

        let variable_data_type = params
            .as_ref()
            .and_then(|p| p.variable_type())
            .and_then(|vt| vt.parse::<DataType>().ok());

        {
            let mut data_state = self.data_state.write().unwrap();
            if let Some(ref dt) = variable_data_type {
                for pin in &result.pins {
                    if pin.definition.kind == PinKind::Data {
                        data_state.pin_types.insert(pin.id, dt.clone());
                    }
                }
            }
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
    pub fn remove_node_raw(&self, node_id: NodeId) -> Result<std::collections::HashSet<NodeId>, String> {
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

            data_state.connections.remove_node(node_id);

            for pin in pins {
                data_state.pins.remove(&pin.id);
            }
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

/// 连接管理
impl GraphInstance {
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
    ) -> Result<(PinId, PinId, Vec<(PinId, PinId)>, Vec<PinChangeSet>, Vec<(PinId, DataType)>), String> {
        use crate::graph::connection::connection_validator::validate_connection;
        use crate::graph::pin::PinKind;

        let from_node_id;
        let to_node_id;
        let mut auto_disconnected: Vec<(PinId, PinId)> = Vec::new();
        let mut auto_disconnected_node_ids: Vec<NodeId> = Vec::new();
        let from_pin;
        let to_pin;
        {
            let data_state = self.data_state.write().unwrap();

            let validated = validate_connection(&data_state, pin_a, pin_b)?;
            from_pin = validated.from_pin;
            to_pin = validated.to_pin;

            from_node_id = data_state.pins.get(&from_pin).map(|p| p.node_id);
            to_node_id = data_state.pins.get(&to_pin).map(|p| p.node_id);

            // Exec output: enforce single outgoing connection
            let is_exec = data_state.pins.get(&from_pin)
                .map(|p| p.definition.kind == PinKind::Exec)
                .unwrap_or(false);

            if is_exec {
                let old_targets = data_state.connections.get_downstream(from_pin);
                for old_to in old_targets {
                    if let Some(node_id) = data_state.pins.get(&old_to).map(|p| p.node_id) {
                        auto_disconnected_node_ids.push(node_id);
                    }
                    data_state.connections.disconnect(from_pin, old_to);
                    auto_disconnected.push((from_pin, old_to));
                }
            }

            // Input pin auto-disconnect (existing behavior)
            if let Some(old_from) = data_state.connections.get_upstream(to_pin) {
                if let Some(node_id) = data_state.pins.get(&old_from).map(|p| p.node_id) {
                    auto_disconnected_node_ids.push(node_id);
                }
            }
            if let Some(pair) = data_state.connections.connect(from_pin, to_pin) {
                auto_disconnected.push(pair);
            }
        }

        let inferred = self.infer_types().unwrap_or_default();

        let mut change_sets = Vec::new();
        let mut resolved = std::collections::HashSet::new();
        for node_id in [to_node_id, from_node_id].into_iter().flatten()
            .chain(auto_disconnected_node_ids.into_iter())
        {
            if resolved.insert(node_id) {
                if let Some(cs) = self.resolve_dynamic_pins(node_id)? {
                    change_sets.push(cs);
                }
            }
        }

        Ok((from_pin, to_pin, auto_disconnected, change_sets, inferred))
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
    pub fn disconnect(&self, from_pin: PinId, to_pin: PinId) -> (Vec<PinChangeSet>, Vec<(PinId, DataType)>) {
        let from_node_id;
        let to_node_id;
        {
            let data_state = self.data_state.write().unwrap();
            from_node_id = data_state.pins.get(&from_pin).map(|p| p.node_id);
            to_node_id = data_state.pins.get(&to_pin).map(|p| p.node_id);
            data_state.connections.disconnect(from_pin, to_pin);
        }

        let inferred = self.infer_types().unwrap_or_default();

        let mut change_sets = Vec::new();
        for node_id in [from_node_id, to_node_id].into_iter().flatten() {
            if let Ok(Some(cs)) = self.resolve_dynamic_pins(node_id) {
                change_sets.push(cs);
            }
        }
        (change_sets, inferred)
    }

    /// 断开指定 Pin 的所有连接（输入和输出）
    ///
    /// 返回 (被删除的连接对列表, 动态 pin 变更集, 推断出的 pin 类型)
    pub fn disconnect_pin(&self, pin_id: PinId) -> (Vec<(PinId, PinId)>, Vec<PinChangeSet>, Vec<(PinId, DataType)>) {
        let mut removed_connections = Vec::new();
        let mut affected_node_ids = std::collections::HashSet::new();
        {
            let data_state = self.data_state.write().unwrap();
            if let Some(p) = data_state.pins.get(&pin_id) {
                affected_node_ids.insert(p.node_id);
            }
            for to_pin in data_state.connections.get_downstream(pin_id) {
                if let Some(p) = data_state.pins.get(&to_pin) {
                    affected_node_ids.insert(p.node_id);
                }
                removed_connections.push((pin_id, to_pin));
            }
            if let Some(from_pin) = data_state.connections.get_upstream(pin_id) {
                if let Some(p) = data_state.pins.get(&from_pin) {
                    affected_node_ids.insert(p.node_id);
                }
                removed_connections.push((from_pin, pin_id));
            }
            data_state.connections.disconnect_all(pin_id);
        }

        let inferred = self.infer_types().unwrap_or_default();

        let mut change_sets = Vec::new();
        for node_id in affected_node_ids {
            if let Ok(Some(cs)) = self.resolve_dynamic_pins(node_id) {
                change_sets.push(cs);
            }
        }
        (removed_connections, change_sets, inferred)
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
        let (definition, instance_params, current_pin_ids);
        {
            let data_state = self.data_state.read().unwrap();
            let node = data_state.nodes.get(&node_id)
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
                        !static_keys.contains(&(pin.definition.name.clone(), pin.definition.direction))
                    } else {
                        false
                    }
                })
                .copied()
                .collect();
        }

        // 创建新的 PinInstance
        let base_order = static_pin_defs.len() as i32;
        let new_pin_instances: Vec<PinInstance> = dynamic_new_defs
            .iter()
            .enumerate()
            .map(|(i, pd)| {
                PinInstance::from_definition(pd, node_id, base_order + i as i32)
            })
            .collect();

        // 如果动态部分没有实质变化，跳过
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

        // 应用变更
        let change_set;
        {
            let mut data_state = self.data_state.write().unwrap();
            let (removed_ids, removed_conns) = data_state.replace_node_pins(
                node_id,
                dynamic_old_pin_ids,
                new_pin_instances.clone(),
            );
            change_set = PinChangeSet {
                node_id,
                removed_pin_ids: removed_ids,
                added_pins: new_pin_instances,
                removed_connections: removed_conns,
            };
        }

        // 重新运行类型推断
        let _ = self.infer_types();

        Ok(Some(change_set))
    }

    /// 构建 PinResolverContext
    fn build_resolver_context(
        &self,
        node_id: NodeId,
        instance_params: &NodeInstanceParams,
    ) -> Result<PinResolverContext, String> {
        let mut input_schemas = std::collections::HashMap::new();
        let data_state = self.data_state.read().unwrap();

        // 遍历节点的输入 pins，查看是否有上游连接
        if let Some(node) = data_state.nodes.get(&node_id) {
            for &pin_id in &node.pin_ids {
                if let Some(pin) = data_state.pins.get(&pin_id) {
                    if !pin.is_input() || !pin.is_data() {
                        continue;
                    }
                    // 查看上游连接
                    if let Some(upstream_pin_id) = data_state.connections.get_upstream(pin_id) {
                        if let Some(upstream_pin) = data_state.pins.get(&upstream_pin_id) {
                            // 获取上游节点的 instance_params（如 dataframe_id）
                            if let Some(upstream_node) = data_state.nodes.get(&upstream_pin.node_id) {
                                // 尝试从上游节点的 instance_params 中提取 schema
                                let schema = self.extract_schema_from_params(
                                    &upstream_node.instance_params,
                                    &upstream_node.definition.node_type,
                                );
                                if let Some(s) = schema {
                                    input_schemas.insert(pin.definition.role.clone(), s);
                                }
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

    /// 从上游节点的 instance_params 提取 DataSchema
    ///
    /// 通过节点类型和 instance_params 中的 dataframe_id，
    /// 使用 schema_provider 查询实际的 DataFrame 列结构。
    fn extract_schema_from_params(
        &self,
        params: &NodeInstanceParams,
        node_type: &str,
    ) -> Option<DataSchema> {
        match node_type {
            "Data:Get DataFrame" => {
                let df_id = params.dataframe_id()?;
                let provider = self.schema_provider.as_ref()?;
                provider(df_id)
            }
            _ => None,
        }
    }
}

/// Repeatable Pin 增删
impl GraphInstance {
    /// 向节点的某个 Repeatable 槽位追加一个新 pin
    ///
    /// `slot_index` 是节点定义 `pin_slots` 数组中的索引，必须指向一个 Repeatable 槽位。
    /// 返回包含新增 pin 信息的 `PinChangeSet`。
    pub fn add_repeatable_pin(
        &self,
        node_id: NodeId,
        slot_index: usize,
    ) -> Result<PinChangeSet, String> {
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
            _ => return Err(format!("Slot index {} is not a Repeatable slot", slot_index)),
        };

        let family_pins = self.get_pin_instances_by_pin_role_family(node_id, template_role);
        let current_count = family_pins.len();

        if let PinSlot::Repeatable { max_count, .. } = slot {
            if let Some(max) = max_count {
                if current_count >= *max {
                    return Err(format!(
                        "Repeatable slot already at max count ({})",
                        max
                    ));
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
            if let Some(node) = data_state.nodes.get_mut(&node_id) {
                node.pin_ids.push(new_pin_id);
            }
        }

        let _ = self.infer_types();

        Ok(PinChangeSet {
            node_id,
            removed_pin_ids: vec![],
            added_pins: vec![new_pin],
            removed_connections: vec![],
        })
    }

    /// 从节点移除一个 Repeatable 槽位的 pin
    ///
    /// 验证 pin 属于某个 Repeatable 槽位且当前数量 > min_count，
    /// 然后断开连接、移除 pin，并重新索引剩余的同族 pin。
    /// 返回包含移除信息的 `PinChangeSet` 以及被移除 pin 在槽位中的索引（用于 undo）。
    pub fn remove_repeatable_pin(
        &self,
        node_id: NodeId,
        pin_id: PinId,
    ) -> Result<(PinChangeSet, usize), String> {
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
        self.reindex_repeatable_pins(node_id, slot_index)?;

        let _ = self.infer_types();

        Ok((
            PinChangeSet {
                node_id,
                removed_pin_ids: vec![pin_id],
                added_pins: vec![],
                removed_connections,
            },
            pin_index_in_family,
        ))
    }

    /// Re-index all pins belonging to a Repeatable slot so their roles and names
    /// are contiguous (Operands(0), Operands(1), ...; A, B, C, ...).
    fn reindex_repeatable_pins(
        &self,
        node_id: NodeId,
        slot_index: usize,
    ) -> Result<(), String> {
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
            None => return Ok(()),
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

        Ok(())
    }
}
