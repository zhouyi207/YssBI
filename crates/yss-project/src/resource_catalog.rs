//! Lightweight resource facts from existing membership and resident authorities.
use crate::{ProjectOperationError, ProjectState, file_resources::ResourceFile};
use yss_graph_document::GraphResourceKind;
use yss_project_identity::{
    ProjectInstanceId, ProjectResourceKind as Kind, ProjectResourceRef, ResourceRevision,
};
use yss_project_model::{ProjectData, doc::DocDocument, file::FilePath, mind::MindDocument};

#[derive(Clone, Debug)]
pub struct ResourceMetadata {
    pub resource: ProjectResourceRef,
    pub name: String,
    pub revision: ResourceRevision,
    pub session_id: Option<String>,
    /// Database dirty state belongs to the database editing owner.
    pub dirty: Option<bool>,
    pub root_topic_id: Option<String>,
}

pub struct ResourceCatalog {
    pub project_name: String,
    pub publication_revision: u64,
    pub authority_generation: u64,
    pub resources: Vec<ResourceMetadata>,
}

fn read_error(error: impl ToString) -> ProjectOperationError {
    ProjectOperationError::TransactionPrepareFailed {
        message: error.to_string(),
    }
}

impl ProjectState {
    /// Scans paths only. Does not load closed graphs, clone document bodies or read data rows.
    pub fn read_resource_catalog(
        &self,
        project: &ProjectInstanceId,
    ) -> Result<ResourceCatalog, ProjectOperationError> {
        self.ensure_project_operational()?;
        let session = self.capture_project_session()?;
        if &session.instance_id != project {
            return Err(ProjectOperationError::StaleProjectLifecycle {
                message: "resource catalog project changed".into(),
            });
        }
        let _lease = self.filesystem().acquire(session.root.clone())?;
        self.validate_project_session(&session)?;
        let graphs =
            crate::scan_graph_resource_index(session.root.as_path()).map_err(read_error)?;
        let charts =
            crate::chart_io::scan_chart_paths(session.root.as_path()).map_err(read_error)?;
        let minds = crate::file_resources::scan_file_paths::<MindDocument>(session.root.as_path())
            .map_err(read_error)?;
        let docs = crate::file_resources::scan_file_paths::<DocDocument>(session.root.as_path())
            .map_err(read_error)?;
        self.validate_project_session(&session)?;
        let publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != project.as_str() {
            return Err(ProjectOperationError::StaleProjectLifecycle {
                message: "resource catalog project changed".into(),
            });
        }
        let data = self.project_data.read().unwrap();
        let revisions = self.graph_resource_revisions.read().unwrap();
        let editing = self.graph_editing.lock().unwrap();
        let mut resources = Vec::new();
        for entry in graphs.entries() {
            let revision = revisions
                .get(&entry.path)
                .copied()
                .unwrap_or(ResourceRevision::INITIAL);
            let state = editing
                .get(&entry.path)
                .map(|metadata| metadata.state(revision));
            resources.push(ResourceMetadata {
                resource: ProjectResourceRef {
                    kind: match entry.kind {
                        GraphResourceKind::EventGraph => Kind::EventGraph,
                        GraphResourceKind::FunctionGraph => Kind::FunctionGraph,
                    },
                    id: entry.path.as_str().into(),
                },
                name: entry.path.display_name().into(),
                revision,
                session_id: state
                    .as_ref()
                    .map(|state| state.version.session_id.to_string()),
                dirty: Some(state.is_some_and(|state| state.dirty)),
                root_topic_id: None,
            });
        }
        drop(editing);
        drop(revisions);
        let chart_revisions = self.chart_revisions.read().unwrap();
        for path in charts {
            resources.push(ResourceMetadata {
                resource: ProjectResourceRef {
                    kind: Kind::Chart,
                    id: path.as_str().into(),
                },
                name: path.display_name().as_str().into(),
                revision: chart_revisions
                    .get(&path)
                    .copied()
                    .unwrap_or(ResourceRevision::INITIAL),
                session_id: None,
                dirty: Some(false),
                root_topic_id: None,
            });
        }
        drop(chart_revisions);
        append_files(&mut resources, &data, minds, Kind::Mind);
        append_files(&mut resources, &data, docs, Kind::Doc);
        for entry in &mut resources {
            if entry.resource.kind == Kind::Mind {
                let path =
                    FilePath::<MindDocument>::parse(&entry.resource.id).map_err(read_error)?;
                entry.root_topic_id = data
                    .minds
                    .get(&path)
                    .map(|file| file.content().root_id.clone());
            }
        }
        let database_revisions = self.database_authority_revisions.read().unwrap();
        for (id, declaration) in &data.databases {
            let revision = database_revisions
                .get(id)
                .copied()
                .ok_or_else(|| read_error("database authority unavailable"))?;
            resources.push(ResourceMetadata {
                resource: ProjectResourceRef {
                    kind: Kind::Database,
                    id: id.clone(),
                },
                name: declaration.name.to_string(),
                revision: ResourceRevision::new(revision),
                session_id: None,
                dirty: None,
                root_topic_id: None,
            });
        }
        resources.sort_by(|left, right| left.resource.cmp(&right.resource));
        Ok(ResourceCatalog {
            project_name: data.metadata.project_name.clone(),
            publication_revision: publication.resource_revision,
            authority_generation: publication.authority_generation(),
            resources,
        })
    }
}

fn append_files<T: ResourceFile>(
    resources: &mut Vec<ResourceMetadata>,
    data: &ProjectData,
    paths: Vec<FilePath<T>>,
    kind: Kind,
) {
    for path in paths {
        let state = T::files(data).get(&path);
        resources.push(ResourceMetadata {
            resource: ProjectResourceRef {
                kind,
                id: path.as_str().into(),
            },
            name: path.name().into(),
            revision: state.map_or(ResourceRevision::INITIAL, |state| state.version.revision),
            session_id: state.map(|state| state.version.session_id.clone()),
            dirty: Some(state.is_some_and(|state| state.dirty())),
            root_topic_id: None,
        });
    }
}
