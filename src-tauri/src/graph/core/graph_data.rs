use super::{GraphPosition, GraphKind};
use serde::{Deserialize, Serialize};

/// 子图数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphData {
    pub id: String,
    pub name: String,
    pub position: GraphPosition,
    pub kind: GraphKind,
}
