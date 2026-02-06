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
}
