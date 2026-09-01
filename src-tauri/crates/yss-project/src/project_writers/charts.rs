use super::*;

impl ProjectState {
    pub fn create_chart_resource(
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
            .charts
            .keys()
            .map(ChartResourcePath::display_name)
            .collect::<Vec<_>>();
        let unique = allocate_unique_resource_name(name, existing);
        let chart_path = ChartResourcePath::from_name(&unique);
        let mut document = ChartDocument::new(
            database_id
                .or_else(|| current.databases.keys().min().cloned())
                .unwrap_or_default(),
        );
        document.revision = match self
            .chart_revisions
            .read()
            .unwrap()
            .get(&chart_path)
            .copied()
        {
            Some(retained) => {
                crate::project_state::checked_resource_revision(chart_path.as_str(), retained)?
            }
            None => ResourceRevision::INITIAL,
        };
        let mutation_context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            BTreeMap::new(),
            BTreeSet::from([chart_key(&chart_path)]),
        );
        let result = self.write_chart_patch(
            &snapshot,
            mutation_context,
            lease,
            chart_path,
            None,
            document,
        );
        if result.is_ok() {
            reservation.complete();
        }
        result
    }

    pub fn duplicate_chart_resource(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        source: &ChartResourcePath,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
    ) -> Result<ProjectResourceMutationFacts, ProjectFilesystemError> {
        let snapshot = self.capture_writer_snapshot(expected_project_instance_id)?;
        let reservation =
            self.reserve_resource_operation(expected_project_instance_id, operation_id)?;
        let lease = self.filesystem().acquire(snapshot.session.root.clone())?;
        let current = self.project_data.read().unwrap().clone();
        let source_document = current.charts.get(source).cloned().ok_or_else(|| {
            ProjectFilesystemError::ChartNotFound {
                path: source.clone(),
            }
        })?;
        let existing = current
            .charts
            .keys()
            .map(ChartResourcePath::display_name)
            .collect::<Vec<_>>();
        let unique = allocate_unique_resource_name(source.display_name(), existing);
        let target = ChartResourcePath::from_name(&unique);
        let mut duplicate = source_document;
        duplicate.revision = match self.chart_revisions.read().unwrap().get(&target).copied() {
            Some(retained) => {
                crate::project_state::checked_resource_revision(target.as_str(), retained)?
            }
            None => ResourceRevision::INITIAL,
        };
        let mutation_context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            BTreeMap::from([(chart_key(source), expected_revision)]),
            BTreeSet::from([chart_key(&target)]),
        );
        let result =
            self.write_chart_patch(&snapshot, mutation_context, lease, target, None, duplicate);
        if result.is_ok() {
            reservation.complete();
        }
        result
    }

    fn write_chart_patch(
        &self,
        snapshot: &WriterSnapshot,
        context: ProjectTransactionContext,
        lease: yss_project_filesystem::ProjectFilesystemLeaseSet,
        chart_path: ChartResourcePath,
        before: Option<ChartDocument>,
        document: ChartDocument,
    ) -> Result<ProjectResourceMutationFacts, ProjectFilesystemError> {
        self.validate_writer_context(&context, snapshot.authority_generation)?;
        let retained_revision = self
            .chart_revisions
            .read()
            .unwrap()
            .get(&chart_path)
            .copied();
        let (new_path, contents) =
            crate::serialize_chart(&chart_path, &document).map_err(prepare_error)?;
        let prepared = ProjectFilesystemTransaction::prepare_with_validator(
            context.filesystem_context(),
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
            ProjectDataPatch::UpsertChart {
                path: chart_path.clone(),
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
        result.deltas = vec![chart_resource_delta(
            &chart_path,
            context.operation_id,
            retained_revision,
            before.as_ref(),
            Some(&document),
        )?]
        .into_boxed_slice();
        Ok(result)
    }

    pub fn save_chart_document(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        chart_path: &ChartResourcePath,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
        mut document: ChartDocument,
    ) -> Result<ProjectResourceMutationFacts, ProjectFilesystemError> {
        let snapshot = self.capture_writer_snapshot(expected_project_instance_id)?;
        let reservation =
            self.reserve_resource_operation(expected_project_instance_id, operation_id)?;
        let lease = self.filesystem().acquire(snapshot.session.root.clone())?;
        let before = self
            .project_data
            .read()
            .unwrap()
            .charts
            .get(chart_path)
            .cloned()
            .ok_or_else(|| ProjectFilesystemError::ChartNotFound {
                path: chart_path.clone(),
            })?;
        document.revision = crate::project_state::checked_resource_revision(
            chart_path.as_str(),
            expected_revision,
        )?;
        let mutation_context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            BTreeMap::from([(chart_key(chart_path), expected_revision)]),
            BTreeSet::new(),
        );
        let result = self.write_chart_patch(
            &snapshot,
            mutation_context,
            lease,
            chart_path.clone(),
            Some(before),
            document,
        );
        if result.is_ok() {
            reservation.complete();
        }
        result
    }

    pub fn rename_chart_resource(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        chart_path: &ChartResourcePath,
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
            yss_resource_lifecycle::LifecycleResourcePath::Chart(chart_path.clone()),
            lifecycle_token,
        )?;
        let lease = self.filesystem().acquire(snapshot.session.root.clone())?;
        self.validate_writer_context(
            &context(
                self,
                snapshot.session.clone(),
                operation_id,
                BTreeMap::from([(chart_key(chart_path), expected_revision)]),
                BTreeSet::new(),
            ),
            snapshot.authority_generation,
        )?;
        self.validate_resource_lifecycle_operation(&ownership.operation)?;

        let target = ChartResourcePath::from_name(new_name);
        let current = self.project_data.read().unwrap().clone();
        let mut moved = current.charts.get(chart_path).cloned().ok_or_else(|| {
            ProjectFilesystemError::ChartNotFound {
                path: chart_path.clone(),
            }
        })?;
        if current.charts.keys().any(|existing| {
            existing != chart_path
                && existing.display_name().portable_key() == new_name.portable_key()
        }) {
            return Err(ProjectFilesystemError::ResourceNameConflict {
                message: format!("a chart named '{}' already exists", new_name.as_str()),
            });
        }
        moved.revision = crate::project_state::checked_resource_revision(
            chart_path.as_str(),
            expected_revision,
        )?;
        let mutation_context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            BTreeMap::from([(chart_key(chart_path), expected_revision)]),
            BTreeSet::from([chart_key(&target)]),
        );
        self.validate_writer_context(&mutation_context, snapshot.authority_generation)?;
        let prepared = ProjectFilesystemTransaction::prepare(
            mutation_context.filesystem_context(),
            lease,
            vec![StagedFilesystemMutation::MoveFile {
                from: chart_path.relative_path().to_path_buf(),
                to: target.relative_path().to_path_buf(),
            }],
        )?;
        self.validate_writer_context(&mutation_context, snapshot.authority_generation)?;
        self.validate_resource_lifecycle_operation(&ownership.operation)?;
        let committed = prepared.commit()?;
        let mut result = match self.apply_project_resource_document_patch(
            &mutation_context,
            ProjectDataPatch::MoveChart {
                from: chart_path.clone(),
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
        result.deltas = vec![chart_move_delta(
            chart_path,
            &target,
            operation_id,
            expected_revision,
            moved.revision,
        )]
        .into_boxed_slice();
        reservation.complete();
        Ok(result)
    }

    pub fn remove_chart_resource(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        chart_path: &ChartResourcePath,
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
            .charts
            .get(chart_path)
            .cloned()
            .ok_or_else(|| ProjectFilesystemError::ChartNotFound {
                path: chart_path.clone(),
            })?;
        let delta = chart_resource_delta(
            chart_path,
            operation_id,
            Some(document.revision),
            Some(&document),
            None,
        )?;
        let mutation_context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            BTreeMap::from([(chart_key(chart_path), expected_revision)]),
            BTreeSet::new(),
        );
        self.validate_writer_context(&mutation_context, snapshot.authority_generation)?;
        let prepared = ProjectFilesystemTransaction::prepare(
            mutation_context.filesystem_context(),
            lease,
            vec![StagedFilesystemMutation::RemoveFile {
                relative_path: chart_path.relative_path().to_path_buf(),
            }],
        )?;
        self.validate_writer_context(&mutation_context, snapshot.authority_generation)?;
        let committed = prepared.commit()?;
        let mut result = match self.apply_project_resource_document_patch(
            &mutation_context,
            ProjectDataPatch::RemoveChart {
                path: chart_path.clone(),
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
