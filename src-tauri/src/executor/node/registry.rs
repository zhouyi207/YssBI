use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};
use super::implementation::GenericNode;
use super::definition::NodeDefinition;

/// 节点注册中心
/// 
/// 现在直接存储 GenericNode 实例作为原型
pub struct NodeRegistry {
    prototypes: RwLock<HashMap<String, Arc<GenericNode>>>,
}

impl NodeRegistry {
    pub fn new() -> Self {
        let registry = Self {
            prototypes: RwLock::new(HashMap::new()),
        };
        registry
    }

    /// 注册节点原型
    pub fn register(&self, node_type: String, proto: Arc<GenericNode>) {
        self.prototypes.write().unwrap().insert(node_type, proto);
    }

    /// 获取节点原型
    pub fn get_prototype(&self, node_type: &str) -> Option<Arc<GenericNode>> {
        self.prototypes.read().unwrap().get(node_type).cloned()
    }

    /// 获取所有节点的定义（用于前端序列化）
    pub fn get_all_definitions(&self) -> Vec<NodeDefinition> {
        let protos = self.prototypes.read().unwrap();
        protos.values()
            .map(|proto| proto.to_definition())
            .collect()
    }
}

pub static REGISTRY: OnceLock<NodeRegistry> = OnceLock::new();

pub fn get_registry() -> &'static NodeRegistry {
    REGISTRY.get_or_init(|| {
        let registry = NodeRegistry::new();
        // 这里可以调用各模块的 register 函数
        super::catalog::register_builtin_nodes(&registry);
        registry
    })
}

/// 获取所有节点定义
pub fn get_all_node_definitions() -> Vec<NodeDefinition> {
    get_registry().get_all_definitions()
}
