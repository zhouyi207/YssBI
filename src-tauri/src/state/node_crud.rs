//! 节点 CRUD 操作

use super::project_state::ProjectState;
use crate::project::{CanvasState, PinDefinition, SerializedNode};

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
        inputs: Option<Vec<PinDefinition>>,
        outputs: Option<Vec<PinDefinition>>,
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
}
