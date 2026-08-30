use super::*;
use crate::project::resource_patch::ResourceDocumentPatch;

pub(super) struct CommittedResourceMutation {
    pub(in crate::project::project_state) operation_id: yss_project_identity::OperationId,
    pub(in crate::project::project_state) project_instance_id: String,
    pub(in crate::project::project_state) publication_revision: u64,
    pub(in crate::project::project_state) moves:
        Vec<crate::project::project_writers::ProjectResourceMove>,
    pub(in crate::project::project_state) deltas: Vec<yss_project_history::ResourceDeltaEvent>,
    pub(in crate::project::project_state) history:
        crate::project::project_writers::ProjectHistoryStatus,
    pub(in crate::project::project_state) expected_graph_paths: Vec<String>,
    #[cfg(test)]
    pub(in crate::project::project_state) completion_test_hook:
        Option<CommittedResourceCompletionTestHook>,
}

impl CommittedResourceMutation {
    pub(in crate::project) fn into_project_facts(
        self,
    ) -> crate::project::project_writers::ProjectResourceMutationFacts {
        let Self {
            operation_id,
            project_instance_id,
            publication_revision,
            moves,
            deltas,
            history,
            expected_graph_paths,
            #[cfg(test)]
                completion_test_hook: _,
        } = self;
        crate::project::project_writers::ProjectResourceMutationFacts::new(
            operation_id,
            yss_project_identity::ProjectInstanceId::from_existing(project_instance_id.into()),
            publication_revision,
            moves.into_iter().collect::<Vec<_>>().into_boxed_slice(),
            deltas,
            crate::project::project_writers::ProjectProjectionStatus::Incomplete {
                invalidated_graph_paths: expected_graph_paths
                    .into_iter()
                    .filter_map(|path| yss_graph_document::GraphResourcePath::new(path).ok())
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            },
            crate::project::project_writers::ProjectHistoryStatus {
                can_undo: history.can_undo,
                can_redo: history.can_redo,
            },
        )
    }
}

#[cfg(test)]
impl CommittedResourceMutation {
    pub(in crate::project::project_state) fn complete(
        self,
        locale: &str,
    ) -> crate::schema::application_event::ResourceMutationResultDto {
        let CommittedResourceMutation {
            operation_id,
            project_instance_id,
            publication_revision,
            moves,
            deltas,
            history,
            expected_graph_paths,
            #[cfg(test)]
            completion_test_hook,
        } = self;
        #[cfg(test)]
        if let Some(hook) = completion_test_hook.as_ref() {
            hook();
        }

        let _ = locale;
        let moves = moves
            .into_iter()
            .map(|value| crate::schema::application_event::ResourceMoveDto {
                from: value.from.to_string(),
                to: value.to.to_string(),
                kind: value.kind,
                name: value.name.to_string(),
            })
            .collect();
        let history = yss_project_history::HistoryStatusDto {
            can_undo: history.can_undo,
            can_redo: history.can_redo,
        };
        crate::schema::application_event::ResourceMutationResultDto {
            operation_id,
            project_instance_id,
            publication_revision,
            moves,
            deltas,
            projection_replacements: Vec::new(),
            projection_status: crate::schema::application_event::ProjectionStatusDto::Incomplete {
                invalidated_graph_paths: expected_graph_paths,
            },
            history,
        }
    }
}

impl ProjectState {
    #[cfg(all(test, any()))]
    pub fn apply_resource_document_patch(
        &self,
        context: &ProjectTransactionContext,
        patch: ResourceDocumentPatch,
    ) -> Result<crate::schema::application_event::ResourceMutationResultDto, ProjectFilesystemError>
    {
        self.apply_resource_document_patch_internal(context, patch, None, None)
            .map(|receipt| receipt.complete("en-US"))
    }

    #[cfg(all(test, any()))]
    pub(in crate::project) fn apply_resource_document_patch_with_environment(
        &self,
        context: &ProjectTransactionContext,
        patch: ResourceDocumentPatch,
        projection_environment: ProjectionEnvironmentSnapshot,
        rename_ownership: Option<&mut ResourceRenameOwnershipLease>,
    ) -> Result<crate::schema::application_event::ResourceMutationResultDto, ProjectFilesystemError>
    {
        let publication = self.mutation_publication.lock().unwrap();
        if !projection_environment.matches_publication(&publication) {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "projection environment changed before patch publication".into(),
            });
        }
        drop(publication);
        self.apply_resource_document_patch_internal(context, patch, None, rename_ownership)
            .map(|receipt| receipt.complete("en-US"))
    }

    pub(in crate::project) fn apply_project_resource_document_patch(
        &self,
        context: &ProjectTransactionContext,
        patch: ResourceDocumentPatch,
        rename_ownership: Option<&mut ResourceRenameOwnershipLease>,
    ) -> Result<crate::project::project_writers::ProjectResourceMutationFacts, ProjectFilesystemError>
    {
        self.apply_resource_document_patch_internal(context, patch, None, rename_ownership)
            .map(CommittedResourceMutation::into_project_facts)
    }

    pub(in crate::project::project_state) fn apply_resource_document_patch_internal(
        &self,
        context: &ProjectTransactionContext,
        mut patch: ResourceDocumentPatch,
        history_head: Option<(bool, HistoryEntryId)>,
        mut rename_ownership: Option<&mut ResourceRenameOwnershipLease>,
    ) -> Result<CommittedResourceMutation, ProjectFilesystemError> {
        self.ensure_project_operational()?;
        self.validate_project_session(&context.session)?;
        preflight_resource_patch_graphs(&patch)?;
        let authority = self.capture_project_authority_for_session(&context.session)?;

        let receipt = {
            let mut publication = self.mutation_publication.lock().unwrap();
            if publication.project_instance_id != context.session.instance_id.as_str() {
                return Err(ProjectFilesystemError::StaleProjectLifecycle {
                    message: "project instance changed before patch publication".into(),
                });
            }
            if !authority.matches_publication(&publication) {
                return Err(ProjectFilesystemError::StaleProjectLifecycle {
                    message: "projection environment changed before patch publication".into(),
                });
            }
            let mut lifecycle = self.resource_lifecycle.boundary();
            let mut data = self.project_data.write().unwrap();
            let mut graph_revisions = self.graph_revisions.write().unwrap();
            let mut variable_revisions = self.variable_revisions.write().unwrap();
            let mut worksheet_revisions = self.worksheet_revisions.write().unwrap();
            let mut history = self.history.write().unwrap();
            self.ensure_project_operational()?;
            validate_context_revisions(
                context,
                &data,
                &graph_revisions,
                &variable_revisions,
                &worksheet_revisions,
            )?;
            normalize_function_patch_revisions(&mut patch, &data, &graph_revisions)?;
            let (worksheet_deltas, worksheet_history) = worksheet_history_publication(
                context.operation_id,
                &patch,
                &data,
                &worksheet_revisions,
            )?;
            let publication_advance = publication.prepare_resource_revision()?;
            let mut deltas =
                canonical_resource_lifecycle_events(context, &patch, &graph_revisions)?;
            deltas.extend(worksheet_deltas);
            let moves = match &patch {
                ResourceDocumentPatch::MoveGraph {
                    from, to, moved, ..
                } => vec![crate::project::project_writers::ProjectResourceMove {
                    from: from.as_str().into(),
                    to: to.as_str().into(),
                    kind: match moved.kind {
                        yss_graph_document::GraphResourceKind::Event => {
                            yss_project_history::ResourceLifecycleKind::Event
                        }
                        yss_graph_document::GraphResourceKind::Function => {
                            yss_project_history::ResourceLifecycleKind::Function
                        }
                    },
                    name: moved.name.clone().into_boxed_str(),
                }],
                ResourceDocumentPatch::MoveWorksheet { from, to, .. } => {
                    vec![crate::project::project_writers::ProjectResourceMove {
                        from: from.as_str().into(),
                        to: to.as_str().into(),
                        kind: yss_project_history::ResourceLifecycleKind::Worksheet,
                        name: to.display_name().as_str().into(),
                    }]
                }
                _ => Vec::new(),
            };
            let resource_history = match &patch {
                ResourceDocumentPatch::MoveGraph {
                    from,
                    to,
                    moved_before,
                    moved,
                    referenced_graphs_before,
                    referenced_graphs,
                    referenced_variables_before,
                    referenced_variables,
                    ..
                } => Some(yss_project_history::ProjectHistoryTransaction::graph_move(
                    context.operation_id,
                    from.clone(),
                    to.clone(),
                    serde_json::to_value(GraphMoveHistoryPayload {
                        moved_before: moved_before.clone(),
                        moved_after: moved.clone(),
                        referenced_graphs_before: referenced_graphs_before.clone(),
                        referenced_graphs_after: referenced_graphs.clone(),
                        referenced_variables_before: referenced_variables_before.clone(),
                        referenced_variables_after: referenced_variables.clone(),
                    })
                    .map_err(|error| {
                        ProjectFilesystemError::TransactionPrepareFailed {
                            message: error.to_string(),
                        }
                    })?,
                )),
                _ => worksheet_history,
            };
            let projection_paths = patch_projection_paths(&patch, &data);
            if let Some((undo, expected_history_id)) = &history_head {
                let current = if *undo {
                    history.next_undo()
                } else {
                    history.next_redo()
                };
                if current.map(|entry| &entry.history_id) != Some(expected_history_id) {
                    return Err(ProjectFilesystemError::TransactionCommitFailed {
                        message: "history head changed during filesystem transaction".into(),
                    });
                }
            }

            if let Some(ownership) = rename_ownership.as_deref_mut() {
                ownership.commit_with_boundary(&mut lifecycle)?;
            }

            match patch {
                ResourceDocumentPatch::InsertGraph { path, resource } => {
                    let revision = resource.document.revision;
                    Self::install_validated_resident_graph(&mut data, path.clone(), resource);
                    graph_revisions.insert(path, revision);
                }
                ResourceDocumentPatch::DeclareGraph { path, revision } => {
                    graph_revisions.insert(path, revision.to_graph_revision());
                }
                ResourceDocumentPatch::RemoveGraph { path, .. } => {
                    let existing = data.graphs.get(&path);
                    let retained_function_revision = if existing.is_some_and(|resource| {
                        resource.kind == yss_graph_document::GraphResourceKind::Function
                    }) {
                        let retained = graph_revisions
                            .get(&path)
                            .copied()
                            .or_else(|| existing.map(|resource| resource.document.revision));
                        let incoming = existing
                            .map(|resource| resource.document.revision)
                            .unwrap_or(yss_graph_document::GraphRevision::INITIAL);
                        Some(authoritative_function_revision(&path, incoming, retained)?)
                    } else {
                        None
                    };
                    let removed_ids = data
                        .variables
                        .iter()
                        .filter(|(_, variable)| {
                            variable_scope_references_path(&variable.scope, path.as_str())
                        })
                        .map(|(id, _)| *id)
                        .collect::<Vec<_>>();
                    let removed_revisions = removed_ids
                        .iter()
                        .map(|id| {
                            let retained = variable_revisions
                                .get(id)
                                .map(|entry| entry.revision)
                                .unwrap_or(yss_project_identity::ResourceRevision::INITIAL);
                            checked_resource_revision(format!("variables/{id}"), retained)
                                .map(|revision| (*id, revision))
                        })
                        .collect::<Result<Vec<_>, _>>()?;

                    data.graphs.remove(&path);
                    if let Some(revision) = retained_function_revision {
                        graph_revisions.insert(path.clone(), revision);
                    } else {
                        graph_revisions.remove(&path);
                    }
                    for (id, revision) in removed_revisions {
                        data.variables.remove(&id);
                        variable_revisions.insert(id, VariableRevisionEntry::deleted(revision));
                    }
                }
                ResourceDocumentPatch::UnloadGraph { path } => {
                    data.graphs.remove(&path);
                    data.variables.retain(|_, variable| {
                        !variable_scope_references_path(&variable.scope, path.as_str())
                    });
                }
                ResourceDocumentPatch::MoveGraph {
                    from,
                    to,
                    moved,
                    referenced_graphs,
                    loaded_referenced_graphs,
                    referenced_variables,
                    ..
                } => {
                    let existing = data.graphs.get(&from);
                    let retained_function_revision =
                        if moved.kind == yss_graph_document::GraphResourceKind::Function {
                            let retained = graph_revisions
                                .get(&from)
                                .copied()
                                .or_else(|| existing.map(|resource| resource.document.revision));
                            let incoming = existing
                                .map(|resource| resource.document.revision)
                                .unwrap_or(yss_graph_document::GraphRevision::INITIAL);
                            Some(authoritative_function_revision(&from, incoming, retained)?)
                        } else {
                            None
                        };
                    let referenced_variable_revisions = referenced_variables
                        .keys()
                        .map(|id| {
                            let retained = variable_revisions
                                .get(id)
                                .map(|entry| entry.revision)
                                .unwrap_or(yss_project_identity::ResourceRevision::INITIAL);
                            checked_resource_revision(format!("variables/{id}"), retained)
                                .map(|revision| (*id, revision))
                        })
                        .collect::<Result<std::collections::HashMap<_, _>, _>>()?;

                    let removed = data.graphs.remove(&from);
                    let was_loaded = removed.is_some();
                    if let Some(revision) = retained_function_revision {
                        graph_revisions.insert(from.clone(), revision);
                    } else {
                        graph_revisions.remove(&from);
                    }
                    graph_revisions.insert(to.clone(), moved.document.revision);
                    if was_loaded {
                        Self::install_validated_resident_graph(&mut data, to, moved);
                    }
                    for (path, resource) in referenced_graphs {
                        graph_revisions.insert(path.clone(), resource.document.revision);
                        if loaded_referenced_graphs.contains(&path) {
                            Self::install_validated_resident_graph(&mut data, path, resource);
                        }
                    }
                    for (id, variable) in referenced_variables {
                        data.variables.insert(id, variable);
                        variable_revisions.insert(
                            id,
                            VariableRevisionEntry::present(referenced_variable_revisions[&id]),
                        );
                    }
                }
                ResourceDocumentPatch::PatchVariables { updates, removals } => {
                    let mut next_revisions = variable_revisions.clone();
                    for id in &removals {
                        let retained = next_revisions
                            .get(id)
                            .map(|entry| entry.revision)
                            .unwrap_or(yss_project_identity::ResourceRevision::INITIAL);
                        let revision =
                            checked_resource_revision(format!("variables/{id}"), retained)?;
                        next_revisions.insert(*id, VariableRevisionEntry::deleted(revision));
                    }
                    for id in updates.keys() {
                        let retained = next_revisions
                            .get(id)
                            .map(|entry| entry.revision)
                            .unwrap_or(yss_project_identity::ResourceRevision::INITIAL);
                        let revision =
                            checked_resource_revision(format!("variables/{id}"), retained)?;
                        next_revisions.insert(*id, VariableRevisionEntry::present(revision));
                    }

                    for id in removals {
                        data.variables.remove(&id);
                    }
                    data.variables.extend(updates);
                    *variable_revisions = next_revisions;
                }
                ResourceDocumentPatch::UpsertWorksheet { path, mut document } => {
                    validate_worksheet_path_insertion(&data, &path)?;
                    let retained_revision = worksheet_revisions.get(&path).copied();
                    let revision = match retained_revision {
                        Some(retained) => checked_resource_revision(path.as_str(), retained)?,
                        None => ResourceRevision::INITIAL,
                    };
                    if document.revision != revision && Some(document.revision) != retained_revision
                    {
                        return Err(ProjectFilesystemError::ResourceRevisionConflict {
                            message: format!(
                                "worksheet '{}' submitted revision {} but authority requires {}",
                                path.as_str(),
                                document.revision.get(),
                                revision.get()
                            ),
                        });
                    }
                    document.revision = revision;
                    data.worksheets.insert(path.clone(), document);
                    worksheet_revisions.insert(path, revision);
                }
                ResourceDocumentPatch::RemoveWorksheet { path, revision } => {
                    let next_revision = checked_resource_revision(path.as_str(), revision)?;
                    data.worksheets.remove(&path);
                    worksheet_revisions.insert(path, next_revision);
                }
                ResourceDocumentPatch::MoveWorksheet {
                    from,
                    to,
                    mut moved,
                } => {
                    let revision = moved.revision;
                    data.worksheets.remove(&from);
                    moved.revision = revision;
                    data.worksheets.insert(to.clone(), moved);
                    worksheet_revisions.insert(from, revision);
                    worksheet_revisions.insert(to, revision);
                }
            }

            if let Some((undo, expected_history_id)) = history_head {
                history
                    .move_resource_head(undo, &expected_history_id)
                    .map_err(|error| ProjectFilesystemError::TransactionCommitFailed {
                        message: error.to_string(),
                    })?;
            } else if let Some(transaction) = resource_history {
                history.record_committed_transaction(transaction);
            } else if deltas.iter().any(|delta| {
                !matches!(
                    &delta.payload,
                    yss_project_history::ResourceDocumentPatch::ResourceLifecycle(_)
                )
            }) {
                let changes = deltas
                    .iter()
                    .filter(|delta| {
                        !matches!(
                            &delta.payload,
                            yss_project_history::ResourceDocumentPatch::ResourceLifecycle(_)
                        )
                    })
                    .map(|delta| yss_project_history::ResourcePatch {
                        resource: delta.resource.clone(),
                        before_revision: delta.from_revision,
                        after_revision: delta.to_revision,
                        forward: delta.payload.clone(),
                        inverse: delta.payload.inverse(),
                    })
                    .collect::<Vec<_>>();
                history.record_committed_transaction(
                    yss_project_history::ProjectHistoryTransaction::new(
                        context.operation_id,
                        changes,
                    ),
                );
            }
            let history = history.status();
            let publication_revision = publication.commit_prepared(publication_advance);
            #[cfg(test)]
            let completion_test_hook = self
                .test_hooks
                .committed_resource_completion_test_hook
                .read()
                .unwrap()
                .clone();
            CommittedResourceMutation {
                operation_id: context.operation_id,
                project_instance_id: publication.project_instance_id.clone(),
                publication_revision,
                moves,
                deltas,
                history: crate::project::project_writers::ProjectHistoryStatus {
                    can_undo: history.can_undo,
                    can_redo: history.can_redo,
                },
                expected_graph_paths: projection_paths,
                #[cfg(test)]
                completion_test_hook,
            }
        };

        Ok(receipt)
    }
}
