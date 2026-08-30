use super::{CompilationBasis, CompileId};
use serde::{Deserialize, Serialize};
use yss_graph_document::{GraphResourcePath, GraphRevision};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GraphSessionId(Box<str>);

impl GraphSessionId {
    pub fn new(value: impl Into<Box<str>>) -> Self {
        Self(value.into())
    }

    pub fn unknown() -> Self {
        Self::new("unknown")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for GraphSessionId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompileProvenance {
    pub project_session_id: GraphSessionId,
    pub graph_path: GraphResourcePath,
    pub basis: CompilationBasis<GraphRevision>,
    pub compile_id: CompileId,
}
