use thiserror::Error;
use yss_graph_document::GraphResourcePath;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphCompileErrorCode {
    InvalidDocument,
    CyclicDataDependency,
    SemanticTypeUnresolved,
    LoweringInvariant,
}

#[derive(Debug, Error)]
pub enum GraphCompileError {
    #[error("graph is invalid for compilation")]
    InvalidGraph {
        graph: GraphResourcePath,
        code: GraphCompileErrorCode,
    },
}
