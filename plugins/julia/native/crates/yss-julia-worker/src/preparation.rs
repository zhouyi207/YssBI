use std::io::Read;
use std::path::Path;
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::{
    JuliaWorkerError, JuliaWorkerErrorCode, assets::ensure_worker_assets,
    configure_dependency_cache,
};
use yss_julia_runtime::{
    background_command, command_output_failure_detail, system_julia_executable,
};

pub(super) fn cancelled() -> JuliaWorkerError {
    JuliaWorkerError::new(
        JuliaWorkerErrorCode::Cancelled,
        "Julia preparation was cancelled.",
    )
}

pub(super) fn run(root: &Path, cancellation: &AtomicBool) -> Result<(), JuliaWorkerError> {
    if cancellation.load(Ordering::Acquire) {
        return Err(cancelled());
    }
    let directory = ensure_worker_assets(root)?;
    let executable = system_julia_executable().map_err(|error| {
        JuliaWorkerError::new(JuliaWorkerErrorCode::RuntimeUnavailable, error.to_string())
    })?;
    let mut command = background_command(executable);
    configure_dependency_cache(&mut command, &directory);
    let mut child = command
        .arg(format!("--project={}", directory.display()))
        .args(["--startup-file=no", "-e", "using Pkg; Pkg.instantiate()"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            JuliaWorkerError::new(
                JuliaWorkerErrorCode::EnvironmentUnavailable,
                error.to_string(),
            )
        })?;
    let stdout = capture(child.stdout.take().unwrap());
    let stderr = capture(child.stderr.take().unwrap());
    let status = loop {
        if cancellation.load(Ordering::Acquire) {
            terminate_child(&mut child);
            let _ = stdout.join();
            let _ = stderr.join();
            return Err(cancelled());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::park_timeout(Duration::from_millis(50)),
            Err(error) => {
                terminate_child(&mut child);
                return Err(JuliaWorkerError::new(
                    JuliaWorkerErrorCode::EnvironmentUnavailable,
                    error.to_string(),
                ));
            }
        }
    };
    let output = std::process::Output {
        status,
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    };
    if output.status.success() {
        Ok(())
    } else {
        Err(JuliaWorkerError::new(
            JuliaWorkerErrorCode::EnvironmentUnavailable,
            command_output_failure_detail(&output),
        ))
    }
}

fn capture(mut reader: impl Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut tail = Vec::new();
        let mut buffer = [0; 4096];
        while let Ok(count) = reader.read(&mut buffer) {
            if count == 0 {
                break;
            }
            tail.extend_from_slice(&buffer[..count]);
            if tail.len() > 64 * 1024 {
                tail.drain(..tail.len() - 64 * 1024);
            }
        }
        tail
    })
}

pub(super) fn terminate_child(child: &mut Child) {
    #[cfg(windows)]
    {
        // The retained Child handle pins the process identity. Stop only this worker tree.
        let mut command = background_command("taskkill");
        let _ = command
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}
