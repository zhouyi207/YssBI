use super::*;

#[path = "history_moves.rs"]
mod history_moves;

pub(super) fn history_project_error(error: ProjectFilesystemError) -> ProjectHistoryMutationError {
    match error {
        ProjectFilesystemError::StaleProjectLifecycle { .. } => {
            ProjectHistoryMutationError::StaleProjectLifecycle(error.to_string().into())
        }
        ProjectFilesystemError::ProjectRecoveryRequired { .. }
        | ProjectFilesystemError::TransactionRollbackFailed {
            recovery_required: true,
            ..
        } => ProjectHistoryMutationError::RecoveryRequired(error.to_string().into()),
        _ => ProjectHistoryMutationError::History(error.to_string().into()),
    }
}

pub(super) fn resolve_history_rollback(
    original: ProjectHistoryMutationError,
    rollback: Result<(), ProjectFilesystemError>,
) -> ProjectHistoryMutationError {
    match rollback {
        Ok(()) => original,
        Err(error) => history_project_error(error),
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct GraphMoveHistoryPayload {
    pub(in crate::project::project_state) moved_before: GraphResourceDocument,
    pub(in crate::project::project_state) moved_after: GraphResourceDocument,
    pub(in crate::project::project_state) referenced_graphs_before:
        BTreeMap<GraphResourcePath, GraphResourceDocument>,
    pub(in crate::project::project_state) referenced_graphs_after:
        BTreeMap<GraphResourcePath, GraphResourceDocument>,
    pub(in crate::project::project_state) referenced_variables_before:
        BTreeMap<yss_variable_contract::VariableId, yss_variable_contract::VariableInstance>,
    pub(in crate::project::project_state) referenced_variables_after:
        BTreeMap<yss_variable_contract::VariableId, yss_variable_contract::VariableInstance>,
}

impl ProjectState {
    #[cfg(all(test, any()))]
    pub fn update_function_signature_observed(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        locale: &str,
        request: MutationRequest<crate::project::FunctionDocumentPatch>,
        observe: impl FnOnce(&crate::schema::application_event::ResourceMutationResultDto),
    ) -> Result<crate::schema::application_event::ResourceMutationResultDto, MutationConflict> {
        let session = self
            .capture_project_session()
            .map_err(|error| match error {
                ProjectFilesystemError::StaleProjectLifecycle { message } => {
                    ProjectHistoryMutationError::StaleProjectLifecycle(message.into())
                }
                error => ProjectHistoryMutationError::RecoveryRequired(error.to_string().into()),
            })?;
        if &session.instance_id != expected_project_instance_id {
            return Err(ProjectHistoryMutationError::StaleProjectLifecycle(
                "function signature command project instance is stale".into(),
            )
            .into());
        }
        let receipt =
            self.commit_function_signature(expected_project_instance_id, graph_path, request)?;
        let result = receipt.complete(locale);
        observe(&result);
        Ok(result)
    }

    pub(crate) fn update_function_signature_for_application(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        request: MutationRequest<crate::project::FunctionDocumentPatch>,
    ) -> Result<
        crate::project::project_writers::ProjectResourceMutationFacts,
        ProjectHistoryMutationError,
    > {
        let session = self
            .capture_project_session()
            .map_err(|error| match error {
                ProjectFilesystemError::StaleProjectLifecycle { message } => {
                    ProjectHistoryMutationError::StaleProjectLifecycle(message.into())
                }
                error => ProjectHistoryMutationError::RecoveryRequired(error.to_string().into()),
            })?;
        if &session.instance_id != expected_project_instance_id {
            return Err(ProjectHistoryMutationError::StaleProjectLifecycle(
                "function signature project instance is stale".into(),
            ));
        }
        self.commit_function_signature(expected_project_instance_id, graph_path, request)
            .map(CommittedResourceMutation::into_project_facts)
    }

    fn commit_function_signature(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        request: MutationRequest<crate::project::FunctionDocumentPatch>,
    ) -> Result<CommittedResourceMutation, ProjectHistoryMutationError> {
        self.ensure_mutation_operational()?;
        let function_key = crate::project::FunctionResourceKey(graph_path.as_str().into());
        let expected_resource = ResourceKey::Function(function_key.clone());
        if request.resource != expected_resource {
            return Err(ProjectHistoryMutationError::ResourceMismatch {
                requested: format!("{:?}", request.resource).into(),
                store: format!("{:?}", expected_resource).into(),
            });
        }
        let session = self
            .capture_project_session()
            .map_err(history_project_error)?;
        let expected_session = self.current_projection_environment_expectation();
        let authority = self
            .capture_project_authority_for_session(&session)
            .map_err(history_project_error)?;
        self.run_mutation_publication_test_hook();
        let mut publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != expected_project_instance_id.as_str() {
            return Err(ProjectHistoryMutationError::StaleProjectLifecycle(
                "caller project changed before signature authority commit".into(),
            ));
        }
        if publication.project_instance_id != expected_session.project_instance_id.as_str() {
            return Err(ProjectHistoryMutationError::StaleProjectLifecycle(
                "project changed before signature authority commit".into(),
            ));
        }
        if !authority.matches_publication(&publication) {
            return Err(ProjectHistoryMutationError::StaleProjectLifecycle(
                "projection environment changed before signature authority commit".into(),
            ));
        }
        let mut data = self.project_data.write().unwrap();
        self.ensure_mutation_operational()?;
        let function = data
            .graphs
            .get(graph_path)
            .and_then(|resource| resource.function.as_ref())
            .ok_or_else(|| ProjectHistoryMutationError::ResourceMismatch {
                requested: format!("{:?}", expected_resource).into(),
                store: format!("{:?}", expected_resource).into(),
            })?;
        if function.revision != request.base_revision {
            return Err(ProjectHistoryMutationError::StaleRevision {
                base_revision: request.base_revision.get(),
                current_revision: function.revision.get(),
            });
        }
        if function.signature != request.payload.before {
            return Err(ProjectHistoryMutationError::History(
                "function patch before-state does not match the current signature".into(),
            ));
        }
        let publication_advance = publication
            .prepare_resource_revision()
            .map_err(|error| ProjectHistoryMutationError::Projection(error.to_string().into()))?;
        let from_revision = function.revision;
        let mut graph_revisions = self.graph_revisions.write().unwrap();
        let mut revisions = self.variable_revisions.write().unwrap();
        let mut documents = project_documents(&data, &revisions);
        let transaction = crate::project::ProjectHistoryTransaction::new(
            request.operation_id,
            vec![crate::project::ResourcePatch::function(
                function_key,
                from_revision,
                request.payload.clone(),
            )],
        );
        let mut history = self.history.write().unwrap();
        history
            .apply_transaction(&mut documents, transaction)
            .map_err(|error| ProjectHistoryMutationError::History(error.to_string().into()))?;
        let to_revision = documents.functions[match &expected_resource {
            ResourceKey::Function(key) => key,
            _ => unreachable!(),
        }]
        .revision;
        replace_project_documents(&mut data, &mut revisions, documents);
        data.graphs
            .get_mut(graph_path)
            .expect("Function owner graph remains loaded")
            .document
            .revision = to_revision.to_graph_revision();
        graph_revisions.insert(graph_path.clone(), to_revision.to_graph_revision());
        let deltas = vec![crate::project::ResourceDeltaEvent {
            resource: expected_resource,
            from_revision,
            to_revision,
            caused_by: Some(request.operation_id),
            payload: crate::project::history::ResourceDocumentPatch::Function(request.payload),
        }];
        let expected_graph_paths = affected_projection_paths(&deltas, &data);
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
                can_undo: history.can_undo(),
                can_redo: history.can_redo(),
            },
            expected_graph_paths,
            #[cfg(test)]
            completion_test_hook,
        })
    }

    #[cfg(all(test, any()))]
    pub fn undo_last_transaction_observed(
        &self,
        project_instance_id: &ProjectInstanceId,
        locale: &str,
        request: MutationRequest<HistoryMutation>,
        observe: impl FnOnce(&crate::schema::application_event::ResourceMutationResultDto),
    ) -> Result<crate::schema::application_event::ResourceMutationResultDto, MutationConflict> {
        let receipt = self.commit_history_direction(project_instance_id, true, request)?;
        let result = receipt.complete(locale);
        observe(&result);
        Ok(result)
    }

    #[cfg(all(test, any()))]
    pub fn redo_last_transaction_observed(
        &self,
        project_instance_id: &ProjectInstanceId,
        locale: &str,
        request: MutationRequest<HistoryMutation>,
        observe: impl FnOnce(&crate::schema::application_event::ResourceMutationResultDto),
    ) -> Result<crate::schema::application_event::ResourceMutationResultDto, MutationConflict> {
        let receipt = self.commit_history_direction(project_instance_id, false, request)?;
        let result = receipt.complete(locale);
        observe(&result);
        Ok(result)
    }

    pub(crate) fn undo_history_for_application(
        &self,
        project_instance_id: &ProjectInstanceId,
        request: MutationRequest<HistoryMutation>,
    ) -> Result<
        crate::project::project_writers::ProjectResourceMutationFacts,
        ProjectHistoryMutationError,
    > {
        self.commit_history_direction(project_instance_id, true, request)
            .map(CommittedResourceMutation::into_project_facts)
    }

    pub(crate) fn redo_history_for_application(
        &self,
        project_instance_id: &ProjectInstanceId,
        request: MutationRequest<HistoryMutation>,
    ) -> Result<
        crate::project::project_writers::ProjectResourceMutationFacts,
        ProjectHistoryMutationError,
    > {
        self.commit_history_direction(project_instance_id, false, request)
            .map(CommittedResourceMutation::into_project_facts)
    }

    fn capture_history_projection_environment(
        &self,
        session: &ProjectSession,
    ) -> Result<ProjectAuthoritySnapshot, ProjectHistoryMutationError> {
        self.capture_project_authority_for_session(session)
            .map_err(history_project_error)
    }

    fn prepare_history_documents(
        &self,
        project_instance_id: &ProjectInstanceId,
        undo: bool,
        request: &MutationRequest<HistoryMutation>,
        expected_history_id: &HistoryEntryId,
        expected_persistence: crate::project::HistoryPersistencePolicy,
    ) -> Result<
        crate::project::history_hydration::PreparedHistoryDocuments,
        ProjectHistoryMutationError,
    > {
        self.ensure_mutation_operational()?;
        let snapshot = {
            let publication = self.mutation_publication.lock().unwrap();
            let staging_basis = self
                .capture_variable_staging_basis(&publication)
                .map_err(history_project_error)?;
            let session = staging_basis.session;
            if publication.project_instance_id != project_instance_id.as_str()
                || session.instance_id != *project_instance_id
            {
                return Err(ProjectHistoryMutationError::StaleProjectLifecycle(
                    "caller project changed before History preparation snapshot".into(),
                ));
            }
            let data = self.project_data.read().unwrap().clone();
            let graph_revisions = self.graph_revisions.read().unwrap().clone();
            let variable_revisions = self.variable_revisions.read().unwrap().clone();
            let worksheet_revisions = self.worksheet_revisions.read().unwrap().clone();
            let history = self.history.read().unwrap().clone();
            let transaction = if undo {
                history.next_undo()
            } else {
                history.next_redo()
            }
            .cloned()
            .ok_or_else(|| {
                ProjectHistoryMutationError::History(
                    if undo {
                        "there is no transaction to undo"
                    } else {
                        "there is no transaction to redo"
                    }
                    .into(),
                )
            })?;
            if transaction.history_id != *expected_history_id
                || transaction.persistence != expected_persistence
            {
                return Err(ProjectHistoryMutationError::History(
                    crate::project::HistoryError::HistoryHeadChanged
                        .to_string()
                        .into(),
                ));
            }
            crate::project::history_hydration::capture_history_preparation_snapshot(
                session.clone(),
                staging_basis.authority_generation,
                undo,
                transaction,
                &request.resource,
                data,
                graph_revisions,
                variable_revisions,
                worksheet_revisions,
                history,
            )
            .map_err(|error| ProjectHistoryMutationError::History(error.into()))?
        };

        crate::project::history_hydration::hydrate_history_preparation(
            snapshot,
            self.filesystem(),
            request,
        )
    }

    #[cfg(test)]
    pub(in crate::project) fn prepare_history_for_test(
        &self,
        undo: bool,
        request: MutationRequest<HistoryMutation>,
    ) -> Result<
        crate::project::history_hydration::PreparedHistoryDocuments,
        ProjectHistoryMutationError,
    > {
        let transaction = {
            let history = self.history.read().unwrap();
            if undo {
                history.next_undo()
            } else {
                history.next_redo()
            }
            .cloned()
            .ok_or_else(|| ProjectHistoryMutationError::History("History is empty".into()))?
        };
        let project_instance_id = ProjectInstanceId::from_existing(
            self.mutation_publication
                .lock()
                .unwrap()
                .project_instance_id
                .clone(),
        );
        self.prepare_history_documents(
            &project_instance_id,
            undo,
            &request,
            &transaction.history_id,
            transaction.persistence,
        )
    }

    fn history_transaction_contains_unloaded_graph(
        &self,
        transaction: &ProjectHistoryTransaction,
        undo: bool,
    ) -> Result<bool, ProjectHistoryMutationError> {
        let data = self.project_data.read().unwrap();
        let graph_revisions = self.graph_revisions.read().unwrap();
        let known_graphs = graph_revisions.keys().cloned().collect();
        let touched = crate::project::history_hydration::discover_touched_resources(
            transaction,
            undo,
            &data,
            &known_graphs,
        )
        .map_err(|error| ProjectHistoryMutationError::History(error.into()))?;
        Ok(touched.graphs.values().any(|residency| {
            *residency == crate::project::history_hydration::HistoryGraphResidency::Unloaded
        }))
    }

    fn commit_history_direction(
        &self,
        project_instance_id: &ProjectInstanceId,
        undo: bool,
        request: MutationRequest<HistoryMutation>,
    ) -> Result<CommittedResourceMutation, ProjectHistoryMutationError> {
        self.ensure_mutation_operational()?;
        let expected_session = self
            .capture_project_session()
            .map_err(history_project_error)?;
        if expected_session.instance_id != *project_instance_id {
            return Err(ProjectHistoryMutationError::StaleProjectLifecycle(
                "caller project changed before History routing".into(),
            ));
        }
        let next_transaction = {
            let history = self.history.read().unwrap();
            if undo {
                history.next_undo()
            } else {
                history.next_redo()
            }
            .cloned()
        };
        let transaction = next_transaction.ok_or_else(|| {
            ProjectHistoryMutationError::History(
                if undo {
                    "there is no transaction to undo"
                } else {
                    "there is no transaction to redo"
                }
                .into(),
            )
        })?;
        match transaction.persistence {
            crate::project::HistoryPersistencePolicy::DurableResourceMove => {
                return match transaction
                    .resource_move
                    .as_ref()
                    .map(|patch| &patch.payload)
                {
                    Some(crate::project::ResourceMoveHistoryPayload::Graph { .. }) => self
                        .commit_graph_move_history_direction(
                            project_instance_id,
                            undo,
                            request,
                            transaction,
                        ),
                    Some(crate::project::ResourceMoveHistoryPayload::Worksheet { .. }) => self
                        .commit_worksheet_move_history_direction(
                            project_instance_id,
                            undo,
                            request,
                            transaction,
                        ),
                    None => Err(ProjectHistoryMutationError::History(
                        "resource move history patch is missing".into(),
                    )),
                };
            }
            crate::project::HistoryPersistencePolicy::DurableVariableEffects => {
                return self.commit_variable_effect_history_direction(
                    project_instance_id,
                    undo,
                    request,
                    transaction,
                );
            }
            crate::project::HistoryPersistencePolicy::InMemoryUntilSave => {
                self.run_history_after_routing_test_hook();
                let touches_worksheet =
                    transaction
                        .resource_lifecycle
                        .as_ref()
                        .is_some_and(|patch| {
                            matches!(
                                patch.payload,
                                crate::project::ResourceLifecycleHistoryPayload::Worksheet { .. }
                            )
                        })
                        || transaction
                            .changes
                            .iter()
                            .any(|change| matches!(change.resource, ResourceKey::Worksheet(_)));
                if touches_worksheet
                    || self.history_transaction_contains_unloaded_graph(&transaction, undo)?
                {
                    let prepared = self.prepare_history_documents(
                        project_instance_id,
                        undo,
                        &request,
                        &transaction.history_id,
                        transaction.persistence,
                    )?;
                    debug_assert!(touches_worksheet || prepared.contains_unloaded_graph);
                    return self.commit_durable_history_documents(prepared, request);
                }
            }
        }
        let routed_history_id = transaction.history_id.clone();
        let routed_persistence = transaction.persistence;
        let projection_environment =
            self.capture_history_projection_environment(&expected_session)?;
        let mut publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != project_instance_id.as_str()
            || publication.project_instance_id != expected_session.instance_id.as_str()
        {
            return Err(ProjectHistoryMutationError::StaleProjectLifecycle(
                "caller project changed before History authority commit".into(),
            ));
        }
        if !projection_environment.matches_publication(&publication) {
            return Err(ProjectHistoryMutationError::StaleProjectLifecycle(
                "projection environment changed before History authority commit".into(),
            ));
        }
        let mut data = self.project_data.write().unwrap();
        let mut graph_revisions = self.graph_revisions.write().unwrap();
        let mut revisions = self.variable_revisions.write().unwrap();
        self.ensure_mutation_operational()?;
        let mut documents = project_documents(&data, &revisions);
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
        let mut history = self.history.write().unwrap();
        let live_head = if undo {
            history.next_undo()
        } else {
            history.next_redo()
        };
        if routed_persistence != crate::project::HistoryPersistencePolicy::InMemoryUntilSave
            || live_head.map(|entry| (&entry.history_id, entry.persistence))
                != Some((&routed_history_id, routed_persistence))
        {
            return Err(ProjectHistoryMutationError::History(
                crate::project::HistoryError::HistoryHeadChanged
                    .to_string()
                    .into(),
            ));
        }
        let publication_advance = publication
            .prepare_resource_revision()
            .map_err(history_project_error)?;
        let transaction = if undo {
            history.undo(&mut documents)
        } else {
            history.redo(&mut documents)
        }
        .map_err(|error| ProjectHistoryMutationError::History(error.to_string().into()))?;
        self.run_mutation_publication_test_hook();
        let deltas = transaction
            .changes
            .iter()
            .map(|change| crate::project::ResourceDeltaEvent {
                resource: change.resource.clone(),
                from_revision: project_document_revision(&before, &change.resource),
                to_revision: project_document_revision(&documents, &change.resource),
                caused_by: Some(request.operation_id),
                payload: if undo {
                    change.inverse.clone()
                } else {
                    change.forward.clone()
                },
            })
            .collect::<Vec<_>>();
        replace_project_documents(&mut data, &mut revisions, documents);
        crate::project::history_hydration::synchronize_function_owner_revisions(
            &mut data,
            &transaction,
        );
        for (path, graph) in &data.graphs {
            graph_revisions.insert(path.clone(), graph.document.revision);
        }
        let expected_graph_paths = affected_projection_paths(&deltas, &data);
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
                can_undo: history.can_undo(),
                can_redo: history.can_redo(),
            },
            expected_graph_paths,
            #[cfg(test)]
            completion_test_hook,
        })
    }

    fn commit_durable_history_documents(
        &self,
        prepared: crate::project::history_hydration::PreparedHistoryDocuments,
        request: MutationRequest<HistoryMutation>,
    ) -> Result<CommittedResourceMutation, ProjectHistoryMutationError> {
        if prepared.transaction.persistence
            != crate::project::HistoryPersistencePolicy::InMemoryUntilSave
            || prepared.basis.persistence
                != crate::project::HistoryPersistencePolicy::InMemoryUntilSave
        {
            return Err(ProjectHistoryMutationError::History(
                "durable graph hydration requires InMemoryUntilSave History policy".into(),
            ));
        }
        let mutations = crate::project::history_hydration::durable_filesystem_mutations(&prepared)?;
        let graph_revision_updates = prepared
            .touched_graphs
            .iter()
            .map(|path| {
                prepared
                    .after_data
                    .graphs
                    .get(path)
                    .map(|graph| (path.clone(), graph.document.revision))
                    .ok_or_else(|| {
                        ProjectHistoryMutationError::History(
                            format!("prepared graph '{path}' is missing from durable state").into(),
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut deltas = prepared
            .transaction
            .changes
            .iter()
            .map(|change| crate::project::ResourceDeltaEvent {
                resource: change.resource.clone(),
                from_revision: project_document_revision(&prepared.before, &change.resource),
                to_revision: project_document_revision(&prepared.after, &change.resource),
                caused_by: Some(request.operation_id),
                payload: if prepared.basis.undo {
                    change.inverse.clone()
                } else {
                    change.forward.clone()
                },
            })
            .collect::<Vec<_>>();
        if let Some(lifecycle) = &prepared.transaction.resource_lifecycle {
            if let crate::project::ResourceLifecycleHistoryPayload::Worksheet { .. } =
                lifecycle.payload
            {
                let mut forward = if prepared.basis.undo {
                    lifecycle.forward.inverse()
                } else {
                    lifecycle.forward.clone()
                };
                let state = forward
                    .before
                    .as_ref()
                    .or(forward.after.as_ref())
                    .expect("validated lifecycle History has one state");
                let worksheet_key = crate::project::WorksheetResourceKey(state.path.clone());
                let resource = ResourceKey::Worksheet(worksheet_key.clone());
                let from_revision = prepared.before.worksheet_revisions[&worksheet_key];
                let to_revision = prepared.after.worksheet_revisions[&worksheet_key];
                if let Some(before) = forward.before.as_mut() {
                    before.revision = from_revision;
                }
                if let Some(after) = forward.after.as_mut() {
                    after.revision = to_revision;
                }
                deltas.push(crate::project::ResourceDeltaEvent {
                    from_revision,
                    to_revision,
                    resource,
                    caused_by: Some(request.operation_id),
                    payload: crate::project::history::ResourceDocumentPatch::ResourceLifecycle(
                        forward,
                    ),
                });
            }
        }
        let mut expected_graph_paths =
            affected_projection_paths(&deltas, &prepared.loaded_after_data);
        expected_graph_paths.retain(|path| {
            GraphResourcePath::new(path)
                .ok()
                .is_some_and(|path| prepared.loaded_after_data.graphs.contains_key(&path))
        });
        let projection_environment =
            self.capture_history_projection_environment(&prepared.basis.session)?;
        let projected_generation = prepared
            .basis
            .authority_generation
            .checked_add(1)
            .ok_or(ProjectFilesystemError::AuthorityGenerationExhausted)
            .map_err(history_project_error)?;
        let history_status = prepared.proposed_history.status();
        #[cfg(test)]
        let completion_test_hook = self
            .test_hooks
            .committed_resource_completion_test_hook
            .read()
            .unwrap()
            .clone();
        self.run_history_after_preparation_test_hook();
        self.validate_project_session(&prepared.basis.session)
            .map_err(history_project_error)?;
        let context = ProjectTransactionContext {
            session: prepared.basis.session.clone(),
            operation_id: request.operation_id,
            affected_resources: prepared.basis.expected_revisions.keys().cloned().collect(),
            expected_revisions: prepared.basis.expected_revisions.clone(),
            expected_absent_resources: Default::default(),
            recovery_marker: Some(self.project_recovery_marker()),
        };
        let filesystem = ProjectFilesystemTransaction::prepare_with_validator(
            context,
            prepared.lease,
            mutations,
            crate::project::history_hydration::validate_durable_history_document,
        )
        .map_err(history_project_error)?;
        let committed_filesystem = filesystem.commit().map_err(history_project_error)?;

        self.run_history_after_disk_commit_test_hook();
        let authority_result = (|| {
            let mut publication = self.mutation_publication.lock().unwrap();
            let identity = self.activation_identity.read().unwrap();
            if publication.project_instance_id != prepared.basis.session.instance_id.as_str()
                || publication.authority_generation() != prepared.basis.authority_generation
                || identity.project_instance_id != prepared.basis.session.instance_id
                || identity.project_root.as_ref() != Some(&prepared.basis.session.root)
                || !projection_environment.matches_publication(&publication)
            {
                return Err(ProjectHistoryMutationError::StaleProjectLifecycle(
                    "project session or authority changed before durable History commit".into(),
                ));
            }
            drop(identity);
            self.ensure_mutation_operational()?;
            let mut data = self.project_data.write().unwrap();
            let mut graph_revisions = self.graph_revisions.write().unwrap();
            let mut variable_revisions = self.variable_revisions.write().unwrap();
            let mut worksheet_revisions = self.worksheet_revisions.write().unwrap();
            let mut history = self.history.write().unwrap();
            let current_head = if prepared.basis.undo {
                history.next_undo()
            } else {
                history.next_redo()
            };
            if current_head.map(|entry| (&entry.history_id, entry.persistence))
                != Some((&prepared.basis.history_id, prepared.basis.persistence))
            {
                return Err(ProjectHistoryMutationError::History(
                    crate::project::HistoryError::HistoryHeadChanged
                        .to_string()
                        .into(),
                ));
            }
            for (path, residency) in &prepared.basis.residency {
                let is_loaded = data.graphs.contains_key(path);
                let expected_loaded =
                    *residency == crate::project::history_hydration::HistoryGraphResidency::Loaded;
                if is_loaded != expected_loaded {
                    return Err(ProjectHistoryMutationError::History(
                        format!("graph '{path}' residency changed before durable History commit")
                            .into(),
                    ));
                }
            }
            for (path, expected) in &prepared.basis.expected_graph_revisions {
                if graph_revisions.get(path).copied() != Some(*expected) {
                    return Err(ProjectHistoryMutationError::History(
                        format!("owning Graph '{path}' changed before durable History commit")
                            .into(),
                    ));
                }
            }
            for (resource, expected) in &prepared.basis.expected_revisions {
                let actual = match resource {
                    ResourceKey::Graph(path) => GraphResourcePath::new(path.as_str())
                        .ok()
                        .and_then(|path| graph_revisions.get(&path).copied())
                        .map(ResourceRevision::from_graph_revision),
                    ResourceKey::Function(key) => GraphResourcePath::new(key.0.as_ref())
                        .ok()
                        .and_then(|path| {
                            data.graphs
                                .get(&path)
                                .and_then(|graph| graph.function.as_ref())
                                .map(|function| function.revision)
                                .or_else(|| {
                                    prepared
                                        .before
                                        .functions
                                        .get(key)
                                        .map(|function| function.revision)
                                })
                        }),
                    ResourceKey::Variable(path) => path
                        .0
                        .strip_prefix("variables/")
                        .and_then(|id| uuid::Uuid::parse_str(id).ok())
                        .map(yss_variable_contract::VariableId::from)
                        .and_then(|id| variable_revisions.get(&id))
                        .and_then(|entry| {
                            let expected_present = prepared
                                .before
                                .variables
                                .get(path)
                                .is_some_and(|document| document.value.is_some());
                            (entry.is_present() == expected_present).then_some(entry.revision)
                        }),
                    ResourceKey::Worksheet(key) => {
                        let path = WorksheetResourcePath::parse(key.0.as_ref()).ok();
                        let revision = path
                            .as_ref()
                            .and_then(|path| worksheet_revisions.get(path).copied());
                        let expected_present = prepared
                            .transaction
                            .resource_lifecycle
                            .as_ref()
                            .filter(|lifecycle| {
                                lifecycle
                                    .forward
                                    .before
                                    .as_ref()
                                    .or(lifecycle.forward.after.as_ref())
                                    .is_some_and(|state| state.path.as_ref() == key.0.as_ref())
                            })
                            .map(|lifecycle| {
                                if prepared.basis.undo {
                                    lifecycle.forward.after.is_some()
                                } else {
                                    lifecycle.forward.before.is_some()
                                }
                            })
                            .unwrap_or_else(|| prepared.before.worksheets.contains_key(key));
                        let actual_present = path
                            .as_ref()
                            .is_some_and(|path| data.worksheets.contains_key(path));
                        (expected_present == actual_present)
                            .then_some(revision)
                            .flatten()
                    }
                    ResourceKey::Database(_) => None,
                };
                if actual != Some(*expected) {
                    return Err(ProjectHistoryMutationError::History(
                        format!("resource {resource:?} changed before durable History commit")
                            .into(),
                    ));
                }
            }

            let publication_advance = publication
                .prepare_resource_revision()
                .map_err(history_project_error)?;
            *data = prepared.loaded_after_data;
            for (path, revision) in graph_revision_updates {
                graph_revisions.insert(path, revision);
            }
            *variable_revisions = prepared.after_variable_revisions;
            *worksheet_revisions = prepared.after_worksheet_revisions;
            *history = prepared.proposed_history;
            let publication_revision = publication.commit_prepared(publication_advance);
            debug_assert_eq!(publication.authority_generation(), projected_generation);
            Ok((
                publication.project_instance_id.clone(),
                publication_revision,
            ))
        })();

        match authority_result {
            Ok((project_instance_id, publication_revision)) => {
                committed_filesystem.finalize();
                Ok(CommittedResourceMutation {
                    operation_id: request.operation_id,
                    project_instance_id,
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
            }
            Err(error) => Err(resolve_history_rollback(
                error,
                committed_filesystem.rollback(),
            )),
        }
    }
}

pub(in crate::project) fn project_documents(
    data: &ProjectData,
    variable_revisions: &std::collections::HashMap<
        yss_variable_contract::VariableId,
        VariableRevisionEntry,
    >,
) -> ProjectDocumentState {
    let mut documents = ProjectDocumentState::new(
        data.graphs
            .iter()
            .map(|(path, graph)| (path.clone(), graph.document.clone()))
            .collect(),
        data.graphs
            .iter()
            .filter_map(|(path, graph)| {
                graph.function.clone().map(|function| {
                    (
                        crate::project::FunctionResourceKey(path.as_str().into()),
                        function,
                    )
                })
            })
            .collect(),
        variable_revisions
            .iter()
            .filter_map(|(id, entry)| {
                let value = if entry.is_present() {
                    Some(
                        serde_json::to_value(data.variables.get(id)?)
                            .expect("variable documents are serializable"),
                    )
                } else {
                    None
                };
                Some((
                    crate::project::VariableResourceKey(format!("variables/{id}").into()),
                    crate::project::VariableDocument {
                        revision: entry.revision,
                        value,
                    },
                ))
            })
            .collect(),
    );
    documents.worksheets = data
        .worksheets
        .iter()
        .map(|(path, document)| {
            (
                crate::project::WorksheetResourceKey(path.as_str().into()),
                document.clone(),
            )
        })
        .collect();
    documents.worksheet_revisions = documents
        .worksheets
        .iter()
        .map(|(key, document)| (key.clone(), document.revision))
        .collect();
    documents
}

pub(super) fn try_project_document_revision(
    documents: &ProjectDocumentState,
    resource: &ResourceKey,
) -> Option<crate::project::ResourceRevision> {
    match resource {
        ResourceKey::Graph(path) => documents
            .graphs
            .get(path)
            .map(|document| ResourceRevision::from_graph_revision(document.revision)),
        ResourceKey::Function(key) => documents
            .functions
            .get(key)
            .map(|document| document.revision),
        ResourceKey::Variable(key) => documents
            .variables
            .get(key)
            .map(|document| document.revision),
        ResourceKey::Worksheet(key) => documents
            .worksheets
            .get(key)
            .map(|document| document.revision),
        ResourceKey::Database(_) => None,
    }
}

pub(super) fn project_document_revision(
    documents: &ProjectDocumentState,
    resource: &ResourceKey,
) -> crate::project::ResourceRevision {
    try_project_document_revision(documents, resource)
        .expect("history transaction resource remains present")
}

pub(in crate::project) fn replace_project_documents(
    data: &mut ProjectData,
    variable_revisions: &mut std::collections::HashMap<
        yss_variable_contract::VariableId,
        VariableRevisionEntry,
    >,
    mut documents: ProjectDocumentState,
) {
    for (path, graph) in &mut data.graphs {
        let key = path.clone();
        if let Some(document) = documents.graphs.remove(&key) {
            graph.document = document;
        }
        let function_key = crate::project::FunctionResourceKey(path.as_str().into());
        if let Some(function) = documents.functions.remove(&function_key) {
            graph.function = Some(function);
        }
    }
    data.worksheets = documents
        .worksheets
        .into_iter()
        .map(|(key, document)| {
            let path = WorksheetResourcePath::parse(key.0.as_ref())
                .expect("history retains valid Worksheet resource paths");
            (path, document)
        })
        .collect();
    for (key, document) in documents.variables {
        let Some(id) = key.0.strip_prefix("variables/") else {
            continue;
        };
        let Ok(uuid) = uuid::Uuid::parse_str(id) else {
            continue;
        };
        let variable_id = yss_variable_contract::VariableId::from(uuid);
        let presence = match document.value {
            Some(value) => {
                let variable = serde_json::from_value(value)
                    .expect("history retains valid variable documents");
                data.variables.insert(variable_id, variable);
                VariablePresence::Present
            }
            None => {
                data.variables.remove(&variable_id);
                VariablePresence::Deleted
            }
        };
        variable_revisions.insert(
            variable_id,
            VariableRevisionEntry {
                revision: document.revision,
                presence,
            },
        );
    }
}
