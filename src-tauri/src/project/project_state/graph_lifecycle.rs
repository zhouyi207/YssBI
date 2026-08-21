use super::*;

#[path = "graph_load.rs"]
mod graph_load;
#[path = "graph_rename.rs"]
mod graph_rename;

pub(super) struct GraphRenameDiskPlan {
    pub(in crate::project::project_state) mutations: Vec<StagedFilesystemMutation>,
    pub(in crate::project::project_state) referenced_graphs_before:
        BTreeMap<GraphResourcePath, GraphResourceDocument>,
    pub(in crate::project::project_state) referenced_graphs_after:
        BTreeMap<GraphResourcePath, GraphResourceDocument>,
}

impl ProjectState {
    pub fn insert_graph(
        &self,
        path: GraphResourcePath,
        mut resource: GraphResourceDocument,
    ) -> Result<GraphResourceDocument, ProjectFilesystemError> {
        self.ensure_project_operational()?;
        validate_graph_resource(&path, &resource)?;
        let mut publication = self.mutation_publication.lock().unwrap();
        let mut data = self.project_data.write().unwrap();
        let mut graph_revisions = self.graph_revisions.write().unwrap();
        self.ensure_project_operational()?;
        let invalidation = CompileProductInvalidation::Graphs(vec![path.clone()]);
        let revision = normalize_function_resource_revision(
            &path,
            &mut resource,
            graph_revisions.get(&path).copied(),
        )?;
        let publication_advance = publication.prepare_authority_generation()?;
        let inserted = Self::install_validated_resident_graph(&mut data, path.clone(), resource);
        graph_revisions.insert(path, revision);
        publication.commit_prepared(publication_advance);
        self.apply_compile_product_invalidation(invalidation);
        Ok(inserted)
    }

    pub(super) fn install_validated_resident_graph(
        data: &mut ProjectData,
        path: GraphResourcePath,
        resource: GraphResourceDocument,
    ) -> GraphResourceDocument {
        data.graphs.insert(path, resource.clone());
        resource
    }

    pub(in crate::project) fn allocate_graph_path_from_snapshot(
        project_path: Option<&str>,
        data: &ProjectData,
        name: &str,
        kind: crate::project::GraphDocumentKind,
    ) -> Result<(GraphResourcePath, String), ProjectFilesystemError> {
        let persisted = if let Some(path) = project_path {
            let root = crate::project::project_root_from_path(path);
            crate::project::scan_graph_resource_index(&root)
                .map_err(|error| ProjectFilesystemError::TransactionPrepareFailed {
                    message: error.to_string(),
                })?
                .entries()
                .iter()
                .filter(|entry| entry.kind == kind)
                .map(|entry| entry.path.clone())
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let existing_names = data
            .graphs
            .iter()
            .filter(|(_, graph)| graph.kind == kind)
            .map(|(path, _)| path)
            .chain(persisted.iter())
            .map(|path| {
                crate::project::ResourceName::parse(path.display_name())
                    .expect("validated graph paths have validated display names")
            })
            .collect::<Vec<_>>();
        let requested = crate::project::ResourceName::parse(name)?;
        let allocated =
            crate::project::allocate_unique_resource_name(&requested, existing_names.iter());
        let path = match kind {
            crate::project::GraphDocumentKind::Event => GraphResourcePath::event(&allocated),
            crate::project::GraphDocumentKind::Function => GraphResourcePath::function(&allocated),
        };

        Ok((path, allocated.as_str().to_owned()))
    }

    pub fn unload_graph_resource(
        &self,
        graph_path: &GraphResourcePath,
    ) -> Result<(), ProjectFilesystemError> {
        self.ensure_project_operational()?;
        let graph_path_text = graph_path.as_str();
        let mut publication = self.mutation_publication.lock().unwrap();
        let mut data = self.project_data.write().unwrap();
        self.ensure_project_operational()?;
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
        Ok(())
    }

    pub(in crate::project) fn acquire_resource_rename_ownership(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        resource_path: crate::project::LifecycleResourcePath,
        lifecycle_token: u64,
    ) -> Result<ResourceRenameOwnershipLease, ProjectFilesystemError> {
        let session = self.capture_project_session()?;
        let publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != expected_project_instance_id.as_str()
            || session.instance_id != *expected_project_instance_id
        {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: format!(
                    "resource '{}' belongs to a different project instance",
                    resource_path
                ),
            });
        }
        let guard = self.resource_lifecycle.register(
            &session,
            &resource_path,
            lifecycle_token,
            ResourceLifecycleIntent::Rename,
        )?;
        drop(publication);
        let operation = ResourceLifecycleOperation::from_guard(session, &guard);
        Ok(ResourceRenameOwnershipLease::new(operation, guard))
    }

    pub(in crate::project) fn validate_resource_lifecycle_operation(
        &self,
        operation: &ResourceLifecycleOperation,
    ) -> Result<(), ProjectFilesystemError> {
        self.validate_project_session(&operation.session)?;
        self.resource_lifecycle.validate(&operation.owner)
    }
}
