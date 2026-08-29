use crate::graph_document::GraphResourcePath;
use crate::project::{OperationId, ResourceRevision};
use crate::project::{ProjectFilesystemError, ProjectInstanceId, ProjectState};

pub(crate) fn remap_variable_scope_path(
    scope: &mut crate::variable::VariableScope,
    from: &str,
    to: &str,
) -> bool {
    let from = crate::project::graph_resource_index::normalize_resource_path(from);
    let to = crate::project::graph_resource_index::normalize_resource_path(to);
    match scope {
        crate::variable::VariableScope::Event { event_path }
            if crate::project::graph_resource_index::normalize_resource_path(event_path)
                == from =>
        {
            *event_path = to;
            true
        }
        crate::variable::VariableScope::Function { function_path }
            if crate::project::graph_resource_index::normalize_resource_path(function_path)
                == from =>
        {
            *function_path = to;
            true
        }
        _ => false,
    }
}

pub(crate) fn remap_graph_document_references(
    document: &mut crate::graph_document::GraphDocument,
    from: &str,
    to: &str,
) -> bool {
    let from = crate::project::graph_resource_index::normalize_resource_path(from);
    let to = crate::project::graph_resource_index::normalize_resource_path(to);
    let mut changed = false;
    for node in document.nodes.values_mut() {
        for value in node.parameters.values_mut() {
            if value.as_str().is_some_and(|path| {
                crate::project::graph_resource_index::normalize_resource_path(path) == from
            }) {
                *value = serde_json::Value::String(to.clone());
                changed = true;
            }
        }
    }
    changed
}

impl ProjectState {
    #[cfg(all(test, any()))]
    pub fn rename_graph_resource_transaction(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        expected_revision: ResourceRevision,
        new_name: &str,
        lifecycle_token: u64,
        operation_id: OperationId,
    ) -> Result<crate::schema::application_event::ResourceMutationResultDto, ProjectFilesystemError>
    {
        let session = self.capture_project_session()?;
        if &session.instance_id != expected_project_instance_id {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project changed before graph rename".into(),
            });
        }
        let reservation = self.reserve_resource_operation(&session.instance_id, operation_id)?;
        let result = self.rename_graph_resource_transaction_impl(
            expected_project_instance_id,
            graph_path,
            expected_revision,
            new_name,
            lifecycle_token,
            operation_id,
        );
        if result.is_ok() {
            reservation.complete();
        }
        result
    }
}
