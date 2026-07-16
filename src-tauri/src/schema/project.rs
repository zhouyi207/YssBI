use super::{DatabaseDeclDTO, VariableInstanceDTO};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 分阶段加载：databases + variables（第一步）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabasesVariablesDTO {
    pub databases: HashMap<String, DatabaseDeclDTO>,
    pub variables: HashMap<String, VariableInstanceDTO>,
}
