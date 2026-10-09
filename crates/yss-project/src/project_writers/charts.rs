use super::*;

impl ProjectState {
    pub fn create_chart_resource(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        name: &ResourceName,
        database_id: Option<String>,
        operation_id: OperationId,
    ) -> Result<ProjectResourceMutationFacts, ProjectOperationError> {
        let (session, authority_generation, ()) =
            self.capture_writer_input(expected_project_instance_id, |_| ())?;
        let reservation =
            self.reserve_resource_operation(expected_project_instance_id, operation_id)?;
        let lease = self.filesystem().acquire(session.root.clone())?;
        let empty_context = context(
            self,
            session.clone(),
            operation_id,
            BTreeMap::new(),
            BTreeSet::new(),
        );
        self.validate_writer_context(&empty_context, authority_generation)?;
        let (chart_path, document) = {
            let current = self.project_data.read().unwrap();
            let existing = current.charts.keys().map(ChartResourcePath::display_name);
            let unique = allocate_unique_resource_name(name, existing);
            let chart_path = ChartResourcePath::from_name(&unique);
            let document = ChartDocument::new(
                database_id
                    .or_else(|| current.databases.keys().min().cloned())
                    .unwrap_or_default(),
            );
            (chart_path, document)
        };
        let mutation_context = context(
            self,
            session,
            operation_id,
            BTreeMap::new(),
            BTreeSet::from([chart_key(&chart_path)]),
        );
        let result = self.write_chart_patch(
            authority_generation,
            mutation_context,
            lease,
            chart_path,
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
        name: Option<String>,
    ) -> Result<ProjectResourceMutationFacts, ProjectOperationError> {
        let (session, authority_generation, ()) =
            self.capture_writer_input(expected_project_instance_id, |_| ())?;
        let reservation =
            self.reserve_resource_operation(expected_project_instance_id, operation_id)?;
        let lease = self.filesystem().acquire(session.root.clone())?;
        let (target, source_document) = {
            let current = self.project_data.read().unwrap();
            let source_document = current.charts.get(source).cloned().ok_or_else(|| {
                ProjectOperationError::ChartNotFound {
                    path: source.clone(),
                }
            })?;
            let existing = current.charts.keys().map(ChartResourcePath::display_name);
            let requested = name.as_deref().map(ResourceName::parse).transpose()?;
            let unique = allocate_unique_resource_name(
                requested.as_ref().unwrap_or_else(|| source.display_name()),
                existing,
            );
            (ChartResourcePath::from_name(&unique), source_document)
        };
        let mutation_context = context(
            self,
            session,
            operation_id,
            BTreeMap::from([(chart_key(source), expected_revision)]),
            BTreeSet::from([chart_key(&target)]),
        );
        let result = self.write_chart_patch(
            authority_generation,
            mutation_context,
            lease,
            target,
            source_document,
        );
        if result.is_ok() {
            reservation.complete();
        }
        result
    }

    fn write_chart_patch(
        &self,
        authority_generation: u64,
        context: ProjectTransactionContext,
        lease: yss_filesystem::FilesystemLeaseSet,
        chart_path: ChartResourcePath,
        document: ChartDocument,
    ) -> Result<ProjectResourceMutationFacts, ProjectOperationError> {
        self.validate_writer_context(&context, authority_generation)?;
        let (new_path, contents) =
            crate::serialize_chart(&chart_path, &document).map_err(prepare_error)?;
        let prepared = FilesystemTransaction::prepare_with_validator(
            context.filesystem_context(),
            lease,
            vec![StagedFilesystemMutation::Write {
                relative_path: new_path,
                contents,
            }],
            validate_document,
        )?;
        self.validate_writer_context(&context, authority_generation)?;
        let committed = prepared.commit()?;
        let result = match self.apply_project_resource_document_patch(
            &context,
            ProjectDataPatch::UpsertChart {
                path: chart_path,
                document,
            },
            None,
            Vec::new(),
        ) {
            Ok(result) => result,
            Err(error) => {
                return match committed.rollback() {
                    Ok(()) => Err(error),
                    Err(rollback_error) => Err(rollback_error.into()),
                };
            }
        };
        committed.finalize();
        Ok(result)
    }

    pub fn save_chart_document(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        chart_path: &ChartResourcePath,
        operation_id: OperationId,
        document: ChartDocument,
        requested_revision: Option<ResourceRevision>,
    ) -> Result<ProjectResourceMutationFacts, ProjectOperationError> {
        let (session, authority_generation, (present, captured_revision)) = self
            .capture_writer_input(expected_project_instance_id, |data| {
                (
                    data.charts.contains_key(chart_path),
                    self.chart_revisions
                        .read()
                        .unwrap()
                        .get(chart_path)
                        .copied(),
                )
            })?;
        let reservation =
            self.reserve_resource_operation(expected_project_instance_id, operation_id)?;
        let lease = self.filesystem().acquire(session.root.clone())?;
        if !present {
            return Err(ProjectOperationError::ChartNotFound {
                path: chart_path.clone(),
            });
        }
        // Explicit Save overwrites the resource; its transaction baseline is Rust-owned.
        let expected_revision = captured_revision.ok_or_else(|| {
            prepare_error(format!(
                "Chart '{}' has no resource revision",
                chart_path.as_str()
            ))
        })?;
        if requested_revision.is_some_and(|revision| revision != expected_revision) {
            return Err(ProjectOperationError::ResourceRevisionConflict {
                message: "chart changed after the editing baseline was captured".into(),
            });
        }
        let mutation_context = context(
            self,
            session,
            operation_id,
            BTreeMap::from([(chart_key(chart_path), expected_revision)]),
            BTreeSet::new(),
        );
        let result = self.write_chart_patch(
            authority_generation,
            mutation_context,
            lease,
            chart_path.clone(),
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
    ) -> Result<ProjectResourceMutationFacts, ProjectOperationError> {
        let (session, authority_generation, ()) =
            self.capture_writer_input(expected_project_instance_id, |_| ())?;
        let reservation =
            self.reserve_resource_operation(expected_project_instance_id, operation_id)?;
        let mut ownership = self.acquire_resource_rename_ownership(
            expected_project_instance_id,
            yss_resource_lifecycle::LifecycleResourcePath::Chart(chart_path.clone()),
            lifecycle_token,
        )?;
        let lease = self.filesystem().acquire(session.root.clone())?;
        self.validate_writer_context(
            &context(
                self,
                session.clone(),
                operation_id,
                BTreeMap::from([(chart_key(chart_path), expected_revision)]),
                BTreeSet::new(),
            ),
            authority_generation,
        )?;
        self.validate_resource_lifecycle_operation(&ownership.operation)?;

        let target = ChartResourcePath::from_name(new_name);
        let moved = {
            let current = self.project_data.read().unwrap();
            let moved = current.charts.get(chart_path).cloned().ok_or_else(|| {
                ProjectOperationError::ChartNotFound {
                    path: chart_path.clone(),
                }
            })?;
            if current.charts.keys().any(|existing| {
                existing != chart_path
                    && existing.display_name().portable_key() == new_name.portable_key()
            }) {
                return Err(ProjectOperationError::ResourceNameConflict {
                    message: format!("a chart named '{}' already exists", new_name.as_str()),
                });
            }
            moved
        };
        let mutation_context = context(
            self,
            session,
            operation_id,
            BTreeMap::from([(chart_key(chart_path), expected_revision)]),
            BTreeSet::from([chart_key(&target)]),
        );
        self.validate_writer_context(&mutation_context, authority_generation)?;
        let prepared = FilesystemTransaction::prepare_with_validator(
            mutation_context.filesystem_context(),
            lease,
            vec![StagedFilesystemMutation::MoveFile {
                from: chart_path.relative_path().to_path_buf(),
                to: target.relative_path().to_path_buf(),
            }],
            crate::project_writers::validate_document,
        )?;
        self.validate_writer_context(&mutation_context, authority_generation)?;
        self.validate_resource_lifecycle_operation(&ownership.operation)?;
        let committed = prepared.commit()?;
        let result = match self.apply_project_resource_document_patch(
            &mutation_context,
            ProjectDataPatch::MoveChart {
                from: chart_path.clone(),
                to: target,
                moved,
            },
            Some(&mut ownership),
            Vec::new(),
        ) {
            Ok(result) => result,
            Err(error) => {
                return match committed.rollback() {
                    Ok(()) => Err(error),
                    Err(rollback_error) => Err(rollback_error.into()),
                };
            }
        };
        committed.finalize();
        reservation.complete();
        Ok(result)
    }

    pub fn remove_chart_resource(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        chart_path: &ChartResourcePath,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
    ) -> Result<ProjectResourceMutationFacts, ProjectOperationError> {
        let (session, authority_generation, present) = self
            .capture_writer_input(expected_project_instance_id, |data| {
                data.charts.contains_key(chart_path)
            })?;
        let reservation =
            self.reserve_resource_operation(expected_project_instance_id, operation_id)?;
        let lease = self.filesystem().acquire(session.root.clone())?;
        if !present {
            return Err(ProjectOperationError::ChartNotFound {
                path: chart_path.clone(),
            });
        }
        let mutation_context = context(
            self,
            session,
            operation_id,
            BTreeMap::from([(chart_key(chart_path), expected_revision)]),
            BTreeSet::new(),
        );
        self.validate_writer_context(&mutation_context, authority_generation)?;
        let prepared = FilesystemTransaction::prepare_with_validator(
            mutation_context.filesystem_context(),
            lease,
            vec![StagedFilesystemMutation::RemoveFile {
                relative_path: chart_path.relative_path().to_path_buf(),
            }],
            crate::project_writers::validate_document,
        )?;
        self.validate_writer_context(&mutation_context, authority_generation)?;
        let committed = prepared.commit()?;
        let result = match self.apply_project_resource_document_patch(
            &mutation_context,
            ProjectDataPatch::RemoveChart {
                path: chart_path.clone(),
                revision: expected_revision,
            },
            None,
            Vec::new(),
        ) {
            Ok(result) => result,
            Err(error) => {
                return match committed.rollback() {
                    Ok(()) => Err(error),
                    Err(rollback_error) => Err(rollback_error.into()),
                };
            }
        };
        committed.finalize();
        reservation.complete();
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use yss_chart_document::ChartType;

    #[test]
    fn chart_rename_preserves_target_tombstone_and_rejects_stale_save() {
        let fixture = fixtures::TempProject::activate("chart-rename-tombstone", ProjectData::new());
        let state = fixture.state();
        let session = state.capture_project_session().unwrap();
        let target_name = ResourceName::parse("Previous").unwrap();
        let target = ChartResourcePath::from_name(&target_name);
        state
            .create_chart_resource(&session.instance_id, &target_name, None, OperationId::new())
            .unwrap();
        let mut old_draft = ChartDocument::new("old-database");
        old_draft.chart_type = ChartType::Scatter;
        let saved = state
            .save_chart_document(
                &session.instance_id,
                &target,
                OperationId::new(),
                old_draft.clone(),
                None,
            )
            .unwrap()
            .into_parts();
        let old_revision = saved.deltas[0].to_revision;
        let removed = state
            .remove_chart_resource(
                &session.instance_id,
                &target,
                old_revision,
                OperationId::new(),
            )
            .unwrap()
            .into_parts();
        let tombstone = removed.deltas[0].to_revision;

        let source_name = ResourceName::parse("Current").unwrap();
        let source = ChartResourcePath::from_name(&source_name);
        let created = state
            .create_chart_resource(
                &session.instance_id,
                &source_name,
                Some("current-database".into()),
                OperationId::new(),
            )
            .unwrap()
            .into_parts();
        let source_revision = created.deltas[0].to_revision;
        let source_document = state
            .load_chart_document(&session.instance_id, &source, None)
            .unwrap();
        let moved = state
            .rename_chart_resource(
                &session.instance_id,
                &source,
                source_revision,
                &target_name,
                1,
                OperationId::new(),
            )
            .unwrap()
            .into_parts();
        let stale_save = state.save_chart_document(
            &session.instance_id,
            &target,
            OperationId::new(),
            old_draft,
            Some(old_revision),
        );
        assert!(
            matches!(
                stale_save,
                Err(ProjectOperationError::ResourceRevisionConflict { .. })
            ),
            "renaming into a deleted path must not authorize its old draft: {stale_save:?}"
        );
        assert_eq!(moved.deltas[0].from_revision, source_revision);
        assert!(moved.deltas[0].to_revision > tombstone);
        assert_eq!(
            state.chart_revisions.read().unwrap()[&target],
            moved.deltas[0].to_revision
        );
        assert_eq!(
            state.chart_revisions.read().unwrap()[&source],
            source_revision.checked_next().unwrap()
        );
        assert!(!session.root.as_path().join(source.relative_path()).exists());
        let on_disk: ChartDocument = serde_json::from_slice(
            &std::fs::read(session.root.as_path().join(target.relative_path())).unwrap(),
        )
        .unwrap();
        assert_eq!(on_disk, source_document);
        assert_eq!(state.get_data().unwrap().charts[&target], source_document);
        assert!(matches!(
            state.save_chart_document(
                &session.instance_id,
                &source,
                OperationId::new(),
                source_document,
                Some(source_revision),
            ),
            Err(ProjectOperationError::ChartNotFound { .. })
        ));
    }

    #[test]
    fn chart_save_overwrites_an_older_draft_using_the_current_resource_revision() {
        let fixture = fixtures::TempProject::activate("chart-overwrite-save", ProjectData::new());
        let state = fixture.state();
        let session = state.capture_project_session().unwrap();
        let name = ResourceName::parse("Chart").unwrap();
        let path = ChartResourcePath::from_name(&name);
        state
            .create_chart_resource(&session.instance_id, &name, None, OperationId::new())
            .unwrap();
        let mut draft = state
            .load_chart_document(&session.instance_id, &path, None)
            .unwrap();
        let mut other = draft.clone();
        other.chart_type = ChartType::Scatter;
        let first = state
            .save_chart_document(&session.instance_id, &path, OperationId::new(), other, None)
            .unwrap()
            .into_parts();
        draft.chart_type = ChartType::Line;
        draft.encodings.x = Some("month".into());
        draft.encodings.y = Some("sales".into());
        let operation_id = OperationId::new();
        let saved = state
            .save_chart_document(
                &session.instance_id,
                &path,
                operation_id,
                draft.clone(),
                None,
            )
            .unwrap()
            .into_parts();

        assert_eq!(saved.operation_id, operation_id);
        assert!(saved.publication_revision > first.publication_revision);
        assert_eq!(saved.deltas[0].from_revision, first.deltas[0].to_revision);
        assert_eq!(saved.deltas[0].to_revision.get(), 2);
        let disk: ChartDocument = serde_json::from_slice(
            &std::fs::read(session.root.as_path().join(path.relative_path())).unwrap(),
        )
        .unwrap();
        assert_eq!(disk, draft);
        assert_eq!(state.get_data().unwrap().charts[&path], draft);
        assert!(matches!(
            state.load_chart_document(
                &session.instance_id,
                &path,
                Some(first.publication_revision)
            ),
            Err(ProjectOperationError::CatalogResourceStale { .. })
        ));
        assert_eq!(
            state
                .load_chart_document(
                    &session.instance_id,
                    &path,
                    Some(saved.publication_revision)
                )
                .unwrap(),
            draft
        );
    }
}
