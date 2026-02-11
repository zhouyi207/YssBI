use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodePosition {
    pub x: f32,
    pub y: f32,
}

impl Default for NodePosition {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}