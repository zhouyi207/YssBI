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
    pub fn delete_node(&self, subgraph_id: &str, node_id: &str) -> Result<(), String> {
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
    pub fn connect_pins(
        &self,
        subgraph_id: &str,
        source_pin_id: &str,
        target_pin_id: &str,
    ) -> Result<Vec<SerializedNode>, String> {
        use crate::schema::pin_types::can_connect;
        use crate::executor::value::{PinTypeDesc, TypeInferenceContext};
        use uuid::Uuid;

        let mut project = self.data.write().unwrap();
        let subgraph = crate::get_subgraph_mut!(project, subgraph_id)
            .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;

        // 找到源和目标 Pin
        let mut source_info: Option<(usize, bool, usize, String)> = None; // (node_idx, is_input, pin_idx, type)
        let mut target_info: Option<(usize, bool, usize, String)> = None;

        for (node_idx, node) in subgraph.nodes.iter().enumerate() {
            for (pin_idx, pin) in node.inputs.iter().enumerate() {
                if pin.id == source_pin_id {
                    source_info = Some((node_idx, true, pin_idx, pin.pin_type.clone()));
                }
                if pin.id == target_pin_id {
                    target_info = Some((node_idx, true, pin_idx, pin.pin_type.clone()));
                }
            }
            for (pin_idx, pin) in node.outputs.iter().enumerate() {
                if pin.id == source_pin_id {
                    source_info = Some((node_idx, false, pin_idx, pin.pin_type.clone()));
                }
                if pin.id == target_pin_id {
                    target_info = Some((node_idx, false, pin_idx, pin.pin_type.clone()));
                }
            }
        }

        let source = source_info.ok_or_else(|| format!("Source pin '{}' not found", source_pin_id))?;
        let target = target_info.ok_or_else(|| format!("Target pin '{}' not found", target_pin_id))?;

        // 验证方向：一个必须是输出，一个必须是输入
        // is_input = true 表示在 inputs 数组中（即是输入 pin）
        // 输出 pin 应该在 outputs 数组中 (is_input = false)
        let (output_info, input_info) = if !source.1 && target.1 {
            // source 是输出，target 是输入
            (source, target)
        } else if source.1 && !target.1 {
            // source 是输入，target 是输出 (交换)
            (target, source)
        } else {
            return Err("Cannot connect: pins must have different directions (one input, one output)".to_string());
        };

        // ✅ 新增：使用类型推断系统进行类型检查
        let output_type = &output_info.3;
        let input_type = &input_info.3;
        
        // 创建临时的类型推断上下文
        let mut type_inference = TypeInferenceContext::new();
        
        // 生成临时的 PinId（用于类型推断）
        let temp_output_pin_id = Uuid::new_v4();
        let temp_input_pin_id = Uuid::new_v4();
        
        // 注册 Pin 类型
        type_inference.register_pin(
            temp_output_pin_id,
            PinTypeDesc::from_string(output_type)
        );
        type_inference.register_pin(
            temp_input_pin_id,
            PinTypeDesc::from_string(input_type)
        );
        
        // 尝试推断连接
        match type_inference.infer_connection(temp_output_pin_id, temp_input_pin_id) {
            Ok(_) => {
                // 类型推断成功，允许连接
            }
            Err(e) => {
                // 类型推断失败，回退到旧的类型检查
                if !can_connect(output_type, input_type) {
                    return Err(format!(
                        "Cannot connect: type '{}' is not compatible with type '{}' ({})",
                        output_type, input_type, e
                    ));
                }
                // 旧的类型检查通过，允许连接（向后兼容）
            }
        }

        // 对于输入 pin（单连接），先移除旧连接
        let old_links_to_remove: Vec<String>;
        {
            let input_node = &subgraph.nodes[input_info.0];
            let input_pin = &input_node.inputs[input_info.2];
            old_links_to_remove = input_pin.links.clone();
        }

        // 从其他 pin 移除指向这个输入 pin 的连接
        for old_link in &old_links_to_remove {
            for node in subgraph.nodes.iter_mut() {
                for pin in node.outputs.iter_mut() {
                    if pin.id == *old_link {
                        pin.links.retain(|l| l != &input_info.3);
                    }
                }
            }
        }

        // 更新连接
        // 1. 输出 pin 添加对输入 pin 的引用
        let output_pin_id = if !output_info.1 {
            subgraph.nodes[output_info.0].outputs[output_info.2].id.clone()
        } else {
            subgraph.nodes[output_info.0].inputs[output_info.2].id.clone()
        };
        let input_pin_id = if input_info.1 {
            subgraph.nodes[input_info.0].inputs[input_info.2].id.clone()
        } else {
            subgraph.nodes[input_info.0].outputs[input_info.2].id.clone()
        };

        // 添加链接到输出 pin
        {
            let output_node = &mut subgraph.nodes[output_info.0];
            let output_pin = &mut output_node.outputs[output_info.2];
            if !output_pin.links.contains(&input_pin_id) {
                output_pin.links.push(input_pin_id.clone());
            }
        }

        // 设置输入 pin 的链接（单连接，覆盖旧值）
        {
            let input_node = &mut subgraph.nodes[input_info.0];
            let input_pin = &mut input_node.inputs[input_info.2];
            input_pin.links = vec![output_pin_id.clone()];
        }

        Ok(subgraph.nodes.clone())
    }

    /// 断开 Pin 的所有连接
    pub fn disconnect_pin(
        &self,
        subgraph_id: &str,
        pin_id: &str,
    ) -> Result<Vec<SerializedNode>, String> {
        let mut project = self.data.write().unwrap();
        let subgraph = crate::get_subgraph_mut!(project, subgraph_id)
            .ok_or_else(|| format!("Subgraph '{}' not found", subgraph_id))?;

        // 收集需要移除的链接
        let mut links_to_remove: Vec<String> = Vec::new();

        // 找到目标 pin 并收集其链接
        for node in subgraph.nodes.iter() {
            for pin in node.inputs.iter().chain(node.outputs.iter()) {
                if pin.id == pin_id {
                    links_to_remove.extend(pin.links.clone());
                }
            }
        }

        // 清除目标 pin 的链接，并从连接的 pin 中移除引用
        for node in subgraph.nodes.iter_mut() {
            for pin in node.inputs.iter_mut().chain(node.outputs.iter_mut()) {
                if pin.id == pin_id {
                    pin.links.clear();
                } else if links_to_remove.contains(&pin.id) || pin.links.contains(&pin_id.to_string()) {
                    pin.links.retain(|l| l != pin_id);
                }
            }
        }

        Ok(subgraph.nodes.clone())
    }
}
