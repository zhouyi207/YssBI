use super::*;
use yss_project_model::ProjectDataPatch;

impl ProjectState {
    pub(super) fn commit_chart_move_history_direction(
        &self,
        project_instance_id: &ProjectInstanceId,
        undo: bool,
        request: MutationRequest<HistoryMutation>,
        transaction: ProjectHistoryTransaction,
    ) -> Result<CommittedResourceMutation, ProjectHistoryMutationError> {
        let history_id = transaction.history_id;
        let move_patch = transaction.resource_move.ok_or_else(|| {
            ProjectHistoryMutationError::History("resource move history patch is missing".into())
        })?;
        if move_patch.kind != yss_project_history::ResourceLifecycleKind::Chart {
            return Err(ProjectHistoryMutationError::History(
                "chart move history has a non-chart kind".into(),
            ));
        }
        let yss_project_history::ResourceMoveHistoryPayload::Chart { document } =
            move_patch.payload
        else {
            return Err(ProjectHistoryMutationError::History(
                "chart move history has a non-chart payload".into(),
            ));
        };
        let source = ChartResourcePath::parse(if undo {
            move_patch.to.as_ref()
        } else {
            move_patch.from.as_ref()
        })
        .map_err(|error| ProjectHistoryMutationError::History(error.to_string().into()))?;
        let target = ChartResourcePath::parse(if undo {
            move_patch.from.as_ref()
        } else {
            move_patch.to.as_ref()
        })
        .map_err(|error| ProjectHistoryMutationError::History(error.to_string().into()))?;
        let session = self
            .capture_project_session()
            .map_err(history_project_error)?;
        if session.instance_id != *project_instance_id {
            return Err(ProjectHistoryMutationError::StaleProjectLifecycle(
                "caller project changed before chart move History preparation".into(),
            ));
        }
        let lease = self
            .filesystem()
            .acquire(session.root.clone())
            .map_err(history_project_error)?;
        self.validate_project_session(&session)
            .map_err(history_project_error)?;
        let current = self
            .project_data
            .read()
            .unwrap()
            .charts
            .get(&source)
            .cloned()
            .ok_or_else(|| {
                ProjectHistoryMutationError::History(
                    format!("chart '{}' is absent", source.as_str()).into(),
                )
            })?;
        let current_revision = self
            .chart_revisions
            .read()
            .unwrap()
            .get(&source)
            .copied()
            .ok_or_else(|| {
                ProjectHistoryMutationError::History(
                    format!("chart '{}' has no revision authority", source.as_str()).into(),
                )
            })?;
        let expected_resource = ResourceKey::Chart(yss_project_history::ChartResourceKey(
            source.as_str().into(),
        ));
        if request.resource != expected_resource {
            return Err(ProjectHistoryMutationError::ResourceMismatch {
                requested: format!("{:?}", request.resource).into(),
                store: format!("{:?}", expected_resource).into(),
            });
        }
        if request.base_revision != current_revision {
            return Err(ProjectHistoryMutationError::StaleRevision {
                base_revision: request.base_revision.get(),
                current_revision: current_revision.get(),
            });
        }
        let mut moved = document;
        moved.revision = checked_resource_revision(source.as_str(), current_revision)
            .map_err(history_project_error)?;
        let context = ProjectTransactionContext {
            session,
            operation_id: request.operation_id,
            affected_resources: vec![ResourceKey::Chart(yss_project_history::ChartResourceKey(
                source.as_str().into(),
            ))],
            expected_revisions: [(
                ResourceKey::Chart(yss_project_history::ChartResourceKey(
                    source.as_str().into(),
                )),
                current_revision,
            )]
            .into_iter()
            .collect(),
            expected_absent_resources: [ResourceKey::Chart(yss_project_history::ChartResourceKey(
                target.as_str().into(),
            ))]
            .into_iter()
            .collect(),
            recovery_marker: Some(self.project_recovery_marker()),
        };
        let prepared = ProjectFilesystemTransaction::prepare(
            context.filesystem_context(),
            lease,
            vec![StagedFilesystemMutation::MoveFile {
                from: source.relative_path().to_path_buf(),
                to: target.relative_path().to_path_buf(),
            }],
        )
        .map_err(history_project_error)?;
        let committed_filesystem = prepared.commit().map_err(history_project_error)?;
        let publication = self.apply_resource_document_patch_internal(
            &context,
            ProjectDataPatch::MoveChart {
                from: source,
                to: target,
                moved: {
                    let _ = current;
                    moved
                },
            },
            Some((undo, history_id)),
            None,
        );
        match publication {
            Ok(receipt) => {
                committed_filesystem.finalize();
                Ok(receipt)
            }
            Err(error) => Err(resolve_history_rollback(
                history_project_error(error),
                committed_filesystem.rollback(),
            )),
        }
    }

    pub(super) fn commit_graph_move_history_direction(
        &self,
        project_instance_id: &ProjectInstanceId,
        undo: bool,
        request: MutationRequest<HistoryMutation>,
        transaction: ProjectHistoryTransaction,
    ) -> Result<CommittedResourceMutation, ProjectHistoryMutationError> {
        let history_id = transaction.history_id;
        let move_patch = transaction.resource_move.ok_or_else(|| {
            ProjectHistoryMutationError::History("resource move history patch is missing".into())
        })?;
        let yss_project_history::ResourceMoveHistoryPayload::Graph {
            persisted_move_payload,
        } = move_patch.payload
        else {
            return Err(ProjectHistoryMutationError::History(
                "graph move history has a non-graph payload".into(),
            ));
        };
        let payload: GraphMoveHistoryPayload = serde_json::from_value(persisted_move_payload)
            .map_err(|error| ProjectHistoryMutationError::History(error.to_string().into()))?;
        let source = GraphResourcePath::new(if undo {
            move_patch.to.as_ref()
        } else {
            move_patch.from.as_ref()
        })
        .map_err(|error| ProjectHistoryMutationError::History(error.to_string().into()))?;
        let target = GraphResourcePath::new(if undo {
            move_patch.from.as_ref()
        } else {
            move_patch.to.as_ref()
        })
        .map_err(|error| ProjectHistoryMutationError::History(error.to_string().into()))?;
        let desired_moved = if undo {
            payload.moved_before.clone()
        } else {
            payload.moved_after.clone()
        };
        let desired_graphs = if undo {
            payload.referenced_graphs_before.clone()
        } else {
            payload.referenced_graphs_after.clone()
        };

        let session = self
            .capture_project_session()
            .map_err(history_project_error)?;
        if session.instance_id != *project_instance_id {
            return Err(ProjectHistoryMutationError::StaleProjectLifecycle(
                "caller project changed before graph move History preparation".into(),
            ));
        }
        let filesystem_lease = self
            .filesystem()
            .acquire(session.root.clone())
            .map_err(history_project_error)?;
        self.validate_project_session(&session)
            .map_err(history_project_error)?;
        let loaded_source = self
            .project_data
            .read()
            .unwrap()
            .graphs
            .get(&source)
            .cloned();
        let current_moved = loaded_source
            .clone()
            .map_or_else(
                || {
                    load_project_graph_from_file(
                        session.root.as_path().to_string_lossy().as_ref(),
                        &source,
                    )
                },
                Ok,
            )
            .map_err(|error| ProjectHistoryMutationError::History(error.to_string().into()))?;
        if request.resource != ResourceKey::Graph(source.clone()) {
            return Err(ProjectHistoryMutationError::ResourceMismatch {
                requested: format!("{:?}", request.resource).into(),
                store: source.as_str().into(),
            });
        }
        let current_revision = self
            .graph_resource_revisions
            .read()
            .unwrap()
            .get(&source)
            .copied()
            .unwrap_or(ResourceRevision::INITIAL);
        if current_revision != request.base_revision {
            return Err(ProjectHistoryMutationError::StaleRevision {
                base_revision: request.base_revision.get(),
                current_revision: current_revision.get(),
            });
        }
        let mut referenced_graphs_before = BTreeMap::new();
        let mut referenced_graphs = BTreeMap::new();
        let mut affected_resources = Vec::new();
        let mut expected_revisions = BTreeMap::new();
        let source_key = ResourceKey::Graph(source.clone());
        if loaded_source.is_some() {
            affected_resources.push(source_key.clone());
        }
        expected_revisions.insert(source_key, current_revision);
        {
            let data = self.project_data.read().unwrap();
            for (path, desired) in desired_graphs {
                let Some(current) = data.graphs.get(&path) else {
                    continue;
                };
                let next = desired;
                let key = ResourceKey::Graph(path.clone());
                affected_resources.push(key.clone());
                let revision = self
                    .graph_resource_revisions
                    .read()
                    .unwrap()
                    .get(&path)
                    .copied()
                    .unwrap_or(ResourceRevision::INITIAL);
                expected_revisions.insert(key, revision);
                referenced_graphs_before.insert(path.clone(), current.clone());
                referenced_graphs.insert(path, next);
            }
        }
        let loaded_referenced_graphs = referenced_graphs.keys().cloned().collect();
        let disk_plan = Self::graph_rename_mutations(
            session.root.as_path(),
            &source,
            &target,
            &desired_moved,
            &loaded_referenced_graphs,
        )
        .map_err(history_project_error)?;
        for (path, before) in disk_plan.referenced_graphs_before {
            let key = ResourceKey::Graph(path.clone());
            affected_resources.push(key.clone());
            expected_revisions.insert(key, ResourceRevision::INITIAL);
            referenced_graphs_before.insert(path, before);
        }
        referenced_graphs.extend(disk_plan.referenced_graphs_after);
        let context = ProjectTransactionContext {
            session,
            operation_id: request.operation_id,
            affected_resources,
            expected_revisions,
            expected_absent_resources: [ResourceKey::Graph(target.clone())].into_iter().collect(),
            recovery_marker: Some(self.project_recovery_marker()),
        };
        let mutations = disk_plan.mutations;
        let prepared = ProjectFilesystemTransaction::prepare_with_validator(
            context.filesystem_context(),
            filesystem_lease,
            mutations,
            |_path, contents| {
                {
                    serde_json::from_slice::<crate::project_io::GraphResourceFile>(contents)
                        .map(|_| ())
                        .map_err(|error| error.to_string())
                }
            },
        )
        .map_err(history_project_error)?;
        let committed_filesystem = prepared.commit().map_err(history_project_error)?;
        let publication = self.apply_resource_document_patch_internal(
            &context,
            ProjectDataPatch::MoveGraph {
                from: source,
                to: target,
                moved_before: Box::new(current_moved),
                moved: desired_moved,
                referenced_graphs_before,
                referenced_graphs,
                loaded_referenced_graphs,
            },
            Some((undo, history_id)),
            None,
        );
        match publication {
            Ok(receipt) => {
                committed_filesystem.finalize();
                Ok(receipt)
            }
            Err(error) => Err(resolve_history_rollback(
                history_project_error(error),
                committed_filesystem.rollback(),
            )),
        }
    }
}
