use super::*;
use yss_display_naming::allocate_unique_display_name;

impl ProjectState {
    pub fn persist_global_variables(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        expected_revisions: BTreeMap<ResourceKey, ResourceRevision>,
        operation_id: OperationId,
    ) -> Result<ProjectSaveResult, ProjectFilesystemError> {
        let snapshot = self.capture_writer_snapshot(expected_project_instance_id)?;
        let authoritative = snapshot
            .data
            .variables
            .iter()
            .filter(|(_, variable)| matches!(variable.scope, VariableScope::Global))
            .map(|(id, _)| {
                (
                    variable_key(id),
                    snapshot
                        .variable_revisions
                        .get(id)
                        .map(|entry| entry.revision)
                        .unwrap_or(ResourceRevision::INITIAL),
                )
            })
            .collect::<BTreeMap<_, _>>();
        if authoritative != expected_revisions {
            return Err(ProjectFilesystemError::ResourceRevisionConflict {
                message: "global variable revisions changed".into(),
            });
        }
        let context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            expected_revisions,
            BTreeSet::new(),
        );
        self.execute_save(
            &snapshot,
            context,
            vec![StagedFilesystemMutation::Write {
                relative_path: yss_project_layout::GLOBAL_VARIABLES_FILE.into(),
                contents: crate::serialize_global_variables(&snapshot.data)
                    .map_err(prepare_error)?,
            }],
        )
    }

    fn stage_global_variable_mutation(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        expected_collection_revision: Option<u64>,
        operation_id: OperationId,
        mutation: GlobalVariableMutation,
    ) -> Result<VariableMutationResult, ProjectFilesystemError> {
        let session = self.capture_project_session()?;
        if &session.instance_id != expected_project_instance_id {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "variable command project instance is stale".into(),
            });
        }
        let (authority_generation, mut globals, revisions, names) = {
            let publication = self.mutation_publication.lock().unwrap();
            if publication.project_instance_id != session.instance_id.as_str() {
                return Err(ProjectFilesystemError::StaleProjectLifecycle {
                    message: "project changed during variable staging".into(),
                });
            }
            if let Some(expected) = expected_collection_revision
                && publication.resource_revision != expected
            {
                return Err(ProjectFilesystemError::ResourceRevisionConflict {
                    message: format!(
                        "global variable collection expected revision {expected}, found {}",
                        publication.resource_revision
                    ),
                });
            }
            let data = self.project_data.read().unwrap();
            (
                publication.authority_generation(),
                data.variables
                    .iter()
                    .filter(|(_, variable)| matches!(variable.scope, VariableScope::Global))
                    .map(|(id, variable)| (*id, variable.clone()))
                    .collect::<std::collections::HashMap<_, _>>(),
                self.variable_revisions.read().unwrap().clone(),
                data.variables
                    .values()
                    .map(|variable| variable.name.clone())
                    .collect::<Vec<_>>(),
            )
        };
        let staged = match mutation {
            GlobalVariableMutation::Create {
                scope,
                name,
                data_type,
                data_value,
                description,
                tags,
            } => {
                let variable = VariableInstance {
                    id: VariableId::new(),
                    name: allocate_unique_display_name(&name, names.iter().map(String::as_str)),
                    data_type,
                    data_value,
                    tabular: None,
                    description,
                    scope,
                    tags,
                };
                let variable = Self::stage_variable(variable)?;
                let history_patch = yss_project_history::ResourcePatch::variable(
                    VariableResourceKey(format!("variables/{}", variable.id).into()),
                    ResourceRevision::INITIAL,
                    yss_project_history::VariableDocumentPatch::new(
                        None,
                        Some(serde_json::to_value(&variable).map_err(prepare_error)?),
                    ),
                );
                StagedGlobalVariableMutation::Create {
                    variable,
                    history_patch,
                }
            }
            GlobalVariableMutation::Update {
                id,
                expected_revision,
                name,
                data_type,
                data_value,
                description,
                tags,
            } => {
                let before = globals
                    .get(&id)
                    .cloned()
                    .ok_or_else(|| prepare_error(format!("variable '{id}' not found")))?;
                let mut variable = before.clone();
                if let Some(name) = name {
                    variable.name = name;
                }
                if let Some(data_type) = data_type {
                    let changed = variable.data_type != data_type;
                    variable.data_type = data_type;
                    if changed && data_value.is_none() {
                        variable.data_value = default_value_for(&variable.data_type);
                    }
                }
                if let Some(data_value) = data_value {
                    variable.data_value = data_value;
                }
                if let Some(description) = description {
                    variable.description = description;
                }
                if let Some(tags) = tags {
                    variable.tags = tags;
                }
                let actual_revision = revisions
                    .get(&id)
                    .map(|entry| entry.revision)
                    .unwrap_or(ResourceRevision::INITIAL);
                if actual_revision != expected_revision {
                    return Err(ProjectFilesystemError::ResourceRevisionConflict {
                        message: format!(
                            "variable 'variables/{id}' expected revision {}, found {}",
                            expected_revision.get(),
                            actual_revision.get()
                        ),
                    });
                }
                let history_patch = yss_project_history::ResourcePatch::variable(
                    VariableResourceKey(format!("variables/{id}").into()),
                    expected_revision,
                    yss_project_history::VariableDocumentPatch::new(
                        Some(serde_json::to_value(&before).map_err(prepare_error)?),
                        Some(serde_json::to_value(&variable).map_err(prepare_error)?),
                    ),
                );
                let variable = Self::stage_variable(variable)?;
                StagedGlobalVariableMutation::Update {
                    variable,
                    expected_revision,
                    history_patch,
                }
            }
            GlobalVariableMutation::Delete {
                id,
                expected_revision,
            } => {
                let variable = globals
                    .get(&id)
                    .cloned()
                    .ok_or_else(|| prepare_error(format!("variable '{id}' not found")))?;
                let actual_revision = revisions
                    .get(&id)
                    .map(|entry| entry.revision)
                    .unwrap_or(ResourceRevision::INITIAL);
                if actual_revision != expected_revision {
                    return Err(ProjectFilesystemError::ResourceRevisionConflict {
                        message: format!(
                            "variable 'variables/{id}' expected revision {}, found {}",
                            expected_revision.get(),
                            actual_revision.get()
                        ),
                    });
                }
                let history_patch = yss_project_history::ResourcePatch::variable(
                    VariableResourceKey(format!("variables/{id}").into()),
                    expected_revision,
                    yss_project_history::VariableDocumentPatch::new(
                        Some(serde_json::to_value(&variable).map_err(prepare_error)?),
                        None,
                    ),
                );
                StagedGlobalVariableMutation::Delete {
                    variable,
                    expected_revision,
                    history_patch,
                }
            }
        };
        if staged.is_delete() {
            globals.remove(&staged.variable().id);
        } else {
            globals.insert(staged.variable().id, staged.variable().clone());
        }
        let key = variable_key(&staged.variable().id);
        let expected_revisions = staged
            .expected_revision()
            .map(|revision| BTreeMap::from([(key.clone(), revision)]))
            .unwrap_or_default();
        let expected_absent_resources = if staged.is_create() {
            BTreeSet::from([key])
        } else {
            BTreeSet::new()
        };
        let context = context(
            self,
            session.clone(),
            operation_id,
            expected_revisions,
            expected_absent_resources,
        );
        let contents = crate::serialize_global_variable_map(globals).map_err(prepare_error)?;
        let lease = self.filesystem().acquire(session.root.clone())?;
        self.validate_writer_context(&context, authority_generation)?;
        if let Some(expected) = expected_collection_revision {
            let publication = self.mutation_publication.lock().unwrap();
            if publication.project_instance_id != session.instance_id.as_str()
                || publication.resource_revision != expected
            {
                return Err(ProjectFilesystemError::ResourceRevisionConflict {
                    message: format!(
                        "global variable collection expected revision {expected}, found {}",
                        publication.resource_revision
                    ),
                });
            }
        }
        let prepared = ProjectFilesystemTransaction::prepare_with_validator(
            context.filesystem_context(),
            lease,
            vec![StagedFilesystemMutation::Write {
                relative_path: yss_project_layout::GLOBAL_VARIABLES_FILE.into(),
                contents,
            }],
            validate_document,
        )?;
        self.validate_writer_context(&context, authority_generation)?;
        let committed = prepared.commit()?;
        let save =
            match self.publish_global_variable_mutation(&context, authority_generation, &staged) {
                Ok(save) => save,
                Err(error) => {
                    return match committed.rollback() {
                        Ok(()) => Err(error),
                        Err(rollback_error) => Err(rollback_error),
                    };
                }
            };
        committed.finalize();
        Ok(VariableMutationResult {
            variable: staged.into_variable(),
            mutation: save,
        })
    }

    fn publish_global_variable_mutation(
        &self,
        context: &ProjectTransactionContext,
        authority_generation: u64,
        staged: &StagedGlobalVariableMutation,
    ) -> Result<ProjectResourceMutationFacts, ProjectFilesystemError> {
        let mut publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != context.session.instance_id.as_str()
            || publication.authority_generation() != authority_generation
        {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project authority changed before variable publication".into(),
            });
        }
        let mut data = self.project_data.write().unwrap();
        let graph_resource_revisions = self.graph_resource_revisions.read().unwrap();
        let mut variable_revisions = self.variable_revisions.write().unwrap();
        let chart_revisions = self.chart_revisions.read().unwrap();
        let mut history = self.history.write().unwrap();
        crate::project_state::validate_context_revisions(
            context,
            &data,
            &graph_resource_revisions,
            &variable_revisions,
            &chart_revisions,
        )?;
        let publication_advance = publication.prepare_resource_revision()?;
        let patch = staged.history_patch().clone();
        let (variable_key, before, after) = match (&patch.resource, &patch.forward) {
            (
                ResourceKey::Variable(variable_key),
                yss_project_history::ResourceDocumentPatch::Variable(document_patch),
            ) => (
                variable_key.clone(),
                document_patch.before.clone(),
                document_patch.after.clone(),
            ),
            _ => {
                return Err(ProjectFilesystemError::TransactionPrepareFailed {
                    message: "global Variable mutation produced an invalid History patch".into(),
                });
            }
        };
        match staged {
            StagedGlobalVariableMutation::Create { variable, .. } => {
                let id = variable.id;
                data.variables.insert(id, variable.clone());
                variable_revisions.insert(
                    id,
                    crate::project_state::VariableRevisionEntry::present(ResourceRevision::new(1)),
                );
            }
            StagedGlobalVariableMutation::Update {
                variable,
                expected_revision,
                ..
            } => {
                let id = variable.id;
                let revision = crate::project_state::checked_resource_revision(
                    format!("variables/{id}"),
                    *expected_revision,
                )?;
                data.variables.insert(id, variable.clone());
                variable_revisions.insert(
                    id,
                    crate::project_state::VariableRevisionEntry::present(revision),
                );
            }
            StagedGlobalVariableMutation::Delete {
                variable,
                expected_revision,
                ..
            } => {
                let id = variable.id;
                let revision = crate::project_state::checked_resource_revision(
                    format!("variables/{id}"),
                    *expected_revision,
                )?;
                data.variables.remove(&id);
                variable_revisions.insert(
                    id,
                    crate::project_state::VariableRevisionEntry::deleted(revision),
                );
            }
        }
        history.record_committed_transaction(
            yss_project_history::ProjectHistoryTransaction::durable_variable_effects(
                context.operation_id,
                vec![patch],
                yss_project_history::VariableEffectHistorySnapshots {
                    before: BTreeMap::from([(variable_key.clone(), before)]),
                    after: BTreeMap::from([(variable_key, after)]),
                },
            ),
        );
        let history = history.status();
        let publication_revision = publication.commit_prepared(publication_advance);
        let history_patch = staged.history_patch();
        Ok(ProjectResourceMutationFacts {
            operation_id: context.operation_id,
            project_instance_id: ProjectInstanceId::from_existing(
                publication.project_instance_id.clone(),
            ),
            publication_revision,
            moves: Box::new([]),
            deltas: vec![yss_project_history::ResourceDeltaEvent {
                resource: history_patch.resource.clone(),
                from_revision: history_patch.before_revision,
                to_revision: history_patch.after_revision,
                caused_by: Some(context.operation_id),
                payload: history_patch.forward.clone(),
            }]
            .into(),
            projection_status: ProjectProjectionStatus::Complete {
                expected_graph_paths: Vec::new().into(),
            },
            history: ProjectHistoryStatus {
                can_undo: history.can_undo,
                can_redo: history.can_redo,
            },
        })
    }

    fn commit_local_variable_mutation(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        expected_collection_revision: Option<u64>,
        operation_id: OperationId,
        mutation: GlobalVariableMutation,
    ) -> Result<VariableMutationResult, ProjectFilesystemError> {
        let session = self.capture_project_session()?;
        if &session.instance_id != expected_project_instance_id {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "variable command project instance is stale".into(),
            });
        }
        let reservation = self.reserve_resource_operation(&session.instance_id, operation_id)?;
        let (authority_generation, revisions, names, current) = {
            let publication = self.mutation_publication.lock().unwrap();
            if publication.project_instance_id != session.instance_id.as_str() {
                return Err(ProjectFilesystemError::StaleProjectLifecycle {
                    message: "project changed during local variable staging".into(),
                });
            }
            if let Some(expected) = expected_collection_revision
                && publication.resource_revision != expected
            {
                return Err(ProjectFilesystemError::ResourceRevisionConflict {
                    message: format!(
                        "local variable collection expected revision {expected}, found {}",
                        publication.resource_revision
                    ),
                });
            }
            let data = self.project_data.read().unwrap();
            let current = match &mutation {
                GlobalVariableMutation::Create { .. } => None,
                GlobalVariableMutation::Update { id, .. }
                | GlobalVariableMutation::Delete { id, .. } => data.variables.get(id).cloned(),
            };
            (
                publication.authority_generation(),
                self.variable_revisions.read().unwrap().clone(),
                data.variables
                    .values()
                    .map(|variable| variable.name.clone())
                    .collect::<Vec<_>>(),
                current,
            )
        };

        let staged = match mutation {
            GlobalVariableMutation::Create {
                scope,
                name,
                data_type,
                data_value,
                description,
                tags,
            } => {
                let variable = VariableInstance {
                    id: VariableId::new(),
                    name: allocate_unique_display_name(&name, names.iter().map(String::as_str)),
                    data_type,
                    data_value,
                    tabular: None,
                    description,
                    scope,
                    tags,
                };
                let variable = Self::stage_variable(variable)?;
                let patch = yss_project_history::ResourcePatch::variable(
                    VariableResourceKey(format!("variables/{}", variable.id).into()),
                    ResourceRevision::INITIAL,
                    yss_project_history::VariableDocumentPatch::new(
                        None,
                        Some(serde_json::to_value(&variable).map_err(prepare_error)?),
                    ),
                );
                StagedGlobalVariableMutation::Create {
                    variable,
                    history_patch: patch,
                }
            }
            GlobalVariableMutation::Update {
                id,
                expected_revision,
                name,
                data_type,
                data_value,
                description,
                tags,
            } => {
                let before =
                    current.ok_or_else(|| prepare_error(format!("variable '{id}' not found")))?;
                if matches!(before.scope, VariableScope::Global) {
                    return Err(prepare_error(format!("variable '{id}' is not local")));
                }
                let actual_revision = revisions
                    .get(&id)
                    .map(|entry| entry.revision)
                    .unwrap_or(ResourceRevision::INITIAL);
                if actual_revision != expected_revision {
                    return Err(ProjectFilesystemError::ResourceRevisionConflict {
                        message: format!(
                            "variable 'variables/{id}' expected revision {}, found {}",
                            expected_revision.get(),
                            actual_revision.get()
                        ),
                    });
                }
                let mut variable = before.clone();
                if let Some(name) = name {
                    variable.name = name;
                }
                if let Some(data_type) = data_type {
                    let changed = variable.data_type != data_type;
                    variable.data_type = data_type;
                    if changed && data_value.is_none() {
                        variable.data_value = default_value_for(&variable.data_type);
                    }
                }
                if let Some(data_value) = data_value {
                    variable.data_value = data_value;
                }
                if let Some(description) = description {
                    variable.description = description;
                }
                if let Some(tags) = tags {
                    variable.tags = tags;
                }
                let patch = yss_project_history::ResourcePatch::variable(
                    VariableResourceKey(format!("variables/{id}").into()),
                    expected_revision,
                    yss_project_history::VariableDocumentPatch::new(
                        Some(serde_json::to_value(&before).map_err(prepare_error)?),
                        Some(serde_json::to_value(&variable).map_err(prepare_error)?),
                    ),
                );
                let variable = Self::stage_variable(variable)?;
                StagedGlobalVariableMutation::Update {
                    variable,
                    expected_revision,
                    history_patch: patch,
                }
            }
            GlobalVariableMutation::Delete {
                id,
                expected_revision,
            } => {
                let variable =
                    current.ok_or_else(|| prepare_error(format!("variable '{id}' not found")))?;
                if matches!(variable.scope, VariableScope::Global) {
                    return Err(prepare_error(format!("variable '{id}' is not local")));
                }
                let actual_revision = revisions
                    .get(&id)
                    .map(|entry| entry.revision)
                    .unwrap_or(ResourceRevision::INITIAL);
                if actual_revision != expected_revision {
                    return Err(ProjectFilesystemError::ResourceRevisionConflict {
                        message: format!(
                            "variable 'variables/{id}' expected revision {}, found {}",
                            expected_revision.get(),
                            actual_revision.get()
                        ),
                    });
                }
                let patch = yss_project_history::ResourcePatch::variable(
                    VariableResourceKey(format!("variables/{id}").into()),
                    expected_revision,
                    yss_project_history::VariableDocumentPatch::new(
                        Some(serde_json::to_value(&variable).map_err(prepare_error)?),
                        None,
                    ),
                );
                StagedGlobalVariableMutation::Delete {
                    variable,
                    expected_revision,
                    history_patch: patch,
                }
            }
        };
        let key = variable_key(&staged.variable().id);
        let context = context(
            self,
            session,
            operation_id,
            staged
                .expected_revision()
                .map(|revision| BTreeMap::from([(key.clone(), revision)]))
                .unwrap_or_default(),
            if staged.is_create() {
                BTreeSet::from([key])
            } else {
                BTreeSet::new()
            },
        );
        let result =
            self.publish_global_variable_mutation(&context, authority_generation, &staged)?;
        reservation.complete();
        Ok(VariableMutationResult {
            variable: staged.into_variable(),
            mutation: result,
        })
    }

    fn variable_is_global(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        id: &VariableId,
    ) -> Result<bool, ProjectFilesystemError> {
        let session = self.capture_project_session()?;
        if &session.instance_id != expected_project_instance_id {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "variable command project instance is stale".into(),
            });
        }
        self.project_data
            .read()
            .unwrap()
            .variables
            .get(id)
            .map(|variable| matches!(variable.scope, VariableScope::Global))
            .ok_or_else(|| prepare_error(format!("variable '{id}' not found")))
    }

    pub fn create_variable_transaction(
        &self,
        request: CreateVariableRequest,
    ) -> Result<VariableMutationResult, ProjectFilesystemError> {
        let CreateVariableRequest {
            project_instance_id,
            name,
            data_type,
            data_value,
            description,
            scope,
            tags,
            expected_collection_revision,
            operation_id,
        } = request;
        let is_global = matches!(scope, VariableScope::Global);
        let mutation = GlobalVariableMutation::Create {
            scope,
            name,
            data_type,
            data_value,
            description,
            tags,
        };
        if is_global {
            self.stage_global_variable_mutation(
                &project_instance_id,
                Some(expected_collection_revision),
                operation_id,
                mutation,
            )
        } else {
            self.commit_local_variable_mutation(
                &project_instance_id,
                Some(expected_collection_revision),
                operation_id,
                mutation,
            )
        }
    }

    pub fn update_variable_transaction(
        &self,
        request: UpdateVariableRequest,
    ) -> Result<VariableMutationResult, ProjectFilesystemError> {
        let UpdateVariableRequest {
            project_instance_id,
            id,
            name,
            data_type,
            data_value,
            description,
            tags,
            expected_revision,
            operation_id,
        } = request;
        let is_global = self.variable_is_global(&project_instance_id, &id)?;
        let mutation = GlobalVariableMutation::Update {
            id,
            expected_revision,
            name,
            data_type,
            data_value,
            description,
            tags,
        };
        if is_global {
            self.stage_global_variable_mutation(&project_instance_id, None, operation_id, mutation)
        } else {
            self.commit_local_variable_mutation(&project_instance_id, None, operation_id, mutation)
        }
    }

    pub fn delete_variable_transaction(
        &self,
        request: DeleteVariableRequest,
    ) -> Result<VariableMutationResult, ProjectFilesystemError> {
        let DeleteVariableRequest {
            project_instance_id,
            id,
            expected_revision,
            operation_id,
        } = request;
        let is_global = self.variable_is_global(&project_instance_id, &id)?;
        let mutation = GlobalVariableMutation::Delete {
            id,
            expected_revision,
        };
        if is_global {
            self.stage_global_variable_mutation(&project_instance_id, None, operation_id, mutation)
        } else {
            self.commit_local_variable_mutation(&project_instance_id, None, operation_id, mutation)
        }
    }
}
