use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use super::WORKER_DIR;
use super::error::{JuliaWorkerError, JuliaWorkerErrorCode};

const WORKER_PROJECT: &str = include_str!("../../../julia/Project.toml");
const WORKER_MANIFEST: &str = include_str!("../../../julia/Manifest.toml");
const WORKER_SCRIPT: &str = include_str!("../../../julia/worker.jl");
const WORKER_PROTOCOL: &str = include_str!("../../../julia/worker_protocol.jl");
const WORKER_SCIENTIFIC_RUNTIME: &str = include_str!("../../../julia/scientific_runtime.jl");
const WORKER_BAYES_FIT_OP: &str = include_str!("../../../julia/ops/bayes_fit.jl");
const WORKER_BAYES_EXPRESSION_OP: &str = include_str!("../../../julia/ops/bayes/expression.jl");
const WORKER_BAYES_RUNTIME_OP: &str = include_str!("../../../julia/ops/bayes/runtime.jl");
const WORKER_BAYES_TURING_GENERIC_NORMAL_OP: &str =
    include_str!("../../../julia/ops/bayes/turing_generic_normal.jl");

pub(super) fn ensure_worker_assets(app_data_dir: &Path) -> Result<PathBuf, JuliaWorkerError> {
    let worker_dir = app_data_dir.join(WORKER_DIR);
    fs::create_dir_all(&worker_dir).map_err(|error| {
        JuliaWorkerError::new(
            JuliaWorkerErrorCode::AssetUpdateFailed,
            format!("Failed to create Julia worker directory: {error}"),
        )
    })?;
    write_asset(&worker_dir.join("Project.toml"), WORKER_PROJECT)?;
    write_asset(&worker_dir.join("Manifest.toml"), WORKER_MANIFEST)?;
    write_asset(&worker_dir.join("worker.jl"), WORKER_SCRIPT)?;
    write_asset(&worker_dir.join("worker_protocol.jl"), WORKER_PROTOCOL)?;
    write_asset(
        &worker_dir.join("scientific_runtime.jl"),
        WORKER_SCIENTIFIC_RUNTIME,
    )?;
    let ops_dir = worker_dir.join("ops");
    fs::create_dir_all(&ops_dir).map_err(|error| {
        JuliaWorkerError::new(
            JuliaWorkerErrorCode::AssetUpdateFailed,
            format!("Failed to create Julia worker ops directory: {error}"),
        )
    })?;
    write_asset(&ops_dir.join("bayes_fit.jl"), WORKER_BAYES_FIT_OP)?;
    let bayes_ops_dir = ops_dir.join("bayes");
    fs::create_dir_all(&bayes_ops_dir).map_err(|error| {
        JuliaWorkerError::new(
            JuliaWorkerErrorCode::AssetUpdateFailed,
            format!("Failed to create Julia Bayesian worker ops directory: {error}"),
        )
    })?;
    write_asset(
        &bayes_ops_dir.join("expression.jl"),
        WORKER_BAYES_EXPRESSION_OP,
    )?;
    write_asset(&bayes_ops_dir.join("runtime.jl"), WORKER_BAYES_RUNTIME_OP)?;
    write_asset(
        &bayes_ops_dir.join("turing_generic_normal.jl"),
        WORKER_BAYES_TURING_GENERIC_NORMAL_OP,
    )?;
    Ok(worker_dir)
}

pub(super) fn write_asset(path: &Path, contents: &str) -> Result<(), JuliaWorkerError> {
    if fs::read_to_string(path).ok().as_deref() == Some(contents) {
        return Ok(());
    }

    let (temporary, mut file) = reserve_asset_temporary(path)?;
    let result = (|| {
        file.write_all(contents.as_bytes()).map_err(|error| {
            JuliaWorkerError::new(
                JuliaWorkerErrorCode::AssetUpdateFailed,
                format!("Failed to write Julia worker asset: {error}"),
            )
        })?;
        file.sync_all().map_err(|error| {
            JuliaWorkerError::new(
                JuliaWorkerErrorCode::AssetUpdateFailed,
                format!("Failed to synchronize Julia worker asset: {error}"),
            )
        })?;
        drop(file);
        atomic_replace_asset(&temporary, path).map_err(|error| {
            JuliaWorkerError::new(
                JuliaWorkerErrorCode::AssetUpdateFailed,
                format!("Failed to publish Julia worker asset: {error}"),
            )
        })
    })();

    if result.is_err()
        && let Err(error) = cleanup_asset_temporary(&temporary)
    {
        tracing::warn!(
            target: "yssbi::julia::worker",
            diagnostic_domain = "execution",
            error = %error,
            "Failed to clean Julia worker asset temporary file"
        );
    }
    result
}

fn reserve_asset_temporary(path: &Path) -> Result<(PathBuf, File), JuliaWorkerError> {
    let parent = path.parent().ok_or_else(|| {
        JuliaWorkerError::new(
            JuliaWorkerErrorCode::AssetUpdateFailed,
            "Julia worker asset has no parent directory.",
        )
    })?;
    let file_name = path.file_name().ok_or_else(|| {
        JuliaWorkerError::new(
            JuliaWorkerErrorCode::AssetUpdateFailed,
            "Julia worker asset has no file name.",
        )
    })?;
    for _ in 0..8 {
        let temporary = parent.join(format!(
            ".{}.{}.tmp",
            file_name.to_string_lossy(),
            Uuid::new_v4()
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => return Ok((temporary, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(JuliaWorkerError::new(
                    JuliaWorkerErrorCode::AssetUpdateFailed,
                    format!("Failed to reserve Julia worker asset temporary file: {error}"),
                ));
            }
        }
    }
    Err(JuliaWorkerError::new(
        JuliaWorkerErrorCode::AssetUpdateFailed,
        "Unable to reserve a Julia worker asset temporary file.",
    ))
}

fn cleanup_asset_temporary(temporary: &Path) -> std::io::Result<()> {
    match fs::remove_file(temporary) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(not(windows))]
fn atomic_replace_asset(temporary: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(temporary, destination)
}

#[cfg(windows)]
fn atomic_replace_asset(temporary: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let source = temporary
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let target = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let replaced = unsafe {
        MoveFileExW(
            source.as_ptr(),
            target.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if replaced == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}
