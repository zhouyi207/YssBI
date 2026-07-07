use crate::schema::{GraphInstanceDTO, VariableInstanceDTO};
use std::collections::HashMap;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadedProjectGraphDTO {
    pub graph: GraphInstanceDTO,
    pub variables: HashMap<String, VariableInstanceDTO>,
}
