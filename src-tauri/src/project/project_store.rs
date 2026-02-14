use crate::database::DatabaseInstance;
use crate::graph::register::catalog::register_builtin_nodes;
use crate::graph::register::NodeRegistry;
use std::collections::HashMap;
use std::sync::Arc;

pub struct ProjectStore {
    pub databases: HashMap<String, DatabaseInstance>,
    pub node_register: Arc<NodeRegistry>,
}

impl Default for ProjectStore {
    fn default() -> Self {
        let node_register = Arc::new(NodeRegistry::new());
        register_builtin_nodes(&node_register);

        // 使用 Tauri 的日志插件，因为此时自定义日志管理器可能还未初始化
        eprintln!("Node registry size: {}", node_register.all().len());


        Self {
            databases: HashMap::new(),
            node_register,
        }
    }
}

impl ProjectStore {
    pub fn new() -> Self {
        Self::default()
    }
}
