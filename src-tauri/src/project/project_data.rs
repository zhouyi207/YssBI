use std::collections::HashMap;
use crate::variable::VariableDefinition;
use super::ProjectMetadata;
use crate::graph::{GraphId, GraphData};

pub struct ProjectData {
    pub variables: HashMap<String, VariableDefinition>,
    pub graphs: HashMap<GraphId, GraphData>,
    pub metadata: ProjectMetadata,
}