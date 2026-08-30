use super::*;

struct ActivationGarbage {
    _publication_project_instance_id: String,
    _path: Option<String>,
    _lifecycle: crate::project::resource_lifecycle::ResourceLifecycleState,
    _data: ProjectData,
    _store: ProjectStore,
    _graph_revisions:
        std::collections::HashMap<GraphResourcePath, yss_graph_document::GraphRevision>,
    _variable_revisions:
        std::collections::HashMap<yss_variable_contract::VariableId, VariableRevisionEntry>,
    _worksheet_revisions:
        std::collections::HashMap<WorksheetResourcePath, crate::project::ResourceRevision>,
    _database_authority_revisions: std::collections::HashMap<String, u64>,
    _identity: ProjectAuthorityExpectation,
    _recovery_message: Option<String>,
    _history: ProjectHistory,
}

pub(in crate::project) struct PublishedProjectActivation {
    instance_id: ProjectInstanceId,
    garbage: ActivationGarbage,
    postcommit_panic: Option<ActivationPanicPayload>,
}

impl PublishedProjectActivation {
    pub(in crate::project) fn dispose(self) -> ProjectInstanceId {
        let Self {
            instance_id,
            garbage,
            postcommit_panic,
        } = self;
        drop(garbage);
        if let Some(payload) = postcommit_panic {
            std::panic::resume_unwind(payload);
        }
        instance_id
    }
}

impl ProjectState {
    pub(in crate::project) fn read_activation_data(
        &self,
        root: &NormalizedProjectRoot,
    ) -> Result<ProjectData, ProjectFilesystemError> {
        crate::project::load_project_from_file(root.as_path().to_string_lossy().as_ref()).map_err(
            |error| ProjectFilesystemError::TransactionPrepareFailed {
                message: error.to_string(),
            },
        )
    }

    pub(in crate::project) fn capture_prepared_authority_basis(
        &self,
        root: &NormalizedProjectRoot,
    ) -> Result<Option<crate::project::PreparedAuthorityBasis>, ProjectFilesystemError> {
        let publication = self.mutation_publication.lock().unwrap();
        let identity = self.activation_identity.read().unwrap();
        Ok((identity.project_root.as_ref() == Some(root)).then(|| {
            crate::project::PreparedAuthorityBasis {
                project_instance_id: ProjectInstanceId::from_existing(
                    publication.project_instance_id.clone(),
                ),
                project_root: root.clone(),
                publication_revision: publication.resource_revision,
                authority_generation: publication.authority_generation,
            }
        }))
    }

    pub(in crate::project) fn publish_project_activation(
        &self,
        prepared: PreparedProjectActivation,
    ) -> Result<PublishedProjectActivation, ProjectFilesystemError> {
        self.publish_project_activation_with_test_hooks(prepared, true)
    }

    pub(in crate::project) fn publish_project_activation_without_test_hooks(
        &self,
        prepared: PreparedProjectActivation,
    ) -> Result<PublishedProjectActivation, ProjectFilesystemError> {
        self.publish_project_activation_with_test_hooks(prepared, false)
    }

    fn publish_project_activation_with_test_hooks(
        &self,
        prepared: PreparedProjectActivation,
        run_test_hooks: bool,
    ) -> Result<PublishedProjectActivation, ProjectFilesystemError> {
        let PreparedProjectActivation {
            session_root: project_root,
            data,
            store,
            graph_revisions,
            variable_revisions,
            worksheet_revisions,
            authority_basis,
            requires_final_rebuild: _,
        } = prepared;
        let path = project_root
            .as_ref()
            .map(|root| root.as_path().to_string_lossy().into_owned());
        let next_instance_id = ProjectInstanceId::new();
        let next_publication_id = next_instance_id.to_string();
        let next_identity = ProjectAuthorityExpectation {
            project_instance_id: next_instance_id.clone(),
            project_root: project_root.clone(),
            project_session_id: store.project_session_id.clone(),
        };
        let database_authority_revisions =
            data.databases.keys().cloned().map(|id| (id, 0)).collect();
        let precommit_panic;
        let postcommit_panic;
        let mut garbage = None;

        {
            let (mut publication, publication_recovered) = match self.mutation_publication.lock() {
                Ok(guard) => (guard, false),
                Err(error) => (error.into_inner(), true),
            };
            let (mut resource_operations, resource_operations_recovered) =
                match self.resource_operations.lock() {
                    Ok(guard) => (guard, false),
                    Err(error) => (error.into_inner(), true),
                };
            let (mut current_path, path_recovered) = match self.project_path.write() {
                Ok(guard) => (guard, false),
                Err(error) => (error.into_inner(), true),
            };
            let (mut lifecycle, lifecycle_recovered) =
                self.resource_lifecycle.boundary_recovering();
            let (mut current_data, data_recovered) = match self.project_data.write() {
                Ok(guard) => (guard, false),
                Err(error) => (error.into_inner(), true),
            };
            let (mut current_store, store_recovered) = match self.project_store.write() {
                Ok(guard) => (guard, false),
                Err(error) => (error.into_inner(), true),
            };
            let (mut current_database_authority_revisions, database_authority_revisions_recovered) =
                match self.database_authority_revisions.write() {
                    Ok(guard) => (guard, false),
                    Err(error) => (error.into_inner(), true),
                };
            let (mut current_graph_revisions, graph_revisions_recovered) =
                match self.graph_revisions.write() {
                    Ok(guard) => (guard, false),
                    Err(error) => (error.into_inner(), true),
                };
            let (mut current_variable_revisions, variable_revisions_recovered) =
                match self.variable_revisions.write() {
                    Ok(guard) => (guard, false),
                    Err(error) => (error.into_inner(), true),
                };
            let (mut current_worksheet_revisions, worksheet_revisions_recovered) =
                match self.worksheet_revisions.write() {
                    Ok(guard) => (guard, false),
                    Err(error) => (error.into_inner(), true),
                };
            let (mut current_identity, identity_recovered) = match self.activation_identity.write()
            {
                Ok(guard) => (guard, false),
                Err(error) => (error.into_inner(), true),
            };
            let (mut recovery, recovery_recovered) = self.recovery_marker.boundary_recovering();
            let (mut history, history_recovered) = match self.history.write() {
                Ok(guard) => (guard, false),
                Err(error) => (error.into_inner(), true),
            };

            if authority_basis.as_ref().is_some_and(|basis| {
                publication.project_instance_id != basis.project_instance_id.as_str()
                    || publication.resource_revision != basis.publication_revision
                    || publication.authority_generation != basis.authority_generation
                    || current_identity.project_root.as_ref() != Some(&basis.project_root)
            }) {
                return Err(ProjectFilesystemError::StaleProjectLifecycle {
                    message: "prepared project activation was superseded by committed authority"
                        .into(),
                });
            }

            let generation = ActivationGenerationTransition::begin(&self.activation_generation)?;
            precommit_panic = run_test_hooks
                .then(|| self.run_activation_publication_panic_test_hook())
                .flatten();

            if precommit_panic.is_none() {
                resource_operations.reset_for_project(next_instance_id.clone());
                let previous_publication_id = publication.reset_to(next_publication_id);
                garbage = Some(ActivationGarbage {
                    _publication_project_instance_id: previous_publication_id,
                    _path: std::mem::replace(&mut *current_path, path),
                    _lifecycle: lifecycle.take_state(),
                    _data: std::mem::replace(&mut *current_data, data),
                    _store: std::mem::replace(&mut *current_store, store),
                    _database_authority_revisions: std::mem::replace(
                        &mut *current_database_authority_revisions,
                        database_authority_revisions,
                    ),
                    _graph_revisions: std::mem::replace(
                        &mut *current_graph_revisions,
                        graph_revisions,
                    ),
                    _variable_revisions: std::mem::replace(
                        &mut *current_variable_revisions,
                        variable_revisions,
                    ),
                    _worksheet_revisions: std::mem::replace(
                        &mut *current_worksheet_revisions,
                        worksheet_revisions,
                    ),
                    _identity: std::mem::replace(&mut *current_identity, next_identity),
                    _recovery_message: std::mem::take(&mut *recovery),
                    _history: std::mem::take(&mut *history),
                });
                postcommit_panic = run_test_hooks
                    .then(|| self.run_activation_store_replaced_test_hook())
                    .flatten();
                generation.complete();

                if publication_recovered {
                    self.mutation_publication.clear_poison();
                }
                if resource_operations_recovered {
                    self.resource_operations.clear_poison();
                }
                if path_recovered {
                    self.project_path.clear_poison();
                }
                if lifecycle_recovered {
                    self.resource_lifecycle.clear_poison();
                }
                if data_recovered {
                    self.project_data.clear_poison();
                }
                if store_recovered {
                    self.project_store.clear_poison();
                }
                if database_authority_revisions_recovered {
                    self.database_authority_revisions.clear_poison();
                }
                if graph_revisions_recovered {
                    self.graph_revisions.clear_poison();
                }
                if variable_revisions_recovered {
                    self.variable_revisions.clear_poison();
                }
                if worksheet_revisions_recovered {
                    self.worksheet_revisions.clear_poison();
                }
                if identity_recovered {
                    self.activation_identity.clear_poison();
                }
                if recovery_recovered {
                    self.recovery_marker.clear_poison();
                }
                if history_recovered {
                    self.history.clear_poison();
                }
            } else {
                postcommit_panic = None;
            }
        }

        if let Some(payload) = precommit_panic {
            std::panic::resume_unwind(payload);
        }
        let garbage = garbage.ok_or_else(|| ProjectFilesystemError::TransactionCommitFailed {
            message: "activation commit did not retain previous authority".into(),
        })?;
        Ok(PublishedProjectActivation {
            instance_id: next_instance_id,
            garbage,
            postcommit_panic,
        })
    }
}
