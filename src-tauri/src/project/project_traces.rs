use super::{GraphResourcePath, ProjectInstanceId, ProjectSession, ProjectState};
use crate::node_system::analysis::{BoundedTraceSink, RunId, RunTraceBundle, TraceBundle};
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceQueryError {
    ProjectStale,
    NotFound,
}

impl fmt::Display for TraceQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectStale => formatter.write_str("trace project is no longer active"),
            Self::NotFound => formatter.write_str("trace was not found"),
        }
    }
}

impl std::error::Error for TraceQueryError {}

impl ProjectState {
    pub fn list_graph_trace_bundles(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
    ) -> Result<Vec<TraceBundle>, TraceQueryError> {
        let document_path =
            crate::node_system::document::GraphResourcePath(graph_path.as_str().into());
        self.query_traces(expected_project_instance_id, |sink| {
            sink.bundles_for_graph(&document_path)
        })
    }

    pub fn get_run_trace_bundle(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        run_id: RunId,
    ) -> Result<RunTraceBundle, TraceQueryError> {
        self.query_traces(expected_project_instance_id, |sink| sink.run_bundle(run_id))?
            .ok_or(TraceQueryError::NotFound)
    }

    pub fn associate_run_trace_incident(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        run_id: RunId,
        incident_id: &str,
    ) -> Result<bool, TraceQueryError> {
        self.query_traces(expected_project_instance_id, |sink| {
            sink.associate_run_incident(run_id, incident_id)
        })
    }

    fn query_traces<T>(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        snapshot: impl FnOnce(&BoundedTraceSink) -> T,
    ) -> Result<T, TraceQueryError> {
        let session = self.expected_trace_session(expected_project_instance_id)?;
        self.validate_trace_session(&session)?;
        let sink = {
            let store = self.project_store.read().unwrap();
            Arc::clone(&store.trace_sink)
        };
        self.validate_trace_session(&session)?;
        let records = snapshot(&sink);

        #[cfg(test)]
        if let Some(hook) = self
            .trace_query_after_snapshot_test_hook
            .read()
            .unwrap()
            .clone()
        {
            hook();
        }

        self.validate_trace_session(&session)?;
        Ok(records)
    }

    fn expected_trace_session(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
    ) -> Result<ProjectSession, TraceQueryError> {
        let session = self
            .capture_project_session()
            .map_err(|_| TraceQueryError::ProjectStale)?;
        if &session.instance_id != expected_project_instance_id {
            return Err(TraceQueryError::ProjectStale);
        }
        Ok(session)
    }

    fn validate_trace_session(&self, session: &ProjectSession) -> Result<(), TraceQueryError> {
        self.validate_project_session(session)
            .map_err(|_| TraceQueryError::ProjectStale)
    }

    #[cfg(test)]
    pub(super) fn set_trace_query_after_snapshot_test_hook(
        &self,
        hook: Arc<dyn Fn() + Send + Sync>,
    ) {
        *self.trace_query_after_snapshot_test_hook.write().unwrap() = Some(hook);
    }
}
