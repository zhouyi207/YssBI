use super::*;

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
        let result = self.recompile(GraphRecompileScope::TopologyEffects {
            seeds: seed_nodes.to_vec(),
            mode,
        });
        (result.change_sets, result.inferred)
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

