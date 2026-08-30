use super::*;
use crate::project::resource_patch::ResourceDocumentPatch;
use yss_resource_naming::ResourceName;

impl ProjectState {
    pub(in crate::project) fn rename_graph_resource_transaction_impl(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        expected_revision: yss_project_identity::ResourceRevision,
        new_name: &str,
        lifecycle_token: u64,
        operation_id: yss_project_identity::OperationId,
    ) -> Result<crate::schema::application_event::ResourceMutationResultDto, ProjectFilesystemError>
    {
        self.ensure_project_operational()?;
        let mut ownership_lease = self.acquire_graph_rename_ownership(
            expected_project_instance_id,
            graph_path,
            lifecycle_token,
        )?;

        let ownership = ownership_lease.operation.clone();
        let root = ownership.session.root.clone();
        let project_path = root.as_path().to_string_lossy().into_owned();
        let filesystem_lease = self.filesystem().acquire(root.clone())?;
        self.validate_resource_lifecycle_operation(&ownership)?;

        let (loaded_source, loaded_source_variables, loaded_metadata) = {
            let data = self.project_data.read().unwrap();
            (
                data.graphs.get(graph_path).cloned(),
                data.variables
                    .iter()
                    .filter(|(_, variable)| {
                        variable_scope_references_path(&variable.scope, graph_path.as_str())
                    })
                    .map(|(id, variable)| (*id, variable.clone()))
                    .collect::<std::collections::HashMap<_, _>>(),
                data.graphs
                    .iter()
                    .map(|(path, resource)| (path.clone(), resource.name.clone(), resource.kind))
                    .collect::<Vec<_>>(),
            )
        };
        let source_was_loaded = loaded_source.is_some();

        let source_result = loaded_source.map_or_else(
            || {
                crate::project::project_io::load_project_graph_document_from_file(
                    &project_path,
                    graph_path,
                )
                .map(|document| {
                    let mut graph = document.document;
                    graph.revision = document.revision.to_graph_revision();
                    (
                        GraphResourceDocument {
                            name: document.name,
                            kind: document.kind,
                            document: graph,
                            function: document.function,
                        },
                        document.local_variables,
                    )
                })
            },
            |resource| Ok((resource, loaded_source_variables)),
        );
        let (mut moved, mut moved_local_variables) = match source_result {
            Ok(resource) => resource,
            Err(error) => {
                self.validate_resource_lifecycle_operation(&ownership)?;
                return Err(ProjectFilesystemError::TransactionPrepareFailed {
                    message: error.to_string(),
                });
            }
        };
        let mut allocation_data = ProjectData::new();
        for (path, name, kind) in loaded_metadata {
            allocation_data
                .graphs
                .insert(path, GraphResourceDocument::new(name, kind));
        }
        let allocation = Self::allocate_graph_rename_path_from_snapshot(
            &project_path,
            &allocation_data,
            graph_path,
            new_name,
            moved.kind,
        );
        let (target, unique_name) = match allocation {
            Ok(value) => value,
            Err(error) => {
                self.validate_resource_lifecycle_operation(&ownership)?;
                return Err(error);
            }
        };
        let moved_before = moved.clone();
        let source_revision = moved.document.revision;
        if ResourceRevision::from_graph_revision(source_revision) != expected_revision {
            return Err(ProjectFilesystemError::ResourceRevisionConflict {
                message: format!("revision for '{}' changed", graph_path),
            });
        }
        moved.name = unique_name;
        moved.document.revision = checked_graph_revision(graph_path.as_str(), source_revision)?;
        crate::project::resource_mutations::remap_graph_document_references(
            &mut moved.document,
            graph_path.as_str(),
            target.as_str(),
        );
        for variable in moved_local_variables.values_mut() {
            crate::project::resource_mutations::remap_variable_scope_path(
                &mut variable.scope,
                graph_path.as_str(),
                target.as_str(),
            );
        }

        let mut referenced_graphs_before = BTreeMap::new();
        let mut referenced_graphs = BTreeMap::new();
        let mut referenced_variables_before = BTreeMap::new();
        let mut referenced_variables = BTreeMap::new();
        let mut loaded_referenced_local_variables = BTreeMap::new();
        let mut expected_revisions = BTreeMap::new();
        let mut affected_resources = Vec::new();
        {
            let data = self.project_data.read().unwrap();
            let variable_revisions = self.variable_revisions.read().unwrap();
            let source_key = ResourceKey::Graph(graph_path.clone());
            if data.graphs.contains_key(graph_path) {
                affected_resources.push(source_key.clone());
            }
            expected_revisions.insert(
                source_key,
                ResourceRevision::from_graph_revision(source_revision),
            );
            for (path, resource) in &data.graphs {
                if path == graph_path
                    || !graph_document_references_path(&resource.document, graph_path.as_str())
                {
                    continue;
                }
                let mut changed = resource.clone();
                crate::project::resource_mutations::remap_graph_document_references(
                    &mut changed.document,
                    graph_path.as_str(),
                    target.as_str(),
                );
                changed.document.revision =
                    checked_graph_revision(path.as_str(), changed.document.revision)?;
                let key = ResourceKey::Graph(path.clone());
                affected_resources.push(key.clone());
                expected_revisions.insert(
                    key,
                    ResourceRevision::from_graph_revision(resource.document.revision),
                );
                referenced_graphs_before.insert(path.clone(), resource.clone());
                referenced_graphs.insert(path.clone(), changed);
            }
            for path in referenced_graphs.keys() {
                loaded_referenced_local_variables.insert(
                    path.clone(),
                    data.variables
                        .iter()
                        .filter(|(_, variable)| {
                            variable_scope_references_path(&variable.scope, path.as_str())
                        })
                        .map(|(id, variable)| (*id, variable.clone()))
                        .collect::<std::collections::HashMap<_, _>>(),
                );
            }
            for (id, variable) in &data.variables {
                if !variable_scope_references_path(&variable.scope, graph_path.as_str()) {
                    continue;
                }
                let mut changed = variable.clone();
                crate::project::resource_mutations::remap_variable_scope_path(
                    &mut changed.scope,
                    graph_path.as_str(),
                    target.as_str(),
                );
                let key = ResourceKey::Variable(yss_project_history::VariableResourceKey(
                    format!("variables/{id}").into(),
                ));
                affected_resources.push(key.clone());
                expected_revisions.insert(
                    key,
                    variable_revisions
                        .get(id)
                        .map(|entry| entry.revision)
                        .unwrap_or(yss_project_identity::ResourceRevision::INITIAL),
                );
                referenced_variables_before.insert(*id, variable.clone());
                referenced_variables.insert(*id, changed);
            }
        }
        if source_was_loaded {
            moved_local_variables = referenced_variables
                .iter()
                .map(|(id, variable)| (*id, variable.clone()))
                .collect();
        }
        let loaded_referenced_graphs = referenced_graphs.keys().cloned().collect();
        let known_graph_revisions = self.graph_revisions.read().unwrap().clone();
        let disk_plan = match Self::graph_rename_mutations(
            root.as_path(),
            graph_path,
            &target,
            &moved,
            moved_local_variables,
            &loaded_referenced_graphs,
            &known_graph_revisions,
        ) {
            Ok(plan) => plan,
            Err(message) => {
                self.validate_resource_lifecycle_operation(&ownership)?;
                return Err(ProjectFilesystemError::TransactionPrepareFailed { message });
            }
        };
        for (path, before) in disk_plan.referenced_graphs_before {
            let key = ResourceKey::Graph(path.clone());
            affected_resources.push(key.clone());
            expected_revisions.insert(
                key,
                ResourceRevision::from_graph_revision(before.document.revision),
            );
            referenced_graphs_before.insert(path, before);
        }
        referenced_graphs.extend(disk_plan.referenced_graphs_after);
        let context = ProjectTransactionContext {
            session: ProjectSession {
                instance_id: ownership.session.instance_id.clone(),
                root: root.clone(),
            },
            operation_id,
            affected_resources,
            expected_revisions,
            expected_absent_resources: [ResourceKey::Graph(target.clone())].into_iter().collect(),
            recovery_marker: Some(self.project_recovery_marker()),
        };
        let mut mutations = disk_plan.mutations;
        for path in &loaded_referenced_graphs {
            let resource = referenced_graphs
                .get(path)
                .expect("loaded referenced graph remains in the rename patch");
            let local_variables = loaded_referenced_local_variables
                .remove(path)
                .unwrap_or_default();
            let contents = crate::project::project_io::serialize_graph_resource_document(
                resource,
                local_variables,
            )
            .map_err(|error| ProjectFilesystemError::TransactionPrepareFailed {
                message: error.to_string(),
            })?;
            mutations.push(StagedFilesystemMutation::Write {
                relative_path: path.as_str().into(),
                contents,
            });
        }
        self.validate_resource_lifecycle_operation(&ownership)?;
        let prepared = ProjectFilesystemTransaction::prepare_with_validator(
            context.clone(),
            filesystem_lease,
            mutations,
            |path, contents| {
                if path == std::path::Path::new(yss_project_layout::GLOBAL_VARIABLES_FILE) {
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
        )?;
        self.validate_resource_lifecycle_operation(&ownership)?;
        let projection_environment = self
            .capture_projection_environment_for_session(&context.session)
            .map_err(|message| ProjectFilesystemError::TransactionPrepareFailed { message })?;
        #[cfg(test)]
        self.run_resource_mutation_test_hook(
            crate::project::resource_mutations::ResourceMutationTestPoint::Prepared,
            Some(&target),
        );
        let committed = prepared.commit()?;
        #[cfg(test)]
        self.run_resource_mutation_test_hook(
            crate::project::resource_mutations::ResourceMutationTestPoint::Committed,
            Some(&target),
        );
        #[cfg(test)]
        if let Some(checkpoint) = self
            .test_hooks
            .graph_rename_io_checkpoint
            .read()
            .unwrap()
            .clone()
        {
            checkpoint();
        }
        #[cfg(test)]
        self.run_resource_mutation_test_hook(
            crate::project::resource_mutations::ResourceMutationTestPoint::BeforePublication,
            Some(&target),
        );
        let publication = self
            .validate_resource_lifecycle_operation(&ownership)
            .and_then(|_| {
                self.apply_resource_document_patch_with_environment(
                    &context,
                    ResourceDocumentPatch::MoveGraph {
                        from: graph_path.clone(),
                        to: target.clone(),
                        moved_before,
                        moved,
                        referenced_graphs_before,
                        referenced_graphs,
                        loaded_referenced_graphs,
                        referenced_variables_before,
                        referenced_variables,
                    },
                    projection_environment,
                    Some(&mut ownership_lease),
                )
            });
        match publication {
            Ok(result) => {
                committed.finalize();
                Ok(result)
            }
            Err(error) => match committed.rollback() {
                Ok(()) => Err(error),
                Err(rollback_error) => Err(rollback_error),
            },
        }
    }

    pub(in crate::project::project_state) fn graph_rename_mutations(
        root: &std::path::Path,
        source: &GraphResourcePath,
        target: &GraphResourcePath,
        moved: &GraphResourceDocument,
        moved_local_variables: std::collections::HashMap<
            yss_variable_contract::VariableId,
            yss_variable_contract::VariableInstance,
        >,
        excluded_graphs: &std::collections::BTreeSet<GraphResourcePath>,
        known_revisions: &std::collections::HashMap<
            GraphResourcePath,
            yss_graph_document::GraphRevision,
        >,
    ) -> Result<GraphRenameDiskPlan, String> {
        let mut plan = GraphRenameDiskPlan {
            mutations: Vec::new(),
            referenced_graphs_before: BTreeMap::new(),
            referenced_graphs_after: BTreeMap::new(),
        };
        for entry in crate::project::scan_graph_resource_index(root)
            .map_err(|error| error.to_string())?
            .entries()
        {
            if entry.path == *source || excluded_graphs.contains(&entry.path) {
                continue;
            }
            let relative_path = std::path::PathBuf::from(entry.path.as_str());
            let contents = crate::project::read_secure_project_file(root, &relative_path)
                .map_err(|error| error.to_string())?;
            let before: crate::project::project_io::GraphDocument =
                serde_json::from_slice(&contents).map_err(|error| error.to_string())?;
            let mut after = before.clone();
            let mut changed = crate::project::resource_mutations::remap_graph_document_references(
                &mut after.document,
                source.as_str(),
                target.as_str(),
            );
            for variable in after.local_variables.values_mut() {
                changed = crate::project::resource_mutations::remap_variable_scope_path(
                    &mut variable.scope,
                    source.as_str(),
                    target.as_str(),
                ) || changed;
            }
            if !changed {
                continue;
            }
            let before_revision = known_revisions
                .get(&entry.path)
                .copied()
                .unwrap_or(before.document.revision);
            after.document.revision = checked_graph_revision(entry.path.as_str(), before_revision)
                .map_err(|error| error.to_string())?;
            let mut before_document = before.document;
            before_document.revision = before_revision;
            plan.referenced_graphs_before.insert(
                entry.path.clone(),
                GraphResourceDocument {
                    name: before.name,
                    kind: before.kind,
                    document: before_document,
                    function: before.function,
                },
            );
            plan.referenced_graphs_after.insert(
                entry.path.clone(),
                GraphResourceDocument {
                    name: after.name.clone(),
                    kind: after.kind,
                    document: after.document.clone(),
                    function: after.function.clone(),
                },
            );
            plan.mutations.push(StagedFilesystemMutation::Write {
                relative_path,
                contents: serde_json::to_vec_pretty(&after).map_err(|error| error.to_string())?,
            });
        }
        let variables = std::path::PathBuf::from(yss_project_layout::GLOBAL_VARIABLES_FILE);
        match crate::project::read_secure_project_file(root, &variables) {
            Ok(contents) => {
                let mut document: crate::project::project_io::GlobalVariablesDocument =
                    serde_json::from_slice(&contents).map_err(|error| error.to_string())?;
                let changed = document
                    .variables
                    .values_mut()
                    .fold(false, |changed, variable| {
                        crate::project::resource_mutations::remap_variable_scope_path(
                            &mut variable.scope,
                            source.as_str(),
                            target.as_str(),
                        ) || changed
                    });
                if changed {
                    plan.mutations.push(StagedFilesystemMutation::Write {
                        relative_path: variables,
                        contents: serde_json::to_vec_pretty(&document)
                            .map_err(|error| error.to_string())?,
                    });
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        plan.mutations.push(StagedFilesystemMutation::Write {
            relative_path: target.as_str().into(),
            contents: crate::project::project_io::serialize_graph_resource_document(
                moved,
                moved_local_variables,
            )
            .map_err(|error| error.to_string())?,
        });
        plan.mutations.push(StagedFilesystemMutation::RemoveFile {
            relative_path: source.as_str().into(),
        });
        Ok(plan)
    }

    fn allocate_graph_rename_path_from_snapshot(
        project_path: &str,
        data: &ProjectData,
        source: &GraphResourcePath,
        name: &str,
        kind: crate::project::GraphDocumentKind,
    ) -> Result<(GraphResourcePath, String), ProjectFilesystemError> {
        let requested = ResourceName::parse(name)?;
        let root = crate::project::project_root_from_path(project_path);
        let persisted = crate::project::scan_graph_resource_index(&root)
            .map_err(|error| ProjectFilesystemError::TransactionPrepareFailed {
                message: error.to_string(),
            })?
            .entries()
            .iter()
            .filter(|entry| entry.kind == kind)
            .map(|entry| entry.path.clone())
            .collect::<Vec<_>>();
        let conflicts = data
            .graphs
            .iter()
            .filter(|(path, graph)| *path != source && graph.kind == kind)
            .map(|(path, _)| path)
            .chain(persisted.iter().filter(|path| *path != source))
            .any(|path| {
                ResourceName::parse(path.display_name())
                    .expect("validated graph paths have validated display names")
                    .portable_key()
                    == requested.portable_key()
            });
        if conflicts {
            return Err(ProjectFilesystemError::ResourceNameConflict {
                message: format!(
                    "a {} named '{}' already exists",
                    match kind {
                        crate::project::GraphDocumentKind::Event => "event",
                        crate::project::GraphDocumentKind::Function => "function",
                    },
                    requested.as_str()
                ),
            });
        }
        let target = match kind {
            crate::project::GraphDocumentKind::Event => GraphResourcePath::new(format!(
                "{}/{}.{}",
                yss_project_layout::EVENTS_DIR,
                requested.as_str(),
                yss_project_layout::EVENT_EXTENSION
            )),
            crate::project::GraphDocumentKind::Function => GraphResourcePath::new(format!(
                "{}/{}.{}",
                yss_project_layout::FUNCTIONS_DIR,
                requested.as_str(),
                yss_project_layout::FUNCTION_EXTENSION
            )),
        }
        .map_err(|error| ProjectFilesystemError::TransactionPrepareFailed {
            message: error.to_string(),
        })?;
        Ok((target, requested.as_str().to_owned()))
    }

    fn acquire_graph_rename_ownership(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        lifecycle_token: u64,
    ) -> Result<ResourceRenameOwnershipLease, ProjectFilesystemError> {
        self.acquire_resource_rename_ownership(
            expected_project_instance_id,
            crate::project::LifecycleResourcePath::Graph(graph_path.clone()),
            lifecycle_token,
        )
    }
}
