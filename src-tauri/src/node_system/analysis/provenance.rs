use super::{CompilationBasis, CompileId};
use crate::node_system::ProjectSessionId;
use crate::node_system::document::{GraphResourcePath, GraphRevision};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompileProvenance {
    pub project_session_id: ProjectSessionId,
    pub graph_path: GraphResourcePath,
    pub basis: CompilationBasis<GraphRevision>,
    pub compile_id: CompileId,
}
