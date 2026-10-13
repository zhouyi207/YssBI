//! Native project and editor read models; mutation authority stays in Application/Project.
use yss_desktop_services::resources;

use std::sync::Arc;

use yss_application::{
    activity_panel::ActivityPanelDocument, project::query::ProjectIndexSnapshot,
};
use yss_project_identity::ProjectInstanceId;

pub struct DesktopProject {
    pub identity: ProjectInstanceId,
    pub resources: Arc<resources::ResourceCatalog>,
    pub index: Arc<yss_project::ProjectIndex>,
    pub panels: Vec<Arc<ActivityPanelDocument>>,
    pub language: String,
}

impl DesktopProject {
    pub fn new(
        identity: ProjectInstanceId,
        snapshot: ProjectIndexSnapshot,
        language: &str,
    ) -> Self {
        let resources = Arc::new(resources::ResourceCatalog::new(
            identity.clone(),
            &snapshot.index,
        ));
        Self {
            resources,
            identity,
            language: language.to_owned(),
            index: Arc::new(snapshot.index),
            panels: snapshot.activity_panels.into_iter().map(Arc::new).collect(),
        }
    }

    /// A contiguous body-only file receipt changes revisions, not project rows or node creation.
    pub(crate) fn accept_file_save(
        &mut self,
        mutation: &yss_application::events::CommittedResourceMutation,
    ) -> bool {
        use yss_project_history::{ResourceDocumentPatch, ResourceKey};
        use yss_project_identity::ProjectResourceKind;
        if mutation.project_instance_id != self.identity
            || self.index.publication_revision.checked_add(1) != Some(mutation.publication_revision)
            || !mutation.moves.is_empty()
            || !mutation.projection_status.affected_graph_paths().is_empty()
            || mutation.deltas.is_empty()
        {
            return false;
        }
        for delta in &mutation.deltas {
            let ResourceDocumentPatch::ResourceLifecycle(patch) = &delta.payload else {
                return false;
            };
            let (Some(before), Some(after)) = (&patch.before, &patch.after) else {
                return false;
            };
            if before.path != after.path || before.name != after.name || before.kind != after.kind {
                return false;
            }
            let current = match &delta.resource {
                ResourceKey::Doc(path) if path.0 == after.path => self
                    .index
                    .docs
                    .iter()
                    .find(|entry| entry.path.as_str() == path.0.as_ref())
                    .map(|entry| (entry.revision, &entry.name)),
                ResourceKey::Mind(path) if path.0 == after.path => self
                    .index
                    .minds
                    .iter()
                    .find(|entry| entry.path.as_str() == path.0.as_ref())
                    .map(|entry| (entry.revision, &entry.name)),
                _ => return false,
            };
            if current != Some((delta.from_revision, &before.name)) {
                return false;
            }
        }
        let index = Arc::make_mut(&mut self.index);
        let resources = Arc::make_mut(&mut self.resources);
        for delta in &mutation.deltas {
            if let ResourceKey::Doc(path) = &delta.resource {
                for entry in index
                    .docs
                    .iter_mut()
                    .filter(|entry| entry.path.as_str() == path.0.as_ref())
                {
                    entry.revision = delta.to_revision;
                }
                resources.set_resource_revision(
                    ProjectResourceKind::Doc,
                    &path.0,
                    delta.to_revision,
                );
            } else if let ResourceKey::Mind(path) = &delta.resource {
                for entry in index
                    .minds
                    .iter_mut()
                    .filter(|entry| entry.path.as_str() == path.0.as_ref())
                {
                    entry.revision = delta.to_revision;
                }
                resources.set_resource_revision(
                    ProjectResourceKind::Mind,
                    &path.0,
                    delta.to_revision,
                );
            }
        }
        index.publication_revision = mutation.publication_revision;
        true
    }
    pub fn first_graph(&self) -> Option<String> {
        self.index
            .event_graphs
            .first()
            .map(|graph| graph.path.clone())
            .or_else(|| {
                self.index
                    .function_graphs
                    .first()
                    .map(|graph| graph.path.clone())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_application::session::ApplicationState;
    use yss_project::docs::DocCommand;
    use yss_project_identity::OperationId;
    use yss_project_model::doc::DocEdit;

    #[test]
    fn body_receipts_advance_catalog_revisions_without_replacing_panel_snapshots() {
        let directory = tempfile::tempdir().unwrap();
        yss_project::fixtures::write_project(
            &yss_project_model::ProjectData::new(),
            directory.path().to_str().unwrap(),
        )
        .unwrap();
        let application = ApplicationState::initialize().unwrap();
        application
            .load_project_for_application(directory.path().join("metadata.yssbi").to_str().unwrap())
            .unwrap();
        let identity = application
            .capture_session()
            .unwrap()
            .project_instance_id()
            .clone();
        let created = application
            .apply_doc_command(
                identity.clone(),
                OperationId::new(),
                DocCommand::Create {
                    name: "Notes".into(),
                },
            )
            .unwrap()
            .snapshot
            .unwrap();
        let mut project = DesktopProject::new(
            identity.clone(),
            application
                .query_project_index(identity.clone(), "en", true)
                .unwrap(),
            "en",
        );
        let panels = project.panels.clone();
        let edited = application
            .apply_doc_command(
                identity.clone(),
                OperationId::new(),
                DocCommand::Edit {
                    path: created.path.clone(),
                    version: created.version,
                    edits: vec![DocEdit::SetMarkdown {
                        markdown: "# Current draft".into(),
                    }],
                },
            )
            .unwrap();
        assert!(project.accept_file_save(&edited.mutation));
        let edited = edited.snapshot.unwrap();
        let saved = application
            .apply_doc_command(
                identity.clone(),
                OperationId::new(),
                DocCommand::Save {
                    path: edited.path.clone(),
                    version: edited.version,
                },
            )
            .unwrap();
        assert!(project.accept_file_save(&saved.mutation));
        let saved = saved.snapshot.unwrap();
        assert_eq!(project.index.docs[0].revision, saved.version.revision);
        let resource = yss_project_identity::ProjectResourceRef {
            kind: yss_project_identity::ProjectResourceKind::Doc,
            id: saved.path.as_str().into(),
        };
        assert_eq!(
            project
                .resources
                .get(&resource)
                .unwrap()
                .revision(yss_assistant_contract::ResourceRevisionKind::Resource),
            Some(saved.version.revision.get())
        );
        assert!(
            panels
                .iter()
                .zip(&project.panels)
                .all(|(before, after)| Arc::ptr_eq(before, after))
        );
        let renamed = application
            .apply_doc_command(
                identity,
                OperationId::new(),
                DocCommand::Rename {
                    path: saved.path,
                    version: saved.version,
                    name: "Renamed".into(),
                },
            )
            .unwrap();
        assert!(
            !project.accept_file_save(&renamed.mutation),
            "renames must read fresh project rows"
        );
    }
}
