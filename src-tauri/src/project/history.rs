use serde::{Deserialize, Serialize};

use crate::graph_document::{GraphDocument, GraphResourcePath, GraphRevision};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectGraphResidency {
    Loaded,
    Unloaded,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectGraphHistoryState {
    pub document: GraphDocument,
    pub revision: GraphRevision,
    pub residency: ProjectGraphResidency,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectGraphHistoryChange {
    pub graph_path: GraphResourcePath,
    pub before: ProjectGraphHistoryState,
    pub after: ProjectGraphHistoryState,
}

impl ProjectGraphHistoryChange {
    pub fn inverse(&self) -> Self {
        Self {
            graph_path: self.graph_path.clone(),
            before: self.after.clone(),
            after: self.before.clone(),
        }
    }
}
