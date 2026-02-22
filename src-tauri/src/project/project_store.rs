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
