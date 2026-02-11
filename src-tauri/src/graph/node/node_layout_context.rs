use crate::graph::GraphData;
use crate::graph::GraphId;
use crate::graph::{NodeId, PinId, PinRole, PinSchema, PinDataType};

pub trait NodeLayoutContext {
    // ---------- 基础查询 ----------

    /// 当前节点所在的 Graph
    fn graph_id(&self) -> GraphId;

    /// 查询某个 pin role 是否存在（是否已被实例化）
    fn has_pin(&self, node: NodeId, role: &PinRole) -> bool;

    /// 查询某个 role 已绑定的 PinId（如果存在）
    fn pin_id(&self, node: NodeId, role: &PinRole) -> Option<PinId>;

    // ---------- 类型 / Schema 推断 ----------

    /// 查询某个输入 pin 的「推断类型」
    /// - 已连接 → 来自上游输出
    /// - 未连接 → NodeDefinition 默认类型
    fn infer_input_type(&self, node: NodeId, role: &PinRole) -> Option<PinDataType>;

    /// 查询某个 pin 的 schema（比如 DataFrame 的 column 信息）
    fn input_schema(&self, node: NodeId, role: &PinRole) -> Option<PinSchema>;

    // ---------- 连接关系 ----------

    /// 查询该 pin role 是否有连接
    fn is_connected(&self, node: NodeId, role: &PinRole) -> bool;

    /// 查询连接到该输入 pin 的上游 pin
    fn connected_source(&self, node: NodeId, role: &PinRole) -> Option<PinId>;
}

impl NodeLayoutContext for GraphData {
    fn graph_id(&self) -> GraphId {
        self.id
    }

    fn has_pin(&self, node: NodeId, role: &PinRole) -> bool {
        self.get_pin_by_role(node, role).is_some()
    }

    fn pin_id(&self, node: NodeId, role: &PinRole) -> Option<PinId> {
        self.get_pin_by_role(node, role).map(|pin| pin.id)
    }

    fn is_connected(&self, node: NodeId, role: &PinRole) -> bool {
        if let Some(pin) = self.get_pin_by_role(node, role) {
            // 检查是否有上游或下游连接
            self.connections.get_upstream(pin.id).is_some()
                || !self.connections.get_downstream(pin.id).is_empty()
        } else {
            false
        }
    }

    fn connected_source(&self, node: NodeId, role: &PinRole) -> Option<PinId> {
        let pin = self.get_pin_by_role(node, role)?;
        self.connections.get_upstream(pin.id)
    }

    fn infer_input_type(&self, node: NodeId, role: &PinRole) -> Option<PinDataType> {
        let pin = self.get_pin_by_role(node, role)?;

        // 如果有连接，从上游获取类型
        if let Some(src_pin_id) = self.connections.get_upstream(pin.id) {
            if let Some(src_pin) = self.get_pin(src_pin_id) {
                return src_pin.definition.data_type.clone();
            }
        }

        // 否则使用节点定义的默认类型
        let definition = self.get_node_definition(node)?;

        // 从 definition 的 pins 中查找对应 role 的默认类型
        for pin_def in &definition.pins {
            if &pin_def.role == role {
                return pin_def.data_type.clone();
            }
        }

        None
    }

    fn input_schema(&self, node: NodeId, role: &PinRole) -> Option<PinSchema> {
        let pin = self.get_pin_by_role(node, role)?;

        // 1. 如果有上游连接，从上游获取 schema
        if let Some(src_pin_id) = self.connections.get_upstream(pin.id) {
            if let Some(schema) = self.get_pin_schema(src_pin_id) {
                return Some(schema);
            }
        }

        // 2. 检查 Pin 自身是否有 schema（例如从节点定义中获取）
        if let Some(schema) = self.get_pin_schema(pin.id) {
            return Some(schema);
        }

        // 3. 如果类型是 DataFrame 但没有 schema，返回 None
        // 这表示 schema 尚未确定，需要在运行时推断
        None
    }
}
