//! Graph 实现
//!
//! ❌ 不负责「定义」
//! ❌ 不负责「编译策略」
//! ❌ 不负责「执行调度」
//! ✅ 持有状态
//! ✅ 提供受控 mutation API

use super::{GraphDataState, GraphKind, GraphPosition};
use crate::graph::connection::Connection;
use crate::graph::node::{NodeId, NodeInstance, NodeInstanceParams, PinResolverContext};
use crate::graph::pin::{PinId, PinInstance, PinRole, PinDirection};
use crate::graph::register::NodeRegistry;
use crate::graph::value::DataValue;
use crate::graph::{DataType, GraphId};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

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
#[derive(Clone, Debug, Serialize, Deserialize)]
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
        }
    }

    pub fn clear(&self) {
        *self.data_state.write().unwrap() = GraphDataState::default();
    }

    /// 设置节点注册表（用于反序列化后恢复）
    pub fn set_registry(&mut self, registry: Arc<NodeRegistry>) {
        self.registry = registry;
    }

    /// 获取节点注册表的引用
    pub fn registry(&self) -> &Arc<NodeRegistry> {
        &self.registry
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
        if let Some(p) = params {
            node = node.with_instance_params(p);
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
        self.remove_node_raw(node_id)?;
        let _ = self.infer_types();
        Ok(())
    }

    /// 删除节点但不运行类型推断（用于批量删除，外部负责最终调一次 infer_types）
    pub fn remove_node_raw(&self, node_id: NodeId) -> Result<(), String> {
        let pins = self.get_pin_instances_by_node_id(node_id);

        {
            let mut data_state = self.data_state.write().unwrap();
            data_state.connections.remove_node(node_id);

            for pin in pins {
                data_state.pins.remove(&pin.id);
            }
            data_state.nodes.remove(&node_id);
        }

        Ok(())
    }

    pub fn get_node_id_by_pin_id(&self, pin_id: PinId) -> NodeId {
        let data_state = self.data_state.read().unwrap();
        let pin_instance = data_state.pins.get(&pin_id).unwrap();
        pin_instance.node_id
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
        let pin_ids = data_state.nodes.get(&node_id).unwrap().pin_ids.clone();
        let pins = data_state.pins.clone();

        pin_ids
            .into_iter()
            .filter_map(|id| pins.get(&id).cloned())
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
    /// 连接两个 pin，自动运行类型推断和动态 pin 重建
    ///
    /// 返回 (被自动断开的旧连接, 动态 pin 变更集)
    pub fn connect(&self, from_pin: PinId, to_pin: PinId) -> Result<(Option<(PinId, PinId)>, Vec<PinChangeSet>), String> {
        let affected_node_id;
        let auto_disconnected;
        {
            let data_state = self.data_state.write().unwrap();
            let pins = data_state.pins.clone();
            if !pins.contains_key(&from_pin) {
                return Err(format!("Source pin {:?} not found", from_pin));
            }
            if !pins.contains_key(&to_pin) {
                return Err(format!("Target pin {:?} not found", to_pin));
            }
            affected_node_id = pins.get(&to_pin).map(|p| p.node_id);
            // 记录 to_pin 上已有的连接（会被自动断开）
            auto_disconnected = data_state.connections.get_upstream(to_pin)
                .map(|old_from| (old_from, to_pin));
            data_state.connections.connect(from_pin, to_pin)?;
        }

        let _ = self.infer_types();

        let mut change_sets = Vec::new();
        if let Some(node_id) = affected_node_id {
            if let Some(cs) = self.resolve_dynamic_pins(node_id)? {
                change_sets.push(cs);
            }
        }

        Ok((auto_disconnected, change_sets))
    }

    pub fn get_downstream_by_pin_id(&self, pin_id: PinId) -> Vec<PinId> {
        let data_state = self.data_state.read().unwrap();
        data_state.connections.get_downstream(pin_id)
    }

    pub fn get_upstream_by_pin_id(&self, pin_id: PinId) -> Option<PinId> {
        let data_state = self.data_state.read().unwrap();
        data_state.connections.get_upstream(pin_id)
    }

    /// 断开连接，自动运行类型推断和动态 pin 重建
    pub fn disconnect(&self, from_pin: PinId, to_pin: PinId) -> Vec<PinChangeSet> {
        let affected_node_id;
        {
            let data_state = self.data_state.write().unwrap();
            affected_node_id = data_state.pins.get(&to_pin).map(|p| p.node_id);
            data_state.connections.disconnect(from_pin, to_pin);
        }

        let _ = self.infer_types();

        let mut change_sets = Vec::new();
        if let Some(node_id) = affected_node_id {
            if let Ok(Some(cs)) = self.resolve_dynamic_pins(node_id) {
                change_sets.push(cs);
            }
        }
        change_sets
    }

    /// 断开指定 Pin 的所有连接（输入和输出）
    ///
    /// 返回 (被删除的连接对列表, 动态 pin 变更集)
    pub fn disconnect_pin(&self, pin_id: PinId) -> (Vec<(PinId, PinId)>, Vec<PinChangeSet>) {
        let affected_node_id;
        let mut removed_connections = Vec::new();
        {
            let data_state = self.data_state.write().unwrap();
            affected_node_id = data_state.pins.get(&pin_id).map(|p| p.node_id);
            // 收集将被删除的连接
            for to_pin in data_state.connections.get_downstream(pin_id) {
                removed_connections.push((pin_id, to_pin));
            }
            if let Some(from_pin) = data_state.connections.get_upstream(pin_id) {
                removed_connections.push((from_pin, pin_id));
            }
            data_state.connections.disconnect_all(pin_id);
        }

        let _ = self.infer_types();

        let mut change_sets = Vec::new();
        if let Some(node_id) = affected_node_id {
            if let Ok(Some(cs)) = self.resolve_dynamic_pins(node_id) {
                change_sets.push(cs);
            }
        }
        (removed_connections, change_sets)
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
    pub fn infer_types(&self) -> Result<(), String> {
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
        // 静态 pins = 由 pin_generator 生成的初始 pins
        let static_pin_defs = if let Some(gen) = &definition.pin_generator {
            gen()?
        } else {
            Vec::new()
        };

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
    /// 当前实现（Plan A）：通过节点类型和 instance_params 静态推断。
    /// 未来演进（Plan B）：从 DataType 携带的 schema 信息中提取。
    fn extract_schema_from_params(
        &self,
        _params: &NodeInstanceParams,
        _node_type: &str,
    ) -> Option<crate::graph::node::DataSchema> {
        // 目前 DataFrame 存储系统尚未实现，返回 None
        // 后续实现时，通过 dataframe_id 查询 ProjectState 中的 DataFrame schema
        None
    }
}
