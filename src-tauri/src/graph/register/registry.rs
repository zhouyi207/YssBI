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

// 手动实现 Clone，因为 RwLock 不支持 Clone
impl Clone for NodeRegistry {
    fn clone(&self) -> Self {
        let definitions = self.definitions.read().unwrap();
        Self {
            definitions: RwLock::new(definitions.clone()),
        }
    }
}

// 手动实现 Debug
impl std::fmt::Debug for NodeRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let definitions = self.definitions.read().unwrap();
        f.debug_struct("NodeRegistry")
            .field("definitions_count", &definitions.len())
            .finish()
    }
}

impl NodeRegistry {
    pub fn new() -> Self {
        Self {
            definitions: RwLock::new(HashMap::new()),
        }
    }

    /// 注册节点定义
    pub fn register(&self, definition: NodeDefinition) {

        self.definitions
            .write()
            .unwrap()
            .insert(definition.node_type.clone(), Arc::new(definition));
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
