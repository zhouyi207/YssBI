use super::*;

fn validate_signature_request<'a>(
    data: &'a ProjectData,
    path: &GraphResourcePath,
    request: &MutationRequest<yss_project_history::FunctionDocumentPatch>,
) -> Result<&'a yss_project_history::FunctionDocument, ProjectResourceMutationError> {
    let resource = ResourceKey::Function(yss_project_history::FunctionResourceKey(
        path.as_str().into(),
    ));
    if request.resource != resource {
        return Err(ProjectResourceMutationError::ResourceMismatch {
            requested: format!("{:?}", request.resource).into(),
            store: format!("{resource:?}").into(),
        });
    }
    let function = data
        .graphs
        .get(path)
        .and_then(|graph| graph.function.as_ref())
        .ok_or_else(|| ProjectResourceMutationError::ResourceMismatch {
            requested: format!("{resource:?}").into(),
            store: format!("{resource:?}").into(),
        })?;
    if function.revision != request.base_revision {
        return Err(ProjectResourceMutationError::StaleRevision {
            base_revision: request.base_revision.get(),
            current_revision: function.revision.get(),
        });
    }
    if function.signature != request.payload.before {
        return Err(ProjectResourceMutationError::Mutation(
            "function patch before-state does not match the current signature".into(),
        ));
    }
    Ok(function)
}

pub(super) fn function_mutation_error(
    error: ProjectFilesystemError,
) -> ProjectResourceMutationError {
    match error {
        ProjectFilesystemError::StaleProjectLifecycle { .. } => {
            ProjectResourceMutationError::StaleProjectLifecycle(error.to_string().into())
        }
        ProjectFilesystemError::ProjectRecoveryRequired { .. }
        | ProjectFilesystemError::TransactionRollbackFailed {
            recovery_required: true,
            ..
        } => ProjectResourceMutationError::RecoveryRequired(error.to_string().into()),
        _ => ProjectResourceMutationError::Mutation(error.to_string().into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::TempProject;
    use yss_graph_document::GraphResourceKind;
    use yss_project_history::{FunctionDocument, FunctionDocumentPatch, FunctionSignature};

    fn function_project(revision: ResourceRevision) -> (TempProject, GraphResourcePath) {
        let path = GraphResourcePath::new("functions/Compute.yssbi-function").unwrap();
        let mut graph = GraphResourceDocument::new("Compute", GraphResourceKind::Function);
        graph.function = Some(FunctionDocument {
            revision,
            signature: FunctionSignature::default(),
        });
        let mut data = ProjectData::new();
        data.graphs.insert(path.clone(), graph);
        (TempProject::activate("function-mutation", data), path)
    }

    #[test]
    fn signature_commit_preserves_revision_and_patch_admission() {
        let (fixture, path) = function_project(ResourceRevision::INITIAL);
        let state = fixture.state();
        let project = ProjectInstanceId::from_existing(state.project_instance_id());
        let after = FunctionSignature {
            return_type: Some("Int64".into()),
            ..FunctionSignature::default()
        };
        let request = MutationRequest::new(
            ResourceKey::Function(yss_project_history::FunctionResourceKey(
                path.as_str().into(),
            )),
            ResourceRevision::INITIAL,
            OperationId::new(),
            FunctionDocumentPatch::new(FunctionSignature::default(), after.clone()),
        );
        let receipt = state
            .update_function_signature(&project, &path, request.clone())
            .unwrap()
            .into_parts();
        let next_revision = ResourceRevision::INITIAL.checked_next().unwrap();
        assert_eq!(receipt.deltas[0].from_revision, ResourceRevision::INITIAL);
        assert_eq!(receipt.deltas[0].to_revision, next_revision);
        let committed = state.get_data().unwrap().graphs[&path].function.clone();
        assert_eq!(committed.as_ref().unwrap().signature, after);
        assert_eq!(committed.as_ref().unwrap().revision, next_revision);
        assert_eq!(state.revision_state_for_test().0[&path], next_revision);

        let session = state.capture_project_session().unwrap();
        let persisted = crate::project_io::load_project_graph_from_file(
            session.root.as_path().to_str().unwrap(),
            &path,
        )
        .unwrap();
        assert_eq!(persisted.function, committed);
        let before_rescan = state
            .read_project_index(&project)
            .unwrap()
            .publication_revision;
        state
            .reconcile_project_change(
                &project,
                yss_project_change::ProjectChange::rescan_required(),
            )
            .unwrap();
        assert_eq!(state.get_data().unwrap().graphs[&path].function, committed);
        assert_eq!(
            state
                .read_project_index(&project)
                .unwrap()
                .publication_revision,
            before_rescan
        );
        state.unload_graph_resource(&path).unwrap();
        assert_eq!(
            state.read_project_index(&project).unwrap().graphs[0].revision,
            next_revision
        );
        state
            .load_graph_document(&project, &path, u64::MAX - 1)
            .unwrap();
        assert_eq!(state.revision_state_for_test().0[&path], next_revision);
        assert_eq!(state.get_data().unwrap().graphs[&path].function, committed);

        let stale = MutationRequest {
            operation_id: OperationId::new(),
            ..request.clone()
        };
        assert!(matches!(
            state.update_function_signature(&project, &path, stale),
            Err(ProjectResourceMutationError::StaleRevision { .. })
        ));
        let mismatched_patch = MutationRequest {
            operation_id: OperationId::new(),
            base_revision: next_revision,
            ..request
        };
        assert!(matches!(
            state.update_function_signature(&project, &path, mismatched_patch),
            Err(ProjectResourceMutationError::Mutation(_))
        ));
        assert_eq!(state.get_data().unwrap().graphs[&path].function, committed);
    }

    #[test]
    fn exhausted_signature_revision_preserves_project_publication_and_data() {
        let revision = ResourceRevision::new(u64::MAX);
        let (fixture, path) = function_project(revision);
        let state = fixture.state();
        let session = state.capture_project_session().unwrap();
        let before_publication = state.coherent_project_read_snapshot(&session).unwrap().1;
        let before = state.get_data().unwrap().graphs[&path].function.clone();
        let before_revisions = state.revision_state_for_test();
        let request = MutationRequest::new(
            ResourceKey::Function(yss_project_history::FunctionResourceKey(
                path.as_str().into(),
            )),
            revision,
            OperationId::new(),
            FunctionDocumentPatch::new(
                FunctionSignature::default(),
                FunctionSignature {
                    return_type: Some("Int64".into()),
                    ..FunctionSignature::default()
                },
            ),
        );
        assert!(matches!(
            state.update_function_signature(&session.instance_id, &path, request),
            Err(ProjectResourceMutationError::Mutation(_))
        ));
        assert_eq!(state.get_data().unwrap().graphs[&path].function, before);
        assert_eq!(state.revision_state_for_test(), before_revisions);
        assert_eq!(
            state.coherent_project_read_snapshot(&session).unwrap().1,
            before_publication
        );
    }
}

impl ProjectState {
    pub fn update_function_signature(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        request: MutationRequest<yss_project_history::FunctionDocumentPatch>,
    ) -> Result<crate::project_writers::ProjectResourceMutationFacts, ProjectResourceMutationError>
    {
        let session = self
            .capture_project_session()
            .map_err(|error| match error {
                ProjectFilesystemError::StaleProjectLifecycle { message } => {
                    ProjectResourceMutationError::StaleProjectLifecycle(message.into())
                }
                error => ProjectResourceMutationError::RecoveryRequired(error.to_string().into()),
            })?;
        if &session.instance_id != expected_project_instance_id {
            return Err(ProjectResourceMutationError::StaleProjectLifecycle(
                "function signature project instance is stale".into(),
            ));
        }
        let snapshot = self
            .capture_writer_snapshot(expected_project_instance_id)
            .map_err(function_mutation_error)?;
        let reservation = self
            .reserve_resource_operation(expected_project_instance_id, request.operation_id)
            .map_err(function_mutation_error)?;
        let lease = self
            .filesystem()
            .acquire(snapshot.session.root.clone())
            .map_err(function_mutation_error)?;
        let function = validate_signature_request(&snapshot.data, graph_path, &request)?;
        let revision = function
            .revision
            .checked_next()
            .map_err(|error| ProjectResourceMutationError::Mutation(error.to_string().into()))?;
        let mutation_context = crate::project_writers::context(
            self,
            snapshot.session.clone(),
            request.operation_id,
            [
                (request.resource.clone(), request.base_revision),
                (
                    ResourceKey::Graph(graph_path.clone()),
                    snapshot
                        .graph_resource_revisions
                        .get(graph_path)
                        .copied()
                        .unwrap_or(ResourceRevision::INITIAL),
                ),
            ]
            .into(),
            Default::default(),
        );
        self.validate_writer_context(&mutation_context, snapshot.authority_generation)
            .map_err(function_mutation_error)?;
        let mut candidate = snapshot.data.graphs[graph_path].clone();
        candidate.function = Some(yss_project_history::FunctionDocument {
            revision,
            signature: request.payload.after.clone(),
        });
        let contents = crate::project_io::serialize_graph_resource_document(&candidate)
            .map_err(|error| ProjectResourceMutationError::Mutation(error.to_string().into()))?;
        let prepared = yss_project_filesystem::ProjectFilesystemTransaction::prepare(
            mutation_context.filesystem_context(),
            lease,
            vec![yss_project_filesystem::StagedFilesystemMutation::Write {
                relative_path: graph_path.as_str().into(),
                contents,
            }],
        )
        .map_err(function_mutation_error)?;
        self.validate_writer_context(&mutation_context, snapshot.authority_generation)
            .map_err(function_mutation_error)?;
        let committed = prepared.commit().map_err(function_mutation_error)?;
        match self.commit_function_signature(expected_project_instance_id, graph_path, request) {
            Ok(result) => {
                committed.finalize();
                reservation.complete();
                Ok(result.into_project_facts())
            }
            Err(error) => {
                committed.rollback().map_err(function_mutation_error)?;
                Err(error)
            }
        }
    }

    fn commit_function_signature(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        request: MutationRequest<yss_project_history::FunctionDocumentPatch>,
    ) -> Result<CommittedResourceMutation, ProjectResourceMutationError> {
        self.ensure_mutation_operational()?;
        let function_key = yss_project_history::FunctionResourceKey(graph_path.as_str().into());
        let expected_resource = ResourceKey::Function(function_key.clone());
        let session = self
            .capture_project_session()
            .map_err(function_mutation_error)?;
        let expected_session = self.current_projection_environment_expectation();
        let authority = self
            .capture_project_authority_for_session(&session)
            .map_err(function_mutation_error)?;
        let mut publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != expected_project_instance_id.as_str() {
            return Err(ProjectResourceMutationError::StaleProjectLifecycle(
                "caller project changed before signature authority commit".into(),
            ));
        }
        if publication.project_instance_id != expected_session.project_instance_id.as_str() {
            return Err(ProjectResourceMutationError::StaleProjectLifecycle(
                "project changed before signature authority commit".into(),
            ));
        }
        if !authority.matches_publication(&publication) {
            return Err(ProjectResourceMutationError::StaleProjectLifecycle(
                "projection environment changed before signature authority commit".into(),
            ));
        }
        let mut data = self.project_data.write().unwrap();
        self.ensure_mutation_operational()?;
        let function = validate_signature_request(&data, graph_path, &request)?;
        let publication_advance = publication
            .prepare_resource_revision()
            .map_err(|error| ProjectResourceMutationError::Projection(error.to_string().into()))?;
        let from_revision = function.revision;
        let mut graph_resource_revisions = self.graph_resource_revisions.write().unwrap();
        let to_revision = from_revision
            .checked_next()
            .map_err(|error| ProjectResourceMutationError::Mutation(error.to_string().into()))?;
        let mut next_data = data.clone();
        next_data
            .graphs
            .get_mut(graph_path)
            .ok_or_else(|| {
                ProjectResourceMutationError::Mutation(
                    format!("Function owner graph '{graph_path}' is not loaded").into(),
                )
            })?
            .function = Some(yss_project_history::FunctionDocument {
            revision: to_revision,
            signature: request.payload.after.clone(),
        });
        let mut next_graph_resource_revisions = graph_resource_revisions.clone();
        let graph_revision = super::checked_resource_revision(
            graph_path.as_str(),
            graph_resource_revisions
                .get(graph_path)
                .copied()
                .unwrap_or(ResourceRevision::INITIAL),
        )
        .map_err(function_mutation_error)?;
        next_graph_resource_revisions.insert(graph_path.clone(), graph_revision);
        let deltas = vec![yss_project_history::ResourceDeltaEvent {
            resource: expected_resource,
            from_revision,
            to_revision,
            caused_by: Some(request.operation_id),
            payload: yss_project_history::ResourceDocumentPatch::Function(request.payload),
        }];
        let expected_graph_paths = affected_projection_paths(&deltas, &next_data);
        *data = next_data;
        *graph_resource_revisions = next_graph_resource_revisions;
        let publication_revision = publication.commit_prepared(publication_advance);
        Ok(CommittedResourceMutation {
            operation_id: request.operation_id,
            project_instance_id: publication.project_instance_id.clone(),
            publication_revision,
            moves: Vec::new(),
            deltas,
            expected_graph_paths,
        })
    }
}
