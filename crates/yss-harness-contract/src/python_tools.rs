//! Model-visible Python scripts of the current project and the port that runs them.

use std::{future::Future, pin::Pin};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{CapabilityContractError, ProjectSessionBinding};

pub const PYTHON_INSPECT_ENVIRONMENT_TOOL: &str = "python_inspect_environment";
pub const PYTHON_READ_SCRIPT_TOOL: &str = "python_read_script";
pub const PYTHON_WRITE_SCRIPT_TOOL: &str = "python_write_script";
pub const PYTHON_INSTALL_PACKAGES_TOOL: &str = "python_install_packages";
pub const PYTHON_RUN_SCRIPT_TOOL: &str = "python_run_script";

pub const MAX_PYTHON_SCRIPT_BYTES: usize = 256 * 1024;
pub const MAX_PYTHON_RUN_SECONDS: u32 = 1_800;
const DEFAULT_PYTHON_RUN_SECONDS: u32 = 300;
const MAX_PYTHON_PACKAGES: usize = 20;
const MAX_PYTHON_ARGUMENTS: usize = 32;

pub fn is_python_tool(name: &str) -> bool {
    matches!(
        name,
        PYTHON_INSPECT_ENVIRONMENT_TOOL
            | PYTHON_READ_SCRIPT_TOOL
            | PYTHON_WRITE_SCRIPT_TOOL
            | PYTHON_INSTALL_PACKAGES_TOOL
            | PYTHON_RUN_SCRIPT_TOOL
    )
}

/// A script file name in the project's `scripts` directory, such as `collect_prices.py`.
fn validate_script_name(name: &str) -> Result<(), CapabilityContractError> {
    let stem = name.strip_suffix(".py").unwrap_or(name);
    let valid = !stem.is_empty()
        && stem.len() <= 80
        && stem
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'));
    if valid {
        Ok(())
    } else {
        Err(CapabilityContractError::InvalidField("name"))
    }
}

/// The file name a valid script name is stored under.
pub fn python_script_file(name: &str) -> String {
    let stem = name.strip_suffix(".py").unwrap_or(name);
    format!("{stem}.py")
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PythonInspectEnvironmentRequest {}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PythonPackage {
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PythonEnvironment {
    /// Whether scripts can run; when false, `setup` says what the user must install.
    pub available: bool,
    pub manager: Option<String>,
    pub python_version: Option<String>,
    pub packages: Vec<PythonPackage>,
    pub scripts: Vec<String>,
    /// Absolute directory a run's files should be written to; also `YSSBI_OUTPUT_DIR`.
    pub output_directory: String,
    pub setup: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PythonReadScriptRequest {
    pub name: String,
}

impl PythonReadScriptRequest {
    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        validate_script_name(&self.name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PythonScript {
    pub name: String,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PythonWriteScriptRequest {
    pub name: String,
    /// The complete script; it replaces any previous content.
    pub content: String,
}

impl PythonWriteScriptRequest {
    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        validate_script_name(&self.name)?;
        if self.content.len() > MAX_PYTHON_SCRIPT_BYTES {
            return Err(CapabilityContractError::InvalidField("content"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PythonScriptWritten {
    pub name: String,
    pub path: String,
    pub bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PythonInstallPackagesRequest {
    /// Requirement specifiers such as `pandas` or `requests>=2.31`.
    pub packages: Vec<String>,
}

impl PythonInstallPackagesRequest {
    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        let valid = |package: &str| {
            let package = package.trim();
            package
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii_alphanumeric())
                && package.len() <= 120
                && package.chars().all(|character| {
                    character.is_ascii_alphanumeric()
                        || matches!(
                            character,
                            '.' | '_' | '-' | '[' | ']' | ',' | '<' | '>' | '=' | '!' | '~'
                        )
                })
        };
        if self.packages.is_empty()
            || self.packages.len() > MAX_PYTHON_PACKAGES
            || !self.packages.iter().all(|package| valid(package))
        {
            return Err(CapabilityContractError::InvalidField("packages"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PythonPackagesInstalled {
    pub requirements: Vec<String>,
    pub output: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PythonRunScriptRequest {
    pub name: String,
    #[serde(default)]
    pub arguments: Vec<String>,
    #[serde(default = "default_run_seconds")]
    #[schemars(range(min = 1, max = 1800))]
    pub timeout_seconds: u32,
}

fn default_run_seconds() -> u32 {
    DEFAULT_PYTHON_RUN_SECONDS
}

impl PythonRunScriptRequest {
    pub fn validate(&self) -> Result<(), CapabilityContractError> {
        validate_script_name(&self.name)?;
        if self.arguments.len() > MAX_PYTHON_ARGUMENTS
            || self.arguments.iter().any(|argument| argument.len() > 1_000)
        {
            return Err(CapabilityContractError::InvalidField("arguments"));
        }
        if self.timeout_seconds == 0 || self.timeout_seconds > MAX_PYTHON_RUN_SECONDS {
            return Err(CapabilityContractError::InvalidField("timeoutSeconds"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PythonOutputFile {
    /// Absolute path; pass it to data import tools.
    pub path: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PythonRun {
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    /// Interleaved stdout and stderr; long output keeps its beginning and end.
    pub output: String,
    pub omitted_lines: usize,
    /// Files created or changed in the output directory by this run.
    pub output_files: Vec<PythonOutputFile>,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PythonFailure {
    #[error("no Python environment is available: {setup}")]
    Unavailable { setup: String },
    #[error("the script does not exist")]
    ScriptNotFound,
    #[error("the project folder is unavailable")]
    ProjectUnavailable,
    #[error("package installation failed")]
    InstallFailed { output: String },
    #[error("the process could not be started")]
    Failed,
}

pub type PythonFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, PythonFailure>> + Send + 'a>>;

/// Project-bound script storage and execution. `cancel` resolves when the call is cancelled;
/// implementations stop the whole process tree then.
pub trait PythonPort: Send + Sync {
    fn inspect(&self, project: &ProjectSessionBinding) -> PythonFuture<'_, PythonEnvironment>;
    fn read_script(
        &self,
        project: &ProjectSessionBinding,
        name: &str,
    ) -> PythonFuture<'_, PythonScript>;
    fn write_script(
        &self,
        project: &ProjectSessionBinding,
        name: &str,
        content: &str,
    ) -> PythonFuture<'_, PythonScriptWritten>;
    fn install_packages(
        &self,
        project: &ProjectSessionBinding,
        packages: &[String],
        cancel: crate::CancellationToken,
    ) -> PythonFuture<'_, PythonPackagesInstalled>;
    fn run_script(
        &self,
        project: &ProjectSessionBinding,
        request: &PythonRunScriptRequest,
        cancel: crate::CancellationToken,
    ) -> PythonFuture<'_, PythonRun>;
}

/// A port for hosts that cannot run Python.
pub struct DisabledPython;

impl PythonPort for DisabledPython {
    fn inspect(&self, _: &ProjectSessionBinding) -> PythonFuture<'_, PythonEnvironment> {
        Box::pin(async { Err(disabled()) })
    }
    fn read_script(&self, _: &ProjectSessionBinding, _: &str) -> PythonFuture<'_, PythonScript> {
        Box::pin(async { Err(disabled()) })
    }
    fn write_script(
        &self,
        _: &ProjectSessionBinding,
        _: &str,
        _: &str,
    ) -> PythonFuture<'_, PythonScriptWritten> {
        Box::pin(async { Err(disabled()) })
    }
    fn install_packages(
        &self,
        _: &ProjectSessionBinding,
        _: &[String],
        _: crate::CancellationToken,
    ) -> PythonFuture<'_, PythonPackagesInstalled> {
        Box::pin(async { Err(disabled()) })
    }
    fn run_script(
        &self,
        _: &ProjectSessionBinding,
        _: &PythonRunScriptRequest,
        _: crate::CancellationToken,
    ) -> PythonFuture<'_, PythonRun> {
        Box::pin(async { Err(disabled()) })
    }
}

fn disabled() -> PythonFailure {
    PythonFailure::Unavailable {
        setup: "Python is not available in this host.".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_names_stay_inside_the_scripts_directory() {
        for (name, valid) in [
            ("collect_prices", true),
            ("collect-prices.py", true),
            ("../secrets", false),
            ("a/b", false),
            (".py", false),
            ("", false),
        ] {
            let request = PythonReadScriptRequest { name: name.into() };
            assert_eq!(request.validate().is_ok(), valid, "{name}");
        }
        assert_eq!(python_script_file("collect.py"), "collect.py");
        assert_eq!(python_script_file("collect"), "collect.py");
    }

    #[test]
    fn package_specifiers_cannot_carry_installer_options() {
        let valid = PythonInstallPackagesRequest {
            packages: vec![
                "pandas".into(),
                "requests>=2.31".into(),
                "akshare[full]".into(),
            ],
        };
        assert!(valid.validate().is_ok());
        for package in ["--index-url=http://evil", "-e .", "pkg @ file:///x", "a b"] {
            let request = PythonInstallPackagesRequest {
                packages: vec![package.into()],
            };
            assert!(request.validate().is_err(), "{package}");
        }
    }
}
