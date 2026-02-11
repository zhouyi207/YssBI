//! Node 注册中心

use crate::graph::node::NodeDefinition;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Node 注册中心
///
/// 管理所有节点定义（原型）
#[derive(Serialize, Deserialize)]
pub struct NodeRegistry {
    definitions: RwLock<HashMap<String, Arc<NodeDefinition>>>,
}

impl NodeRegistry {
    pub fn new() -> Self {
        Self {
            definitions: RwLock::new(HashMap::new()),
        }
    }

    /// 注册节点定义
    pub fn register(&self, definition: NodeDefinition) {
        let mut category = definition.category.clone();
        category.push(definition.name.clone());
        let node_type = category.join(":");
        self.definitions
            .write()
            .unwrap()
            .insert(node_type, Arc::new(definition));
    }

    /// 获取节点定义
    pub fn get(&self, node_type: &str) -> Option<Arc<NodeDefinition>> {
        self.definitions.read().unwrap().get(node_type).cloned()
    }

    /// 获取所有节点定义
    pub fn all(&self) -> Vec<Arc<NodeDefinition>> {
        self.definitions.read().unwrap().values().cloned().collect()
    }

    /// 获取所有节点类型
    pub fn node_types(&self) -> Vec<String> {
        self.definitions.read().unwrap().keys().cloned().collect()
    }

    /// 检查节点类型是否存在
    pub fn contains(&self, node_type: &str) -> bool {
        self.definitions.read().unwrap().contains_key(node_type)
    }
}

impl Default for NodeRegistry {
    fn default() -> Self {
        Self::new()
    }
}
