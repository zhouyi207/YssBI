use crate::ProjectState;
use yss_project_change::{ProjectChange, ProjectIndexInvalidation};
use yss_project_filesystem::ProjectFilesystemError;
use yss_project_identity::{ProjectInstanceId, ResourceRevision};

impl ProjectState {
    pub fn reconcile_project_change(
        &self,
        project: &ProjectInstanceId,
        change: ProjectChange,
    ) -> Result<Option<ProjectIndexInvalidation>, ProjectFilesystemError> {
        if !change.affects_project_index() {
            return Ok(None);
        }

        let session = self.capture_project_session()?;
        if &session.instance_id != project {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "watched project is no longer active".into(),
            });
        }
        let _lease = self.filesystem().acquire(session.root.clone())?;
        self.validate_project_session(&session)?;
        crate::project_io::read_project_manifest_from_root(session.root.as_path())
            .map_err(read_error)?;
        let index = crate::scan_graph_resource_index(session.root.as_path()).map_err(read_error)?;
        let charts =
            crate::chart_io::load_charts_from_root(session.root.as_path()).map_err(read_error)?;
        let current = self.get_data()?;
        let mut graph_changes = Vec::new();
        for (path, previous) in &current.graphs {
            let Some(entry) = index.get_by_path(path.as_str()) else {
                graph_changes.push((path.clone(), None));
                continue;
            };
            let document = crate::project_io::read_graph_document(
                &session.root.as_path().join(entry.path.as_str()),
                entry.kind,
            )
            .map_err(read_error)?;
            let mut incoming = yss_project_model::GraphResourceDocument {
                name: document.name,
                kind: document.kind,
                document: document.document,
                function: document.function,
            };
            if let (Some(next), Some(previous)) = (&mut incoming.function, &previous.function) {
                next.revision = previous.revision;
            }
            if &incoming != previous {
                graph_changes.push((path.clone(), Some(incoming)));
            }
        }
        let mut chart_changes = Vec::new();
        for (path, incoming) in &charts {
            let mut incoming = incoming.clone();
            if let Some(previous) = current.charts.get(path) {
                incoming.revision = previous.revision;
                if &incoming == previous {
                    continue;
                }
            }
            chart_changes.push((path.clone(), Some(incoming)));
        }
        for path in current
            .charts
            .keys()
            .filter(|path| !charts.contains_key(*path))
        {
            chart_changes.push((path.clone(), None));
        }
        self.validate_project_session(&session)?;
        if !graph_changes.is_empty() || !chart_changes.is_empty() {
            let mut publication = self.mutation_publication.lock().unwrap();
            if publication.project_instance_id != project.as_str() {
                return Err(ProjectFilesystemError::StaleProjectLifecycle {
                    message: "watched project changed during rescan".into(),
                });
            }
            let mut data = self.project_data.write().unwrap();
            let mut graph_revisions = self.graph_resource_revisions.write().unwrap();
            let mut chart_revisions = self.chart_revisions.write().unwrap();
            // Prepare all fallible revision advances before publishing any part of the rescan.
            let next_graph_revisions = graph_changes
                .iter()
                .map(|(path, _)| next_revision(path.as_str(), graph_revisions.get(path).copied()))
                .collect::<Result<Vec<_>, _>>()?;
            let next_chart_revisions = chart_changes
                .iter()
                .map(|(path, _)| next_revision(path.as_str(), chart_revisions.get(path).copied()))
                .collect::<Result<Vec<_>, _>>()?;
            let advance = publication.prepare_resource_revision()?;
            for ((path, incoming), revision) in graph_changes.into_iter().zip(next_graph_revisions)
            {
                if let Some(mut resource) = incoming {
                    if let Some(function) = resource.function.as_mut() {
                        function.revision = revision;
                    }
                    if data.graphs.contains_key(&path) {
                        data.graphs.insert(path.clone(), resource);
                    }
                } else {
                    data.graphs.remove(&path);
                }
                graph_revisions.insert(path, revision);
            }
            for ((path, incoming), revision) in chart_changes.into_iter().zip(next_chart_revisions)
            {
                if let Some(mut document) = incoming {
                    document.revision = revision;
                    data.charts.insert(path.clone(), document);
                } else {
                    data.charts.remove(&path);
                }
                chart_revisions.insert(path, revision);
            }
            publication.commit_prepared(advance);
        }
        Ok(Some(ProjectIndexInvalidation::new(project.clone())))
    }
}

fn read_error(error: crate::ProjectError) -> ProjectFilesystemError {
    ProjectFilesystemError::TransactionPrepareFailed {
        message: error.to_string(),
    }
}

fn next_revision(
    path: &str,
    retained: Option<ResourceRevision>,
) -> Result<ResourceRevision, ProjectFilesystemError> {
    retained
        .map(|revision| crate::project_state::checked_resource_revision(path, revision))
        .transpose()
        .map(|revision| revision.unwrap_or(ResourceRevision::INITIAL))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use yss_project_change::{ProjectFileChangeKind, ProjectRelativePath};
    use yss_project_model::ProjectData;

    #[test]
    fn unrelated_change_is_a_noop_instead_of_an_error() {
        let fixture = fixtures::TempProject::activate("watcher-unrelated", ProjectData::new());
        let project = ProjectInstanceId::from_existing(fixture.state().project_instance_id());
        let result = fixture
            .state()
            .reconcile_project_change(
                &project,
                ProjectChange::file(
                    ProjectRelativePath::try_new("README.md").unwrap(),
                    ProjectFileChangeKind::Modified,
                ),
            )
            .unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn project_reconciliation_returns_typed_index_invalidation() {
        let fixture = fixtures::TempProject::activate("watcher-reconcile", ProjectData::new());
        let project = ProjectInstanceId::from_existing(fixture.state().project_instance_id());
        let invalidation = fixture
            .state()
            .reconcile_project_change(
                &project,
                ProjectChange::file(
                    ProjectRelativePath::try_new("metadata.yssbi").unwrap(),
                    ProjectFileChangeKind::Modified,
                ),
            )
            .unwrap()
            .expect("metadata changes invalidate the project index");

        assert_eq!(invalidation.project_instance_id(), &project);
    }

    #[test]
    fn external_deletion_does_not_resurrect_resident_graphs_or_charts() {
        let mut data = ProjectData::new();
        let graph = yss_graph_document::GraphResourcePath::new("events/Event.yssbi-event").unwrap();
        data.graphs.insert(
            graph.clone(),
            yss_project_model::GraphResourceDocument::new(
                "Event",
                yss_graph_document::GraphResourceKind::Event,
            ),
        );
        let (chart, document) = fixtures::chart("Chart", "data");
        data.charts.insert(chart.clone(), document);
        let fixture = fixtures::TempProject::activate("external-deletion", data);
        let state = fixture.state();
        let session = state.capture_project_session().unwrap();
        std::fs::remove_file(session.root.as_path().join(graph.as_str())).unwrap();
        std::fs::remove_file(session.root.as_path().join(chart.relative_path())).unwrap();
        // Index queries must reflect disk membership even before the watcher drains.
        let index = state.read_project_index(&session.instance_id).unwrap();
        assert!(index.graphs.is_empty() && index.charts.is_empty());
        state
            .reconcile_project_change(&session.instance_id, ProjectChange::rescan_required())
            .unwrap();
        let data = state.get_data().unwrap();
        assert!(data.graphs.is_empty() && data.charts.is_empty());
        let revision = state
            .read_project_index(&session.instance_id)
            .unwrap()
            .publication_revision;
        assert!(revision > 0);
        state
            .reconcile_project_change(&session.instance_id, ProjectChange::rescan_required())
            .unwrap();
        assert_eq!(
            state
                .read_project_index(&session.instance_id)
                .unwrap()
                .publication_revision,
            revision
        );
    }
}
