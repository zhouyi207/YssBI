use crate::database::DatabaseInstance;
use crate::graph::register::catalog::register_builtin_nodes;
use crate::graph::register::NodeRegistry;
use std::collections::HashMap;

pub struct ProjectStore {
    pub databases: HashMap<String, DatabaseInstance>,
    pub node_register: NodeRegistry,
}

impl Default for ProjectStore {
    fn default() -> Self {
        let mut node_register = NodeRegistry::new();
        register_builtin_nodes(&mut node_register);
        Self {
            databases: HashMap::new(),
            node_register: node_register,
        }
    }
}

impl ProjectStore {
    pub fn new() -> Self {
        Self::default()
    }
}
