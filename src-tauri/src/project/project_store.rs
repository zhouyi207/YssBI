use crate::database::DatabaseInstance;
use std::collections::HashMap;

pub struct ProjectStore {
    pub databases: HashMap<String, DatabaseInstance>,
}

impl Default for ProjectStore {
    fn default() -> Self {
        Self {
            databases: HashMap::new(),
        }
    }
}

impl ProjectStore {
    pub fn new() -> Self {
        Self::default()
    }
}
