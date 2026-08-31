use super::*;

impl ProjectState {
    pub fn flush_project_documents(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        operation_id: OperationId,
    ) -> Result<ProjectSaveResult, ProjectFilesystemError> {
        let snapshot = self.capture_writer_snapshot(expected_project_instance_id)?;
        let mut expected = BTreeMap::new();
        let mut mutations = vec![
            StagedFilesystemMutation::Write {
                relative_path: yss_project_layout::PROJECT_METADATA_FILE.into(),
                contents: crate::serialize_project_manifest(&snapshot.data)
                    .map_err(prepare_error)?,
            },
            StagedFilesystemMutation::Write {
                relative_path: yss_project_layout::GLOBAL_VARIABLES_FILE.into(),
                contents: crate::serialize_global_variables(&snapshot.data)
                    .map_err(prepare_error)?,
            },
        ];
        for (id, variable) in &snapshot.data.variables {
            if matches!(variable.scope, VariableScope::Global) {
                expected.insert(
                    variable_key(id),
                    snapshot
                        .variable_revisions
                        .get(id)
                        .map(|entry| entry.revision)
                        .unwrap_or(ResourceRevision::INITIAL),
                );
            }
        }
        let mut graph_paths = snapshot.data.graphs.keys().cloned().collect::<Vec<_>>();
        graph_paths.sort();
        for path in graph_paths {
            let resource = snapshot.data.graphs.get(&path).ok_or_else(|| {
                prepare_error(format!("graph '{path}' disappeared from the save snapshot"))
            })?;
            expected.insert(
                graph_key(&path),
                ResourceRevision::from_graph_revision(resource.document.revision),
            );
            if let Some(function) = &resource.function {
                if function.revision
                    != ResourceRevision::from_graph_revision(resource.document.revision)
                {
                    return Err(prepare_error(format!(
                        "function '{}' signature and graph revisions differ",
                        path
                    )));
                }
                expected.insert(function_key(&path), function.revision);
            }
            let (relative_path, contents) =
                crate::serialize_graph_document(&snapshot.data, &path).map_err(prepare_error)?;
            mutations.push(StagedFilesystemMutation::Write {
                relative_path,
                contents,
            });
        }
        let mut worksheet_paths = snapshot.data.worksheets.keys().cloned().collect::<Vec<_>>();
        worksheet_paths.sort();
        for path in worksheet_paths {
            let document = snapshot.data.worksheets.get(&path).ok_or_else(|| {
                prepare_error(format!(
                    "Worksheet '{}' disappeared from the save snapshot",
                    path.as_str()
                ))
            })?;
            expected.insert(worksheet_key(&path), document.revision);
            let (relative_path, contents) =
                crate::serialize_worksheet(&path, document).map_err(prepare_error)?;
            mutations.push(StagedFilesystemMutation::Write {
                relative_path,
                contents,
            });
        }
        let context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            expected,
            BTreeSet::new(),
        );
        self.execute_save(&snapshot, context, mutations)
    }

    pub fn save_graph_document(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
    ) -> Result<ProjectSaveResult, ProjectFilesystemError> {
        let snapshot = self.capture_writer_snapshot(expected_project_instance_id)?;
        let resource = snapshot
            .data
            .graphs
            .get(graph_path)
            .ok_or_else(|| prepare_error(format!("graph '{}' is not loaded", graph_path)))?;
        if resource.document.revision != expected_revision.to_graph_revision() {
            return Err(ProjectFilesystemError::ResourceRevisionConflict {
                message: format!("graph '{}' revision changed", graph_path),
            });
        }
        let mut expected = BTreeMap::from([(graph_key(graph_path), expected_revision)]);
        if let Some(function) = &resource.function {
            if function.revision != expected_revision {
                return Err(ProjectFilesystemError::ResourceRevisionConflict {
                    message: format!(
                        "function '{}' signature revision differs from graph",
                        graph_path
                    ),
                });
            }
            expected.insert(function_key(graph_path), expected_revision);
        }
        let (relative_path, contents) =
            crate::serialize_graph_document(&snapshot.data, graph_path).map_err(prepare_error)?;
        let context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            expected,
            BTreeSet::new(),
        );
        self.execute_save(
            &snapshot,
            context,
            vec![StagedFilesystemMutation::Write {
                relative_path,
                contents,
            }],
        )
    }
}
