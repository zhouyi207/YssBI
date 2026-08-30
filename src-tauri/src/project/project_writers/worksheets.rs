use super::*;
#[cfg(test)]
use crate::schema::application_event::ResourceMutationResultDto;

impl ProjectState {
    #[cfg(test)]
    pub fn create_worksheet_resource_transaction(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        name: &ResourceName,
        database_id: Option<String>,
        operation_id: OperationId,
    ) -> Result<ResourceMutationResultDto, ProjectFilesystemError> {
        self.create_worksheet_resource_facts(
            expected_project_instance_id,
            name,
            database_id,
            operation_id,
        )
        .map(ProjectResourceMutationFacts::into_transport)
    }

    pub(crate) fn create_worksheet_resource_facts(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        name: &ResourceName,
        database_id: Option<String>,
        operation_id: OperationId,
    ) -> Result<ProjectResourceMutationFacts, ProjectFilesystemError> {
        let snapshot = self.capture_writer_snapshot(expected_project_instance_id)?;
        let reservation =
            self.reserve_resource_operation(expected_project_instance_id, operation_id)?;
        let lease = self.filesystem().acquire(snapshot.session.root.clone())?;
        let empty_context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            BTreeMap::new(),
            BTreeSet::new(),
        );
        self.validate_writer_context(&empty_context, snapshot.authority_generation)?;
        let current = self.project_data.read().unwrap().clone();
        let existing = current
            .worksheets
            .keys()
            .map(WorksheetResourcePath::display_name)
            .collect::<Vec<_>>();
        let unique = allocate_unique_resource_name(name, existing);
        let worksheet_path = WorksheetResourcePath::from_name(&unique);
        let mut document = WorksheetDocument::new(
            database_id
                .or_else(|| current.databases.keys().min().cloned())
                .unwrap_or_default(),
        );
        document.revision = match self
            .worksheet_revisions
            .read()
            .unwrap()
            .get(&worksheet_path)
            .copied()
        {
            Some(retained) => crate::project::project_state::checked_resource_revision(
                worksheet_path.as_str(),
                retained,
            )?,
            None => ResourceRevision::INITIAL,
        };
        let mutation_context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            BTreeMap::new(),
            BTreeSet::from([worksheet_key(&worksheet_path)]),
        );
        let result = self.write_worksheet_patch(
            &snapshot,
            mutation_context,
            lease,
            worksheet_path,
            None,
            document,
        );
        if result.is_ok() {
            reservation.complete();
        }
        result
    }

    #[cfg(test)]
    pub fn duplicate_worksheet_resource_transaction(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        source: &WorksheetResourcePath,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
    ) -> Result<ResourceMutationResultDto, ProjectFilesystemError> {
        self.duplicate_worksheet_resource_facts(
            expected_project_instance_id,
            source,
            expected_revision,
            operation_id,
        )
        .map(ProjectResourceMutationFacts::into_transport)
    }

    pub(crate) fn duplicate_worksheet_resource_facts(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        source: &WorksheetResourcePath,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
    ) -> Result<ProjectResourceMutationFacts, ProjectFilesystemError> {
        let snapshot = self.capture_writer_snapshot(expected_project_instance_id)?;
        let reservation =
            self.reserve_resource_operation(expected_project_instance_id, operation_id)?;
        let lease = self.filesystem().acquire(snapshot.session.root.clone())?;
        let current = self.project_data.read().unwrap().clone();
        let source_document = current.worksheets.get(source).cloned().ok_or_else(|| {
            ProjectFilesystemError::WorksheetNotFound {
                path: source.clone(),
            }
        })?;
        let existing = current
            .worksheets
            .keys()
            .map(WorksheetResourcePath::display_name)
            .collect::<Vec<_>>();
        let unique = allocate_unique_resource_name(source.display_name(), existing);
        let target = WorksheetResourcePath::from_name(&unique);
        let mut duplicate = source_document;
        duplicate.revision = match self
            .worksheet_revisions
            .read()
            .unwrap()
            .get(&target)
            .copied()
        {
            Some(retained) => {
                crate::project::project_state::checked_resource_revision(target.as_str(), retained)?
            }
            None => ResourceRevision::INITIAL,
        };
        let mutation_context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            BTreeMap::from([(worksheet_key(source), expected_revision)]),
            BTreeSet::from([worksheet_key(&target)]),
        );
        let result =
            self.write_worksheet_patch(&snapshot, mutation_context, lease, target, None, duplicate);
        if result.is_ok() {
            reservation.complete();
        }
        result
    }

    fn write_worksheet_patch(
        &self,
        snapshot: &WriterSnapshot,
        context: ProjectTransactionContext,
        lease: crate::project::ProjectFilesystemLeaseSet,
        worksheet_path: WorksheetResourcePath,
        before: Option<WorksheetDocument>,
        document: WorksheetDocument,
    ) -> Result<ProjectResourceMutationFacts, ProjectFilesystemError> {
        self.validate_writer_context(&context, snapshot.authority_generation)?;
        let retained_revision = self
            .worksheet_revisions
            .read()
            .unwrap()
            .get(&worksheet_path)
            .copied();
        let (new_path, contents) = crate::project::serialize_worksheet(&worksheet_path, &document)
            .map_err(prepare_error)?;
        let prepared = ProjectFilesystemTransaction::prepare_with_validator(
            context.clone(),
            lease,
            vec![StagedFilesystemMutation::Write {
                relative_path: new_path,
                contents,
            }],
            validate_document,
        )?;
        self.validate_writer_context(&context, snapshot.authority_generation)?;
        let committed = prepared.commit()?;
        let mut result = match self.apply_project_resource_document_patch(
            &context,
            ProjectDataPatch::UpsertWorksheet {
                path: worksheet_path.clone(),
                document: document.clone(),
            },
            None,
        ) {
            Ok(result) => result,
            Err(error) => {
                return match committed.rollback() {
                    Ok(()) => Err(error),
                    Err(rollback_error) => Err(rollback_error),
                };
            }
        };
        committed.finalize();
        result.deltas = vec![worksheet_resource_delta(
            &worksheet_path,
            context.operation_id,
            retained_revision,
            before.as_ref(),
            Some(&document),
        )?]
        .into_boxed_slice();
        Ok(result)
    }

    #[cfg(test)]
    pub fn save_worksheet_document(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        worksheet_path: &WorksheetResourcePath,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
        document: WorksheetDocument,
    ) -> Result<ResourceMutationResultDto, ProjectFilesystemError> {
        self.save_worksheet_document_facts(
            expected_project_instance_id,
            worksheet_path,
            expected_revision,
            operation_id,
            document,
        )
        .map(ProjectResourceMutationFacts::into_transport)
    }

    pub(crate) fn save_worksheet_document_facts(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        worksheet_path: &WorksheetResourcePath,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
        mut document: WorksheetDocument,
    ) -> Result<ProjectResourceMutationFacts, ProjectFilesystemError> {
        let snapshot = self.capture_writer_snapshot(expected_project_instance_id)?;
        let reservation =
            self.reserve_resource_operation(expected_project_instance_id, operation_id)?;
        let lease = self.filesystem().acquire(snapshot.session.root.clone())?;
        let before = self
            .project_data
            .read()
            .unwrap()
            .worksheets
            .get(worksheet_path)
            .cloned()
            .ok_or_else(|| ProjectFilesystemError::WorksheetNotFound {
                path: worksheet_path.clone(),
            })?;
        document.revision = crate::project::project_state::checked_resource_revision(
            worksheet_path.as_str(),
            expected_revision,
        )?;
        let mutation_context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            BTreeMap::from([(worksheet_key(worksheet_path), expected_revision)]),
            BTreeSet::new(),
        );
        let result = self.write_worksheet_patch(
            &snapshot,
            mutation_context,
            lease,
            worksheet_path.clone(),
            Some(before),
            document,
        );
        if result.is_ok() {
            reservation.complete();
        }
        result
    }

    #[cfg(test)]
    pub fn rename_worksheet_resource_transaction(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        worksheet_path: &WorksheetResourcePath,
        expected_revision: ResourceRevision,
        new_name: &ResourceName,
        lifecycle_token: u64,
        operation_id: OperationId,
    ) -> Result<ResourceMutationResultDto, ProjectFilesystemError> {
        self.rename_worksheet_resource_facts(
            expected_project_instance_id,
            worksheet_path,
            expected_revision,
            new_name,
            lifecycle_token,
            operation_id,
        )
        .map(ProjectResourceMutationFacts::into_transport)
    }

    pub(crate) fn rename_worksheet_resource_facts(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        worksheet_path: &WorksheetResourcePath,
        expected_revision: ResourceRevision,
        new_name: &ResourceName,
        lifecycle_token: u64,
        operation_id: OperationId,
    ) -> Result<ProjectResourceMutationFacts, ProjectFilesystemError> {
        let snapshot = self.capture_writer_snapshot(expected_project_instance_id)?;
        let reservation =
            self.reserve_resource_operation(expected_project_instance_id, operation_id)?;
        let mut ownership = self.acquire_resource_rename_ownership(
            expected_project_instance_id,
            yss_resource_lifecycle::LifecycleResourcePath::Worksheet(worksheet_path.clone()),
            lifecycle_token,
        )?;
        let lease = self.filesystem().acquire(snapshot.session.root.clone())?;
        self.validate_writer_context(
            &context(
                self,
                snapshot.session.clone(),
                operation_id,
                BTreeMap::from([(worksheet_key(worksheet_path), expected_revision)]),
                BTreeSet::new(),
            ),
            snapshot.authority_generation,
        )?;
        self.validate_resource_lifecycle_operation(&ownership.operation)?;

        let target = WorksheetResourcePath::from_name(new_name);
        let current = self.project_data.read().unwrap().clone();
        let mut moved = current
            .worksheets
            .get(worksheet_path)
            .cloned()
            .ok_or_else(|| ProjectFilesystemError::WorksheetNotFound {
                path: worksheet_path.clone(),
            })?;
        if current.worksheets.keys().any(|existing| {
            existing != worksheet_path
                && existing.display_name().portable_key() == new_name.portable_key()
        }) {
            return Err(ProjectFilesystemError::ResourceNameConflict {
                message: format!("a worksheet named '{}' already exists", new_name.as_str()),
            });
        }
        moved.revision = crate::project::project_state::checked_resource_revision(
            worksheet_path.as_str(),
            expected_revision,
        )?;
        let mutation_context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            BTreeMap::from([(worksheet_key(worksheet_path), expected_revision)]),
            BTreeSet::from([worksheet_key(&target)]),
        );
        self.validate_writer_context(&mutation_context, snapshot.authority_generation)?;
        let prepared = ProjectFilesystemTransaction::prepare(
            mutation_context.clone(),
            lease,
            vec![StagedFilesystemMutation::MoveFile {
                from: worksheet_path.relative_path().to_path_buf(),
                to: target.relative_path().to_path_buf(),
            }],
        )?;
        self.validate_writer_context(&mutation_context, snapshot.authority_generation)?;
        self.validate_resource_lifecycle_operation(&ownership.operation)?;
        let committed = prepared.commit()?;
        let mut result = match self.apply_project_resource_document_patch(
            &mutation_context,
            ProjectDataPatch::MoveWorksheet {
                from: worksheet_path.clone(),
                to: target.clone(),
                moved: moved.clone(),
            },
            Some(&mut ownership),
        ) {
            Ok(result) => result,
            Err(error) => {
                return match committed.rollback() {
                    Ok(()) => Err(error),
                    Err(rollback_error) => Err(rollback_error),
                };
            }
        };
        committed.finalize();
        result.deltas = vec![worksheet_move_delta(
            worksheet_path,
            &target,
            operation_id,
            expected_revision,
            moved.revision,
        )]
        .into_boxed_slice();
        reservation.complete();
        Ok(result)
    }

    #[cfg(test)]
    pub fn remove_worksheet_resource_transaction(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        worksheet_path: &WorksheetResourcePath,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
    ) -> Result<ResourceMutationResultDto, ProjectFilesystemError> {
        self.remove_worksheet_resource_facts(
            expected_project_instance_id,
            worksheet_path,
            expected_revision,
            operation_id,
        )
        .map(ProjectResourceMutationFacts::into_transport)
    }

    pub(crate) fn remove_worksheet_resource_facts(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        worksheet_path: &WorksheetResourcePath,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
    ) -> Result<ProjectResourceMutationFacts, ProjectFilesystemError> {
        let snapshot = self.capture_writer_snapshot(expected_project_instance_id)?;
        let reservation =
            self.reserve_resource_operation(expected_project_instance_id, operation_id)?;
        let lease = self.filesystem().acquire(snapshot.session.root.clone())?;
        let document = self
            .project_data
            .read()
            .unwrap()
            .worksheets
            .get(worksheet_path)
            .cloned()
            .ok_or_else(|| ProjectFilesystemError::WorksheetNotFound {
                path: worksheet_path.clone(),
            })?;
        let delta = worksheet_resource_delta(
            worksheet_path,
            operation_id,
            Some(document.revision),
            Some(&document),
            None,
        )?;
        let mutation_context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            BTreeMap::from([(worksheet_key(worksheet_path), expected_revision)]),
            BTreeSet::new(),
        );
        self.validate_writer_context(&mutation_context, snapshot.authority_generation)?;
        let prepared = ProjectFilesystemTransaction::prepare(
            mutation_context.clone(),
            lease,
            vec![StagedFilesystemMutation::RemoveFile {
                relative_path: worksheet_path.relative_path().to_path_buf(),
            }],
        )?;
        self.validate_writer_context(&mutation_context, snapshot.authority_generation)?;
        let committed = prepared.commit()?;
        let mut result = match self.apply_project_resource_document_patch(
            &mutation_context,
            ProjectDataPatch::RemoveWorksheet {
                path: worksheet_path.clone(),
                revision: expected_revision,
            },
            None,
        ) {
            Ok(result) => result,
            Err(error) => {
                return match committed.rollback() {
                    Ok(()) => Err(error),
                    Err(rollback_error) => Err(rollback_error),
                };
            }
        };
        committed.finalize();
        result.deltas = vec![delta].into_boxed_slice();
        reservation.complete();
        Ok(result)
    }
}
