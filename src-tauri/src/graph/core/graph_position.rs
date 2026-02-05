use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphPosition {
    pub x: f64,
    pub y: f64,
    pub scale: f64,
}

impl Default for GraphPosition {
    fn default() -> Self {
        GraphPosition {
            x: 0.0,
            y: 0.0,
            scale: 1.0,
        }
    }
}