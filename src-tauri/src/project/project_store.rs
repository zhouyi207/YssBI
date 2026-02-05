use std::{
    collections::HashMap,
};

pub struct ProjectStore {
    pub df: HashMap<String, String>,
}

impl Default for ProjectStore {
    fn default() -> Self {
        Self {
            df: HashMap::new(),
        }
    }
}

impl ProjectStore {
    pub fn new() -> Self {
        Self {
            df: HashMap::new(),
        }
    }
}
