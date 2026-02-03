//! 节点 CRUD 操作

use super::project_state::ProjectState;
use crate::project::{CanvasState, PinDefDto, SerializedNode};

impl ProjectState {
    // ==================== Nodes CRUD ====================

    pub fn get_nodes(&self, subgraph_id: &str) -> Result<Vec<SerializedNode>, String> {
        let project = self.data.read().unwrap();
        let subgraph = crate::get_subgraph!(project, subgraph_id)
            .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;
        Ok(subgraph.nodes.clone())
    }

    pub fn set_nodes(&self, subgraph_id: &str, nodes: Vec<SerializedNode>) -> Result<(), String> {
        let mut project = self.data.write().unwrap();
        let subgraph = crate::get_subgraph_mut!(project, subgraph_id)
            .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;
        subgraph.nodes = nodes;
        Ok(())
    }

    /// 创建单个节点
    pub fn create_node(
        &self,
        subgraph_id: &str,
        node: SerializedNode,
    ) -> Result<SerializedNode, String> {
        let mut project = self.data.write().unwrap();
        let subgraph = crate::get_subgraph_mut!(project, subgraph_id)
            .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;

        // 检查节点ID是否已存在
        if subgraph.nodes.iter().any(|n| n.id == node.id) {
            return Err(format!(
                "Node with id '{}' already exists in subgraph '{}'",
                node.id, subgraph_id
            ));
        }

        subgraph.nodes.push(node.clone());
        Ok(node)
    }

    /// 删除单个节点
    /// 
    /// 注意：此函数现在会级联删除所有相关的连接
    pub fn delete_node(&self, subgraph_id: &str, node_id: &str) -> Result<(), String> {
        // 1. 先删除节点的所有连接
        self.delete_connections_for_node(subgraph_id, node_id)?;

        // 2. 然后删除节点本身
        let mut project = self.data.write().unwrap();
        let subgraph = crate::get_subgraph_mut!(project, subgraph_id)
            .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;

        let original_len = subgraph.nodes.len();
        subgraph.nodes.retain(|n| n.id != node_id);

        if subgraph.nodes.len() == original_len {
            return Err(format!(
                "Node with id '{}' not found in subgraph '{}'",
                node_id, subgraph_id
            ));
        }

        Ok(())
    }

    /// 批量创建节点 (后端生成 ID 并修复连接关系)
    pub fn create_nodes(
        &self,
        subgraph_id: &str,
        nodes: Vec<SerializedNode>,
    ) -> Result<Vec<SerializedNode>, String> {
        let mut project = self.data.write().unwrap();
        let subgraph = crate::get_subgraph_mut!(project, subgraph_id)
            .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;

        let mut id_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        let mut new_nodes = nodes.clone();

        // 1. 生成新 ID (节点和引脚)
        for node in &mut new_nodes {
            let old_id = node.id.clone();
            let new_id = format!("node-{}", uuid::Uuid::new_v4());
            node.id = new_id.clone();
            id_map.insert(old_id, new_id);

            // 处理 Inputs
            for pin in &mut node.inputs {
                let old_pin_id = pin.id.clone();
                let new_pin_id = format!("pin-{}", uuid::Uuid::new_v4());
                pin.id = new_pin_id.clone();
                id_map.insert(old_pin_id, new_pin_id);
            }
            // 处理 Outputs
            for pin in &mut node.outputs {
                let old_pin_id = pin.id.clone();
                let new_pin_id = format!("pin-{}", uuid::Uuid::new_v4());
                pin.id = new_pin_id.clone();
                id_map.insert(old_pin_id, new_pin_id);
            }
        }

        // 2. 更新连接关系 (Remap links)
        for node in &mut new_nodes {
            for pin in node.inputs.iter_mut().chain(node.outputs.iter_mut()) {
                let old_links = pin.links.clone();
                pin.links.clear();
                for link in old_links {
                    // 只保留指向这批新节点内部的链接
                    if let Some(new_target_id) = id_map.get(&link) {
                        pin.links.push(new_target_id.clone());
                    }
                }
            }
        }

        // 3. 保存
        subgraph.nodes.extend(new_nodes.clone());

        Ok(new_nodes)
    }

    /// 批量创建节点并重新映射连接
    /// 
    /// 此函数用于复制/粘贴操作，会：
    /// 1. 生成新的节点和 pin ID
    /// 2. 重新映射连接到新的 pin ID
    /// 3. 创建连接对象
    /// 
    /// 返回：(新节点列表, ID 映射表)
    pub fn create_nodes_with_connections(
        &self,
        subgraph_id: &str,
        nodes: Vec<SerializedNode>,
        connections: Vec<crate::project::ConnectionDto>,
    ) -> Result<(Vec<SerializedNode>, std::collections::HashMap<String, String>), String> {
        use uuid::Uuid;

        let mut project = self.data.write().unwrap();
        let subgraph = crate::get_subgraph_mut!(project, subgraph_id)
            .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;

        let mut id_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        let mut new_nodes = nodes.clone();

        // 1. 生成新 ID (节点和引脚)
        for node in &mut new_nodes {
            let old_id = node.id.clone();
            let new_id = format!("node-{}", Uuid::new_v4());
            node.id = new_id.clone();
            id_map.insert(old_id, new_id);

            // 处理 Inputs
            for pin in &mut node.inputs {
                let old_pin_id = pin.id.clone();
                let new_pin_id = format!("pin-{}", Uuid::new_v4());
                pin.id = new_pin_id.clone();
                id_map.insert(old_pin_id, new_pin_id);
            }
            // 处理 Outputs
            for pin in &mut node.outputs {
                let old_pin_id = pin.id.clone();
                let new_pin_id = format!("pin-{}", Uuid::new_v4());
                pin.id = new_pin_id.clone();
                id_map.insert(old_pin_id, new_pin_id);
            }
        }

        // 2. 重新映射连接
        let mut new_connections = Vec::new();
        for conn in connections {
            // 查找新的 pin ID
            if let (Some(new_source), Some(new_target)) = (
                id_map.get(&conn.source_pin),
                id_map.get(&conn.target_pin),
            ) {
                new_connections.push(crate::project::ConnectionDto {
                    id: format!("conn-{}", Uuid::new_v4()),
                    source_pin: new_source.clone(),
                    target_pin: new_target.clone(),
                });
            }
        }

        // 3. 保存节点和连接
        subgraph.nodes.extend(new_nodes.clone());
        subgraph.connections.extend(new_connections);

        Ok((new_nodes, id_map))
    }

    pub fn update_canvas(&self, subgraph_id: &str, canvas: CanvasState) -> Result<(), String> {
        let mut project = self.data.write().unwrap();
        let subgraph = crate::get_subgraph_mut!(project, subgraph_id)
            .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;
        subgraph.canvas = canvas;
        Ok(())
    }

    // ==================== SubGraph 输入输出 ====================

    pub fn update_subgraph_io(
        &self,
        subgraph_id: &str,
        inputs: Option<Vec<PinDefDto>>,
        outputs: Option<Vec<PinDefDto>>,
    ) -> Result<(), String> {
        let mut project = self.data.write().unwrap();
        let subgraph = crate::get_subgraph_mut!(project, subgraph_id)
            .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;

        if let Some(inputs) = inputs {
            subgraph.inputs = inputs;
        }
        if let Some(outputs) = outputs {
            subgraph.outputs = outputs;
        }
        Ok(())
    }

    pub fn rename_subgraph(&self, subgraph_id: &str, new_name: String) -> Result<(), String> {
        let mut project = self.data.write().unwrap();
        let subgraph = crate::get_subgraph_mut!(project, subgraph_id)
            .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;
        subgraph.name = new_name;
        Ok(())
    }

    /// 连接两个 Pin
    /// 返回更新后的节点列表
    /// 
    /// 注意：此函数现在使用 Connection 系统
    /// 
    /// 连接规则：
    /// - Exec Output 只能连接一个目标（删除该 output 的其他连接）
    /// - Data Input 只能有一个来源（删除指向该 input 的其他连接）
    /// - Exec Input 可以有多个来源
    /// - Data Output 可以连接多个目标
    pub fn connect_pins(
        &self,
        subgraph_id: &str,
        source_pin_id: &str,
        target_pin_id: &str,
    ) -> Result<Vec<SerializedNode>, String> {
        // 1. 获取源和目标 pin 的信息
        let project = self.data.read().unwrap();
        let subgraph = crate::get_subgraph!(project, subgraph_id)
            .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;

        let mut source_pin_type: Option<String> = None;
        let mut target_pin_type: Option<String> = None;

        for node in subgraph.nodes.iter() {
            // 源 pin 必须是 output
            if let Some(pin) = node.outputs.iter().find(|p| p.id == source_pin_id) {
                source_pin_type = Some(pin.pin_type.clone());
            }
            // 目标 pin 必须是 input
            if let Some(pin) = node.inputs.iter().find(|p| p.id == target_pin_id) {
                target_pin_type = Some(pin.pin_type.clone());
            }
        }

        let source_type = source_pin_type.ok_or_else(|| format!("Source pin '{}' not found or not an output", source_pin_id))?;
        let target_type = target_pin_type.ok_or_else(|| format!("Target pin '{}' not found or not an input", target_pin_id))?;

        // 2. 根据 pin 类型确定需要删除的旧连接
        let mut connections_to_delete = Vec::new();

        // 规则 1: Exec Output 只能连接一个目标
        // 如果源是 Exec 类型的 output，删除该 output 的其他连接
        if source_type.to_lowercase() == "exec" {
            connections_to_delete.extend(
                subgraph
                    .connections
                    .iter()
                    .filter(|c| c.source_pin == source_pin_id && c.target_pin != target_pin_id)
                    .map(|c| c.id.clone())
            );
        }

        // 规则 2: Data Input 只能有一个来源
        // 如果目标是 Data 类型的 input（非 Exec），删除指向该 input 的其他连接
        if target_type.to_lowercase() != "exec" {
            connections_to_delete.extend(
                subgraph
                    .connections
                    .iter()
                    .filter(|c| c.target_pin == target_pin_id && c.source_pin != source_pin_id)
                    .map(|c| c.id.clone())
            );
        }

        drop(project); // 释放读锁

        // 3. 删除旧连接
        for conn_id in connections_to_delete {
            self.delete_connection(subgraph_id, &conn_id)?;
        }

        // 4. 使用新的 create_connection 函数创建连接
        let _connection = self.create_connection(subgraph_id, source_pin_id, target_pin_id)?;

        // 5. 返回更新后的节点列表
        let project = self.data.read().unwrap();
        let subgraph = crate::get_subgraph!(project, subgraph_id)
            .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;
        Ok(subgraph.nodes.clone())
    }

    /// 断开 Pin 的所有连接
    /// 
    /// 注意：此函数现在使用 Connection 系统
    pub fn disconnect_pin(
        &self,
        subgraph_id: &str,
        pin_id: &str,
    ) -> Result<Vec<SerializedNode>, String> {
        // 使用新的 delete_connections_for_pin 函数
        self.delete_connections_for_pin(subgraph_id, pin_id)?;

        // 返回更新后的节点列表
        let project = self.data.read().unwrap();
        let subgraph = crate::get_subgraph!(project, subgraph_id)
            .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;
        Ok(subgraph.nodes.clone())
    }

    // ==================== Connection CRUD ====================

    /// 创建连接
    ///
    /// 在两个 Pin 之间创建连接关系
    /// 验证：Pin 存在性、方向兼容性、类型兼容性
    pub fn create_connection(
        &self,
        subgraph_id: &str,
        source_pin_id: &str,
        target_pin_id: &str,
    ) -> Result<crate::project::ConnectionDto, String> {
        use crate::executor::value::{PinTypeDesc, TypeInferenceContext};
        use crate::schema::pin_types::can_connect;
        use uuid::Uuid;

        let mut project = self.data.write().unwrap();
        let subgraph = crate::get_subgraph_mut!(project, subgraph_id)
            .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;

        // 1. 查找源和目标 Pin
        let mut source_info: Option<(bool, String)> = None; // (is_input, type)
        let mut target_info: Option<(bool, String)> = None;

        for node in subgraph.nodes.iter() {
            for pin in node.inputs.iter() {
                if pin.id == source_pin_id {
                    source_info = Some((true, pin.pin_type.clone()));
                }
                if pin.id == target_pin_id {
                    target_info = Some((true, pin.pin_type.clone()));
                }
            }
            for pin in node.outputs.iter() {
                if pin.id == source_pin_id {
                    source_info = Some((false, pin.pin_type.clone()));
                }
                if pin.id == target_pin_id {
                    target_info = Some((false, pin.pin_type.clone()));
                }
            }
        }

        let source = source_info.ok_or_else(|| format!("Source pin '{}' not found", source_pin_id))?;
        let target = target_info.ok_or_else(|| format!("Target pin '{}' not found", target_pin_id))?;

        // 2. 验证方向：源必须是输出，目标必须是输入
        if source.0 {
            return Err("Source pin must be an output pin".to_string());
        }
        if !target.0 {
            return Err("Target pin must be an input pin".to_string());
        }

        // 3. 验证类型兼容性
        let source_type = &source.1;
        let target_type = &target.1;

        // 使用类型推断系统进行类型检查
        let mut type_inference = TypeInferenceContext::new();
        let temp_source_id = Uuid::new_v4();
        let temp_target_id = Uuid::new_v4();

        type_inference.register_pin(temp_source_id, PinTypeDesc::from_string(source_type));
        type_inference.register_pin(temp_target_id, PinTypeDesc::from_string(target_type));

        match type_inference.infer_connection(temp_source_id, temp_target_id) {
            Ok(_) => {
                // 类型推断成功
            }
            Err(e) => {
                // 类型推断失败，回退到旧的类型检查
                if !can_connect(source_type, target_type) {
                    return Err(format!(
                        "Cannot connect: type '{}' is not compatible with type '{}' ({})",
                        source_type, target_type, e
                    ));
                }
            }
        }

        // 4. 检查是否已存在相同的连接
        if subgraph.connections.iter().any(|c| 
            c.source_pin == source_pin_id && c.target_pin == target_pin_id
        ) {
            return Err(format!(
                "Connection already exists between '{}' and '{}'",
                source_pin_id, target_pin_id
            ));
        }

        // 5. 创建连接
        let connection = crate::project::ConnectionDto {
            id: format!("conn-{}", Uuid::new_v4()),
            source_pin: source_pin_id.to_string(),
            target_pin: target_pin_id.to_string(),
        };

        subgraph.connections.push(connection.clone());

        Ok(connection)
    }

    /// 删除连接
    ///
    /// 根据连接 ID 删除连接
    pub fn delete_connection(
        &self,
        subgraph_id: &str,
        connection_id: &str,
    ) -> Result<(), String> {
        let mut project = self.data.write().unwrap();
        let subgraph = crate::get_subgraph_mut!(project, subgraph_id)
            .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;

        let original_len = subgraph.connections.len();
        subgraph.connections.retain(|c| c.id != connection_id);

        if subgraph.connections.len() == original_len {
            return Err(format!("Connection '{}' not found", connection_id));
        }

        Ok(())
    }

    /// 删除 Pin 的所有连接
    ///
    /// 删除所有引用指定 Pin 的连接
    /// 返回被删除的连接 ID 列表
    pub fn delete_connections_for_pin(
        &self,
        subgraph_id: &str,
        pin_id: &str,
    ) -> Result<Vec<String>, String> {
        let mut project = self.data.write().unwrap();
        let subgraph = crate::get_subgraph_mut!(project, subgraph_id)
            .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;

        let removed_ids: Vec<String> = subgraph
            .connections
            .iter()
            .filter(|c| c.source_pin == pin_id || c.target_pin == pin_id)
            .map(|c| c.id.clone())
            .collect();

        subgraph
            .connections
            .retain(|c| c.source_pin != pin_id && c.target_pin != pin_id);

        Ok(removed_ids)
    }

    /// 删除节点的所有连接
    ///
    /// 删除所有引用指定节点上任何 Pin 的连接
    /// 返回被删除的连接 ID 列表
    pub fn delete_connections_for_node(
        &self,
        subgraph_id: &str,
        node_id: &str,
    ) -> Result<Vec<String>, String> {
        use std::collections::HashSet;

        let mut project = self.data.write().unwrap();
        let subgraph = crate::get_subgraph_mut!(project, subgraph_id)
            .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;

        // 找到节点的所有 Pin ID
        let pin_ids: HashSet<String> = subgraph
            .nodes
            .iter()
            .filter(|n| n.id == node_id)
            .flat_map(|n| {
                n.inputs
                    .iter()
                    .chain(n.outputs.iter())
                    .map(|p| p.id.clone())
            })
            .collect();

        let removed_ids: Vec<String> = subgraph
            .connections
            .iter()
            .filter(|c| pin_ids.contains(&c.source_pin) || pin_ids.contains(&c.target_pin))
            .map(|c| c.id.clone())
            .collect();

        subgraph
            .connections
            .retain(|c| !pin_ids.contains(&c.source_pin) && !pin_ids.contains(&c.target_pin));

        Ok(removed_ids)
    }

    /// 获取所有连接
    ///
    /// 返回子图中的所有连接
    pub fn get_connections(
        &self,
        subgraph_id: &str,
    ) -> Result<Vec<crate::project::ConnectionDto>, String> {
        let project = self.data.read().unwrap();
        let subgraph = crate::get_subgraph!(project, subgraph_id)
            .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;
        Ok(subgraph.connections.clone())
    }
}
