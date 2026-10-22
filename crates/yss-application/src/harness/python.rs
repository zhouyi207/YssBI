//! Harness Python tools bound to the folder of the conversation's project.

use std::path::PathBuf;
use std::sync::Arc;
use yss_harness_contract::{
    CancellationToken, ProjectSessionBinding, PythonEnvironment, PythonFailure, PythonFuture,
    PythonPackagesInstalled, PythonPort, PythonRun, PythonRunScriptRequest, PythonScript,
    PythonScriptWritten,
};
use yss_python::PythonRuntime;

use crate::session::ApplicationState;

pub struct ApplicationPython {
    application: ApplicationState,
    runtime: Arc<PythonRuntime>,
}

impl ApplicationPython {
    pub fn new(application: ApplicationState, app_dir: PathBuf) -> Self {
        Self {
            application,
            runtime: Arc::new(PythonRuntime::new(app_dir.join("python-environments"))),
        }
    }

    async fn with_project<T: Send + 'static>(
        &self,
        project: &ProjectSessionBinding,
        action: impl FnOnce(&PythonRuntime, PathBuf) -> Result<T, PythonFailure> + Send + 'static,
    ) -> Result<T, PythonFailure> {
        let root = self
            .application
            .query_project_path(project.project_instance_id().clone())
            .map_err(|_| PythonFailure::ProjectUnavailable)?
            .ok_or(PythonFailure::ProjectUnavailable)?;
        let runtime = self.runtime.clone();
        tokio::task::spawn_blocking(move || action(&runtime, PathBuf::from(root)))
            .await
            .map_err(|_| PythonFailure::Failed)?
    }
}

impl PythonPort for ApplicationPython {
    fn inspect(&self, project: &ProjectSessionBinding) -> PythonFuture<'_, PythonEnvironment> {
        let project = project.clone();
        Box::pin(async move {
            self.with_project(&project, |runtime, root| runtime.inspect(&root))
                .await
        })
    }

    fn read_script(
        &self,
        project: &ProjectSessionBinding,
        name: &str,
    ) -> PythonFuture<'_, PythonScript> {
        let (project, name) = (project.clone(), name.to_owned());
        Box::pin(async move {
            self.with_project(&project, move |runtime, root| {
                runtime.read_script(&root, &name)
            })
            .await
        })
    }

    fn write_script(
        &self,
        project: &ProjectSessionBinding,
        name: &str,
        content: &str,
    ) -> PythonFuture<'_, PythonScriptWritten> {
        let (project, name, content) = (project.clone(), name.to_owned(), content.to_owned());
        Box::pin(async move {
            self.with_project(&project, move |runtime, root| {
                runtime.write_script(&root, &name, &content)
            })
            .await
        })
    }

    fn install_packages(
        &self,
        project: &ProjectSessionBinding,
        packages: &[String],
        cancel: CancellationToken,
    ) -> PythonFuture<'_, PythonPackagesInstalled> {
        let (project, packages) = (project.clone(), packages.to_vec());
        Box::pin(async move {
            self.with_project(&project, move |runtime, root| {
                runtime.install_packages(&root, &packages, &cancel)
            })
            .await
        })
    }

    fn run_script(
        &self,
        project: &ProjectSessionBinding,
        request: &PythonRunScriptRequest,
        cancel: CancellationToken,
    ) -> PythonFuture<'_, PythonRun> {
        let (project, request) = (project.clone(), request.clone());
        Box::pin(async move {
            self.with_project(&project, move |runtime, root| {
                runtime.run_script(&root, &request, &cancel)
            })
            .await
        })
    }
}
