use std::fs;
use std::path::{Path, PathBuf};

use super::error::{JuliaWorkerError, JuliaWorkerErrorCode};
use super::{TASK_DIR, WORKER_DIR};

#[derive(Debug, PartialEq, Eq)]
pub struct JuliaWorkerTaskDirectory {
    app_root: PathBuf,
    tasks_root: PathBuf,
    path: PathBuf,
    task_id: String,
}

impl JuliaWorkerTaskDirectory {
    pub(crate) fn create(app_data_dir: &Path, task_id: &str) -> Result<Self, JuliaWorkerError> {
        validate_task_id(task_id)?;
        fs::create_dir_all(app_data_dir).map_err(|error| {
            JuliaWorkerError::new(
                JuliaWorkerErrorCode::TaskDirectoryCreateFailed,
                format!("Failed to create Julia app data directory: {error}"),
            )
        })?;
        let app_root = fs::canonicalize(app_data_dir).map_err(|error| {
            JuliaWorkerError::new(
                JuliaWorkerErrorCode::TaskDirectoryCreateFailed,
                format!("Failed to resolve Julia app data directory: {error}"),
            )
        })?;
        let requested_tasks_root = app_data_dir.join(WORKER_DIR).join(TASK_DIR);
        fs::create_dir_all(&requested_tasks_root).map_err(|error| {
            JuliaWorkerError::new(
                JuliaWorkerErrorCode::TaskDirectoryCreateFailed,
                format!("Failed to create Julia tasks directory: {error}"),
            )
        })?;
        let tasks_root = fs::canonicalize(&requested_tasks_root).map_err(|error| {
            JuliaWorkerError::new(
                JuliaWorkerErrorCode::TaskDirectoryCreateFailed,
                format!("Failed to resolve Julia tasks directory: {error}"),
            )
        })?;
        let expected_tasks_root = app_root.join(WORKER_DIR).join(TASK_DIR);
        if tasks_root != expected_tasks_root {
            return Err(JuliaWorkerError::new(
                JuliaWorkerErrorCode::TaskDirectoryInvalid,
                "Julia tasks directory is not an app-owned canonical descendant.",
            ));
        }

        let requested_path = requested_tasks_root.join(task_id);
        fs::create_dir(&requested_path).map_err(|error| {
            JuliaWorkerError::new(
                JuliaWorkerErrorCode::TaskDirectoryCreateFailed,
                format!("Failed to create Julia task directory: {error}"),
            )
        })?;
        let path = fs::canonicalize(&requested_path).map_err(|error| {
            JuliaWorkerError::new(
                JuliaWorkerErrorCode::TaskDirectoryCreateFailed,
                format!("Failed to resolve Julia task directory: {error}"),
            )
        })?;
        if path != tasks_root.join(task_id) || path.parent() != Some(tasks_root.as_path()) {
            return Err(JuliaWorkerError::new(
                JuliaWorkerErrorCode::TaskDirectoryInvalid,
                "Julia task directory is not the exact canonical task descendant.",
            ));
        }

        Ok(Self {
            app_root,
            tasks_root,
            path,
            task_id: task_id.to_string(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn task_id(&self) -> &str {
        &self.task_id
    }

    pub fn cleanup(&self) -> Result<(), JuliaWorkerError> {
        match fs::symlink_metadata(&self.path) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(task_cleanup_error(error)),
        }

        let current_app_root = fs::canonicalize(&self.app_root).map_err(task_cleanup_error)?;
        let current_tasks_root = fs::canonicalize(current_app_root.join(WORKER_DIR).join(TASK_DIR))
            .map_err(task_cleanup_error)?;
        let current_path = fs::canonicalize(&self.path).map_err(task_cleanup_error)?;
        if current_app_root != self.app_root
            || current_tasks_root != self.tasks_root
            || current_tasks_root != current_app_root.join(WORKER_DIR).join(TASK_DIR)
            || current_path != self.path
            || current_path != current_tasks_root.join(&self.task_id)
            || current_path.parent() != Some(current_tasks_root.as_path())
        {
            return Err(JuliaWorkerError::new(
                JuliaWorkerErrorCode::TaskDirectoryInvalid,
                "Julia task cleanup target no longer has its exact canonical ownership.",
            ));
        }

        fs::remove_dir_all(&current_path).map_err(task_cleanup_error)
    }
}

impl Drop for JuliaWorkerTaskDirectory {
    fn drop(&mut self) {
        if let Err(error) = self.cleanup() {
            tracing::warn!(
                target: "yssbi::julia::worker",
                diagnostic_domain = "execution",
                error_code = error.code().as_str(),
                diagnostic = error.diagnostic(),
                "Failed to clean Julia worker task directory"
            );
        }
    }
}

fn validate_task_id(task_id: &str) -> Result<(), JuliaWorkerError> {
    let valid = !task_id.is_empty()
        && task_id.len() <= 128
        && task_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'));
    if valid {
        Ok(())
    } else {
        Err(JuliaWorkerError::new(
            JuliaWorkerErrorCode::TaskDirectoryInvalid,
            "Julia worker task ID is not canonical.",
        ))
    }
}

fn task_cleanup_error(error: std::io::Error) -> JuliaWorkerError {
    JuliaWorkerError::new(
        JuliaWorkerErrorCode::TaskDirectoryCleanupFailed,
        format!("Failed to clean Julia task directory: {error}"),
    )
}
