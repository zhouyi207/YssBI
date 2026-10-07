use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("failed to serialize project data")]
    Serialize(#[source] serde_json::Error),

    #[error("failed to deserialize project data")]
    Deserialize(#[source] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("project file not found: {0}")]
    FileNotFound(PathBuf),

    #[error("invalid project format: {0}")]
    InvalidProjectFormat(String),

    #[error("graph file '{}' is structurally invalid", path.display())]
    InvalidGraphDocument { path: PathBuf, message: String },
}

impl From<yss_graph_document::GraphResourcePathError> for ProjectError {
    fn from(source: yss_graph_document::GraphResourcePathError) -> Self {
        Self::InvalidProjectFormat(source.to_string())
    }
}
