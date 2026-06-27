use crate::graph::ConnectionManager;
use crate::graph::DataType;
use crate::graph::TypeVarId;
use crate::graph::pin::{PinDefinition, PinDirection, PinOrder};
use crate::graph::{NodeId, NodeInstance, PinId, PinInstance};
use std::collections::{HashMap, HashSet};

/// - 所有 Node 实例
/// - 所有 Pin 实例
/// - 所有连接关系
///
/// 不再直接序列化：磁盘格式由 `GraphInstance` 的自定义 serde 负责，运行期缓存
/// （`pin_types` / `type_var_bindings`）始终在加载后重建。
#[derive(Clone, Debug)]
pub struct GraphDataState {
    pub nodes: HashMap<NodeId, NodeInstance>,
    pub pins: HashMap<PinId, PinInstance>,
    pub connections: ConnectionManager,

    /// 类型推断缓存（不持久化；加载后 infer_types 重建）
    pub pin_types: HashMap<PinId, DataType>,

    // 运行时 TypeVar 绑定缓存（不持久化；加载后 infer_types 重建）
    pub type_var_bindings: HashMap<TypeVarId, DataType>,
}

impl Default for GraphDataState {
    fn default() -> Self {
        Self {
            nodes: Default::default(),
            pins: Default::default(),
            connections: Default::default(),
            pin_types: Default::default(),
            type_var_bindings: Default::default(),
        }
    }
}

impl GraphDataState {
    pub fn add_node(&mut self, node_instance: NodeInstance) {
        self.nodes.insert(node_instance.id, node_instance);
    }

    pub fn add_pins(&mut self, pin_instances: Vec<PinInstance>) {
        for pin_instance in pin_instances {
            self.pins.insert(pin_instance.id, pin_instance);
        }
    }

    pub fn remove_node(&mut self, node_id: NodeId) {
        self.nodes.remove(&node_id);
    }

    pub fn remove_pins(&mut self, pin_ids: Vec<PinId>) {
        for pin_id in pin_ids {
            self.pins.remove(&pin_id);
            self.pin_types.remove(&pin_id);
        }
    }

    pub fn prune_orphan_pin_types(&mut self) {
        let live_pin_ids: HashSet<PinId> = self.pins.keys().copied().collect();
        self.pin_types
            .retain(|pin_id, _| live_pin_ids.contains(pin_id));
    }

    /// 移除已不存在节点的 TypeVar 绑定（推断缓存）
    pub fn prune_orphan_type_var_bindings(&mut self) {
        let live_type_var_ids: HashSet<TypeVarId> = self
            .nodes
            .values()
            .flat_map(|node| node.type_var_map.keys().copied())
            .collect();
        self.type_var_bindings
            .retain(|var_id, _| live_type_var_ids.contains(var_id));
    }

    /// 清理孤立连接并重建连接索引
    pub fn reconcile_connections(&mut self) {
        let live_pins: HashSet<PinId> = self.pins.keys().copied().collect();
        self.connections.prune_orphan_links(&live_pins);
        self.connections.rebuild_indices_from_pins(&self.pins);
    }

    /// 保存前清理推断缓存与连接索引
    pub fn prepare_for_persistence(&mut self) {
        self.reconcile_connections();
        self.prune_orphan_pin_types();
        self.prune_orphan_type_var_bindings();
    }

    /// 将节点的「动态 pin 集合」对齐到 `target_defs`，**保留未变更列的 pin 身份**。
    ///
    /// 连接以 pin id 为键，故此前「整组替换 + 全新 id」会丢失所有连线。这里改为
    /// 按 `(direction, name)`（列名即稳定身份）对齐：
    /// - 同名存活列：复用既有 `PinInstance`（保留 id 与连接），就地更新 `definition`/`order`
    /// - 新增列：创建新 `PinInstance`
    /// - 消失列：移除 pin 并断开其连接
    ///
    /// 重排 `node.pin_ids = 静态 pin（原序）+ 动态 pin（target 顺序）`，纯重排不再
    /// 改动 id，因而连线得以保留。
    ///
    /// 返回供事件通知使用的变更集。
    pub fn reconcile_node_pins(
        &mut self,
        node_id: NodeId,
        old_dynamic_ids: &[PinId],
        target_defs: &[PinDefinition],
        base_order: i32,
    ) -> DynamicPinReconcile {
        let key = |dir: PinDirection, name: &str| (dir, name.to_string());

        // 既有动态 pin：(direction, name) -> id
        let mut existing: HashMap<(PinDirection, String), PinId> = HashMap::new();
        for &pid in old_dynamic_ids {
            if let Some(p) = self.pins.get(&pid) {
                existing.insert(key(p.definition.direction, &p.definition.name), pid);
            }
        }

        let target_keys: HashSet<(PinDirection, String)> = target_defs
            .iter()
            .map(|d| key(d.direction, &d.name))
            .collect();

        // 移除消失列：仅断开其连接
        let mut removed_pin_ids = Vec::new();
        let mut removed_connections = Vec::new();
        for &pid in old_dynamic_ids {
            let k = self
                .pins
                .get(&pid)
                .map(|p| key(p.definition.direction, &p.definition.name));
            let Some(k) = k else { continue };
            if target_keys.contains(&k) {
                continue;
            }
            for to_pin in self.connections.get_downstream(pid) {
                removed_connections.push((pid, to_pin));
            }
            if let Some(from_pin) = self.connections.get_upstream(pid) {
                removed_connections.push((from_pin, pid));
            }
            self.connections.disconnect_all(pid);
            self.pins.remove(&pid);
            self.pin_types.remove(&pid);
            removed_pin_ids.push(pid);
        }

        // 构建新的动态顺序：存活列复用 id，新列新建
        let mut new_dynamic_order: Vec<PinId> = Vec::with_capacity(target_defs.len());
        let mut added_pins: Vec<PinInstance> = Vec::new();
        let mut updated_pins: Vec<PinInstance> = Vec::new();
        for (i, def) in target_defs.iter().enumerate() {
            let order = base_order + i as i32;
            let k = key(def.direction, &def.name);
            if let Some(&pid) = existing.get(&k) {
                if let Some(pin) = self.pins.get_mut(&pid) {
                    pin.definition = def.clone();
                    pin.order = PinOrder(order);
                    updated_pins.push(pin.clone());
                }
                new_dynamic_order.push(pid);
            } else {
                let pin = PinInstance::from_definition(def, node_id, order);
                let pid = pin.id;
                self.connections.register_pin(pid, node_id);
                self.pins.insert(pid, pin.clone());
                added_pins.push(pin);
                new_dynamic_order.push(pid);
            }
        }

        // 重排 node.pin_ids：静态 pin 保持原序 + 动态 pin 按 target 顺序
        let old_set: HashSet<PinId> = old_dynamic_ids.iter().copied().collect();
        if let Some(node) = self.nodes.get_mut(&node_id) {
            let statics: Vec<PinId> = node
                .pin_ids
                .iter()
                .copied()
                .filter(|id| !old_set.contains(id))
                .collect();
            node.pin_ids = statics.into_iter().chain(new_dynamic_order).collect();
        }

        DynamicPinReconcile {
            removed_pin_ids,
            added_pins,
            updated_pins,
            removed_connections,
        }
    }
}

/// `reconcile_node_pins` 的变更集（供 `PinChangeSet` / 事件通知使用）
#[derive(Debug, Clone, Default)]
pub struct DynamicPinReconcile {
    pub removed_pin_ids: Vec<PinId>,
    pub added_pins: Vec<PinInstance>,
    pub updated_pins: Vec<PinInstance>,
    pub removed_connections: Vec<(PinId, PinId)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::node::{NodeDefinition, NodeInstance, NodeInstanceParams, NodePosition};
    use crate::graph::pin::{DataRole, PinDataTypeDefinition, PinDefinition};
    use std::sync::Arc;

    fn dyn_out(name: &str) -> PinDefinition {
        PinDefinition::data_output(
            name,
            DataRole::Custom(name.to_string()),
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )
        .with_dynamic(true)
    }

    fn series_input(name: &str) -> PinDefinition {
        PinDefinition::data_input(
            name,
            DataRole::Input,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )
    }

    /// 构建一个节点：1 个静态 DataFrame 输入 + `cols` 个动态输出 pin。
    /// 返回 (state, node_id, 动态 pin id 列表[与 cols 同序], 静态输入 pin id)
    fn setup(cols: &[&str]) -> (GraphDataState, NodeId, Vec<PinId>, PinId) {
        let mut ds = GraphDataState::default();
        let node_id = NodeId::new();

        let input_def = PinDefinition::data_input(
            "DataFrame",
            DataRole::Input,
            PinDataTypeDefinition::concrete(DataType::DataFrame),
        );
        let input_pin = PinInstance::from_definition(&input_def, node_id, 0);
        let input_id = input_pin.id;
        ds.connections.register_pin(input_id, node_id);
        ds.pins.insert(input_id, input_pin);

        let mut pin_ids = vec![input_id];
        let mut dynamic_ids = Vec::new();
        for (i, c) in cols.iter().enumerate() {
            let pin = PinInstance::from_definition(&dyn_out(c), node_id, 1 + i as i32);
            dynamic_ids.push(pin.id);
            pin_ids.push(pin.id);
            ds.connections.register_pin(pin.id, node_id);
            ds.pins.insert(pin.id, pin);
        }

        ds.add_node(NodeInstance {
            id: node_id,
            definition: Arc::new(NodeDefinition::placeholder(
                "Decompose DataFrame".to_string(),
            )),
            type_var_map: Default::default(),
            position: NodePosition::default(),
            instance_params: NodeInstanceParams::default(),
            pin_ids,
        });

        (ds, node_id, dynamic_ids, input_id)
    }

    /// 连接动态 pin 到一个外部消费者 pin，返回消费者 pin id。
    fn connect_to_consumer(ds: &mut GraphDataState, from_pin: PinId) -> PinId {
        let other_node = NodeId::new();
        let consumer = PinInstance::from_definition(&series_input("In"), other_node, 0);
        let consumer_id = consumer.id;
        ds.connections.register_pin(consumer_id, other_node);
        ds.pins.insert(consumer_id, consumer);
        ds.connections.connect(from_pin, consumer_id);
        consumer_id
    }

    fn pin_id_by_name(ds: &GraphDataState, name: &str) -> Option<PinId> {
        ds.pins
            .values()
            .find(|p| p.definition.name == name)
            .map(|p| p.id)
    }

    #[test]
    fn reconcile_preserves_ids_and_connections_on_reorder() {
        let (mut ds, node_id, dyn_ids, input_id) = setup(&["a", "b", "c"]);
        let b_id = dyn_ids[1];
        let consumer_id = connect_to_consumer(&mut ds, b_id);

        // 同一组列、仅顺序变化：[c, a, b]
        let target = vec![dyn_out("c"), dyn_out("a"), dyn_out("b")];
        let r = ds.reconcile_node_pins(node_id, &dyn_ids, &target, 1);

        assert!(r.removed_pin_ids.is_empty(), "重排不应移除任何 pin");
        assert!(r.added_pins.is_empty(), "重排不应新增任何 pin");
        assert!(r.removed_connections.is_empty(), "重排不应断开连接");

        // b 复用原 id，连接仍在
        assert_eq!(pin_id_by_name(&ds, "b"), Some(b_id), "b pin id 应被保留");
        assert!(
            ds.connections
                .all_connections()
                .iter()
                .any(|c| c.from_pin == b_id && c.to_pin == consumer_id),
            "b -> consumer 连接应被保留"
        );

        // node.pin_ids = 静态(DataFrame) + 动态(c, a, b)
        let node = ds.nodes.get(&node_id).unwrap();
        let names: Vec<String> = node
            .pin_ids
            .iter()
            .map(|pid| ds.pins.get(pid).unwrap().definition.name.clone())
            .collect();
        assert_eq!(names, vec!["DataFrame", "c", "a", "b"]);
        assert_eq!(node.pin_ids[0], input_id, "静态输入 pin 顺序不变");
    }

    #[test]
    fn reconcile_removes_only_missing_columns() {
        let (mut ds, node_id, dyn_ids, _input) = setup(&["a", "b", "c"]);
        let (a_id, b_id, c_id) = (dyn_ids[0], dyn_ids[1], dyn_ids[2]);
        let consumer_id = connect_to_consumer(&mut ds, b_id);

        // 移除 b：[a, c]
        let target = vec![dyn_out("a"), dyn_out("c")];
        let r = ds.reconcile_node_pins(node_id, &dyn_ids, &target, 1);

        assert_eq!(r.removed_pin_ids, vec![b_id], "仅 b 被移除");
        assert!(r.added_pins.is_empty(), "无新增列");
        assert!(
            r.removed_connections
                .iter()
                .any(|(f, t)| *f == b_id && *t == consumer_id),
            "b 的连接应在 removed_connections 中"
        );

        // a / c 保留原 id，b 消失
        assert!(!ds.pins.contains_key(&b_id), "b pin 应被移除");
        assert_eq!(pin_id_by_name(&ds, "a"), Some(a_id), "a id 不变");
        assert_eq!(pin_id_by_name(&ds, "c"), Some(c_id), "c id 不变");
        assert!(
            !ds.connections
                .all_connections()
                .iter()
                .any(|c| c.to_pin == consumer_id),
            "b 的连接应被断开"
        );
    }
}
