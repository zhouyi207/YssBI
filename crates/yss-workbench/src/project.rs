//! Native project and editor read models; mutation authority stays in Application/Project.
use yss_desktop_services::resources;

use std::sync::Arc;

use yss_application::{
    activity_panel::ActivityPanelDocument, project::query::ProjectPresentationSnapshot,
};
use yss_project_identity::ProjectInstanceId;

pub(crate) enum FileIndexUpdate {
    /// The receipt cannot be applied locally; the caller re-reads the project.
    Rejected,
    /// Only resource revisions advanced.
    Revisions,
    /// Project rows changed and the activity documents were rebuilt.
    Rows,
}

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
        snapshot: ProjectPresentationSnapshot,
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

    /// Applies a contiguous authored-file receipt to the accepted index without another disk
    /// scan. Graph, database and non-contiguous receipts still re-read the project.
    pub(crate) fn accept_file_receipt(
        &mut self,
        mutation: &yss_application::events::CommittedResourceMutation,
    ) -> FileIndexUpdate {
        use yss_project_history::{ResourceKey, ResourceLifecycleKind};
        if mutation.project_instance_id != self.identity
            || self.index.publication_revision.checked_add(1) != Some(mutation.publication_revision)
            || !mutation.projection_status.affected_graph_paths().is_empty()
            || mutation.deltas.is_empty()
            || mutation.moves.iter().any(|moved| {
                !matches!(
                    moved.kind,
                    ResourceLifecycleKind::Doc
                        | ResourceLifecycleKind::Mind
                        | ResourceLifecycleKind::Chart
                )
            })
        {
            return FileIndexUpdate::Rejected;
        }
        let mut index = (*self.index).clone();
        let mut rows = false;
        for delta in &mutation.deltas {
            let applied = match &delta.resource {
                ResourceKey::Doc(key) => apply_file_delta(&mut index.docs, &key.0, delta),
                ResourceKey::Mind(key) => apply_file_delta(&mut index.minds, &key.0, delta),
                ResourceKey::Chart(key) => apply_file_delta(&mut index.charts, &key.0, delta),
                _ => None,
            };
            let Some(changed_rows) = applied else {
                return FileIndexUpdate::Rejected;
            };
            rows |= changed_rows;
        }
        index.publication_revision = mutation.publication_revision;
        self.resources = Arc::new(resources::ResourceCatalog::new(
            self.identity.clone(),
            &index,
        ));
        if !rows {
            self.index = Arc::new(index);
            return FileIndexUpdate::Revisions;
        }
        let mut rows = yss_application::activity_panel::project_activity_panel(Some(&index));
        // Project rows bind to the node catalog of the same publication. File receipts leave that
        // catalog intact, so the rebuilt rows keep its publication instead of forcing a re-query.
        if let Some(nodes) = self.panels.iter().find(|panel| panel.panel_id == "nodes") {
            rows.publication_revision = nodes.publication_revision;
        }
        let rows = Arc::new(rows);
        self.panels = self
            .panels
            .iter()
            .map(|panel| {
                if panel.panel_id == "project" {
                    rows.clone()
                } else {
                    panel.clone()
                }
            })
            .collect();
        self.index = Arc::new(index);
        FileIndexUpdate::Rows
    }

    /// Name and revision of an indexed authored file.
    pub(crate) fn indexed_file(
        &self,
        path: &str,
    ) -> Option<(String, yss_project_identity::ResourceRevision)> {
        fn find<T: yss_project_model::file::FileContent>(
            entries: &[yss_project::file_resources::FileIndexEntry<T>],
            path: &str,
        ) -> Option<(String, yss_project_identity::ResourceRevision)> {
            entries
                .iter()
                .find(|entry| entry.path.as_str() == path)
                .map(|entry| (entry.name.clone(), entry.revision))
        }
        find(&self.index.docs, path)
            .or_else(|| find(&self.index.minds, path))
            .or_else(|| find(&self.index.charts, path))
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

/// Applies one file delta to its index list; `Some(true)` means project rows changed.
fn apply_file_delta<T: yss_project_model::file::FileContent>(
    entries: &mut Vec<yss_project::file_resources::FileIndexEntry<T>>,
    path: &str,
    delta: &yss_project_history::ResourceDeltaEvent,
) -> Option<bool> {
    use yss_project_history::ResourceDocumentPatch;
    let position = entries.iter().position(|entry| entry.path.as_str() == path);
    match &delta.payload {
        ResourceDocumentPatch::ResourceLifecycle(patch) => {
            match (&patch.before, &patch.after, position) {
                (Some(before), Some(after), Some(position))
                    if before.path == after.path
                        && before.name == after.name
                        && before.kind == after.kind
                        && entries[position].revision == delta.from_revision =>
                {
                    entries[position].revision = delta.to_revision;
                    Some(false)
                }
                (None, Some(after), None) if after.path.as_ref() == path => {
                    insert_file_entry(entries, path, delta.to_revision)?;
                    Some(true)
                }
                (Some(_), None, Some(position))
                    if entries[position].revision == delta.from_revision =>
                {
                    entries.remove(position);
                    Some(true)
                }
                _ => None,
            }
        }
        ResourceDocumentPatch::ResourceMove(patch)
            if patch.to.as_ref() == path && position.is_none() =>
        {
            let source = entries
                .iter()
                .position(|entry| entry.path.as_str() == patch.from.as_ref())?;
            if entries[source].revision != delta.from_revision {
                return None;
            }
            entries.remove(source);
            insert_file_entry(entries, path, delta.to_revision)?;
            Some(true)
        }
        _ => None,
    }
}

/// Keeps the disk index order produced by the project scan.
fn insert_file_entry<T: yss_project_model::file::FileContent>(
    entries: &mut Vec<yss_project::file_resources::FileIndexEntry<T>>,
    path: &str,
    revision: yss_project_identity::ResourceRevision,
) -> Option<()> {
    let path = yss_project_model::file::FilePath::<T>::parse(path).ok()?;
    let position = entries
        .binary_search_by(|entry| entry.path.as_str().cmp(path.as_str()))
        .err()?;
    entries.insert(
        position,
        yss_project::file_resources::FileIndexEntry {
            kind: T::KIND,
            name: path.name().to_owned(),
            path,
            revision,
        },
    );
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_application::session::ApplicationState;
    use yss_chart_document::ChartDocument;
    use yss_project::file_resources::{FileCommand, FileIndexEntry, FileSnapshot, ResourceFile};
    use yss_project_identity::{OperationId, ProjectInstanceId};
    use yss_project_model::{
        chart::ChartEdit,
        doc::{DocDocument, DocEdit},
        file::FileContent,
        mind::MindDocument,
    };

    fn entries<T: FileContent>(entries: &[FileIndexEntry<T>]) -> Vec<(String, String, u64)> {
        entries
            .iter()
            .map(|entry| {
                (
                    entry.path.as_str().to_owned(),
                    entry.name.clone(),
                    entry.revision.get(),
                )
            })
            .collect()
    }

    /// The incremental project must equal one built from a fresh project query.
    fn assert_matches_fresh(
        project: &DesktopProject,
        application: &ApplicationState,
        identity: &ProjectInstanceId,
    ) {
        let fresh = DesktopProject::new(
            identity.clone(),
            application
                .query_project_presentation(identity.clone(), "en")
                .unwrap(),
            "en",
        );
        assert_eq!(
            project.index.publication_revision,
            fresh.index.publication_revision
        );
        assert_eq!(entries(&project.index.docs), entries(&fresh.index.docs));
        assert_eq!(entries(&project.index.minds), entries(&fresh.index.minds));
        assert_eq!(entries(&project.index.charts), entries(&fresh.index.charts));
        let rows = |project: &DesktopProject| {
            project
                .panels
                .iter()
                .find(|panel| panel.panel_id == "project")
                .cloned()
                .unwrap()
        };
        assert!(rows(project).same_content(&rows(&fresh)));
        let catalog = |project: &DesktopProject| {
            project
                .resources
                .entries()
                .iter()
                .map(|entry| {
                    (
                        entry.resource().clone(),
                        entry.name().to_owned(),
                        entry.revision(yss_assistant_contract::ResourceRevisionKind::Resource),
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(catalog(project), catalog(&fresh));
    }

    fn apply<T: ResourceFile>(
        application: &ApplicationState,
        project: &mut DesktopProject,
        command: FileCommand<T>,
        expected: fn(&FileIndexUpdate) -> bool,
    ) -> Option<FileSnapshot<T>> {
        let identity = project.identity.clone();
        let result = application
            .apply_file_command(identity.clone(), OperationId::new(), command)
            .unwrap();
        let update = project.accept_file_receipt(&result.mutation);
        assert!(
            expected(&update),
            "unexpected index update for a file receipt"
        );
        assert_matches_fresh(project, application, &identity);
        result.snapshot
    }

    fn lifecycle<T: ResourceFile>(
        application: &ApplicationState,
        project: &mut DesktopProject,
        edit: impl Fn(&T) -> T::Edit,
    ) {
        let rows = |update: &FileIndexUpdate| matches!(update, FileIndexUpdate::Rows);
        let revisions = |update: &FileIndexUpdate| matches!(update, FileIndexUpdate::Revisions);
        let created = apply::<T>(
            application,
            project,
            FileCommand::Create {
                name: "Report".into(),
            },
            rows,
        )
        .unwrap();
        let panels = project.panels.clone();
        let edited = apply(
            application,
            project,
            FileCommand::Edit {
                edits: vec![edit(&created.content)],
                path: created.path,
                version: created.version,
            },
            revisions,
        )
        .unwrap();
        let saved = apply(
            application,
            project,
            FileCommand::Save {
                path: edited.path,
                version: edited.version,
            },
            revisions,
        )
        .unwrap();
        assert!(
            panels
                .iter()
                .zip(&project.panels)
                .all(|(before, after)| Arc::ptr_eq(before, after)),
            "body receipts keep the project rows"
        );
        let renamed = apply(
            application,
            project,
            FileCommand::Rename {
                path: saved.path,
                version: saved.version,
                name: "Renamed".into(),
            },
            rows,
        )
        .unwrap();
        apply(
            application,
            project,
            FileCommand::Duplicate {
                path: renamed.path.clone(),
                version: renamed.version.clone(),
                name: None,
            },
            rows,
        );
        apply(
            application,
            project,
            FileCommand::Delete {
                path: renamed.path,
                version: renamed.version,
            },
            rows,
        );
    }

    #[test]
    fn file_receipts_update_the_index_rows_and_catalog_like_a_fresh_query() {
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
        let mut project = DesktopProject::new(
            identity.clone(),
            application
                .query_project_presentation(identity.clone(), "en")
                .unwrap(),
            "en",
        );
        lifecycle::<DocDocument>(&application, &mut project, |_| DocEdit::SetMarkdown {
            markdown: "# Current draft".into(),
        });
        lifecycle::<MindDocument>(&application, &mut project, |mind| {
            yss_project_model::mind::MindEdit::SetContent {
                node_id: mind.root_id.clone(),
                content: "Plan".into(),
            }
        });
        lifecycle::<ChartDocument>(&application, &mut project, |chart| {
            let mut document = chart.clone();
            document.chart_type = yss_chart_document::ChartType::Line;
            ChartEdit::SetDocument { document }
        });

        let graph = application
            .create_event_graph(identity, "Main".into(), OperationId::new())
            .unwrap();
        assert!(matches!(
            project.accept_file_receipt(&graph),
            FileIndexUpdate::Rejected
        ));
    }
}
