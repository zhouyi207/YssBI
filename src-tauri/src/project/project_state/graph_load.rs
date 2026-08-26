use super::*;

pub(in crate::project::project_state) struct CommittedGraphLoad {
    #[cfg(test)]
    resource: GraphResourceDocument,
    projection_source: Option<ProjectionSourceSnapshot>,
}

impl ProjectState {
    fn register_graph_load_intent(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        token: u64,
    ) -> Result<
        (
            ResourceLifecycleOperation,
            crate::project::ResourceLifecycleGuard,
            bool,
        ),
        ProjectFilesystemError,
    > {
        let session = self.capture_project_session()?;
        if &session.instance_id != expected_project_instance_id {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: format!(
                    "graph '{}' belongs to a different project instance",
                    graph_path
                ),
            });
        }
        let guard = self.resource_lifecycle.register(
            &session,
            graph_path,
            token,
            ResourceLifecycleIntent::Load,
        )?;
        let operation = ResourceLifecycleOperation::from_guard(session, &guard);
        let cached = self
            .project_data
            .read()
            .unwrap()
            .graphs
            .contains_key(graph_path);
        Ok((operation, guard, cached))
    }

    fn complete_graph_load(
        &self,
        operation: &ResourceLifecycleOperation,
        guard: &mut crate::project::ResourceLifecycleGuard,
        mut resource: GraphResourceDocument,
        local_variables: Option<
            std::collections::HashMap<
                crate::variable::VariableId,
                crate::variable::VariableInstance,
            >,
        >,
        include_projection: bool,
    ) -> Result<CommittedGraphLoad, ProjectFilesystemError> {
        self.ensure_project_operational()?;
        validate_graph_resource(operation.owner.graph_path(), &resource)?;
        let projection_environment = if include_projection {
            let expected = self
                .projection_environment_expectation_for_identity(
                    operation.session.instance_id.as_str(),
                    &operation.session.root,
                )
                .map_err(|message| ProjectFilesystemError::TransactionPrepareFailed { message })?;
            Some(
                self.capture_projection_environment(&expected)
                    .map_err(|message| ProjectFilesystemError::TransactionPrepareFailed {
                        message,
                    })?,
            )
        } else {
            None
        };
        let mut publication = self.mutation_publication.lock().unwrap();
        let path = self.project_path.read().unwrap();
        if publication.project_instance_id != operation.session.instance_id.as_str()
            || path.is_none()
        {
            return Err(operation.stale_error());
        }
        if projection_environment
            .as_ref()
            .is_some_and(|environment| !environment.matches_publication(&publication))
        {
            return Err(ProjectFilesystemError::TransactionPrepareFailed {
                message:
                    "stale_project_lifecycle: projection environment changed before graph load commit"
                        .into(),
            });
        }
        let mut lifecycle = self.resource_lifecycle.boundary();
        lifecycle.validate(&operation.owner)?;
        self.ensure_project_operational()?;
        let mut data = self.project_data.write().unwrap();
        let mut graph_revisions = self.graph_revisions.write().unwrap();
        let mut variable_revisions = self.variable_revisions.write().unwrap();
        let revision = normalize_loaded_function_resource_revision(
            operation.owner.graph_path(),
            &mut resource,
            graph_revisions.get(operation.owner.graph_path()).copied(),
        )?;
        let publication_advance = publication.prepare_authority_generation()?;
        lifecycle.commit_guard(guard, ResourceLifecycleIntent::Load)?;
        let inserted = Self::install_validated_resident_graph(
            &mut data,
            operation.owner.graph_path().clone(),
            resource,
        );
        graph_revisions.insert(operation.owner.graph_path().clone(), revision);
        if let Some(local_variables) = local_variables {
            for (id, variable) in local_variables {
                match variable_revisions.get(&id).copied() {
                    Some(entry) if !entry.is_present() => {}
                    Some(_) => {
                        data.variables.insert(id, variable);
                    }
                    None => {
                        data.variables.insert(id, variable);
                        variable_revisions.insert(
                            id,
                            VariableRevisionEntry::present(
                                crate::project::ResourceRevision::INITIAL,
                            ),
                        );
                    }
                }
            }
        }
        publication.commit_prepared(publication_advance);
        self.invalidate_graph_compile_products(operation.owner.graph_path());
        let projection_source = include_projection.then(|| {
            self.projection_source_snapshot(
                &data,
                projection_environment.expect("projection environment was captured"),
                publication.project_instance_id.clone(),
                publication.authority_generation(),
                graph_revisions.clone(),
                variable_revisions.clone(),
                self.database_authority_revisions.read().unwrap().clone(),
            )
        });
        drop(variable_revisions);
        drop(graph_revisions);
        drop(data);
        drop(lifecycle);
        drop(path);
        drop(publication);
        #[cfg(not(test))]
        drop(inserted);
        Ok(CommittedGraphLoad {
            #[cfg(test)]
            resource: inserted,
            projection_source,
        })
    }

    fn complete_cached_graph_load(
        &self,
        operation: &ResourceLifecycleOperation,
        guard: &mut crate::project::ResourceLifecycleGuard,
        include_projection: bool,
    ) -> Result<CommittedGraphLoad, ProjectFilesystemError> {
        self.ensure_project_operational()?;
        let projection_environment = if include_projection {
            let expected = self
                .projection_environment_expectation_for_identity(
                    operation.session.instance_id.as_str(),
                    &operation.session.root,
                )
                .map_err(|message| ProjectFilesystemError::TransactionPrepareFailed { message })?;
            Some(
                self.capture_projection_environment(&expected)
                    .map_err(|message| ProjectFilesystemError::TransactionPrepareFailed {
                        message,
                    })?,
            )
        } else {
            None
        };
        let publication = self.mutation_publication.lock().unwrap();
        let path = self.project_path.read().unwrap();
        if publication.project_instance_id != operation.session.instance_id.as_str()
            || path.is_none()
        {
            return Err(operation.stale_error());
        }
        if projection_environment
            .as_ref()
            .is_some_and(|environment| !environment.matches_publication(&publication))
        {
            return Err(ProjectFilesystemError::TransactionPrepareFailed {
                message:
                    "stale_project_lifecycle: projection environment changed before graph load commit"
                        .into(),
            });
        }
        let mut lifecycle = self.resource_lifecycle.boundary();
        lifecycle.validate(&operation.owner)?;
        self.ensure_project_operational()?;
        let data = self.project_data.read().unwrap();
        let current_resource = data
            .graphs
            .get(operation.owner.graph_path())
            .ok_or_else(|| ProjectFilesystemError::TransactionPrepareFailed {
                message: format!("graph '{}' not loaded", operation.owner.graph_path()),
            })?;
        #[cfg(test)]
        let resource = current_resource.clone();
        #[cfg(not(test))]
        let _ = current_resource;
        let graph_revisions = self.graph_revisions.read().unwrap();
        let variable_revisions = self.variable_revisions.read().unwrap();
        lifecycle.commit_guard(guard, ResourceLifecycleIntent::Load)?;
        let projection_source = include_projection.then(|| {
            self.projection_source_snapshot(
                &data,
                projection_environment.expect("projection environment was captured"),
                publication.project_instance_id.clone(),
                publication.authority_generation(),
                graph_revisions.clone(),
                variable_revisions.clone(),
                self.database_authority_revisions.read().unwrap().clone(),
            )
        });
        Ok(CommittedGraphLoad {
            #[cfg(test)]
            resource,
            projection_source,
        })
    }

    fn load_graph_for_lifecycle_commit(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        token: u64,
        include_projection: bool,
        before_commit: Option<&dyn Fn() -> Result<(), ProjectFilesystemError>>,
    ) -> Result<CommittedGraphLoad, ProjectFilesystemError> {
        let (operation, guard, cached) =
            self.register_graph_load_intent(expected_project_instance_id, graph_path, token)?;
        self.load_graph_for_registered_lifecycle_commit(
            operation,
            guard,
            cached,
            include_projection,
            before_commit,
        )
    }

    pub(in crate::project::project_state) fn load_graph_for_registered_lifecycle_commit(
        &self,
        operation: ResourceLifecycleOperation,
        mut guard: crate::project::ResourceLifecycleGuard,
        cached: bool,
        include_projection: bool,
        before_commit: Option<&dyn Fn() -> Result<(), ProjectFilesystemError>>,
    ) -> Result<CommittedGraphLoad, ProjectFilesystemError> {
        if cached {
            if let Some(before_commit) = before_commit {
                before_commit()?;
            }
            return self.complete_cached_graph_load(&operation, &mut guard, include_projection);
        }

        let filesystem_lease = self.filesystem().acquire(operation.session.root.clone())?;
        self.validate_resource_lifecycle_operation(&operation)?;
        let loaded = crate::project::project_io::load_project_graph_document_from_file(
            operation.session.root.as_path().to_string_lossy().as_ref(),
            operation.owner.graph_path(),
        );
        if loaded.is_err() {
            self.validate_resource_lifecycle_operation(&operation)?;
        }
        let loaded = loaded.map_err(|error| match error {
            crate::project::ProjectError::InvalidGraphDocument { source, .. } => {
                ProjectFilesystemError::InvalidGraphDocument {
                    path: operation.owner.graph_path().clone(),
                    source,
                }
            }
            error => ProjectFilesystemError::TransactionPrepareFailed {
                message: error.to_string(),
            },
        })?;
        let mut graph = loaded.document;
        graph.revision = loaded.revision.to_graph_revision();
        let resource = GraphResourceDocument {
            name: loaded.name,
            kind: loaded.kind,
            document: graph,
            function: loaded.function,
        };
        drop(filesystem_lease);
        if let Some(before_commit) = before_commit {
            before_commit()?;
        }
        self.complete_graph_load(
            &operation,
            &mut guard,
            resource,
            Some(loaded.local_variables),
            include_projection,
        )
    }

    #[cfg(test)]
    pub(crate) fn load_graph_resource(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        token: u64,
    ) -> Result<GraphResourceDocument, ProjectFilesystemError> {
        self.load_graph_for_lifecycle_commit(
            expected_project_instance_id,
            graph_path,
            token,
            false,
            None,
        )
        .map(|committed| committed.resource)
    }

    pub fn load_graph_projection(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        lifecycle_token: u64,
        locale: &str,
    ) -> Result<EditorGraphProjectionDto, ProjectFilesystemError> {
        let committed = self.load_graph_for_lifecycle_commit(
            expected_project_instance_id,
            graph_path,
            lifecycle_token,
            true,
            None,
        )?;
        committed
            .projection_source
            .as_ref()
            .expect("projection load requests a projection snapshot")
            .graph_projection(graph_path, locale)
            .map_err(|message| ProjectFilesystemError::TransactionPrepareFailed { message })
    }

    pub fn unload_graph_resource_for_lifecycle(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        token: u64,
    ) -> Result<bool, ProjectFilesystemError> {
        let session = self.capture_project_session()?;
        if &session.instance_id != expected_project_instance_id {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: format!(
                    "graph '{}' belongs to a different project instance",
                    graph_path
                ),
            });
        }
        let mut guard = self.resource_lifecycle.register(
            &session,
            graph_path,
            token,
            ResourceLifecycleIntent::Unload,
        )?;
        let operation = ResourceLifecycleOperation::from_guard(session, &guard);
        let mut publication = self.mutation_publication.lock().unwrap();
        let path = self.project_path.read().unwrap();
        if publication.project_instance_id != operation.session.instance_id.as_str()
            || path.is_none()
        {
            return Err(operation.stale_error());
        }
        let mut lifecycle = self.resource_lifecycle.boundary();
        lifecycle.validate(&operation.owner)?;
        self.ensure_project_operational()?;
        let graph_path_text = graph_path.as_str();
        let mut data = self.project_data.write().unwrap();
        let will_change = data.graphs.contains_key(graph_path)
            || data
                .variables
                .values()
                .any(|variable| match &variable.scope {
                    crate::variable::VariableScope::Global => false,
                    crate::variable::VariableScope::Event { event_path } => {
                        event_path == graph_path_text
                    }
                    crate::variable::VariableScope::Function { function_path } => {
                        function_path == graph_path_text
                    }
                });
        let publication_advance = will_change
            .then(|| publication.prepare_authority_generation())
            .transpose()?;
        lifecycle.commit_guard(&mut guard, ResourceLifecycleIntent::Unload)?;
        let removed = data.graphs.remove(graph_path);
        let graph_removed = removed.is_some();
        let variable_count = data.variables.len();
        data.variables.retain(|_, variable| match &variable.scope {
            crate::variable::VariableScope::Global => true,
            crate::variable::VariableScope::Event { event_path } => event_path != graph_path_text,
            crate::variable::VariableScope::Function { function_path } => {
                function_path != graph_path_text
            }
        });
        let variables_removed = data.variables.len() != variable_count;
        let changed = graph_removed || variables_removed;
        if let Some(publication_advance) = publication_advance {
            debug_assert!(changed);
            publication.commit_prepared(publication_advance);
            self.invalidate_graph_compile_products(graph_path);
        }
        Ok(changed)
    }
}
