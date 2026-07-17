//! System Julia discovery and installation.
//!
//! YssBI uses the Julia executable available to the operating system. It does
//! not maintain a separate project-local Julia runtime.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub mod worker;

const VERSION_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JuliaRuntimeState {
    Missing,
    Ready,
    Invalid,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct JuliaRuntimeStatus {
    pub state: JuliaRuntimeState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
enum RuntimeProbe {
    Missing,
    Ready {
        executable: PathBuf,
        version: String,
    },
    Invalid {
        executable: PathBuf,
        message: String,
    },
}

/// Inspects the Julia executable configured for the current system.
pub fn get_runtime_status() -> JuliaRuntimeStatus {
    status_from_probe(inspect_system_julia())
}

/// Installs the latest Julia release for the system, then returns its status.
pub fn install_latest_julia() -> Result<JuliaRuntimeStatus, String> {
    if let status @ JuliaRuntimeStatus {
        state: JuliaRuntimeState::Ready,
        ..
    } = get_runtime_status()
    {
        return Ok(status);
    }

    install_juliaup()?;
    let status = get_runtime_status();
    if status.state == JuliaRuntimeState::Ready {
        Ok(status)
    } else {
        Err(status.message.unwrap_or_else(|| {
            "Julia installation completed, but Julia could not be started.".to_string()
        }))
    }
}

fn inspect_system_julia() -> RuntimeProbe {
    let Some(executable) = find_system_julia() else {
        return RuntimeProbe::Missing;
    };

    match julia_version(&executable) {
        Ok(version) => RuntimeProbe::Ready {
            executable,
            version,
        },
        Err(message) => RuntimeProbe::Invalid {
            executable,
            message,
        },
    }
}

pub fn system_julia_executable() -> Result<PathBuf, String> {
    find_system_julia().ok_or_else(|| "Julia was not found on the system PATH.".to_string())
}

fn find_system_julia() -> Option<PathBuf> {
    find_executable_in_path(julia_executable_name()).or_else(windows_juliaup_alias)
}

fn julia_executable_name() -> &'static str {
    if cfg!(windows) { "julia.exe" } else { "julia" }
}

fn find_executable_in_path(executable_name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(executable_name))
        .find(|candidate| candidate.is_file())
}

fn windows_juliaup_alias() -> Option<PathBuf> {
    windows_app_execution_alias(julia_executable_name())
}

fn windows_app_execution_alias(executable_name: &str) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let local_app_data = std::env::var_os("LOCALAPPDATA")?;
        let alias = PathBuf::from(local_app_data)
            .join("Microsoft")
            .join("WindowsApps")
            .join(executable_name);
        alias.is_file().then_some(alias)
    }

    #[cfg(not(windows))]
    {
        let _ = executable_name;
        None
    }
}

fn juliaup_command() -> Command {
    windows_app_execution_alias("juliaup.exe")
        .map(Command::new)
        .unwrap_or_else(|| Command::new("juliaup"))
}

fn install_juliaup() -> Result<(), String> {
    #[cfg(windows)]
    {
        run_command(
            Command::new("winget").args([
                "install",
                "--id",
                "JuliaLang.Juliaup",
                "--exact",
                "--source",
                "winget",
                "--accept-package-agreements",
                "--accept-source-agreements",
            ]),
            "install Juliaup",
        )?;
        run_command(
            juliaup_command().args(["add", "release"]),
            "install the latest Julia release",
        )?;
        run_command(
            juliaup_command().args(["default", "release"]),
            "select the latest Julia release",
        )
    }

    #[cfg(not(windows))]
    {
        Err("Automatic Julia installation is currently supported on Windows only. Install Julia from https://julialang.org/downloads/ and refresh the status.".to_string())
    }
}

fn run_command(command: &mut Command, operation: &str) -> Result<(), String> {
    let output = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("Failed to {operation}: {error}"))?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let detail = (!stderr.is_empty())
        .then_some(stderr)
        .unwrap_or_else(|| String::from_utf8_lossy(&output.stdout).trim().to_string());
    Err(format!("Failed to {operation}: {detail}"))
}

fn status_from_probe(probe: RuntimeProbe) -> JuliaRuntimeStatus {
    match probe {
        RuntimeProbe::Missing => JuliaRuntimeStatus {
            state: JuliaRuntimeState::Missing,
            version: None,
            install_dir: None,
            message: Some("Julia was not found on the system PATH.".to_string()),
        },
        RuntimeProbe::Ready {
            executable,
            version,
        } => JuliaRuntimeStatus {
            state: JuliaRuntimeState::Ready,
            version: Some(version),
            install_dir: executable
                .parent()
                .map(|path| path.to_string_lossy().into_owned()),
            message: None,
        },
        RuntimeProbe::Invalid {
            executable,
            message,
        } => JuliaRuntimeStatus {
            state: JuliaRuntimeState::Invalid,
            version: None,
            install_dir: executable
                .parent()
                .map(|path| path.to_string_lossy().into_owned()),
            message: Some(message),
        },
    }
}

fn julia_version(executable: &Path) -> Result<String, String> {
    let mut child = Command::new(executable)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Failed to start Julia: {error}"))?;

    let started = Instant::now();
    loop {
        if child
            .try_wait()
            .map_err(|error| format!("Failed while checking Julia: {error}"))?
            .is_some()
        {
            let output = child
                .wait_with_output()
                .map_err(|error| format!("Failed to read Julia version: {error}"))?;
            if !output.status.success() {
                return Err(format!(
                    "Julia --version exited with status {}.",
                    output.status
                ));
            }
            let stdout = String::from_utf8_lossy(&output.stdout);
            return parse_julia_version(&stdout)
                .ok_or_else(|| "Julia did not report a recognizable version.".to_string());
        }

        if started.elapsed() >= VERSION_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Julia --version timed out after 5 seconds.".to_string());
        }

        thread::sleep(Duration::from_millis(25));
    }
}

fn parse_julia_version(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        line.trim()
            .strip_prefix("julia version ")
            .map(str::trim)
            .filter(|version| !version.is_empty())
            .map(ToOwned::to_owned)
    })
}

#[cfg(test)]
mod tests {
    use super::{
        JuliaRuntimeState, RuntimeProbe, julia_executable_name, parse_julia_version,
        status_from_probe,
    };
    use std::path::PathBuf;

    #[test]
    fn parses_julia_version_output() {
        assert_eq!(
            parse_julia_version("julia version 1.11.3\n"),
            Some("1.11.3".to_string())
        );
        assert_eq!(parse_julia_version("not julia\n"), None);
    }

    #[test]
    fn ready_status_reports_the_system_executable_directory() {
        let executable = PathBuf::from("system")
            .join("bin")
            .join(julia_executable_name());
        let status = status_from_probe(RuntimeProbe::Ready {
            executable,
            version: "1.11.3".to_string(),
        });

        assert_eq!(status.state, JuliaRuntimeState::Ready);
        assert_eq!(
            status.install_dir.as_deref(),
            Some(
                PathBuf::from("system")
                    .join("bin")
                    .to_string_lossy()
                    .as_ref()
            )
        );
        assert_eq!(status.version.as_deref(), Some("1.11.3"));
    }

    #[test]
    fn missing_status_does_not_refer_to_a_project_runtime_directory() {
        let status = status_from_probe(RuntimeProbe::Missing);
        assert_eq!(status.state, JuliaRuntimeState::Missing);
        assert_eq!(status.install_dir, None);
    }
}
