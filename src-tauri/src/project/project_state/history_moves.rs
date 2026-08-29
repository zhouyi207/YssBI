use super::*;
use crate::project::resource_patch::ResourceDocumentPatch;

impl ProjectState {
    pub(super) fn commit_variable_effect_history_direction(
        &self,
        project_instance_id: &ProjectInstanceId,
        undo: bool,
        request: MutationRequest<HistoryMutation>,
        transaction: ProjectHistoryTransaction,
    ) -> Result<CommittedResourceMutation, ProjectHistoryMutationError> {
        let history_id = transaction.history_id.clone();
        let session = self
            .capture_project_session()
            .map_err(history_project_error)?;
        if session.instance_id != *project_instance_id {
            return Err(ProjectHistoryMutationError::StaleProjectLifecycle(
                "caller project changed before durable variable History preparation".into(),
            ));
        }
        let expected_project_path = self.get_path().ok_or_else(|| {
            ProjectHistoryMutationError::History(
                "no project is active for variable persistence".into(),
            )
        })?;
        let authority = self
            .capture_project_authority_for_session(&session)
            .map_err(history_project_error)?;
        let filesystem_lease = self
            .filesystem()
            .acquire(session.root.clone())
            .map_err(history_project_error)?;
        self.validate_project_session(&session)
            .map_err(history_project_error)?;

        let (data_snapshot, graph_revisions, variable_revisions, history_snapshot) = {
            let publication = self.mutation_publication.lock().unwrap();
            let path = self.project_path.read().unwrap();
            if publication.project_instance_id != session.instance_id.as_str()
                || path.as_deref() != Some(expected_project_path.as_str())
            {
                return Err(ProjectHistoryMutationError::StaleProjectLifecycle(
                    "project changed before durable History snapshot".into(),
                ));
            }
            (
                self.project_data.read().unwrap().clone(),
                self.graph_revisions.read().unwrap().clone(),
                self.variable_revisions.read().unwrap().clone(),
                self.history.read().unwrap().clone(),
            )
        };
        let mut documents = project_documents(&data_snapshot, &variable_revisions);
        let current_revision = try_project_document_revision(&documents, &request.resource)
            .ok_or_else(|| {
                ProjectHistoryMutationError::History(
                    format!(
                        "history anchor resource {:?} was not found",
                        request.resource
                    )
                    .into(),
                )
            })?;
        if current_revision != request.base_revision {
            return Err(ProjectHistoryMutationError::StaleRevision {
                base_revision: request.base_revision.get(),
                current_revision: current_revision.get(),
            });
        }
        let before = documents.clone();
        let mut proposed_history = history_snapshot;
        let applied = if undo {
            proposed_history.undo(&mut documents)
        } else {
            proposed_history.redo(&mut documents)
        }
        .map_err(|error| ProjectHistoryMutationError::History(error.to_string().into()))?;
        if applied.history_id != history_id {
            return Err(ProjectHistoryMutationError::History(
                crate::project::HistoryError::HistoryHeadChanged
                    .to_string()
                    .into(),
            ));
        }
        let mut proposed_data = data_snapshot.clone();
        let mut proposed_revisions = variable_revisions.clone();
        replace_project_documents(
            &mut proposed_data,
            &mut proposed_revisions,
            documents.clone(),
        );
        let ids = install_variable_effect_snapshots(&mut proposed_data, &transaction, undo)
            .map_err(|error| ProjectHistoryMutationError::History(error.into()))?;

        let mut expected_revisions = BTreeMap::new();
        for change in &transaction.changes {
            expected_revisions.insert(
                change.resource.clone(),
                project_document_revision(&before, &change.resource),
            );
        }
        for id in &ids {
            let scope = variable_history_scope(&proposed_data, &transaction, *id, undo)
                .map_err(|error| ProjectHistoryMutationError::History(error.into()))?;
            if let Some(graph_path) = variable_scope_graph_path(&scope)
                .map_err(|error| ProjectHistoryMutationError::History(error.into()))?
            {
                let revision = graph_revisions.get(&graph_path).copied().ok_or_else(|| {
                    ProjectHistoryMutationError::History(
                        format!("local variable graph '{graph_path}' is not loaded").into(),
                    )
                })?;
                expected_revisions.insert(
                    ResourceKey::Graph(graph_path.clone()),
                    ResourceRevision::from_graph_revision(revision),
                );
            }
        }
        let mutations =
            variable_effect_filesystem_mutations(&proposed_data, &ids, &transaction, undo)
                .map_err(|error| ProjectHistoryMutationError::History(error.into()))?;
        let context = ProjectTransactionContext {
            session,
            operation_id: request.operation_id,
            affected_resources: expected_revisions.keys().cloned().collect(),
            expected_revisions,
            expected_absent_resources: Default::default(),
            recovery_marker: Some(self.project_recovery_marker()),
        };
        let prepared = ProjectFilesystemTransaction::prepare_with_validator(
            context.clone(),
            filesystem_lease,
            mutations,
            validate_variable_effect_document,
        )
        .map_err(history_project_error)?;
        let committed_filesystem = prepared.commit().map_err(history_project_error)?;
        self.run_history_after_disk_commit_test_hook();

        let authority_result = (|| {
            let mut publication = self.mutation_publication.lock().unwrap();
            let path = self.project_path.read().unwrap();
            if publication.project_instance_id != context.session.instance_id.as_str()
                || path.as_deref() != Some(expected_project_path.as_str())
            {
                return Err(ProjectHistoryMutationError::StaleProjectLifecycle(
                    "project changed before durable History authority commit".into(),
                ));
            }
            if !authority.matches_publication(&publication) {
                return Err(ProjectHistoryMutationError::StaleProjectLifecycle(
                    "projection environment changed before durable History authority commit".into(),
                ));
            }
            let mut data = self.project_data.write().unwrap();
            let graph_revisions = self.graph_revisions.read().unwrap();
            let mut revisions = self.variable_revisions.write().unwrap();
            validate_context_revisions(
                &context,
                &data,
                &graph_revisions,
                &revisions,
                &self.worksheet_revisions.read().unwrap(),
            )
            .map_err(|error| ProjectHistoryMutationError::History(error.to_string().into()))?;
            self.run_mutation_publication_test_hook();
            let current_history = self.history.read().unwrap();
            let current_head = if undo {
                current_history.next_undo()
            } else {
                current_history.next_redo()
            };
            if current_head.map(|entry| &entry.history_id) != Some(&history_id) {
                return Err(ProjectHistoryMutationError::History(
                    crate::project::HistoryError::HistoryHeadChanged
                        .to_string()
                        .into(),
                ));
            }
            let mut next_history = current_history.clone();
            drop(current_history);
            let mut current_documents = project_documents(&data, &revisions);
            let before = current_documents.clone();
            let applied = if undo {
                next_history.undo(&mut current_documents)
            } else {
                next_history.redo(&mut current_documents)
            }
            .map_err(|error| ProjectHistoryMutationError::History(error.to_string().into()))?;
            if applied.history_id != history_id {
                return Err(ProjectHistoryMutationError::History(
                    crate::project::HistoryError::HistoryHeadChanged
                        .to_string()
                        .into(),
                ));
            }
            let mut next_data = data.clone();
            let mut next_revisions = revisions.clone();
            replace_project_documents(
                &mut next_data,
                &mut next_revisions,
                current_documents.clone(),
            );
            install_variable_effect_snapshots(&mut next_data, &transaction, undo)
                .map_err(|error| ProjectHistoryMutationError::History(error.into()))?;
            let deltas = transaction
                .changes
                .iter()
                .map(|change| crate::project::ResourceDeltaEvent {
                    resource: change.resource.clone(),
                    from_revision: project_document_revision(&before, &change.resource),
                    to_revision: project_document_revision(&current_documents, &change.resource),
                    caused_by: Some(request.operation_id),
                    payload: if undo {
                        change.inverse.clone()
                    } else {
                        change.forward.clone()
                    },
                })
                .collect::<Vec<_>>();
            let expected_graph_paths = affected_projection_paths(&deltas, &next_data);
            let publication_advance = publication
                .prepare_resource_revision()
                .map_err(history_project_error)?;
            *data = next_data;
            *revisions = next_revisions;
            let history_status = next_history.status();
            *self.history.write().unwrap() = next_history;
            let publication_revision = publication.commit_prepared(publication_advance);
            #[cfg(test)]
            let completion_test_hook = self
                .test_hooks
                .committed_resource_completion_test_hook
                .read()
                .unwrap()
                .clone();
            Ok(CommittedResourceMutation {
                operation_id: request.operation_id,
                project_instance_id: publication.project_instance_id.clone(),
                publication_revision,
                moves: Vec::new(),
                deltas,
                history: crate::project::project_writers::ProjectHistoryStatus {
                    can_undo: history_status.can_undo,
                    can_redo: history_status.can_redo,
                },
                expected_graph_paths,
                #[cfg(test)]
                completion_test_hook,
            })
        })();

        match authority_result {
            Ok(result) => {
                committed_filesystem.finalize();
                Ok(result)
            }
            Err(error) => Err(resolve_history_rollback(
                error,
                committed_filesystem.rollback(),
            )),
        }
    }

    pub(super) fn commit_worksheet_move_history_direction(
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
        if move_patch.kind != crate::project::ResourceLifecycleKind::Worksheet {
            return Err(ProjectHistoryMutationError::History(
                "worksheet move history has a non-worksheet kind".into(),
            ));
        }
        let crate::project::ResourceMoveHistoryPayload::Worksheet { document } = move_patch.payload
        else {
            return Err(ProjectHistoryMutationError::History(
                "worksheet move history has a non-worksheet payload".into(),
            ));
        };
        let source = WorksheetResourcePath::parse(if undo {
            move_patch.to.as_ref()
        } else {
            move_patch.from.as_ref()
        })
        .map_err(|error| ProjectHistoryMutationError::History(error.to_string().into()))?;
        let target = WorksheetResourcePath::parse(if undo {
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
                "caller project changed before worksheet move History preparation".into(),
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
            .worksheets
            .get(&source)
            .cloned()
            .ok_or_else(|| {
                ProjectHistoryMutationError::History(
                    format!("worksheet '{}' is absent", source.as_str()).into(),
                )
            })?;
        let current_revision = self
            .worksheet_revisions
            .read()
            .unwrap()
            .get(&source)
            .copied()
            .ok_or_else(|| {
                ProjectHistoryMutationError::History(
                    format!("worksheet '{}' has no revision authority", source.as_str()).into(),
                )
            })?;
        let expected_resource =
            ResourceKey::Worksheet(crate::project::WorksheetResourceKey(source.as_str().into()));
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
            affected_resources: vec![ResourceKey::Worksheet(
                crate::project::WorksheetResourceKey(source.as_str().into()),
            )],
            expected_revisions: [(
                ResourceKey::Worksheet(crate::project::WorksheetResourceKey(
                    source.as_str().into(),
                )),
                current_revision,
            )]
            .into_iter()
            .collect(),
            expected_absent_resources: [ResourceKey::Worksheet(
                crate::project::WorksheetResourceKey(target.as_str().into()),
            )]
            .into_iter()
            .collect(),
            recovery_marker: Some(self.project_recovery_marker()),
        };
        let prepared = ProjectFilesystemTransaction::prepare(
            context.clone(),
            lease,
            vec![StagedFilesystemMutation::MoveFile {
                from: source.relative_path().to_path_buf(),
                to: target.relative_path().to_path_buf(),
            }],
        )
        .map_err(history_project_error)?;
        let committed_filesystem = prepared.commit().map_err(history_project_error)?;
        self.run_history_after_disk_commit_test_hook();
        let publication = self.apply_resource_document_patch_internal(
            &context,
            ResourceDocumentPatch::MoveWorksheet {
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
        let history_id = transaction.history_id.clone();
        let move_patch = transaction.resource_move.ok_or_else(|| {
            ProjectHistoryMutationError::History("resource move history patch is missing".into())
        })?;
        let crate::project::ResourceMoveHistoryPayload::Graph {
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
        let mut desired_moved = if undo {
            payload.moved_before.clone()
        } else {
            payload.moved_after.clone()
        };
        let desired_graphs = if undo {
            payload.referenced_graphs_before.clone()
        } else {
            payload.referenced_graphs_after.clone()
        };
        let desired_variables = if undo {
            payload.referenced_variables_before.clone()
        } else {
            payload.referenced_variables_after.clone()
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
        let current_revision = loaded_source
            .as_ref()
            .map(|resource| resource.document.revision)
            .or_else(|| self.graph_revisions.read().unwrap().get(&source).copied())
            .unwrap_or(current_moved.document.revision);
        if ResourceRevision::from_graph_revision(current_revision) != request.base_revision {
            return Err(ProjectHistoryMutationError::StaleRevision {
                base_revision: request.base_revision.get(),
                current_revision: current_revision.get(),
            });
        }
        desired_moved.document.revision = checked_graph_revision(source.as_str(), current_revision)
            .map_err(history_project_error)?;

        let mut referenced_graphs_before = BTreeMap::new();
        let mut referenced_graphs = BTreeMap::new();
        let mut referenced_variables_before = BTreeMap::new();
        let mut referenced_variables = BTreeMap::new();
        let mut affected_resources = Vec::new();
        let mut expected_revisions = BTreeMap::new();
        let source_key = ResourceKey::Graph(source.clone());
        if loaded_source.is_some() {
            affected_resources.push(source_key.clone());
        }
        expected_revisions.insert(
            source_key,
            ResourceRevision::from_graph_revision(current_revision),
        );
        {
            let data = self.project_data.read().unwrap();
            let variable_revisions = self.variable_revisions.read().unwrap();
            for (path, desired) in desired_graphs {
                let Some(current) = data.graphs.get(&path) else {
                    continue;
                };
                let mut next = desired;
                next.document.revision =
                    checked_graph_revision(path.as_str(), current.document.revision)
                        .map_err(history_project_error)?;
                let key = ResourceKey::Graph(path.clone());
                affected_resources.push(key.clone());
                expected_revisions.insert(
                    key,
                    ResourceRevision::from_graph_revision(current.document.revision),
                );
                referenced_graphs_before.insert(path.clone(), current.clone());
                referenced_graphs.insert(path, next);
            }
            for (id, desired) in desired_variables {
                let Some(current) = data.variables.get(&id) else {
                    continue;
                };
                let key = ResourceKey::Variable(crate::project::VariableResourceKey(
                    format!("variables/{id}").into(),
                ));
                affected_resources.push(key.clone());
                expected_revisions.insert(
                    key,
                    variable_revisions
                        .get(&id)
                        .map(|entry| entry.revision)
                        .unwrap_or(crate::project::ResourceRevision::INITIAL),
                );
                referenced_variables_before.insert(id, current.clone());
                referenced_variables.insert(id, desired);
            }
        }
        let loaded_referenced_graphs = referenced_graphs.keys().cloned().collect();
        let known_graph_revisions = self.graph_revisions.read().unwrap().clone();
        let disk_plan = Self::graph_rename_mutations(
            session.root.as_path(),
            &source,
            &target,
            &desired_moved,
            referenced_variables
                .values()
                .cloned()
                .map(|variable| (variable.id, variable))
                .collect(),
            &loaded_referenced_graphs,
            &known_graph_revisions,
        )
        .map_err(|error| ProjectHistoryMutationError::History(error.into()))?;
        for (path, before) in disk_plan.referenced_graphs_before {
            let key = ResourceKey::Graph(path.clone());
            affected_resources.push(key.clone());
            expected_revisions.insert(
                key,
                ResourceRevision::from_graph_revision(before.document.revision),
            );
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
            context.clone(),
            filesystem_lease,
            mutations,
            |path, contents| {
                if path == std::path::Path::new(crate::project::GLOBAL_VARIABLES_FILE) {
                    serde_json::from_slice::<crate::project::project_io::GlobalVariablesDocument>(
                        contents,
                    )
                    .map(|_| ())
                    .map_err(|error| error.to_string())
                } else {
                    serde_json::from_slice::<crate::project::project_io::GraphDocument>(contents)
                        .map(|_| ())
                        .map_err(|error| error.to_string())
                }
            },
        )
        .map_err(history_project_error)?;
        let committed_filesystem = prepared.commit().map_err(history_project_error)?;
        self.run_graph_move_history_io_checkpoint();
        let publication = self.apply_resource_document_patch_internal(
            &context,
            ResourceDocumentPatch::MoveGraph {
                from: source,
                to: target,
                moved_before: current_moved,
                moved: desired_moved,
                referenced_graphs_before,
                referenced_graphs,
                loaded_referenced_graphs,
                referenced_variables_before,
                referenced_variables,
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
