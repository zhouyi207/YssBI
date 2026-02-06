use crate::project::ProjectError;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct FrontendError {
    pub code: String,
    pub message: String,
}

impl From<ProjectError> for FrontendError {
    fn from(err: ProjectError) -> Self {
        match err {
            ProjectError::FileNotFound(path) => FrontendError {
                code: "FILE_NOT_FOUND".into(),
                message: format!("File not found: {}", path.display()),
            },

            ProjectError::Deserialize(_) => FrontendError {
                code: "INVALID_PROJECT".into(),
                message: "Project file is invalid or corrupted".into(),
            },

            ProjectError::Io(_) => FrontendError {
                code: "IO_ERROR".into(),
                message: "Failed to read project file".into(),
            },

            _ => FrontendError {
                code: "UNKNOWN".into(),
                message: "Unknown error".into(),
            },
        }
    }
}
