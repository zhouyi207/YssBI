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
        let document = ChartDocument::new(
            database_id
                .or_else(|| current.databases.keys().min().cloned())
                .unwrap_or_default(),
        );
        let mutation_context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            BTreeMap::new(),
            BTreeSet::from([chart_key(&chart_path)]),
        );
        let result =
            self.write_chart_patch(&snapshot, mutation_context, lease, chart_path, document);
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
        let mutation_context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            BTreeMap::from([(chart_key(source), expected_revision)]),
            BTreeSet::from([chart_key(&target)]),
        );
        let result =
            self.write_chart_patch(&snapshot, mutation_context, lease, target, source_document);
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
        document: ChartDocument,
    ) -> Result<ProjectResourceMutationFacts, ProjectFilesystemError> {
        self.validate_writer_context(&context, snapshot.authority_generation)?;
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
        let result = match self.apply_project_resource_document_patch(
            &context,
            ProjectDataPatch::UpsertChart {
                path: chart_path,
                document,
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
        Ok(result)
    }

    pub fn save_chart_document(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        chart_path: &ChartResourcePath,
        operation_id: OperationId,
        document: ChartDocument,
    ) -> Result<ProjectResourceMutationFacts, ProjectFilesystemError> {
        let snapshot = self.capture_writer_snapshot(expected_project_instance_id)?;
        let reservation =
            self.reserve_resource_operation(expected_project_instance_id, operation_id)?;
        let lease = self.filesystem().acquire(snapshot.session.root.clone())?;
        if !snapshot.data.charts.contains_key(chart_path) {
            return Err(ProjectFilesystemError::ChartNotFound {
                path: chart_path.clone(),
            });
        }
        // Explicit Save overwrites the resource; its transaction baseline is Rust-owned.
        let expected_revision = snapshot
            .chart_revisions
            .get(chart_path)
            .copied()
            .ok_or_else(|| {
                prepare_error(format!(
                    "Chart '{}' has no resource revision",
                    chart_path.as_str()
                ))
            })?;
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
        let moved = current.charts.get(chart_path).cloned().ok_or_else(|| {
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
        let result = match self.apply_project_resource_document_patch(
            &mutation_context,
            ProjectDataPatch::MoveChart {
                from: chart_path.clone(),
                to: target,
                moved,
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
        if !snapshot.data.charts.contains_key(chart_path) {
            return Err(ProjectFilesystemError::ChartNotFound {
                path: chart_path.clone(),
            });
        }
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
        let result = match self.apply_project_resource_document_patch(
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
        reservation.complete();
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    #[test]
    fn chart_lifecycle_uses_project_receipts_without_persisted_revisions() {
        use yss_project_history::ResourceDocumentPatch;

        let fixture = fixtures::TempProject::activate("chart-lifecycle", ProjectData::new());
        let state = fixture.state();
        let session = state.capture_project_session().unwrap();
        let name = ResourceName::parse("Chart").unwrap();
        let path = ChartResourcePath::from_name(&name);
        let created = state
            .create_chart_resource(&session.instance_id, &name, None, OperationId::new())
            .unwrap()
            .into_parts();
        let duplicate = state
            .duplicate_chart_resource(
                &session.instance_id,
                &path,
                created.deltas[0].to_revision,
                OperationId::new(),
            )
            .unwrap()
            .into_parts();
        assert!(
            matches!(&duplicate.deltas[0].payload, ResourceDocumentPatch::ResourceLifecycle(patch) if patch.before.is_none() && patch.after.is_some())
        );
        let renamed = state
            .rename_chart_resource(
                &session.instance_id,
                &path,
                created.deltas[0].to_revision,
                &ResourceName::parse("Renamed").unwrap(),
                1,
                OperationId::new(),
            )
            .unwrap()
            .into_parts();
        let target = ChartResourcePath::parse("charts/Renamed.yssbi-chart").unwrap();
        assert!(
            matches!(&renamed.deltas[0].payload, ResourceDocumentPatch::ResourceMove(patch) if patch.from.as_ref() == path.as_str() && patch.to.as_ref() == target.as_str())
        );
        assert_eq!(
            renamed.deltas[0].from_revision,
            created.deltas[0].to_revision
        );
        let index = state.read_project_index(&session.instance_id).unwrap();
        assert_eq!(
            index
                .charts
                .iter()
                .find(|entry| entry.chart_path == target)
                .unwrap()
                .revision,
            renamed.deltas[0].to_revision
        );
        let bytes = std::fs::read(session.root.as_path().join(target.relative_path())).unwrap();
        assert!(
            serde_json::from_slice::<serde_json::Value>(&bytes)
                .unwrap()
                .get("revision")
                .is_none()
        );
        let removed = state
            .remove_chart_resource(
                &session.instance_id,
                &target,
                renamed.deltas[0].to_revision,
                OperationId::new(),
            )
            .unwrap()
            .into_parts();
        assert!(
            matches!(&removed.deltas[0].payload, ResourceDocumentPatch::ResourceLifecycle(patch) if patch.before.as_ref().is_some_and(|before| before.revision == renamed.deltas[0].to_revision) && patch.after.is_none())
        );
        assert!(!session.root.as_path().join(target.relative_path()).exists());
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
        other.chart_type = "scatter".into();
        let first = state
            .save_chart_document(&session.instance_id, &path, OperationId::new(), other)
            .unwrap()
            .into_parts();
        draft.chart_type = "line".into();
        draft.encodings.x = Some("month".into());
        draft.encodings.y = Some("sales".into());
        let operation_id = OperationId::new();
        let saved = state
            .save_chart_document(&session.instance_id, &path, operation_id, draft.clone())
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
            Err(ProjectFilesystemError::CatalogResourceStale { .. })
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
