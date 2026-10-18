use crate::ProjectIndexInvalidation;
use crate::ProjectOperationError;
use crate::ProjectState;
use crate::file_resources::ResourceFile;
use yss_chart_document::ChartDocument;
use yss_filesystem::change::FilesystemChange;
use yss_project_history::{ChartResourceKey, DocResourceKey, MindResourceKey, ResourceKey};
use yss_project_identity::{ProjectInstanceId, ResourceRevision};
use yss_project_model::{
    ProjectData,
    doc::DocDocument,
    file::{FilePatch, FilePath},
    mind::MindDocument,
};

impl ProjectState {
    pub fn reconcile_project_change(
        &self,
        project: &ProjectInstanceId,
        change: FilesystemChange,
    ) -> Result<Option<ProjectIndexInvalidation>, ProjectOperationError> {
        if !crate::filesystem_change_affects_project_index(&change) {
            return Ok(None);
        }

        let session = self.capture_project_session()?;
        if &session.instance_id != project {
            return Err(ProjectOperationError::StaleProjectLifecycle {
                message: "watched project is no longer active".into(),
            });
        }
        let _lease = self.filesystem().acquire(session.root.clone());
        self.validate_project_session(&session)?;
        crate::project_io::read_project_manifest_from_root(session.root.as_path())
            .map_err(ProjectOperationError::prepare_failed)?;
        let targeted = match &change {
            FilesystemChange::File(file) => file
                .relative_path()
                .as_path()
                .to_str()
                .and_then(resource_key),
            FilesystemChange::RescanRequired => None,
        };
        // A retained single-file hint is a resource boundary. Only authoritative rescan
        // and unclassified metadata/database/directory hints inspect other resources.
        let index = if targeted.is_none() {
            Some(
                crate::scan_graph_resource_index(session.root.as_path())
                    .map_err(ProjectOperationError::prepare_failed)?,
            )
        } else {
            None
        };
        let current = self.get_data()?;
        let root = session.root.as_path();
        let mind_changes = file_changes::<MindDocument>(root, &current, targeted.as_ref())?;
        let doc_changes = file_changes::<DocDocument>(root, &current, targeted.as_ref())?;
        let chart_changes = file_changes::<ChartDocument>(root, &current, targeted.as_ref())?;
        let mut graph_changes = Vec::new();
        for (path, previous) in &current.graphs {
            if targeted.as_ref().is_some_and(
                |resource| !matches!(resource, ResourceKey::Graph(selected) if selected == path),
            ) {
                continue;
            }
            let exists = if let Some(index) = &index {
                index.get_by_path(path.as_str()).is_some()
            } else {
                resource_exists(session.root.as_path(), path.as_str())?
            };
            if !exists {
                graph_changes.push((path.clone(), None));
                continue;
            }
            let contents = yss_filesystem::read_secure_file(
                session.root.as_path(),
                std::path::Path::new(path.as_str()),
            )
            .map_err(|error| {
                ProjectOperationError::prepare_failed(crate::ProjectError::from(error))
            })?;
            let document = crate::project_io::parse_graph_resource_document(
                &contents,
                &session.root.as_path().join(path.as_str()),
                path.kind(),
            )
            .map_err(ProjectOperationError::prepare_failed)?;
            let mut incoming = yss_project_model::GraphResourceDocument {
                name: path.display_name().to_owned(),
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
        let unchanged = graph_changes.is_empty()
            && chart_changes.is_empty()
            && mind_changes.is_empty()
            && doc_changes.is_empty();
        let known_clean = match &targeted {
            Some(ResourceKey::Graph(path)) => current.graphs.contains_key(path),
            Some(resource) => {
                clean_file::<MindDocument>(&current, resource)
                    || clean_file::<DocDocument>(&current, resource)
                    || clean_file::<ChartDocument>(&current, resource)
            }
            None => false,
        };
        if unchanged && known_clean {
            self.validate_project_session(&session)?;
            return Ok(None);
        }
        // Nonresident/new/deleted graph hints still reach their dependency consumers,
        // without forcing the graph body into residency.
        let invalidation = match targeted {
            Some(resource) => {
                ProjectIndexInvalidation::for_resources(project.clone(), vec![resource])
            }
            None => ProjectIndexInvalidation::new(project.clone()),
        };
        self.validate_project_session(&session)?;
        #[cfg(test)]
        if let Some(hook) = self
            .test_hooks
            .project_change_after_read_test_hook
            .read()
            .unwrap()
            .clone()
        {
            hook();
        }
        if !graph_changes.is_empty()
            || !chart_changes.is_empty()
            || !mind_changes.is_empty()
            || !doc_changes.is_empty()
        {
            let mut publication = self.mutation_publication.lock().unwrap();
            if publication.project_instance_id != project.as_str() {
                return Err(ProjectOperationError::StaleProjectLifecycle {
                    message: "watched project changed during rescan".into(),
                });
            }
            let mut data = self.project_data.write().unwrap();
            // Undo can make the current body match the file after the scan prepared a difference.
            // Retain its editing identity/history instead of installing the same body again.
            graph_changes.retain(|(path, incoming)| {
                !self.is_graph_modified(path) && data.graphs.get(path) != incoming.as_ref()
            });
            if graph_changes.is_empty()
                && chart_changes.is_empty()
                && mind_changes.is_empty()
                && doc_changes.is_empty()
            {
                return Ok(Some(invalidation));
            }
            let mut graph_revisions = self.graph_resource_revisions.write().unwrap();
            // Prepare all fallible revision advances before publishing any part of the rescan.
            let next_graph_revisions = graph_changes
                .iter()
                .map(|(path, _)| next_revision(path.as_str(), graph_revisions.get(path).copied()))
                .collect::<Result<Vec<_>, _>>()?;
            let advance = publication.prepare_resource_revision()?;
            for patch in mind_changes {
                crate::file_resources::apply_patch(&mut data, patch);
            }
            for patch in doc_changes {
                crate::file_resources::apply_patch(&mut data, patch);
            }
            for patch in chart_changes {
                crate::file_resources::apply_patch(&mut data, patch);
            }
            for ((path, incoming), revision) in graph_changes.into_iter().zip(next_graph_revisions)
            {
                self.graph_editing.lock().unwrap().remove(&path);
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
            publication.commit_prepared(advance);
        }
        Ok(Some(invalidation))
    }
}

fn resource_exists(root: &std::path::Path, path: &str) -> Result<bool, ProjectOperationError> {
    match std::fs::symlink_metadata(root.join(path)) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(ProjectOperationError::prepare_failed(
            crate::ProjectError::from(error),
        )),
    }
}

fn resource_key(path: &str) -> Option<ResourceKey> {
    if let Ok(path) = yss_graph_document::GraphResourcePath::new(path) {
        Some(ResourceKey::Graph(path))
    } else if yss_project_model::doc::DocPath::parse(path).is_ok() {
        Some(ResourceKey::Doc(DocResourceKey(path.into())))
    } else if yss_project_model::mind::MindPath::parse(path).is_ok() {
        Some(ResourceKey::Mind(MindResourceKey(path.into())))
    } else if yss_project_model::chart::ChartPath::parse(path).is_ok() {
        Some(ResourceKey::Chart(ChartResourceKey(path.into())))
    } else {
        None
    }
}

fn targeted_file<T: ResourceFile>(resource: &ResourceKey) -> Option<FilePath<T>> {
    let path = match resource {
        ResourceKey::Mind(key) => &key.0,
        ResourceKey::Doc(key) => &key.0,
        ResourceKey::Chart(key) => &key.0,
        _ => return None,
    };
    FilePath::<T>::parse(path)
        .ok()
        .filter(|path| T::key(path) == *resource)
}

fn file_changes<T: ResourceFile>(
    root: &std::path::Path,
    current: &ProjectData,
    targeted: Option<&ResourceKey>,
) -> Result<Vec<FilePatch<T>>, ProjectOperationError> {
    match targeted {
        None => crate::file_resources::external_changes::<T>(root, current, None),
        Some(resource) => match targeted_file::<T>(resource) {
            Some(path) => crate::file_resources::external_changes(root, current, Some(&path)),
            None => Ok(Vec::new()),
        },
    }
}

fn clean_file<T: ResourceFile>(current: &ProjectData, resource: &ResourceKey) -> bool {
    targeted_file::<T>(resource).is_some_and(|path| {
        T::files(current)
            .get(&path)
            .is_some_and(|state| !state.dirty())
    })
}

fn next_revision(
    path: &str,
    retained: Option<ResourceRevision>,
) -> Result<ResourceRevision, ProjectOperationError> {
    retained
        .map(|revision| crate::project_state::checked_resource_revision(path, revision))
        .transpose()
        .map(|revision| revision.unwrap_or(ResourceRevision::INITIAL))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use yss_filesystem::change::{FileChangeKind, RelativePath};
    use yss_project_model::ProjectData;

    #[test]
    fn rescan_preserves_redo_when_undo_matches_the_file_during_preparation() {
        use crate::GraphHistoryAction;
        use std::sync::{Arc, Mutex};
        use yss_graph_document::{
            DocumentNode, GraphDocumentOperation, GraphDocumentPatch, GraphResourceKind,
            GraphResourcePath, NodeId, NodePosition,
        };
        use yss_graph_document_edit::apply_graph_document_patch;
        use yss_project_identity::OperationId;
        let path = GraphResourcePath::new("events/Watched.yssbi-event").unwrap();
        let mut data = ProjectData::new();
        data.graphs.insert(
            path.clone(),
            yss_project_model::GraphResourceDocument::new("Watched", GraphResourceKind::EventGraph),
        );
        let fixture = fixtures::TempProject::activate("watcher-undo-during-read", data);
        let state = fixture.state();
        let session = state.capture_project_session().unwrap();
        let original = state
            .read_graph_editing(&session.instance_id, &path)
            .unwrap();
        let patch = GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
            node: DocumentNode {
                id: NodeId::new(),
                node_type: "yssbi.tests.node".parse().unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: Default::default(),
                user_label: None,
            },
        }]);
        let capture = state
            .capture_graph_edit(
                &session.instance_id,
                &path,
                original.state.version,
                OperationId::new(),
                [0; 32],
            )
            .unwrap();
        let mut edited = (*original.document).clone();
        apply_graph_document_patch(&mut edited, &patch).unwrap();
        state
            .commit_graph_edit(capture, Arc::new(edited), GraphHistoryAction::Edit(patch))
            .unwrap();
        let expected = Arc::new(Mutex::new(None));
        let hook_state = state.clone();
        let hook_project = session.instance_id.clone();
        let hook_path = path.clone();
        let hook_expected = Arc::clone(&expected);
        *state
            .test_hooks
            .project_change_after_read_test_hook
            .write()
            .unwrap() = Some(Arc::new(move || {
            let current = hook_state
                .read_graph_editing(&hook_project, &hook_path)
                .unwrap();
            let patch = hook_state
                .graph_history_patch(&hook_project, &hook_path, current.state.version, false)
                .unwrap()
                .unwrap();
            let capture = hook_state
                .capture_graph_edit(
                    &hook_project,
                    &hook_path,
                    current.state.version,
                    OperationId::new(),
                    [0; 32],
                )
                .unwrap();
            let mut document = (*current.document).clone();
            apply_graph_document_patch(&mut document, &patch).unwrap();
            let undone = hook_state
                .commit_graph_edit(capture, Arc::new(document), GraphHistoryAction::Undo)
                .unwrap();
            assert!(!undone.editing.dirty && undone.editing.can_redo);
            *hook_expected.lock().unwrap() = Some(undone.editing);
        }));
        let publication = state
            .read_project_index(&session.instance_id)
            .unwrap()
            .publication_revision;
        let result = state
            .reconcile_project_change(&session.instance_id, FilesystemChange::rescan_required());
        state
            .test_hooks
            .project_change_after_read_test_hook
            .write()
            .unwrap()
            .take();
        result.unwrap();
        let current = state
            .read_graph_editing(&session.instance_id, &path)
            .unwrap();
        assert_eq!(
            current.state,
            expected.lock().unwrap().clone().unwrap(),
            "a stale rescan difference replaced the current editing session/history"
        );
        assert_eq!(current.document.as_ref(), original.document.as_ref());
        assert_eq!(
            state
                .read_project_index(&session.instance_id)
                .unwrap()
                .publication_revision,
            publication
        );
    }

    #[test]
    fn unrelated_change_is_a_noop_instead_of_an_error() {
        let fixture = fixtures::TempProject::activate("watcher-unrelated", ProjectData::new());
        let project = ProjectInstanceId::from_existing(fixture.state().project_instance_id());
        let result = fixture
            .state()
            .reconcile_project_change(
                &project,
                FilesystemChange::file(
                    RelativePath::try_new("README.md").unwrap(),
                    FileChangeKind::Modified,
                ),
            )
            .unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn local_file_commit_echo_is_quiet_but_external_changes_and_rescan_are_delivered() {
        use crate::docs::DocCommand;
        use yss_project_identity::OperationId;
        let fixture = fixtures::TempProject::activate("watcher-targeted-doc", ProjectData::new());
        let state = fixture.state();
        let session = state.capture_project_session().unwrap();
        let document = state
            .apply_file_command(
                &session.instance_id,
                OperationId::new(),
                DocCommand::Create {
                    name: "Notes".into(),
                },
            )
            .unwrap()
            .snapshot
            .unwrap();
        let event = || {
            FilesystemChange::file(
                RelativePath::try_new(document.path.as_str()).unwrap(),
                FileChangeKind::Modified,
            )
        };
        assert_eq!(
            state
                .reconcile_project_change(&session.instance_id, event())
                .unwrap(),
            None
        );
        std::fs::write(
            session.root.as_path().join(document.path.as_str()),
            "# External change",
        )
        .unwrap();
        let changed = state
            .reconcile_project_change(&session.instance_id, event())
            .unwrap()
            .unwrap();
        assert_eq!(
            changed.resources(),
            Some(
                [ResourceKey::Doc(DocResourceKey(
                    document.path.as_str().into()
                ))]
                .as_slice()
            )
        );
        assert_eq!(
            state
                .read_file(&session.instance_id, &document.path)
                .unwrap()
                .content
                .0,
            "# External change"
        );
        assert_eq!(
            state
                .reconcile_project_change(&session.instance_id, event())
                .unwrap(),
            None
        );
        let rescan = state
            .reconcile_project_change(&session.instance_id, FilesystemChange::rescan_required())
            .unwrap()
            .unwrap();
        assert!(
            rescan.resources().is_none(),
            "rescan must remain authoritative even when the saved bodies match"
        );
    }

    #[test]
    fn authoritative_rescan_reconciles_all_changed_documents() {
        use crate::docs::DocCommand;
        use yss_project_identity::OperationId;
        let fixture = fixtures::TempProject::activate("watcher-coalesced", ProjectData::new());
        let state = fixture.state();
        let session = state.capture_project_session().unwrap();
        let documents = ["First", "Second"].map(|name| {
            state
                .apply_file_command(
                    &session.instance_id,
                    OperationId::new(),
                    DocCommand::Create { name: name.into() },
                )
                .unwrap()
                .snapshot
                .unwrap()
        });
        for document in &documents {
            std::fs::write(
                session.root.as_path().join(document.path.as_str()),
                "# Changed",
            )
            .unwrap();
        }
        let single = state
            .reconcile_project_change(
                &session.instance_id,
                FilesystemChange::file(
                    RelativePath::try_new(documents[0].path.as_str()).unwrap(),
                    FileChangeKind::Modified,
                ),
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            single.resources(),
            Some(
                [ResourceKey::Doc(DocResourceKey(
                    documents[0].path.as_str().into()
                ))]
                .as_slice()
            )
        );
        assert_eq!(
            state
                .read_file(&session.instance_id, &documents[1].path)
                .unwrap()
                .content,
            documents[1].content
        );
        let changed = state
            .reconcile_project_change(&session.instance_id, FilesystemChange::rescan_required())
            .unwrap()
            .unwrap();
        assert!(changed.resources().is_none());
        for document in &documents {
            assert_eq!(
                state
                    .read_file(&session.instance_id, &document.path)
                    .unwrap()
                    .content
                    .0,
                "# Changed"
            );
        }
    }

    #[test]
    fn document_hint_preserves_unrelated_dirty_graphs_and_never_reads_unloaded_bodies() {
        use crate::{GraphHistoryAction, docs::DocCommand};
        use std::sync::Arc;
        use yss_graph_document::{
            DocumentNode, GraphDocumentOperation, GraphDocumentPatch, GraphResourceKind,
            GraphResourcePath, NodeId, NodePosition,
        };
        use yss_graph_document_edit::apply_graph_document_patch;
        use yss_project_identity::OperationId;
        let dirty = GraphResourcePath::new("events/Dirty.yssbi-event").unwrap();
        let clean = GraphResourcePath::new("events/Clean.yssbi-event").unwrap();
        let unloaded = GraphResourcePath::new("functions/Unloaded.yssbi-function").unwrap();
        let mut data = ProjectData::new();
        for path in [&dirty, &clean, &unloaded] {
            data.graphs.insert(
                path.clone(),
                yss_project_model::GraphResourceDocument::new(
                    path.display_name(),
                    if path == &unloaded {
                        GraphResourceKind::FunctionGraph
                    } else {
                        GraphResourceKind::EventGraph
                    },
                ),
            );
        }
        let fixture = fixtures::TempProject::activate("watcher-isolated-doc", data);
        let state = fixture.state();
        let session = state.capture_project_session().unwrap();
        let original = state
            .read_graph_editing(&session.instance_id, &dirty)
            .unwrap();
        let patch = GraphDocumentPatch::new(vec![GraphDocumentOperation::InsertNode {
            node: DocumentNode {
                id: NodeId::new(),
                node_type: "yssbi.tests.node".parse().unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: Default::default(),
                user_label: None,
            },
        }]);
        let capture = state
            .capture_graph_edit(
                &session.instance_id,
                &dirty,
                original.state.version,
                OperationId::new(),
                [0; 32],
            )
            .unwrap();
        let mut authored = (*original.document).clone();
        apply_graph_document_patch(&mut authored, &patch).unwrap();
        let dirty_state = state
            .commit_graph_edit(capture, Arc::new(authored), GraphHistoryAction::Edit(patch))
            .unwrap()
            .editing;
        assert!(dirty_state.dirty && dirty_state.can_undo);
        let clean_before = state
            .read_graph_editing(&session.instance_id, &clean)
            .unwrap();
        state
            .unload_graph_resource(
                &state.capture_project_session().unwrap().instance_id,
                &unloaded,
                None,
            )
            .unwrap();
        std::fs::write(
            session.root.as_path().join(unloaded.as_str()),
            b"unrelated unreadable body",
        )
        .unwrap();
        let document = state
            .apply_file_command(
                &session.instance_id,
                OperationId::new(),
                DocCommand::Create {
                    name: "Notes".into(),
                },
            )
            .unwrap()
            .snapshot
            .unwrap();
        let edited = state
            .apply_file_command(
                &session.instance_id,
                OperationId::new(),
                DocCommand::Edit {
                    path: document.path,
                    version: document.version,
                    edits: vec![yss_project_model::doc::DocEdit::SetMarkdown {
                        markdown: "# Saved".into(),
                    }],
                },
            )
            .unwrap()
            .snapshot
            .unwrap();
        let document = state
            .apply_file_command(
                &session.instance_id,
                OperationId::new(),
                DocCommand::Save {
                    path: edited.path,
                    version: edited.version,
                },
            )
            .unwrap()
            .snapshot
            .unwrap();
        let doc_event = || {
            FilesystemChange::file(
                RelativePath::try_new(document.path.as_str()).unwrap(),
                FileChangeKind::Modified,
            )
        };
        assert!(
            state
                .reconcile_project_change(&session.instance_id, doc_event())
                .unwrap()
                .is_none()
        );
        std::fs::write(
            session.root.as_path().join(document.path.as_str()),
            "# External",
        )
        .unwrap();
        let changed = state
            .reconcile_project_change(&session.instance_id, doc_event())
            .unwrap()
            .unwrap();
        assert_eq!(
            changed.resources(),
            Some(
                [ResourceKey::Doc(DocResourceKey(
                    document.path.as_str().into()
                ))]
                .as_slice()
            )
        );
        assert_eq!(
            state
                .read_graph_editing(&session.instance_id, &dirty)
                .unwrap()
                .state,
            dirty_state
        );
        assert_eq!(
            state
                .read_graph_editing(&session.instance_id, &clean)
                .unwrap()
                .state,
            clean_before.state
        );
        assert!(state.read_resident_graph(&unloaded).unwrap().is_none());

        // A genuine graph hint still commits its new body and addresses its consumers,
        // without broadening to either the dirty graph or the unreadable unloaded function.
        let mut incoming = ProjectData::new();
        let mut resource = state.read_resident_graph(&clean).unwrap().unwrap();
        let id = NodeId::new();
        Arc::make_mut(&mut resource.document).nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: "yssbi.tests.node".parse().unwrap(),
                position: NodePosition { x: 10.0, y: 20.0 },
                parameters: Default::default(),
                user_label: None,
            },
        );
        incoming.graphs.insert(clean.clone(), resource);
        fixtures::write_graph(&incoming, session.root.as_path().to_str().unwrap(), &clean).unwrap();
        let graph_changed = state
            .reconcile_project_change(
                &session.instance_id,
                FilesystemChange::file(
                    RelativePath::try_new(clean.as_str()).unwrap(),
                    FileChangeKind::Modified,
                ),
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            graph_changed.resources(),
            Some([ResourceKey::Graph(clean.clone())].as_slice())
        );
        let clean_after = state
            .read_graph_editing(&session.instance_id, &clean)
            .unwrap();
        assert!(clean_after.state.version.revision > clean_before.state.version.revision);
        assert!(clean_after.document.nodes.contains_key(&id));
        assert_eq!(
            state
                .read_graph_editing(&session.instance_id, &dirty)
                .unwrap()
                .state,
            dirty_state
        );
    }

    #[test]
    fn graph_hints_keep_new_and_unloaded_function_index_changes_visible() {
        use crate::docs::DocCommand;
        use yss_graph_document::{GraphResourceKind, GraphResourcePath};
        use yss_project_identity::OperationId;
        let retained = GraphResourcePath::new("functions/Retained.yssbi-function").unwrap();
        let removed = GraphResourcePath::new("functions/Removed.yssbi-function").unwrap();
        let added = GraphResourcePath::new("functions/Added.yssbi-function").unwrap();
        let mut data = ProjectData::new();
        for path in [&retained, &removed] {
            data.graphs.insert(
                path.clone(),
                yss_project_model::GraphResourceDocument::new(
                    path.display_name(),
                    GraphResourceKind::FunctionGraph,
                ),
            );
        }
        let fixture = fixtures::TempProject::activate("watcher-unloaded-index", data);
        let state = fixture.state();
        let session = state.capture_project_session().unwrap();
        for path in [&retained, &removed] {
            state
                .unload_graph_resource(
                    &state.capture_project_session().unwrap().instance_id,
                    path,
                    None,
                )
                .unwrap();
        }
        let document = state
            .apply_file_command(
                &session.instance_id,
                OperationId::new(),
                DocCommand::Create {
                    name: "Notes".into(),
                },
            )
            .unwrap()
            .snapshot
            .unwrap();
        let mut incoming = ProjectData::new();
        incoming.graphs.insert(
            added.clone(),
            yss_project_model::GraphResourceDocument::new(
                added.display_name(),
                GraphResourceKind::FunctionGraph,
            ),
        );
        fixtures::write_graph(&incoming, session.root.as_path().to_str().unwrap(), &added).unwrap();
        std::fs::remove_file(session.root.as_path().join(removed.as_str())).unwrap();
        assert!(
            state
                .reconcile_project_change(
                    &session.instance_id,
                    FilesystemChange::file(
                        RelativePath::try_new(document.path.as_str()).unwrap(),
                        FileChangeKind::Modified
                    ),
                )
                .unwrap()
                .is_none(),
            "a document echo does not reconcile unrelated graph paths"
        );
        for path in [&retained, &removed, &added] {
            let changed = state
                .reconcile_project_change(
                    &session.instance_id,
                    FilesystemChange::file(
                        RelativePath::try_new(path.as_str()).unwrap(),
                        FileChangeKind::Modified,
                    ),
                )
                .unwrap()
                .unwrap();
            assert_eq!(
                changed.resources(),
                Some([ResourceKey::Graph(path.clone())].as_slice())
            );
            assert!(
                state.read_resident_graph(path).unwrap().is_none(),
                "index invalidation must not load function bodies"
            );
        }
        let index = state.read_project_index(&session.instance_id).unwrap();
        assert!(
            index
                .function_graphs
                .iter()
                .any(|entry| entry.path == added.as_str())
        );
        assert!(
            !index
                .function_graphs
                .iter()
                .any(|entry| entry.path == removed.as_str())
        );
    }

    #[test]
    fn project_reconciliation_returns_typed_index_invalidation() {
        let fixture = fixtures::TempProject::activate("watcher-reconcile", ProjectData::new());
        let project = ProjectInstanceId::from_existing(fixture.state().project_instance_id());
        let invalidation = fixture
            .state()
            .reconcile_project_change(
                &project,
                FilesystemChange::file(
                    RelativePath::try_new("metadata.yssbi").unwrap(),
                    FileChangeKind::Modified,
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
                yss_graph_document::GraphResourceKind::EventGraph,
            ),
        );
        let (chart, document) = fixtures::chart("Chart", "data");
        data.charts.insert(
            chart.clone(),
            yss_project_model::file::FileState::new(document, "chart".into()).unwrap(),
        );
        let fixture = fixtures::TempProject::activate("external-deletion", data);
        let state = fixture.state();
        let session = state.capture_project_session().unwrap();
        std::fs::remove_file(session.root.as_path().join(graph.as_str())).unwrap();
        std::fs::remove_file(session.root.as_path().join(chart.as_str())).unwrap();
        // Index queries must reflect disk membership even before the watcher drains.
        let index = state.read_project_index(&session.instance_id).unwrap();
        assert!(
            index.event_graphs.is_empty()
                && index.function_graphs.is_empty()
                && index.charts.is_empty()
        );
        state
            .reconcile_project_change(&session.instance_id, FilesystemChange::rescan_required())
            .unwrap();
        let data = state.get_data().unwrap();
        assert!(data.graphs.is_empty() && data.charts.is_empty());
        let revision = state
            .read_project_index(&session.instance_id)
            .unwrap()
            .publication_revision;
        assert!(revision > 0);
        state
            .reconcile_project_change(&session.instance_id, FilesystemChange::rescan_required())
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
