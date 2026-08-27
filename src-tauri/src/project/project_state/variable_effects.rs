//! Durable persistence and authoritative publication of runtime variable effects.

use super::*;
use crate::project::variable_tabular::normalize_variable_tabular;

type PreparedVariableEffectAuthority<'a> = Box<
    dyn FnMut(
            Option<(
                &crate::node_system::runtime::CancellationToken,
                Option<crate::node_system::runtime::RunDeadline>,
            )>,
        ) -> Result<VariableEffectCommitResult, VariableEffectCommitError>
        + 'a,
>;

struct VariableAuthorityPriorState {
    data: ProjectData,
    revisions: std::collections::HashMap<crate::variable::VariableId, VariableRevisionEntry>,
    history: ProjectHistory,
    publication_revision: u64,
    authority_generation: u64,
}

struct VariableAuthorityInstallGuard<'a> {
    data: &'a mut ProjectData,
    revisions:
        &'a mut std::collections::HashMap<crate::variable::VariableId, VariableRevisionEntry>,
    history: &'a mut ProjectHistory,
    publication: &'a mut MutationPublication,
    prior: Option<VariableAuthorityPriorState>,
    armed: bool,
}

impl<'a> VariableAuthorityInstallGuard<'a> {
    fn new(
        data: &'a mut ProjectData,
        revisions: &'a mut std::collections::HashMap<
            crate::variable::VariableId,
            VariableRevisionEntry,
        >,
        history: &'a mut ProjectHistory,
        publication: &'a mut MutationPublication,
        prior: VariableAuthorityPriorState,
    ) -> Self {
        Self {
            data,
            revisions,
            history,
            publication,
            prior: Some(prior),
            armed: true,
        }
    }

    fn install(
        &mut self,
        next_data: ProjectData,
        next_revisions: std::collections::HashMap<
            crate::variable::VariableId,
            VariableRevisionEntry,
        >,
        next_history: ProjectHistory,
        publication_revision: u64,
        authority_generation: u64,
        #[cfg(test)] panic_hook: Option<&VariableAuthorityAssignmentPanicTestHook>,
    ) {
        *self.data = next_data;
        #[cfg(test)]
        if let Some(panic_hook) = panic_hook {
            panic_hook();
        }
        *self.revisions = next_revisions;
        #[cfg(test)]
        if let Some(panic_hook) = panic_hook {
            panic_hook();
        }
        *self.history = next_history;
        #[cfg(test)]
        if let Some(panic_hook) = panic_hook {
            panic_hook();
        }
        self.publication.resource_revision = publication_revision;
        self.publication.authority_generation = authority_generation;
        #[cfg(test)]
        if let Some(panic_hook) = panic_hook {
            panic_hook();
        }
    }

    fn commit(mut self) -> VariableAuthorityPriorState {
        self.armed = false;
        self.prior
            .take()
            .expect("variable authority prior state exists until commit")
    }
}

impl Drop for VariableAuthorityInstallGuard<'_> {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let prior = self
            .prior
            .take()
            .expect("armed variable authority guard owns prior state");
        self.publication.resource_revision = prior.publication_revision;
        self.publication.authority_generation = prior.authority_generation;
        *self.history = prior.history;
        *self.revisions = prior.revisions;
        *self.data = prior.data;
    }
}

impl ProjectState {
    #[cfg(test)]
    pub(in crate::project) fn commit_variable_effects(
        &self,
        expected_session_id: &crate::node_system::ProjectSessionId,
        effects: Vec<crate::node_system::runtime::VariableWriteEffect>,
    ) -> Result<VariableEffectCommitResult, VariableEffectCommitError> {
        let mut prepared =
            self.prepare_variable_effects_receipt(expected_session_id, effects, None)?;
        prepared(None)
    }

    #[cfg(test)]
    pub(in crate::project) fn commit_variable_effects_for_run(
        &self,
        expected_session_id: &crate::node_system::ProjectSessionId,
        effects: Vec<crate::node_system::runtime::VariableWriteEffect>,
        cancellation: &crate::node_system::runtime::CancellationToken,
        deadline: Option<crate::node_system::runtime::RunDeadline>,
    ) -> Result<VariableEffectCommitResult, crate::node_system::runtime::RunError> {
        let terminal = Some((cancellation, deadline));
        let mut prepared = self
            .prepare_variable_effects_receipt(expected_session_id, effects, terminal)
            .map_err(variable_effect_run_error)?;
        prepared(terminal).map_err(variable_effect_run_error)
    }

    pub(super) fn prepare_variable_effects_receipt<'a>(
        &'a self,
        expected_session_id: &crate::node_system::ProjectSessionId,
        effects: Vec<crate::node_system::runtime::VariableWriteEffect>,
        terminal: Option<(
            &crate::node_system::runtime::CancellationToken,
            Option<crate::node_system::runtime::RunDeadline>,
        )>,
    ) -> Result<PreparedVariableEffectAuthority<'a>, VariableEffectCommitError> {
        let current_session_id = self
            .project_store
            .read()
            .unwrap()
            .project_session_id
            .clone();
        if &current_session_id != expected_session_id {
            return Err(VariableEffectCommitError::SessionChanged {
                expected: expected_session_id.clone(),
                current: current_session_id,
            });
        }
        let expected_session_id = expected_session_id.clone();
        if effects.is_empty() {
            check_variable_effect_terminal(terminal)?;
            let expected_path = self.get_path();
            let (expected_project_instance_id, expected_revision, expected_generation) = {
                let publication = self.mutation_publication.lock().unwrap();
                (
                    publication.project_instance_id.clone(),
                    publication.resource_revision,
                    publication.authority_generation(),
                )
            };
            return Ok(Box::new(move |terminal| {
                let publication = self.mutation_publication.lock().unwrap();
                let path = self.project_path.read().unwrap();
                let _data = self.project_data.write().unwrap();
                let store = self.project_store.write().unwrap();
                let _graph_revisions = self.graph_revisions.read().unwrap();
                let _variable_revisions = self.variable_revisions.write().unwrap();
                let _worksheet_revisions = self.worksheet_revisions.write().unwrap();
                let _history = self.history.write().unwrap();
                if publication.project_instance_id != expected_project_instance_id
                    || publication.resource_revision != expected_revision
                    || publication.authority_generation() != expected_generation
                    || *path != expected_path
                    || store.project_session_id != expected_session_id
                {
                    return Err(variable_effect_persistence_error(
                        "project changed before empty variable authority commit",
                    ));
                }
                check_variable_effect_terminal(terminal)?;
                Ok(VariableEffectCommitResult {
                    variable_ids: Box::new([]),
                    resource_mutation: None,
                })
            }));
        }

        let session = self
            .capture_project_session()
            .map_err(variable_effect_persistence_error)?;
        let expected_project_path = self.get_path().ok_or_else(|| {
            variable_effect_persistence_error("no project is active for variable persistence")
        })?;
        let projection_environment = self
            .capture_projection_environment_for_execution_session(&session, &expected_session_id)
            .map_err(variable_effect_persistence_error)?;
        let (
            data_snapshot,
            graph_revisions,
            variable_revisions,
            history_snapshot,
            publication_revision_basis,
            authority_generation_basis,
            database_revisions,
        ) = {
            let publication = self.mutation_publication.lock().unwrap();
            let path = self.project_path.read().unwrap();
            if publication.project_instance_id != session.instance_id.as_str()
                || path.as_deref() != Some(expected_project_path.as_str())
            {
                return Err(variable_effect_persistence_error(
                    "project changed before variable persistence snapshot",
                ));
            }
            (
                self.project_data.read().unwrap().clone(),
                self.graph_revisions.read().unwrap().clone(),
                self.variable_revisions.read().unwrap().clone(),
                self.history.read().unwrap().clone(),
                publication.resource_revision,
                publication.authority_generation(),
                self.database_authority_revisions.read().unwrap().clone(),
            )
        };

        let mut expected_revisions = BTreeMap::new();
        let mut changes = Vec::with_capacity(effects.len());
        let mut history_before = BTreeMap::new();
        let mut history_after = BTreeMap::new();
        let mut ids = Vec::with_capacity(effects.len());
        let mut local_graph_paths = std::collections::HashSet::new();
        let mut writes_globals = false;
        for effect in &effects {
            let id = variable_effect_id(effect)?;
            let resource_key = ResourceKey::Variable(
                crate::node_system::document::VariableResourceKey(effect.resource.as_str().into()),
            );
            let current = data_snapshot.variables.get(&id).ok_or_else(|| {
                VariableEffectCommitError::Conflict {
                    resource: resource_key.clone(),
                    expected_revision: effect.expected_revision,
                    current_revision: None,
                }
            })?;
            let revision = variable_revisions
                .get(&id)
                .map(|entry| entry.revision)
                .unwrap_or(crate::project::ResourceRevision::INITIAL);
            if revision != effect.expected_revision
                || serde_json::to_value(current).map_err(variable_effect_invalid_error)?
                    != serde_json::to_value(&effect.before)
                        .map_err(variable_effect_invalid_error)?
            {
                return Err(VariableEffectCommitError::Conflict {
                    resource: resource_key,
                    expected_revision: effect.expected_revision,
                    current_revision: Some(revision),
                });
            }
            expected_revisions.insert(resource_key, revision);
            match &current.scope {
                crate::variable::VariableScope::Global => writes_globals = true,
                crate::variable::VariableScope::Event { event_path }
                | crate::variable::VariableScope::Function {
                    function_path: event_path,
                } => {
                    let graph_path = GraphResourcePath::new(event_path)
                        .map_err(variable_effect_invalid_error)?;
                    let graph_revision =
                        graph_revisions.get(&graph_path).copied().ok_or_else(|| {
                            variable_effect_persistence_error(format!(
                                "local variable graph '{}' is not loaded",
                                graph_path
                            ))
                        })?;
                    expected_revisions.insert(
                        ResourceKey::Graph(graph_path.clone()),
                        ResourceRevision::from_graph_revision(graph_revision),
                    );
                    local_graph_paths.insert(graph_path);
                }
            }
            let variable_key =
                crate::node_system::document::VariableResourceKey(effect.resource.as_str().into());
            let mut canonical_after = current.clone();
            canonical_after.data_value = effect.after.clone();
            normalize_variable_tabular(&mut canonical_after)
                .map_err(variable_effect_invalid_error)?;
            changes.push(crate::node_system::document::ResourcePatch::variable(
                variable_key.clone(),
                revision,
                crate::node_system::document::VariableDocumentPatch::new(
                    Some(serde_json::to_value(current).map_err(variable_effect_invalid_error)?),
                    Some(
                        serde_json::to_value(&canonical_after)
                            .map_err(variable_effect_invalid_error)?,
                    ),
                ),
            ));
            history_before.insert(
                variable_key.clone(),
                Some(serde_json::to_value(current).map_err(variable_effect_invalid_error)?),
            );
            history_after.insert(
                variable_key,
                Some(serde_json::to_value(canonical_after).map_err(variable_effect_invalid_error)?),
            );
            ids.push(id);
        }

        let transaction = ProjectHistoryTransaction::durable_variable_effects(
            crate::project::OperationId::new(),
            changes,
            crate::node_system::document::VariableEffectHistorySnapshots {
                before: history_before,
                after: history_after,
            },
        );
        let deltas = transaction
            .changes
            .iter()
            .map(|change| crate::node_system::document::ResourceDeltaEvent {
                resource: change.resource.clone(),
                from_revision: change.before_revision,
                to_revision: change.after_revision,
                caused_by: Some(transaction.caused_by),
                payload: change.forward.clone(),
            })
            .collect::<Vec<_>>();
        let mut proposed_data = data_snapshot.clone();
        let mut proposed_revisions = variable_revisions.clone();
        let mut proposed_documents = project_documents(&proposed_data, &proposed_revisions);
        let mut proposed_history = history_snapshot.clone();
        proposed_history
            .apply_transaction(&mut proposed_documents, transaction.clone())
            .map_err(|error| VariableEffectCommitError::History {
                message: error.to_string().into(),
            })?;
        replace_project_documents(
            &mut proposed_data,
            &mut proposed_revisions,
            proposed_documents,
        );
        install_variable_effect_snapshots(&mut proposed_data, &transaction, false)
            .map_err(variable_effect_invalid_error)?;
        {
            let store = self.project_store.read().unwrap();
            if store.project_session_id != expected_session_id {
                return Err(VariableEffectCommitError::SessionChanged {
                    expected: expected_session_id.clone(),
                    current: store.project_session_id.clone(),
                });
            }
        }
        for id in &ids {
            let variable = proposed_data
                .variables
                .get_mut(id)
                .expect("effect variable exists");
            normalize_variable_tabular(variable).map_err(variable_effect_invalid_error)?;
        }

        let mut mutations = Vec::new();
        if writes_globals {
            let variables = proposed_data
                .variables
                .iter()
                .filter(|(_, variable)| {
                    matches!(variable.scope, crate::variable::VariableScope::Global)
                })
                .map(|(id, variable)| (*id, variable.clone()))
                .collect();
            mutations.push(StagedFilesystemMutation::Write {
                relative_path: crate::project::GLOBAL_VARIABLES_FILE.into(),
                contents: serde_json::to_vec_pretty(
                    &crate::project::project_io::GlobalVariablesDocument {
                        schema_version: crate::project::project_io::SCHEMA_VERSION,
                        variables,
                    },
                )
                .map_err(variable_effect_invalid_error)?,
            });
        }
        for graph_path in &local_graph_paths {
            let graph = proposed_data.graphs.get(graph_path).ok_or_else(|| {
                variable_effect_persistence_error(format!(
                    "local variable graph '{}' is not loaded",
                    graph_path
                ))
            })?;
            let local_variables = proposed_data
                .variables
                .iter()
                .filter(|(_, variable)| variable_scope_matches_graph(&variable.scope, graph_path))
                .map(|(id, variable)| (*id, variable.clone()))
                .collect();
            mutations.push(StagedFilesystemMutation::Write {
                relative_path: graph_path.as_str().into(),
                contents: crate::project::project_io::serialize_graph_resource_document(
                    graph,
                    local_variables,
                )
                .map_err(variable_effect_persistence_error)?,
            });
        }

        let context = ProjectTransactionContext {
            session,
            operation_id: transaction.caused_by,
            affected_resources: expected_revisions.keys().cloned().collect(),
            expected_revisions,
            expected_absent_resources: Default::default(),
            recovery_marker: Some(self.project_recovery_marker()),
        };
        let filesystem_lease = self
            .filesystem()
            .acquire(context.session.root.clone())
            .map_err(variable_effect_persistence_error)?;
        self.validate_project_session(&context.session)
            .map_err(variable_effect_persistence_error)?;
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
        .map_err(variable_effect_persistence_error)?;
        check_variable_effect_terminal(terminal)?;
        let committed_filesystem = prepared
            .commit()
            .map_err(variable_effect_persistence_error)?;

        self.run_mutation_publication_test_hook();
        let publication_revision = publication_revision_basis
            .checked_add(1)
            .ok_or_else(|| variable_effect_persistence_error("resource revision overflowed"))?;
        let authority_generation = authority_generation_basis
            .checked_add(1)
            .ok_or_else(|| variable_effect_persistence_error("authority generation overflowed"))?;
        let expected_graph_paths = affected_projection_paths(&deltas, &proposed_data);
        let history_status = proposed_history.status();
        let projection_source = self.projection_source_snapshot(
            &proposed_data,
            projection_environment.clone(),
            context.session.instance_id.to_string(),
            authority_generation,
            graph_revisions.clone(),
            proposed_revisions.clone(),
            database_revisions,
        );
        #[cfg(test)]
        let completion_test_hook = self
            .test_hooks
            .committed_resource_completion_test_hook
            .read()
            .unwrap()
            .clone();
        #[cfg(test)]
        let assignment_panic_hook = self
            .test_hooks
            .variable_authority_assignment_panic_test_hook
            .read()
            .unwrap()
            .clone();
        let resource_mutation = Some(
            CommittedResourceMutation {
                operation_id: transaction.caused_by,
                project_instance_id: context.session.instance_id.to_string(),
                publication_revision,
                moves: Vec::new(),
                deltas,
                history: history_status,
                projection_source,
                expected_graph_paths,
                #[cfg(test)]
                completion_test_hook,
            }
            .complete("en-US"),
        );
        let mut variable_ids = Some(ids.into_boxed_slice());
        let mut resource_mutation = resource_mutation;
        let mut proposed_data = Some(proposed_data);
        let mut proposed_revisions = Some(proposed_revisions);
        let mut proposed_history = Some(proposed_history);
        let mut prior_state = Some(VariableAuthorityPriorState {
            data: data_snapshot,
            revisions: variable_revisions,
            history: history_snapshot,
            publication_revision: publication_revision_basis,
            authority_generation: authority_generation_basis,
        });
        let mut committed_filesystem = Some(committed_filesystem);

        Ok(Box::new(move |terminal| {
            let authority_result = (|| {
                let mut publication = self.mutation_publication.lock().unwrap();
                let path = self.project_path.read().unwrap();
                let mut data = self.project_data.write().unwrap();
                let store = self.project_store.read().unwrap();
                let graph_revisions = self.graph_revisions.read().unwrap();
                let mut revisions = self.variable_revisions.write().unwrap();
                let worksheet_revisions = self.worksheet_revisions.read().unwrap();
                let mut history = self.history.write().unwrap();
                if publication.project_instance_id != context.session.instance_id.as_str()
                    || path.as_deref() != Some(expected_project_path.as_str())
                    || publication.resource_revision != publication_revision_basis
                    || publication.authority_generation() != authority_generation_basis
                {
                    return Err(variable_effect_persistence_error(
                        "project changed before variable authority commit",
                    ));
                }
                if !projection_environment.matches_publication(&publication) {
                    return Err(variable_effect_persistence_error(
                        "projection environment changed before variable authority commit",
                    ));
                }
                if store.project_session_id != expected_session_id {
                    return Err(VariableEffectCommitError::SessionChanged {
                        expected: expected_session_id.clone(),
                        current: store.project_session_id.clone(),
                    });
                }
                drop(store);
                validate_context_revisions(
                    &context,
                    &data,
                    &graph_revisions,
                    &revisions,
                    &worksheet_revisions,
                )
                .map_err(variable_effect_persistence_error)?;
                check_variable_effect_terminal(terminal)?;

                // Result publication acquires its registry and artifact locks before entering
                // this project authority section. Project-side code must never acquire those
                // result locks while retaining any of the guards below.
                let mut install = VariableAuthorityInstallGuard::new(
                    &mut data,
                    &mut revisions,
                    &mut history,
                    &mut publication,
                    prior_state
                        .take()
                        .expect("prepared variable prior state installs once"),
                );
                let installed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    install.install(
                        proposed_data
                            .take()
                            .expect("prepared variable data installs once"),
                        proposed_revisions
                            .take()
                            .expect("prepared variable revisions installs once"),
                        proposed_history
                            .take()
                            .expect("prepared variable history installs once"),
                        publication_revision,
                        authority_generation,
                        #[cfg(test)]
                        assignment_panic_hook.as_ref(),
                    );
                }));
                if let Err(payload) = installed {
                    drop(install);
                    drop(history);
                    drop(worksheet_revisions);
                    drop(revisions);
                    drop(graph_revisions);
                    drop(data);
                    drop(path);
                    drop(publication);
                    std::panic::resume_unwind(payload);
                }
                Ok(install.commit())
            })();

            match authority_result {
                Ok(prior) => {
                    drop(prior);
                    committed_filesystem
                        .take()
                        .expect("prepared filesystem commit finalizes once")
                        .finalize();
                    Ok(VariableEffectCommitResult {
                        variable_ids: variable_ids
                            .take()
                            .expect("prepared variable ids publish once"),
                        resource_mutation: resource_mutation.take(),
                    })
                }
                Err(error) => Err(error),
            }
        }))
    }
}

pub(in crate::project) fn variable_effect_run_error(
    error: VariableEffectCommitError,
) -> crate::node_system::runtime::RunError {
    match error {
        VariableEffectCommitError::DeadlineExceeded { phase } => {
            crate::node_system::runtime::RunError::DeadlineExceeded { phase }
        }
        VariableEffectCommitError::Cancelled => crate::node_system::runtime::RunError::Cancelled,
        error => crate::node_system::runtime::RunError::ResourceSnapshotMismatch(
            error.to_string().into(),
        ),
    }
}

fn check_variable_effect_terminal(
    terminal: Option<(
        &crate::node_system::runtime::CancellationToken,
        Option<crate::node_system::runtime::RunDeadline>,
    )>,
) -> Result<(), VariableEffectCommitError> {
    let Some((cancellation, deadline)) = terminal else {
        return Ok(());
    };
    cancellation
        .check()
        .map_err(|_| VariableEffectCommitError::Cancelled)?;
    if let Some(deadline) = deadline {
        deadline
            .check(
                cancellation,
                crate::node_system::runtime::RunPhase::ResultPublication,
            )
            .map_err(|error| match error {
                crate::node_system::runtime::RunError::DeadlineExceeded { phase } => {
                    VariableEffectCommitError::DeadlineExceeded { phase }
                }
                crate::node_system::runtime::RunError::Cancelled => {
                    VariableEffectCommitError::Cancelled
                }
                _ => unreachable!("terminal check has only cancellation or deadline outcomes"),
            })?;
    }
    Ok(())
}

pub(in crate::project) fn install_variable_effect_snapshots(
    data: &mut ProjectData,
    transaction: &ProjectHistoryTransaction,
    undo: bool,
) -> Result<Vec<crate::variable::VariableId>, String> {
    let snapshots = transaction
        .variable_effect_snapshots
        .as_ref()
        .ok_or_else(|| "durable variable-effect history is missing snapshots".to_string())?;
    let selected = if undo {
        &snapshots.before
    } else {
        &snapshots.after
    };
    let mut ids = Vec::with_capacity(selected.len());
    for (key, snapshot) in selected {
        let id = key
            .0
            .strip_prefix("variables/")
            .ok_or_else(|| format!("invalid variable history resource '{}'", key.0))
            .and_then(|value| uuid::Uuid::parse_str(value).map_err(|error| error.to_string()))
            .map(crate::variable::VariableId::from)?;
        match snapshot {
            Some(snapshot) => {
                let variable: crate::variable::VariableInstance =
                    serde_json::from_value(snapshot.clone()).map_err(|error| error.to_string())?;
                if variable.id != id {
                    return Err(format!(
                        "variable history snapshot does not match resource '{}'",
                        key.0
                    ));
                }
                data.variables.insert(id, variable);
            }
            None => {
                data.variables.remove(&id);
            }
        }
        ids.push(id);
    }
    Ok(ids)
}

fn variable_effect_id(
    effect: &crate::node_system::runtime::VariableWriteEffect,
) -> Result<crate::variable::VariableId, VariableEffectCommitError> {
    effect
        .resource
        .as_str()
        .strip_prefix("variables/")
        .ok_or_else(|| VariableEffectCommitError::InvalidEffect {
            message: format!("invalid variable resource '{}'", effect.resource.as_str()).into(),
        })
        .and_then(|value| {
            uuid::Uuid::parse_str(value).map_err(|error| VariableEffectCommitError::InvalidEffect {
                message: error.to_string().into(),
            })
        })
        .map(crate::variable::VariableId::from)
}

fn variable_effect_invalid_error(error: impl ToString) -> VariableEffectCommitError {
    VariableEffectCommitError::InvalidEffect {
        message: error.to_string().into(),
    }
}

fn variable_effect_persistence_error(error: impl ToString) -> VariableEffectCommitError {
    VariableEffectCommitError::Persistence {
        message: error.to_string().into(),
    }
}

pub(in crate::project) fn variable_scope_graph_path(
    scope: &crate::variable::VariableScope,
) -> Result<Option<GraphResourcePath>, String> {
    match scope {
        crate::variable::VariableScope::Global => Ok(None),
        crate::variable::VariableScope::Event { event_path }
        | crate::variable::VariableScope::Function {
            function_path: event_path,
        } => GraphResourcePath::new(event_path)
            .map(Some)
            .map_err(|error| error.to_string()),
    }
}

pub(in crate::project) fn variable_history_scope(
    data: &ProjectData,
    transaction: &ProjectHistoryTransaction,
    id: crate::variable::VariableId,
    undo: bool,
) -> Result<crate::variable::VariableScope, String> {
    if let Some(variable) = data.variables.get(&id) {
        return Ok(variable.scope.clone());
    }
    let snapshots = transaction
        .variable_effect_snapshots
        .as_ref()
        .ok_or_else(|| "durable variable-effect history is missing snapshots".to_string())?;
    let opposite = if undo {
        &snapshots.after
    } else {
        &snapshots.before
    };
    let key = crate::node_system::document::VariableResourceKey(format!("variables/{id}").into());
    let snapshot = opposite
        .get(&key)
        .and_then(Option::as_ref)
        .ok_or_else(|| format!("variable history cannot recover scope for '{id}'"))?;
    let variable: crate::variable::VariableInstance =
        serde_json::from_value(snapshot.clone()).map_err(|error| error.to_string())?;
    if variable.id != id {
        return Err(format!(
            "variable history snapshot does not match resource 'variables/{id}'"
        ));
    }
    Ok(variable.scope)
}

pub(in crate::project) fn variable_effect_filesystem_mutations(
    data: &ProjectData,
    ids: &[crate::variable::VariableId],
    transaction: &ProjectHistoryTransaction,
    undo: bool,
) -> Result<Vec<StagedFilesystemMutation>, String> {
    let mut writes_globals = false;
    let mut local_graph_paths = std::collections::BTreeSet::new();
    for id in ids {
        let scope = variable_history_scope(data, transaction, *id, undo)?;
        match variable_scope_graph_path(&scope)? {
            Some(path) => {
                local_graph_paths.insert(path);
            }
            None => writes_globals = true,
        }
    }

    let mut mutations = Vec::new();
    if writes_globals {
        let variables = data
            .variables
            .iter()
            .filter(|(_, variable)| {
                matches!(variable.scope, crate::variable::VariableScope::Global)
            })
            .map(|(id, variable)| (*id, variable.clone()))
            .collect();
        mutations.push(StagedFilesystemMutation::Write {
            relative_path: crate::project::GLOBAL_VARIABLES_FILE.into(),
            contents: serde_json::to_vec_pretty(
                &crate::project::project_io::GlobalVariablesDocument {
                    schema_version: crate::project::project_io::SCHEMA_VERSION,
                    variables,
                },
            )
            .map_err(|error| error.to_string())?,
        });
    }
    for graph_path in local_graph_paths {
        let graph = data
            .graphs
            .get(&graph_path)
            .ok_or_else(|| format!("local variable graph '{graph_path}' is not loaded"))?;
        let local_variables = data
            .variables
            .iter()
            .filter(|(_, variable)| variable_scope_matches_graph(&variable.scope, &graph_path))
            .map(|(id, variable)| (*id, variable.clone()))
            .collect();
        mutations.push(StagedFilesystemMutation::Write {
            relative_path: graph_path.as_str().into(),
            contents: crate::project::project_io::serialize_graph_resource_document(
                graph,
                local_variables,
            )
            .map_err(|error| error.to_string())?,
        });
    }
    Ok(mutations)
}

pub(in crate::project) fn validate_variable_effect_document(
    path: &std::path::Path,
    contents: &[u8],
) -> Result<(), String> {
    if path == std::path::Path::new(crate::project::GLOBAL_VARIABLES_FILE) {
        serde_json::from_slice::<crate::project::project_io::GlobalVariablesDocument>(contents)
            .map(|_| ())
            .map_err(|error| error.to_string())
    } else {
        serde_json::from_slice::<crate::project::project_io::GraphDocument>(contents)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

fn variable_scope_matches_graph(
    scope: &crate::variable::VariableScope,
    graph_path: &GraphResourcePath,
) -> bool {
    match scope {
        crate::variable::VariableScope::Event { event_path } => event_path == graph_path.as_str(),
        crate::variable::VariableScope::Function { function_path } => {
            function_path == graph_path.as_str()
        }
        crate::variable::VariableScope::Global => false,
    }
}

#[derive(Debug)]
pub(in crate::project) struct VariableEffectCommitResult {
    pub variable_ids: Box<[crate::variable::VariableId]>,
    pub resource_mutation: Option<crate::event::ResourceMutationResultDto>,
}

#[derive(Debug, Clone, PartialEq)]
pub(in crate::project) enum VariableEffectCommitError {
    Cancelled,
    DeadlineExceeded {
        phase: crate::node_system::runtime::RunPhase,
    },
    SessionChanged {
        expected: crate::node_system::ProjectSessionId,
        current: crate::node_system::ProjectSessionId,
    },
    Conflict {
        resource: crate::node_system::document::ResourceKey,
        expected_revision: crate::project::ResourceRevision,
        current_revision: Option<crate::project::ResourceRevision>,
    },
    InvalidEffect {
        message: Box<str>,
    },
    History {
        message: Box<str>,
    },
    Persistence {
        message: Box<str>,
    },
}

impl std::fmt::Display for VariableEffectCommitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => formatter.write_str("variable effect commit was cancelled"),
            Self::DeadlineExceeded { phase } => {
                write!(formatter, "run deadline exceeded during {phase:?}")
            }
            Self::SessionChanged { expected, current } => write!(
                formatter,
                "project session changed from '{}' to '{}' before variable effects committed",
                expected.as_str(),
                current.as_str()
            ),
            Self::Conflict {
                resource,
                expected_revision,
                current_revision,
            } => write!(
                formatter,
                "variable effect conflict for {resource:?}: expected revision {}, current revision {:?}",
                expected_revision.get(),
                current_revision.map(|revision| revision.get())
            ),
            Self::InvalidEffect { message }
            | Self::History { message }
            | Self::Persistence { message } => formatter.write_str(message),
        }
    }
}
