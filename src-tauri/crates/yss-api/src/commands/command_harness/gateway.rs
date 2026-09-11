use yss_application::execution::ApplicationState;
use yss_automation_contract::{
    AutomationCapabilityRequest, CapabilityControl, CapabilityFailure, CapabilityFailureCode,
    CapabilityFuture, CapabilityGatewayPort, CapabilityInvocationContext, ToolEffect,
};

/// Scheduling adapter for the synchronous Application capability boundary.
pub struct ApplicationCapabilityGateway {
    application: ApplicationState,
    graph_clients: std::sync::Arc<super::HarnessGraphClientHub>,
}

impl ApplicationCapabilityGateway {
    pub fn new(
        application: ApplicationState,
        graph_clients: std::sync::Arc<super::HarnessGraphClientHub>,
    ) -> Self {
        Self {
            application,
            graph_clients,
        }
    }
}

struct CancelQueryOnDrop {
    control: CapabilityControl,
    armed: bool,
}

impl Drop for CancelQueryOnDrop {
    fn drop(&mut self) {
        if self.armed {
            self.control.cancel_query();
        }
    }
}

impl CapabilityGatewayPort for ApplicationCapabilityGateway {
    fn invoke<'a>(
        &'a self,
        context: CapabilityInvocationContext,
        request: AutomationCapabilityRequest,
        control: CapabilityControl,
    ) -> CapabilityFuture<'a> {
        if super::graph_client::graph_path(&request).is_some() {
            return Box::pin(self.graph_clients.invoke(context, request, control));
        }
        let application = self.application.clone();
        let read_only = request.capability_id().descriptor().effect == ToolEffect::Inspect;
        Box::pin(run_on_blocking_pool(control, read_only, move |control| {
            application.invoke_automation_capability(context, request, &control)
        }))
    }
}

async fn run_on_blocking_pool<T: Send + 'static>(
    control: CapabilityControl,
    read_only: bool,
    operation: impl FnOnce(CapabilityControl) -> Result<T, CapabilityFailure> + Send + 'static,
) -> Result<T, CapabilityFailure> {
    control.check()?;
    let worker_control = control.clone();
    let task = tauri::async_runtime::spawn_blocking(move || operation(worker_control));
    // Read-only work can be abandoned after signalling the query. A write must return
    // its actual receipt, even when its deadline passes during commit.
    let outcome = if read_only {
        let mut query_guard = CancelQueryOnDrop {
            control: control.clone(),
            armed: true,
        };
        let result = tokio::select! {
            result = task => result,
            reason = control.cancellation().cancelled() => return Err(CapabilityFailure::new(
                if reason == yss_automation_contract::CancellationReason::DeadlineElapsed {
                    CapabilityFailureCode::DeadlineElapsed
                } else { CapabilityFailureCode::Cancelled }
            )),
            _ = tokio::time::sleep_until(control.deadline().into()) => {
                return Err(CapabilityFailure::new(CapabilityFailureCode::DeadlineElapsed));
            }
        };
        query_guard.armed = false;
        result
    } else {
        task.await
    };
    outcome.map_err(|_| CapabilityFailure::new(CapabilityFailureCode::InternalFailure))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::Arc, time::Duration};
    use yss_application::execution::{ApplicationSessionEpoch, ApplicationSessionSlot};
    use yss_automation_contract::{
        AutomationCapabilityResult, CancellationToken, CapabilityInvocationId, HarnessSessionId,
        InspectDatasetProfileRequest, PrincipalId, ProjectSessionBinding,
    };
    use yss_project_identity::OperationId;

    #[tokio::test]
    async fn blocking_query_cancellation_timeout_and_panic_have_bounded_outcomes() {
        for (cancel, expected) in [
            (true, CapabilityFailureCode::Cancelled),
            (false, CapabilityFailureCode::DeadlineElapsed),
        ] {
            let control = CapabilityControl::new(
                CancellationToken::default(),
                Duration::from_millis(if cancel { 1000 } else { 30 }),
            );
            let cancellation = control.cancellation().clone();
            let (entered, started) = tokio::sync::oneshot::channel();
            let (finished, done) = tokio::sync::oneshot::channel();
            let work = run_on_blocking_pool(control, true, move |control| {
                let _ = entered.send(());
                while !control
                    .cancellation_flag()
                    .load(std::sync::atomic::Ordering::Acquire)
                {
                    std::thread::sleep(Duration::from_millis(1));
                }
                let _ = finished.send(());
                Ok(())
            });
            let (result, ()) = tokio::time::timeout(Duration::from_secs(2), async {
                tokio::join!(work, async {
                    started.await.unwrap();
                    if cancel {
                        cancellation.cancel(yss_automation_contract::CancellationReason::User);
                    }
                })
            })
            .await
            .unwrap();
            assert_eq!(result.unwrap_err().code, expected);
            tokio::time::timeout(Duration::from_secs(2), done)
                .await
                .unwrap()
                .unwrap();
        }
        let result: Result<(), _> = run_on_blocking_pool(
            CapabilityControl::new(CancellationToken::default(), Duration::from_secs(1)),
            true,
            |_| panic!("synthetic worker panic"),
        )
        .await;
        assert_eq!(
            result.unwrap_err().code,
            CapabilityFailureCode::InternalFailure
        );
    }

    #[test]
    fn dataset_profile_runs_from_tokio_and_returns_real_quality_facts() {
        struct Directory(std::path::PathBuf);
        impl Drop for Directory {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let directory = Directory(
            std::env::temp_dir().join(format!("yss-harness-profile-{}", uuid::Uuid::new_v4())),
        );
        std::fs::create_dir(&directory.0).unwrap();
        let project = Arc::new(yss_project::ProjectState::new());
        let created = project
            .create_project_transaction("Profile", &directory.0.join("project"), OperationId::new())
            .unwrap();
        project
            .activate_project_from_path(&created.metadata_path)
            .unwrap();
        let backend = Arc::new(yss_sci_runtime::SciRuntimeBackend::new());
        let candidate =
            yss_application::execution::session_factory::build_current_project_candidate(
                ApplicationSessionEpoch::INITIAL,
                project,
                [],
                backend.clone(),
            )
            .unwrap();
        let application =
            ApplicationState::from_composition(Arc::new(ApplicationSessionSlot::new()), backend);
        application.install_candidate(candidate).unwrap();
        let session = application.capture_session().unwrap();
        let path = directory.0.join("source.csv");
        std::fs::write(&path, "x,label\n1,a\n2,\n3,c\n").unwrap();
        let dataset = application
            .load_database_for_application(
                session.project_instance_id().clone(),
                OperationId::new(),
                yss_database_contract::DatabaseImportSource::Csv {
                    path: path.to_string_lossy().into(),
                    delimiter: ',',
                    has_header: true,
                    infer_schema_length: Some(10),
                },
            )
            .unwrap()
            .data;
        let context = CapabilityInvocationContext::new(
            PrincipalId::try_new("user-1").unwrap(),
            HarnessSessionId::try_new("session-1").unwrap(),
            CapabilityInvocationId::try_new("capability-1").unwrap(),
            ProjectSessionBinding::new(
                session.project_instance_id().clone(),
                session.project_session_id().clone(),
            ),
        );
        drop(session);
        let gateway = ApplicationCapabilityGateway::new(
            application,
            Arc::new(super::super::HarnessGraphClientHub::new()),
        );
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        let control = CapabilityControl::new(CancellationToken::default(), Duration::from_secs(5));
        let result = runtime
            .block_on(gateway.invoke(
                context,
                AutomationCapabilityRequest::InspectDatasetProfile(InspectDatasetProfileRequest {
                    database_id: dataset.id,
                }),
                control.clone(),
            ))
            .unwrap();
        control.check().unwrap();
        let AutomationCapabilityResult::DatasetProfileInspection(profile) = result else {
            panic!("wrong capability result")
        };
        assert_eq!(
            (
                profile.row_count,
                profile.column_count,
                profile.total_nulls,
                profile.rows_with_nulls
            ),
            (3, 2, 1, 1)
        );
        drop(gateway);
        drop(runtime);
    }
}
