//! Read a retained editor through its current Project-owned location after missed move receipts.
use super::{
    ApplicationState, OpenGraphApplicationError, OpenGraphApplicationReceipt,
    OpenGraphProjectError, OpenGraphRequest, map_project_open_error, open_graph_in_session,
    revalidate_application_session,
};

impl ApplicationState {
    pub fn refresh_graph(
        &self,
        mut request: OpenGraphRequest,
        editing_session: uuid::Uuid,
    ) -> Result<OpenGraphApplicationReceipt, OpenGraphApplicationError> {
        let captured = self.capture_session()?;
        if request.project_instance_id() != captured.project_instance_id() {
            return Err(OpenGraphProjectError::ProjectIdentityMismatch {
                requested: request.project_instance_id().clone(),
            }
            .into());
        }
        let location = captured
            .project()
            .graph_editing_path(request.project_instance_id(), editing_session)
            .map_err(|error| map_project_open_error(request.graph_path(), error))?;
        let retained = location.is_some();
        if let Some(path) = location {
            request.graph_path = path;
        }
        let _editing = captured
            .coordinate_graph_edit(request.graph_path())
            .map_err(|_| OpenGraphApplicationError::EditingBusy)?;
        let graph = open_graph_in_session(self, &captured, request)?;
        // The metadata can move or be replaced while a read resolves. A different resource
        // at the same path must not be presented as recovery of the retained session.
        if retained
            && (graph.editing().version.session_id != editing_session
                || captured
                    .project()
                    .graph_editing_path(graph.project_instance_id(), editing_session)
                    .map_err(|error| map_project_open_error(graph.graph_path(), error))?
                    .as_ref()
                    != Some(graph.graph_path()))
        {
            return Err(OpenGraphProjectError::StaleProjectAuthority {
                graph: graph.graph_path().clone(),
            }
            .into());
        }
        revalidate_application_session(self, &captured)?;
        Ok(graph)
    }
}
