use super::*;
use yss_project_model::ProjectDataPatch;

pub(super) struct CommittedResourceMutation {
    pub(in crate::project_state) operation_id: yss_project_identity::OperationId,
    pub(in crate::project_state) project_instance_id: String,
    pub(in crate::project_state) publication_revision: u64,
    pub(in crate::project_state) moves: Vec<crate::project_writers::ProjectResourceMove>,
    pub(in crate::project_state) deltas: Vec<yss_project_history::ResourceDeltaEvent>,
    pub(in crate::project_state) history: crate::project_writers::ProjectHistoryStatus,
    pub(in crate::project_state) expected_graph_paths: Vec<String>,
}

impl CommittedResourceMutation {
    pub(crate) fn into_project_facts(self) -> crate::project_writers::ProjectResourceMutationFacts {
        let Self {
            operation_id,
            project_instance_id,
            publication_revision,
            moves,
            deltas,
            history,
            expected_graph_paths,
        } = self;
        crate::project_writers::ProjectResourceMutationFacts::new(
            operation_id,
            yss_project_identity::ProjectInstanceId::from_existing(project_instance_id),
            publication_revision,
            moves.into_iter().collect::<Vec<_>>().into_boxed_slice(),
            deltas,
            crate::project_writers::ProjectProjectionStatus::Incomplete {
                invalidated_graph_paths: expected_graph_paths
                    .into_iter()
                    .filter_map(|path| yss_graph_document::GraphResourcePath::new(path).ok())
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            },
            crate::project_writers::ProjectHistoryStatus {
                can_undo: history.can_undo,
                can_redo: history.can_redo,
            },
        )
    }
}

impl ProjectState {
    pub(crate) fn apply_project_resource_document_patch(
        &self,
        context: &ProjectTransactionContext,
        patch: ProjectDataPatch,
        rename_ownership: Option<&mut ResourceRenameOwnershipLease>,
    ) -> Result<crate::project_writers::ProjectResourceMutationFacts, ProjectFilesystemError> {
        self.apply_resource_document_patch_internal(context, patch, None, rename_ownership)
            .map(CommittedResourceMutation::into_project_facts)
    }

    pub(in crate::project_state) fn apply_resource_document_patch_internal(
        &self,
        context: &ProjectTransactionContext,
        mut patch: ProjectDataPatch,
        history_head: Option<(bool, HistoryEntryId)>,
        rename_ownership: Option<&mut ResourceRenameOwnershipLease>,
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
            let mut graph_resource_revisions = self.graph_resource_revisions.write().unwrap();
            let mut variable_revisions = self.variable_revisions.write().unwrap();
            let mut chart_revisions = self.chart_revisions.write().unwrap();
            let mut history = self.history.write().unwrap();
            self.ensure_project_operational()?;
            validate_context_revisions(
                context,
                &data,
                &graph_resource_revisions,
                &variable_revisions,
                &chart_revisions,
            )?;
            normalize_function_patch_revisions(&mut patch, &data, &graph_resource_revisions)?;
            let (chart_deltas, chart_history) =
                chart_history_publication(context.operation_id, &patch, &data, &chart_revisions)?;
            let publication_advance = publication.prepare_resource_revision()?;
            let mut deltas =
                canonical_resource_lifecycle_events(context, &patch, &graph_resource_revisions)?;
            deltas.extend(chart_deltas);
            let moves = match &patch {
                ProjectDataPatch::MoveGraph {
                    from, to, moved, ..
                } => vec![crate::project_writers::ProjectResourceMove {
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
                ProjectDataPatch::MoveChart { from, to, .. } => {
                    vec![crate::project_writers::ProjectResourceMove {
                        from: from.as_str().into(),
                        to: to.as_str().into(),
                        kind: yss_project_history::ResourceLifecycleKind::Chart,
                        name: to.display_name().as_str().into(),
                    }]
                }
                _ => Vec::new(),
            };
            let resource_history = match &patch {
                ProjectDataPatch::MoveGraph {
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
                        moved_before: moved_before.as_ref().clone(),
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
                _ => chart_history,
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

            if let Some(ownership) = rename_ownership {
                ownership.commit_with_boundary(&mut lifecycle)?;
            }

            match patch {
                ProjectDataPatch::InsertGraph { path, resource } => {
                    let revision = graph_resource_revisions
                        .get(&path)
                        .copied()
                        .unwrap_or(ResourceRevision::INITIAL);
                    Self::install_validated_resident_graph(&mut data, path.clone(), resource);
                    graph_resource_revisions.insert(path, revision);
                }
                ProjectDataPatch::DeclareGraph { path, revision } => {
                    graph_resource_revisions.insert(path, revision);
                }
                ProjectDataPatch::RemoveGraph { path, .. } => {
                    let existing = data.graphs.get(&path);
                    let retained_function_revision = if existing.is_some_and(|resource| {
                        resource.kind == yss_graph_document::GraphResourceKind::Function
                    }) {
                        let retained = graph_resource_revisions.get(&path).copied();
                        let incoming = existing
                            .and_then(|resource| resource.function.as_ref())
                            .map(|function| function.revision)
                            .unwrap_or(ResourceRevision::INITIAL);
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
                        graph_resource_revisions.insert(path.clone(), revision);
                    } else {
                        graph_resource_revisions.remove(&path);
                    }
                    for (id, revision) in removed_revisions {
                        data.variables.remove(&id);
                        variable_revisions.insert(id, VariableRevisionEntry::deleted(revision));
                    }
                }
                ProjectDataPatch::UnloadGraph { path } => {
                    data.graphs.remove(&path);
                    data.variables.retain(|_, variable| {
                        !variable_scope_references_path(&variable.scope, path.as_str())
                    });
                }
                ProjectDataPatch::MoveGraph {
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
                            let retained = graph_resource_revisions.get(&from).copied();
                            let incoming = existing
                                .and_then(|resource| resource.function.as_ref())
                                .map(|function| function.revision)
                                .unwrap_or(ResourceRevision::INITIAL);
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

                    let source_revision = graph_resource_revisions
                        .get(&from)
                        .copied()
                        .unwrap_or(ResourceRevision::INITIAL);
                    let removed = data.graphs.remove(&from);
                    let was_loaded = removed.is_some();
                    if let Some(revision) = retained_function_revision {
                        graph_resource_revisions.insert(from.clone(), revision);
                    } else {
                        graph_resource_revisions.remove(&from);
                    }
                    let moved_revision = checked_resource_revision(from.as_str(), source_revision)?;
                    graph_resource_revisions.insert(to.clone(), moved_revision);
                    if was_loaded {
                        Self::install_validated_resident_graph(&mut data, to, moved);
                    }
                    for (path, resource) in referenced_graphs {
                        let revision = checked_resource_revision(
                            path.as_str(),
                            graph_resource_revisions
                                .get(&path)
                                .copied()
                                .unwrap_or(ResourceRevision::INITIAL),
                        )?;
                        graph_resource_revisions.insert(path.clone(), revision);
                        if loaded_referenced_graphs.contains(&path) {
                            Self::install_validated_resident_graph(&mut data, path, resource);
                        }
                    }
                    for (id, variable) in referenced_variables {
                        let revision =
                            referenced_variable_revisions
                                .get(&id)
                                .copied()
                                .ok_or_else(|| {
                                    ProjectFilesystemError::ResourceRevisionConflict {
                                        message: format!(
                                            "moved Variable '{id}' is missing its prepared revision"
                                        ),
                                    }
                                })?;
                        data.variables.insert(id, variable);
                        variable_revisions.insert(id, VariableRevisionEntry::present(revision));
                    }
                }
                ProjectDataPatch::PatchVariables { updates, removals } => {
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
                ProjectDataPatch::UpsertChart { path, mut document } => {
                    validate_chart_path_insertion(&data, &path)?;
                    let retained_revision = chart_revisions.get(&path).copied();
                    let revision = match retained_revision {
                        Some(retained) => checked_resource_revision(path.as_str(), retained)?,
                        None => ResourceRevision::INITIAL,
                    };
                    if document.revision != revision && Some(document.revision) != retained_revision
                    {
                        return Err(ProjectFilesystemError::ResourceRevisionConflict {
                            message: format!(
                                "chart '{}' submitted revision {} but authority requires {}",
                                path.as_str(),
                                document.revision.get(),
                                revision.get()
                            ),
                        });
                    }
                    document.revision = revision;
                    data.charts.insert(path.clone(), document);
                    chart_revisions.insert(path, revision);
                }
                ProjectDataPatch::RemoveChart { path, revision } => {
                    let next_revision = checked_resource_revision(path.as_str(), revision)?;
                    data.charts.remove(&path);
                    chart_revisions.insert(path, next_revision);
                }
                ProjectDataPatch::MoveChart {
                    from,
                    to,
                    mut moved,
                } => {
                    let revision = moved.revision;
                    data.charts.remove(&from);
                    moved.revision = revision;
                    data.charts.insert(to.clone(), moved);
                    chart_revisions.insert(from, revision);
                    chart_revisions.insert(to, revision);
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
            CommittedResourceMutation {
                operation_id: context.operation_id,
                project_instance_id: publication.project_instance_id.clone(),
                publication_revision,
                moves,
                deltas,
                history: crate::project_writers::ProjectHistoryStatus {
                    can_undo: history.can_undo,
                    can_redo: history.can_redo,
                },
                expected_graph_paths: projection_paths,
            }
        };

        Ok(receipt)
    }
}
