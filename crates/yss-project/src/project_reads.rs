use crate::ProjectOperationError;
use crate::file_resources::{FileIndexEntry, ResourceFile};
use crate::{ProjectIndex, ProjectSession, ProjectState};
use yss_function_editor_projection::FunctionEditorProjection;
use yss_project_identity::ProjectInstanceId;
use yss_project_identity::ResourceRevision;
use yss_project_model::ProjectData;

impl ProjectState {
    pub fn read_project_index(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
    ) -> Result<ProjectIndex, ProjectOperationError> {
        let session = expected_session(self, expected_project_instance_id)?;
        let _lease = self.filesystem().acquire(session.root.clone());
        self.validate_project_session(&session)?;
        let mut index = crate::project_io::read_project_index_from_root(session.root.as_path())
            .map_err(read_error)?;
        self.validate_project_session(&session)?;
        self.coherent_project_read(&session, |data, publication| {
            let graph_revisions = self.graph_resource_revisions.read().unwrap();
            let database_revisions = self.database_authority_revisions.read().unwrap();
            if data
                .databases
                .keys()
                .any(|id| !database_revisions.contains_key(id))
            {
                return Err(stale_catalog(
                    "loaded database is missing its revision authority",
                ));
            }
            overlay_authoritative_project_index(
                data,
                &graph_revisions,
                &database_revisions,
                &mut index,
            )?;
            index.project_instance_id = publication.project_instance_id.clone();
            index.publication_revision = publication.resource_revision;
            index.authority_generation = publication.authority_generation();
            Ok(())
        })??;
        self.validate_project_session(&session)?;
        self.validate_project_index_version(
            &session.instance_id,
            index.publication_revision,
            index.authority_generation,
        )?;
        Ok(index)
    }

    /// Revalidate a captured index without rescanning files or cloning project data.
    pub fn validate_project_index_version(
        &self,
        project: &ProjectInstanceId,
        publication_revision: u64,
        authority_generation: u64,
    ) -> Result<(), ProjectOperationError> {
        let session = expected_session(self, project)?;
        self.validate_project_session(&session)?;
        let publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != project.as_str() {
            return Err(stale_project_lifecycle(
                "project changed before index validation",
            ));
        }
        if publication.resource_revision != publication_revision
            || publication.authority_generation() != authority_generation
        {
            return Err(stale_catalog(
                "project index authority changed before publication",
            ));
        }
        Ok(())
    }
}

fn stale_project_lifecycle(message: impl Into<String>) -> ProjectOperationError {
    ProjectOperationError::StaleProjectLifecycle {
        message: message.into(),
    }
}

fn stale_catalog(message: impl Into<String>) -> ProjectOperationError {
    ProjectOperationError::CatalogResourceStale {
        message: message.into(),
    }
}

fn expected_session(
    state: &ProjectState,
    expected_project_instance_id: &ProjectInstanceId,
) -> Result<ProjectSession, ProjectOperationError> {
    let session = state.capture_project_session()?;
    if &session.instance_id != expected_project_instance_id {
        return Err(ProjectOperationError::StaleProjectLifecycle {
            message: format!(
                "requested project instance '{}' is no longer active",
                expected_project_instance_id
            ),
        });
    }
    Ok(session)
}

fn overlay_authoritative_project_index(
    data: &ProjectData,
    graph_resource_revisions: &std::collections::HashMap<
        yss_graph_document::GraphResourcePath,
        ResourceRevision,
    >,
    database_revisions: &std::collections::HashMap<String, u64>,
    index: &mut ProjectIndex,
) -> Result<(), ProjectOperationError> {
    index.databases = data
        .databases
        .iter()
        .map(|(id, declaration)| crate::ProjectDatabaseIndexEntry {
            id: id.clone(),
            resource_path: yss_project_identity::ProjectResourcePath::new(format!(
                "databases/{id}"
            )),
            revision: yss_project_identity::ResourceRevision::new(database_revisions[id]),
            engine: declaration.engine.clone(),
            schema_version: declaration.schema_version,
            required: declaration.required,
            name: Some(declaration.name.to_string()),
        })
        .collect();
    index
        .databases
        .sort_by(|left, right| left.id.cmp(&right.id));
    overlay_file_revisions(data, &mut index.minds);
    overlay_file_revisions(data, &mut index.docs);
    overlay_file_revisions(data, &mut index.charts);
    for entry in &mut index.event_graphs {
        let path = yss_graph_document::GraphResourcePath::new(&entry.path)
            .map_err(|error| stale_catalog(error.to_string()))?;
        entry.revision = graph_resource_revisions
            .get(&path)
            .copied()
            .unwrap_or(ResourceRevision::INITIAL);
    }
    for entry in &mut index.function_graphs {
        let path = yss_graph_document::GraphResourcePath::new(&entry.path)
            .map_err(|error| stale_catalog(error.to_string()))?;
        entry.revision = graph_resource_revisions
            .get(&path)
            .copied()
            .unwrap_or(ResourceRevision::INITIAL);
        if let Some(resource) = data.graphs.get(&path) {
            let function = resource
                .function
                .as_ref()
                .ok_or_else(|| stale_catalog("function file has no signature"))?;
            entry.function_revision = function.revision;
            entry.function_signature = function.signature.clone();
            entry.function_editor_projection = FunctionEditorProjection::try_from(function)
                .map_err(|message| ProjectOperationError::TransactionPrepareFailed {
                    message: message.to_string(),
                })?;
        }
    }
    Ok(())
}

// Disk owns membership; resident revision overrides must never resurrect a deleted file.
fn overlay_file_revisions<T: ResourceFile>(data: &ProjectData, entries: &mut [FileIndexEntry<T>]) {
    for entry in entries {
        if let Some(state) = T::files(data).get(&entry.path) {
            entry.revision = state.version.revision;
        }
    }
}

fn read_error(error: crate::ProjectError) -> ProjectOperationError {
    ProjectOperationError::TransactionPrepareFailed {
        message: error.to_string(),
    }
}
