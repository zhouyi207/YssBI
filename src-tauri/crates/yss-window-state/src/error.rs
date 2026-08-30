use std::io;

#[derive(Debug, thiserror::Error)]
pub enum WindowStateError {
    #[error("window state path has no parent directory")]
    MissingParentDirectory,
    #[error("failed to create the window state directory")]
    CreateDirectory(#[source] io::Error),
    #[error("failed to serialize window state")]
    Serialize(#[source] serde_json::Error),
    #[error("failed to reserve a window state temporary file")]
    ReserveTemporaryFile(#[source] io::Error),
    #[error("failed to persist window state")]
    Persist(#[source] io::Error),
    #[error("failed to persist window state and remove its temporary file: {cleanup}")]
    PersistAndCleanup {
        #[source]
        source: io::Error,
        cleanup: io::Error,
    },
    #[error("main window was not found")]
    MainWindowNotFound,
    #[error("failed to {operation} the main window")]
    MainWindowOperation {
        operation: &'static str,
        #[source]
        source: tauri::Error,
    },
}
