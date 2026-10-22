//! Project Python scripts executed by the Harness through the host's Python port.

use serde_json::Value;
use yss_harness_contract::{
    CapabilityControl, CapabilityFailure, CapabilityFailureCode, PYTHON_INSPECT_ENVIRONMENT_TOOL,
    PYTHON_INSTALL_PACKAGES_TOOL, PYTHON_READ_SCRIPT_TOOL, PYTHON_RUN_SCRIPT_TOOL,
    PYTHON_WRITE_SCRIPT_TOOL, ProjectSessionBinding, PythonFailure, PythonInstallPackagesRequest,
    PythonPort, PythonReadScriptRequest, PythonRunScriptRequest, PythonWriteScriptRequest,
};

pub(crate) async fn invoke(
    python: &dyn PythonPort,
    tool: &str,
    request: &Value,
    project: &ProjectSessionBinding,
    control: &CapabilityControl,
) -> Result<Value, CapabilityFailure> {
    control.check()?;
    let cancel = control.cancellation().clone();
    let work = async {
        let encode = |value: Result<Value, serde_json::Error>| {
            value.map_err(|_| CapabilityFailure::new(CapabilityFailureCode::INTERNAL_FAILURE))
        };
        match tool {
            PYTHON_INSPECT_ENVIRONMENT_TOOL => encode(serde_json::to_value(
                python.inspect(project).await.map_err(failure)?,
            )),
            PYTHON_READ_SCRIPT_TOOL => {
                let request: PythonReadScriptRequest = decode(tool, request)?;
                request
                    .validate()
                    .map_err(|error| error.into_failure(tool, "name"))?;
                encode(serde_json::to_value(
                    python
                        .read_script(project, &request.name)
                        .await
                        .map_err(failure)?,
                ))
            }
            PYTHON_WRITE_SCRIPT_TOOL => {
                let request: PythonWriteScriptRequest = decode(tool, request)?;
                request
                    .validate()
                    .map_err(|error| error.into_failure(tool, "content"))?;
                encode(serde_json::to_value(
                    python
                        .write_script(project, &request.name, &request.content)
                        .await
                        .map_err(failure)?,
                ))
            }
            PYTHON_INSTALL_PACKAGES_TOOL => {
                let request: PythonInstallPackagesRequest = decode(tool, request)?;
                request
                    .validate()
                    .map_err(|error| error.into_failure(tool, "packages"))?;
                encode(serde_json::to_value(
                    python
                        .install_packages(project, &request.packages, cancel)
                        .await
                        .map_err(failure)?,
                ))
            }
            PYTHON_RUN_SCRIPT_TOOL => {
                let request: PythonRunScriptRequest = decode(tool, request)?;
                request
                    .validate()
                    .map_err(|error| error.into_failure(tool, "timeoutSeconds"))?;
                encode(serde_json::to_value(
                    python
                        .run_script(project, &request, cancel)
                        .await
                        .map_err(failure)?,
                ))
            }
            _ => Err(
                CapabilityFailure::new(CapabilityFailureCode::INVALID_REQUEST)
                    .with_detail("capabilityId", tool),
            ),
        }
    };
    tokio::select! {
        biased;
        // The port stops its processes on the same token; this only stops waiting.
        _ = control.cancellation().cancelled() => {
            Err(control.check().err().unwrap_or_else(|| CapabilityFailure::new(CapabilityFailureCode::CANCELLED)))
        }
        _ = tokio::time::sleep_until(control.deadline().into()) => {
            Err(CapabilityFailure::new(CapabilityFailureCode::DEADLINE_ELAPSED))
        }
        result = work => result,
    }
}

fn decode<T: serde::de::DeserializeOwned>(
    tool: &str,
    request: &Value,
) -> Result<T, CapabilityFailure> {
    serde_json::from_value(request.clone()).map_err(|_| {
        CapabilityFailure::new(CapabilityFailureCode::INVALID_REQUEST)
            .with_detail("capabilityId", tool)
    })
}

fn failure(error: PythonFailure) -> CapabilityFailure {
    match error {
        PythonFailure::Unavailable { setup } => {
            CapabilityFailure::new(CapabilityFailureCode::RESULT_UNAVAILABLE)
                .with_detail("reason", "python_unavailable")
                .with_detail("nextStep", setup)
        }
        PythonFailure::ScriptNotFound => {
            CapabilityFailure::new(CapabilityFailureCode::RESOURCE_UNAVAILABLE)
                .with_detail("reason", "python_script_not_found")
                .with_detail(
                    "nextStep",
                    "Inspect the environment for existing scripts or write the script first.",
                )
        }
        PythonFailure::ProjectUnavailable => {
            CapabilityFailure::new(CapabilityFailureCode::PROJECT_SESSION_UNAVAILABLE)
                .with_detail("reason", "project_folder_unavailable")
        }
        PythonFailure::InstallFailed { output } => {
            CapabilityFailure::new(CapabilityFailureCode::MUTATION_REJECTED)
                .with_detail("reason", "python_install_failed")
                .with_detail("output", output)
        }
        PythonFailure::Failed => CapabilityFailure::new(CapabilityFailureCode::INTERNAL_FAILURE)
            .with_detail("reason", "python_process_failed"),
    }
}
