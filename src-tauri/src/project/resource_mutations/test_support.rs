use super::fixture_result_path;
use crate::project::{GraphDocumentKind, ProjectFilesystemError, ProjectState};
use yss_graph_document::{GraphResourcePath, GraphRevision};
use yss_project_identity::ProjectInstanceId;
use yss_project_identity::{OperationId, ResourceRevision};

#[cfg(test)]
pub(crate) struct GraphRenameFixtureResult {
    pub(crate) path: GraphResourcePath,
    pub(crate) publication: crate::schema::application_event::ResourceMutationResultDto,
}

#[cfg(test)]
impl std::ops::Deref for GraphRenameFixtureResult {
    type Target = GraphResourcePath;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

#[cfg(test)]
impl PartialEq<GraphResourcePath> for GraphRenameFixtureResult {
    fn eq(&self, other: &GraphResourcePath) -> bool {
        self.path == *other
    }
}

#[cfg(test)]
impl std::fmt::Debug for GraphRenameFixtureResult {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GraphRenameFixtureResult")
            .field("path", &self.path)
            .field(
                "publication_revision",
                &self.publication.publication_revision,
            )
            .finish()
    }
}

#[cfg(all(test, any()))]
impl ProjectState {
    pub(crate) fn create_graph_resource_fixture(
        &self,
        name: &str,
        kind: GraphDocumentKind,
    ) -> Result<GraphResourcePath, String> {
        let session = self
            .capture_project_session()
            .map_err(|error| error.to_string())?;
        let result = self
            .create_graph_resource_transaction(&session.instance_id, name, kind, OperationId::new())
            .map_err(|error| error.to_string())?;
        fixture_result_path(&result).ok_or_else(|| "create result omitted graph path".into())
    }

    pub(crate) fn duplicate_graph_resource_fixture(
        &self,
        source: &GraphResourcePath,
    ) -> Result<GraphResourcePath, String> {
        let session = self
            .capture_project_session()
            .map_err(|error| error.to_string())?;
        let revision = self
            .graph_revisions
            .read()
            .unwrap()
            .get(source)
            .copied()
            .unwrap_or(GraphRevision::INITIAL);
        let result = self
            .duplicate_graph_resource_transaction(
                &session.instance_id,
                source,
                ResourceRevision::from_graph_revision(revision),
                OperationId::new(),
            )
            .map_err(|error| error.to_string())?;
        fixture_result_path(&result).ok_or_else(|| "duplicate result omitted graph path".into())
    }

    pub(crate) fn remove_graph_resource_fixture(
        &self,
        graph_path: &GraphResourcePath,
    ) -> Result<(), String> {
        let session = self
            .capture_project_session()
            .map_err(|error| error.to_string())?;
        let revision = self
            .graph_revisions
            .read()
            .unwrap()
            .get(graph_path)
            .copied()
            .unwrap_or(GraphRevision::INITIAL);
        self.remove_graph_resource_transaction(
            &session.instance_id,
            graph_path,
            ResourceRevision::from_graph_revision(revision),
            OperationId::new(),
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
    }

    pub(crate) fn rename_graph_resource_fixture(
        &self,
        expected_project_instance_id: &str,
        graph_path: &GraphResourcePath,
        new_name: &str,
    ) -> Result<GraphRenameFixtureResult, ProjectFilesystemError> {
        let revision = self
            .project_data
            .read()
            .unwrap()
            .graphs
            .get(graph_path)
            .map(|graph| graph.document.revision)
            .or_else(|| {
                self.graph_revisions
                    .read()
                    .unwrap()
                    .get(graph_path)
                    .copied()
            })
            .unwrap_or(GraphRevision::INITIAL);
        let expected_project_instance_id =
            ProjectInstanceId::from_existing(expected_project_instance_id.to_string());
        let mut token = 1;
        let publication = loop {
            match self.rename_graph_resource_transaction(
                &expected_project_instance_id,
                graph_path,
                ResourceRevision::from_graph_revision(revision),
                new_name,
                token,
                OperationId::new(),
            ) {
                Ok(publication) => break publication,
                Err(ProjectFilesystemError::StaleResourceLifecycle { .. })
                    if self.project_instance_id() == expected_project_instance_id.as_str()
                        && token < 16 =>
                {
                    token += 1;
                }
                Err(error) => return Err(error),
            }
        };
        let path = publication
            .moves
            .first()
            .map(|moved| GraphResourcePath::new(moved.to.clone()))
            .transpose()
            .map_err(|error| ProjectFilesystemError::TransactionCommitFailed {
                message: error.to_string(),
            })?
            .ok_or_else(|| ProjectFilesystemError::TransactionCommitFailed {
                message: "rename result omitted move target".into(),
            })?;
        Ok(GraphRenameFixtureResult { path, publication })
    }
}
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResourceMutationTestPoint {
    Planned,
    Prepared,
    Committed,
    BeforePublication,
}

#[cfg(test)]
pub(crate) type ResourceMutationTestHook = std::sync::Arc<
    dyn Fn(ResourceMutationTestPoint, Option<&yss_graph_document::GraphResourcePath>) + Send + Sync,
>;

#[cfg(test)]
impl ProjectState {
    pub(crate) fn set_resource_mutation_test_hook(&self, hook: Option<ResourceMutationTestHook>) {
        *self.test_hooks.resource_mutation_test_hook.write().unwrap() = hook;
    }

    pub(crate) fn run_resource_mutation_test_hook(
        &self,
        point: ResourceMutationTestPoint,
        path: Option<&yss_graph_document::GraphResourcePath>,
    ) {
        let hook = self
            .test_hooks
            .resource_mutation_test_hook
            .read()
            .unwrap()
            .clone();
        if let Some(hook) = hook {
            hook(point, path);
        }
    }
}
