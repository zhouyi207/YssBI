use std::collections::{BTreeMap, HashMap};

use crate::{ProjectState, ResourceLifecycleOperation};

use yss_graph_document::{
    ConnectionId, DynamicMemberLocator, DynamicPortBinding, GraphDocument, GraphResourcePath,
    NodeId, PortAddress, PortInstanceId, PortRef,
};
use yss_project_filesystem::{
    ProjectFilesystemError, ProjectFilesystemTransaction, StagedFilesystemMutation,
};
use yss_project_identity::{ProjectInstanceId, ResourceRevision};
use yss_project_model::{GraphResourceDocument, ProjectDataPatch};
use yss_resource_lifecycle::{LifecycleResourcePath, ResourceLifecycleIntent};
use yss_resource_naming::{ResourceName, allocate_unique_resource_name};

use crate::project_writers::{ProjectResourceMutationFacts, context};

impl ProjectState {
    pub fn read_graph_resource_snapshot(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
    ) -> Result<GraphResourceDocument, ProjectFilesystemError> {
        self.ensure_project_operational()?;
        let session = self.capture_project_session()?;
        if &session.instance_id != expected_project_instance_id {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "graph read project instance is stale".into(),
            });
        }
        let resident = self.get_data()?.graphs.get(graph_path).cloned();
        let resource = match resident {
            Some(resource) => resource,
            None => crate::project_io::load_project_graph_from_file(
                session.root.as_path().to_string_lossy().as_ref(),
                graph_path,
            )
            .map_err(|error| ProjectFilesystemError::TransactionPrepareFailed {
                message: error.to_string(),
            })?,
        };
        self.validate_project_session(&session)?;
        Ok(resource)
    }

    pub fn create_graph_resource(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        name: &str,
        mut resource: GraphResourceDocument,
        operation_id: yss_project_identity::OperationId,
    ) -> Result<ProjectResourceMutationFacts, ProjectFilesystemError> {
        let snapshot = self.capture_writer_snapshot(expected_project_instance_id)?;
        let reservation =
            self.reserve_resource_operation(expected_project_instance_id, operation_id)?;
        let lease = self.filesystem().acquire(snapshot.session.root.clone())?;
        // Application has already allocated the shell's identity. Never silently rename it here.
        let name = ResourceName::parse(name)?;
        let path = renamed_graph_path(&name, resource.kind)?;
        let mutation_context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            BTreeMap::new(),
            [yss_project_history::ResourceKey::Graph(path.clone())].into(),
        );
        self.validate_writer_context(&mutation_context, snapshot.authority_generation)?;
        let revision = snapshot
            .graph_resource_revisions
            .get(&path)
            .copied()
            .map(|retained| super::checked_resource_revision(path.as_str(), retained))
            .transpose()?
            .unwrap_or(ResourceRevision::INITIAL);
        resource.name = name.as_str().to_owned();
        if let Some(function) = resource.function.as_mut() {
            function.revision = revision;
        }
        let contents =
            crate::project_io::serialize_graph_resource_document(&resource).map_err(|error| {
                ProjectFilesystemError::TransactionPrepareFailed {
                    message: error.to_string(),
                }
            })?;
        let prepared = ProjectFilesystemTransaction::prepare_with_validator(
            mutation_context.filesystem_context(),
            lease,
            vec![StagedFilesystemMutation::Write {
                relative_path: path.as_str().into(),
                contents,
            }],
            |_, bytes| {
                serde_json::from_slice::<crate::GraphResourceFile>(bytes)
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            },
        )?;
        self.validate_writer_context(&mutation_context, snapshot.authority_generation)?;
        let committed = prepared.commit()?;
        match self.apply_project_resource_document_patch(
            &mutation_context,
            ProjectDataPatch::InsertGraph { path, resource },
            None,
        ) {
            Ok(result) => {
                committed.finalize();
                reservation.complete();
                Ok(result)
            }
            Err(error) => {
                committed.rollback()?;
                Err(error)
            }
        }
    }

    pub fn duplicate_graph_resource(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        source_path: &GraphResourcePath,
        expected_revision: ResourceRevision,
        operation_id: yss_project_identity::OperationId,
    ) -> Result<ProjectResourceMutationFacts, ProjectFilesystemError> {
        let snapshot = self.capture_writer_snapshot(expected_project_instance_id)?;
        let reservation =
            self.reserve_resource_operation(expected_project_instance_id, operation_id)?;
        let lease = self.filesystem().acquire(snapshot.session.root.clone())?;
        let mut mutation_context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            [(
                yss_project_history::ResourceKey::Graph(source_path.clone()),
                expected_revision,
            )]
            .into(),
            Default::default(),
        );
        self.validate_writer_context(&mutation_context, snapshot.authority_generation)?;
        let source = match snapshot.data.graphs.get(source_path) {
            Some(resource) => resource.clone(),
            None => crate::project_io::load_project_graph_from_file(
                snapshot.session.root.as_path().to_string_lossy().as_ref(),
                source_path,
            )
            .map_err(|error| ProjectFilesystemError::TransactionPrepareFailed {
                message: error.to_string(),
            })?,
        };
        let (target, name) = Self::allocate_graph_path_from_snapshot(
            snapshot.session.root.as_path().to_str(),
            &snapshot.data,
            &format!("{} Copy", source.name),
            source.kind,
        )?;
        mutation_context
            .expected_absent_resources
            .insert(yss_project_history::ResourceKey::Graph(target.clone()));
        let revision = snapshot
            .graph_resource_revisions
            .get(&target)
            .copied()
            .map(|retained| super::checked_resource_revision(target.as_str(), retained))
            .transpose()?
            .unwrap_or(ResourceRevision::INITIAL);
        let mut duplicate = source;
        duplicate.name = name;
        duplicate.document = duplicate_document(&duplicate.document, source_path, &target);
        if let Some(function) = duplicate.function.as_mut() {
            function.revision = revision;
        }
        let contents =
            crate::project_io::serialize_graph_resource_document(&duplicate).map_err(|error| {
                ProjectFilesystemError::TransactionPrepareFailed {
                    message: error.to_string(),
                }
            })?;
        let prepared = ProjectFilesystemTransaction::prepare(
            mutation_context.filesystem_context(),
            lease,
            vec![StagedFilesystemMutation::Write {
                relative_path: target.as_str().into(),
                contents,
            }],
        )?;
        self.validate_writer_context(&mutation_context, snapshot.authority_generation)?;
        let committed = prepared.commit()?;
        match self.apply_project_resource_document_patch(
            &mutation_context,
            ProjectDataPatch::DeclareGraph {
                path: target,
                revision,
            },
            None,
        ) {
            Ok(result) => {
                committed.finalize();
                reservation.complete();
                Ok(result)
            }
            Err(error) => {
                committed.rollback()?;
                Err(error)
            }
        }
    }

    pub fn remove_graph_resource(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        expected_revision: ResourceRevision,
        operation_id: yss_project_identity::OperationId,
    ) -> Result<ProjectResourceMutationFacts, ProjectFilesystemError> {
        let snapshot = self.capture_writer_snapshot(expected_project_instance_id)?;
        let reservation =
            self.reserve_resource_operation(expected_project_instance_id, operation_id)?;
        let lease = self.filesystem().acquire(snapshot.session.root.clone())?;
        let mutation_context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            [(
                yss_project_history::ResourceKey::Graph(graph_path.clone()),
                expected_revision,
            )]
            .into(),
            Default::default(),
        );
        self.validate_writer_context(&mutation_context, snapshot.authority_generation)?;
        let prepared = ProjectFilesystemTransaction::prepare(
            mutation_context.filesystem_context(),
            lease,
            vec![StagedFilesystemMutation::RemoveFile {
                relative_path: graph_path.as_str().into(),
            }],
        )?;
        self.validate_writer_context(&mutation_context, snapshot.authority_generation)?;
        let committed = prepared.commit()?;
        match self.apply_project_resource_document_patch(
            &mutation_context,
            ProjectDataPatch::RemoveGraph {
                path: graph_path.clone(),
                revision: expected_revision,
            },
            None,
        ) {
            Ok(result) => {
                committed.finalize();
                reservation.complete();
                Ok(result)
            }
            Err(error) => {
                committed.rollback()?;
                Err(error)
            }
        }
    }

    pub fn load_graph_document(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        lifecycle_token: u64,
    ) -> Result<GraphDocument, ProjectFilesystemError> {
        let session = self.capture_project_session()?;
        if &session.instance_id != expected_project_instance_id {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "graph load project instance is stale".into(),
            });
        }
        if let Some(document) = self
            .project_data
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .graphs
            .get(graph_path)
            .map(|resource| resource.document.clone())
        {
            return Ok(document);
        }

        let mut lifecycle_guard = self.resource_lifecycle.register(
            &session.instance_id,
            graph_path,
            lifecycle_token,
            ResourceLifecycleIntent::Load,
        )?;
        let operation = ResourceLifecycleOperation::from_guard(session.clone(), &lifecycle_guard);
        let _filesystem_lease = self.filesystem().acquire(session.root.clone())?;
        self.validate_resource_lifecycle_operation(&operation)?;
        let loaded = crate::project_io::load_project_graph_document_from_file(
            session.root.as_path().to_string_lossy().as_ref(),
            graph_path,
        )
        .map_err(|error| ProjectFilesystemError::TransactionPrepareFailed {
            message: error.to_string(),
        })?;
        self.run_graph_load_after_read_test_hook();
        self.validate_resource_lifecycle_operation(&operation)?;

        let resource = GraphResourceDocument {
            name: loaded.name,
            kind: loaded.kind,
            document: loaded.document,
            function: loaded.function,
        };

        let mut publication = self
            .mutation_publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if publication.project_instance_id != operation.session.instance_id.as_str() {
            return Err(operation.stale_error());
        }
        let mut lifecycle = self.resource_lifecycle.boundary();
        lifecycle.validate(&operation.owner)?;
        self.ensure_project_operational()?;
        let mut data = self
            .project_data
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut graph_resource_revisions = self
            .graph_resource_revisions
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let publication_advance = publication.prepare_authority_generation()?;
        lifecycle.commit_guard(&mut lifecycle_guard, ResourceLifecycleIntent::Load)?;
        Self::install_validated_resident_graph(&mut data, graph_path.clone(), resource);
        graph_resource_revisions
            .entry(graph_path.clone())
            .or_insert(ResourceRevision::INITIAL);
        publication.commit_prepared(publication_advance);
        drop(graph_resource_revisions);
        drop(data);
        drop(lifecycle);
        drop(publication);

        self.project_data
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .graphs
            .get(graph_path)
            .map(|resource| resource.document.clone())
            .ok_or_else(|| ProjectFilesystemError::TransactionCommitFailed {
                message: "graph load committed without a resident document".into(),
            })
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
                message: "graph unload project instance is stale".into(),
            });
        }
        let mut guard = self.resource_lifecycle.register(
            &session.instance_id,
            graph_path,
            token,
            ResourceLifecycleIntent::Unload,
        )?;
        let operation = ResourceLifecycleOperation::from_guard(session, &guard);
        self.validate_resource_lifecycle_operation(&operation)?;

        let mut publication = self
            .mutation_publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if publication.project_instance_id != expected_project_instance_id.as_str() {
            return Err(operation.stale_error());
        }
        let mut lifecycle = self.resource_lifecycle.boundary();
        lifecycle.validate(&operation.owner)?;
        self.ensure_project_operational()?;
        let mut data = self
            .project_data
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let graph_removed = data.graphs.remove(graph_path).is_some();
        let publication_advance = graph_removed
            .then(|| publication.prepare_authority_generation())
            .transpose()?;
        lifecycle.commit_guard(&mut guard, ResourceLifecycleIntent::Unload)?;
        if let Some(publication_advance) = publication_advance {
            publication.commit_prepared(publication_advance);
        }
        Ok(graph_removed)
    }

    pub(super) fn install_validated_resident_graph(
        data: &mut yss_project_model::ProjectData,
        path: GraphResourcePath,
        resource: GraphResourceDocument,
    ) -> GraphResourceDocument {
        data.graphs.insert(path, resource.clone());
        resource
    }

    pub fn allocate_graph_path(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        name: &str,
        kind: yss_graph_document::GraphResourceKind,
    ) -> Result<(GraphResourcePath, String), ProjectFilesystemError> {
        let session = self.capture_project_session()?;
        if &session.instance_id != expected_project_instance_id {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "graph allocation project instance is stale".into(),
            });
        }
        let data = self.get_data()?;
        Self::allocate_graph_path_from_snapshot(session.root.as_path().to_str(), &data, name, kind)
    }

    pub(crate) fn allocate_graph_path_from_snapshot(
        project_path: Option<&str>,
        data: &yss_project_model::ProjectData,
        name: &str,
        kind: yss_graph_document::GraphResourceKind,
    ) -> Result<(GraphResourcePath, String), ProjectFilesystemError> {
        let persisted = project_path
            .map(|path| {
                let root = yss_project_filesystem::project_root_from_path(path);
                crate::scan_graph_resource_index(&root)
                    .map_err(|error| ProjectFilesystemError::TransactionPrepareFailed {
                        message: error.to_string(),
                    })
                    .map(|index| {
                        index
                            .entries()
                            .iter()
                            .filter(|entry| entry.kind == kind)
                            .map(|entry| entry.path.clone())
                            .collect::<Vec<_>>()
                    })
            })
            .transpose()?
            .unwrap_or_default();
        let existing = data
            .graphs
            .iter()
            .filter(|(_, graph)| graph.kind == kind)
            .map(|(path, _)| path)
            .chain(persisted.iter())
            .map(|path| ResourceName::parse(path.display_name()))
            .collect::<Result<Vec<_>, _>>()?;
        let requested = ResourceName::parse(name)?;
        let allocated = allocate_unique_resource_name(&requested, existing.iter());
        let (directory, extension) = match kind {
            yss_graph_document::GraphResourceKind::Event => (
                yss_project_layout::EVENTS_DIR,
                yss_project_layout::EVENT_EXTENSION,
            ),
            yss_graph_document::GraphResourceKind::Function => (
                yss_project_layout::FUNCTIONS_DIR,
                yss_project_layout::FUNCTION_EXTENSION,
            ),
        };
        let path =
            GraphResourcePath::new(format!("{directory}/{}.{extension}", allocated.as_str()))
                .map_err(|error| ProjectFilesystemError::TransactionPrepareFailed {
                    message: error.to_string(),
                })?;
        Ok((path, allocated.as_str().to_owned()))
    }

    pub fn unload_graph_resource(
        &self,
        graph_path: &GraphResourcePath,
    ) -> Result<(), ProjectFilesystemError> {
        let project_instance_id = self.capture_project_session()?.instance_id;
        let _ = self.unload_graph_resource_for_lifecycle(
            &project_instance_id,
            graph_path,
            self.next_lifecycle_token(graph_path)?,
        )?;
        Ok(())
    }

    fn next_lifecycle_token(
        &self,
        graph_path: &GraphResourcePath,
    ) -> Result<u64, ProjectFilesystemError> {
        let session = self.capture_project_session()?;
        let guard = self.resource_lifecycle.allocate_and_register(
            &session.instance_id,
            graph_path,
            ResourceLifecycleIntent::Unload,
        )?;
        let token = guard.owner().token;
        drop(guard);
        Ok(token)
    }

    pub(crate) fn acquire_resource_rename_ownership(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        resource_path: LifecycleResourcePath,
        lifecycle_token: u64,
    ) -> Result<crate::ResourceRenameOwnershipLease, ProjectFilesystemError> {
        let session = self.capture_project_session()?;
        if &session.instance_id != expected_project_instance_id {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "resource rename project instance is stale".into(),
            });
        }
        let guard = self.resource_lifecycle.register(
            &session.instance_id,
            resource_path,
            lifecycle_token,
            ResourceLifecycleIntent::Rename,
        )?;
        let operation = ResourceLifecycleOperation::from_guard(session, &guard);
        Ok(crate::ResourceRenameOwnershipLease::new(operation, guard))
    }

    pub(crate) fn validate_resource_lifecycle_operation(
        &self,
        operation: &ResourceLifecycleOperation,
    ) -> Result<(), ProjectFilesystemError> {
        self.validate_project_session(&operation.session)?;
        Ok(self.resource_lifecycle.validate(&operation.owner)?)
    }

    pub fn rename_graph_resource(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        expected_revision: ResourceRevision,
        new_name: &str,
        lifecycle_token: u64,
        operation_id: yss_project_identity::OperationId,
    ) -> Result<ProjectResourceMutationFacts, ProjectFilesystemError> {
        self.ensure_project_operational()?;
        let snapshot = self.capture_writer_snapshot(expected_project_instance_id)?;
        let session = snapshot.session.clone();
        let reservation =
            self.reserve_resource_operation(expected_project_instance_id, operation_id)?;
        let mut ownership = self.acquire_resource_rename_ownership(
            expected_project_instance_id,
            LifecycleResourcePath::Graph(graph_path.clone()),
            lifecycle_token,
        )?;
        self.validate_resource_lifecycle_operation(&ownership.operation)?;
        let current_data = &snapshot.data;
        let mut source = if let Some(resource) = current_data.graphs.get(graph_path) {
            resource.clone()
        } else {
            let persisted = crate::project_io::load_project_graph_document_from_file(
                session.root.as_path().to_string_lossy().as_ref(),
                graph_path,
            )
            .map_err(|error| ProjectFilesystemError::TransactionPrepareFailed {
                message: error.to_string(),
            })?;
            GraphResourceDocument {
                name: persisted.name,
                kind: persisted.kind,
                document: persisted.document,
                function: persisted.function,
            }
        };
        let current_revision = snapshot
            .graph_resource_revisions
            .get(graph_path)
            .copied()
            .unwrap_or(ResourceRevision::INITIAL);
        if current_revision != expected_revision {
            return Err(ProjectFilesystemError::ResourceRevisionConflict {
                message: format!("graph '{}' revision changed", graph_path),
            });
        }

        let requested = ResourceName::parse(new_name)?;
        let target = renamed_graph_path(&requested, source.kind)?;
        let target_name_key = requested.portable_key();
        if current_data.graphs.iter().any(|(path, resource)| {
            path != graph_path
                && resource.kind == source.kind
                && ResourceName::parse(path.display_name())
                    .is_ok_and(|name| name.portable_key() == target_name_key)
        }) {
            return Err(ProjectFilesystemError::ResourceNameConflict {
                message: format!("a graph named '{}' already exists", requested.as_str()),
            });
        }
        if std::fs::symlink_metadata(session.root.as_path().join(target.as_str())).is_ok() {
            return Err(ProjectFilesystemError::ResourceNameConflict {
                message: format!("a graph named '{}' already exists", requested.as_str()),
            });
        }

        let retained_target = snapshot
            .graph_resource_revisions
            .get(&target)
            .copied()
            .unwrap_or(ResourceRevision::INITIAL);
        let next_revision = current_revision
            .max(retained_target)
            .checked_next()
            .map_err(|error| ProjectFilesystemError::ResourceRevisionOverflow {
                resource: graph_path.as_str().to_owned(),
                retained: error.retained,
            })?;
        source.name = requested.as_str().to_owned();
        if let Some(function) = source.function.as_mut() {
            function.revision = next_revision;
        }

        let target_contents = crate::project_io::serialize_graph_resource_document(&source)
            .map_err(|error| ProjectFilesystemError::TransactionPrepareFailed {
                message: error.to_string(),
            })?;

        let mut referenced = Vec::new();
        for (path, resource) in &current_data.graphs {
            if path == graph_path || !document_references(&resource.document, graph_path.as_str()) {
                continue;
            }
            let mut changed = resource.clone();
            if !remap_document_references(
                &mut changed.document,
                graph_path.as_str(),
                target.as_str(),
            ) {
                continue;
            }
            super::normalize_function_resource_revision(
                path,
                &mut changed,
                snapshot.graph_resource_revisions.get(path).copied(),
            )?;
            let contents = crate::project_io::serialize_graph_resource_document(&changed).map_err(
                |error| ProjectFilesystemError::TransactionPrepareFailed {
                    message: error.to_string(),
                },
            )?;
            referenced.push((path.clone(), changed, contents));
        }

        let filesystem_lease = self.filesystem().acquire(session.root.clone())?;
        self.validate_resource_lifecycle_operation(&ownership.operation)?;
        let expected_revisions = std::iter::once((
            yss_project_history::ResourceKey::Graph(graph_path.clone()),
            expected_revision,
        ))
        .chain(referenced.iter().map(|(path, _, _)| {
            (
                yss_project_history::ResourceKey::Graph(path.clone()),
                snapshot
                    .graph_resource_revisions
                    .get(path)
                    .copied()
                    .unwrap_or(ResourceRevision::INITIAL),
            )
        }))
        .collect();
        let context = context(
            self,
            session.clone(),
            operation_id,
            expected_revisions,
            [yss_project_history::ResourceKey::Graph(target.clone())].into(),
        );
        self.validate_writer_context(&context, snapshot.authority_generation)?;
        let mut mutations = vec![
            StagedFilesystemMutation::Write {
                relative_path: target.as_str().into(),
                contents: target_contents,
            },
            StagedFilesystemMutation::RemoveFile {
                relative_path: graph_path.as_str().into(),
            },
        ];
        mutations.extend(referenced.iter().map(|(path, _, contents)| {
            StagedFilesystemMutation::Write {
                relative_path: path.as_str().into(),
                contents: contents.clone(),
            }
        }));
        let prepared = ProjectFilesystemTransaction::prepare(
            context.filesystem_context(),
            filesystem_lease,
            mutations,
        )?;
        self.validate_resource_lifecycle_operation(&ownership.operation)?;
        self.validate_writer_context(&context, snapshot.authority_generation)?;
        let committed = prepared.commit()?;

        match self.apply_project_resource_document_patch(
            &context,
            ProjectDataPatch::MoveGraph {
                from: graph_path.clone(),
                to: target,
                moved: source,
                referenced_graphs: referenced
                    .iter()
                    .map(|(path, graph, _)| (path.clone(), graph.clone()))
                    .collect(),
                loaded_referenced_graphs: referenced
                    .iter()
                    .map(|(path, _, _)| path.clone())
                    .collect(),
            },
            Some(&mut ownership),
        ) {
            Ok(result) => {
                committed.finalize();
                reservation.complete();
                Ok(result)
            }
            Err(error) => {
                committed.rollback()?;
                Err(error)
            }
        }
    }
}

fn renamed_graph_path(
    name: &ResourceName,
    kind: yss_graph_document::GraphResourceKind,
) -> Result<GraphResourcePath, ProjectFilesystemError> {
    let (directory, extension) = match kind {
        yss_graph_document::GraphResourceKind::Event => (
            yss_project_layout::EVENTS_DIR,
            yss_project_layout::EVENT_EXTENSION,
        ),
        yss_graph_document::GraphResourceKind::Function => (
            yss_project_layout::FUNCTIONS_DIR,
            yss_project_layout::FUNCTION_EXTENSION,
        ),
    };
    GraphResourcePath::new(format!("{directory}/{}.{extension}", name.as_str())).map_err(|error| {
        ProjectFilesystemError::TransactionPrepareFailed {
            message: error.to_string(),
        }
    })
}

fn duplicate_document(
    document: &GraphDocument,
    source: &GraphResourcePath,
    target: &GraphResourcePath,
) -> GraphDocument {
    let node_ids = document
        .nodes
        .keys()
        .copied()
        .map(|id| (id, NodeId::new()))
        .collect::<HashMap<_, _>>();
    let mut instance_ids = HashMap::new();
    let mut collect_instance = |address: &PortAddress| {
        if let PortRef::Instance { instance_id, .. } = address.port {
            instance_ids
                .entry(instance_id)
                .or_insert_with(PortInstanceId::new);
        }
    };
    for address in document.port_bindings.keys() {
        collect_instance(address);
    }
    for address in document.input_states.keys() {
        collect_instance(address);
    }
    for connection in document.connections.values() {
        collect_instance(&connection.output);
        collect_instance(&connection.input);
    }

    let mut duplicate = document.clone();
    let constant_ids = document
        .constants
        .keys()
        .map(|id| (*id, yss_graph_document::ConstantId::new()))
        .collect::<BTreeMap<_, _>>();
    duplicate.constants = document
        .constants
        .values()
        .map(|constant| {
            let constant = constant.copy_with_id(constant_ids[&constant.id]);
            (constant.id, constant)
        })
        .collect();
    duplicate.nodes = document
        .nodes
        .values()
        .map(|node| {
            let mut node = node.clone();
            node.id = node_ids.get(&node.id).copied().unwrap_or(node.id);
            if node.node_type.as_str() == "yssbi.constant.get"
                && let Some(id) = node
                    .parameters
                    .iter()
                    .find(|(key, _)| key.as_str() == "constant")
                    .map(|(_, value)| value)
                    .and_then(|value| value.as_str())
                    .and_then(|id| id.parse::<yss_graph_document::ConstantId>().ok())
                    .and_then(|id| constant_ids.get(&id))
            {
                node.parameters.insert(
                    "constant".parse().expect("constant parameter key"),
                    serde_json::Value::String(id.to_string()),
                );
            }
            for value in node.parameters.values_mut() {
                if value.as_str().is_some_and(|value| {
                    crate::graph_resource_index::normalize_resource_path(value) == source.as_str()
                }) {
                    *value = serde_json::Value::String(target.as_str().to_owned());
                }
            }
            (node.id, node)
        })
        .collect::<BTreeMap<_, _>>();
    duplicate.connections = document
        .connections
        .values()
        .map(|connection| {
            let mut connection = connection.clone();
            connection.id = ConnectionId::new();
            connection.output = duplicate_address(&connection.output, &node_ids, &instance_ids);
            connection.input = duplicate_address(&connection.input, &node_ids, &instance_ids);
            (connection.id, connection)
        })
        .collect::<BTreeMap<_, _>>();
    duplicate.port_bindings = document
        .port_bindings
        .iter()
        .map(|(address, binding)| {
            (
                duplicate_address(address, &node_ids, &instance_ids),
                duplicate_binding(binding, source, target),
            )
        })
        .collect::<BTreeMap<_, _>>();
    duplicate.input_states = document
        .input_states
        .iter()
        .map(|(address, state)| {
            (
                duplicate_address(address, &node_ids, &instance_ids),
                state.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    duplicate
}

fn duplicate_address(
    address: &PortAddress,
    node_ids: &HashMap<NodeId, NodeId>,
    instance_ids: &HashMap<PortInstanceId, PortInstanceId>,
) -> PortAddress {
    let node_id = node_ids
        .get(&address.node_id)
        .copied()
        .unwrap_or(address.node_id);
    let port = match &address.port {
        PortRef::Declared { key } => PortRef::Declared { key: key.clone() },
        PortRef::Instance {
            template,
            instance_id,
        } => PortRef::Instance {
            template: template.clone(),
            instance_id: instance_ids
                .get(instance_id)
                .copied()
                .unwrap_or(*instance_id),
        },
    };
    PortAddress { node_id, port }
}

fn duplicate_binding(
    binding: &DynamicPortBinding,
    source: &GraphResourcePath,
    target: &GraphResourcePath,
) -> DynamicPortBinding {
    match binding {
        DynamicPortBinding::UserCreated { order } => DynamicPortBinding::UserCreated {
            order: order.clone(),
        },
        DynamicPortBinding::Resolved {
            origin,
            order,
            last_known,
        } => DynamicPortBinding::Resolved {
            origin: duplicate_locator(origin, source, target),
            order: order.clone(),
            last_known: last_known.clone(),
        },
        DynamicPortBinding::Orphan {
            origin,
            order,
            last_known,
        } => DynamicPortBinding::Orphan {
            origin: duplicate_locator(origin, source, target),
            order: order.clone(),
            last_known: last_known.clone(),
        },
    }
}

fn duplicate_locator(
    locator: &DynamicMemberLocator,
    source: &GraphResourcePath,
    target: &GraphResourcePath,
) -> DynamicMemberLocator {
    match locator {
        DynamicMemberLocator::FunctionParameter {
            function,
            parameter,
        } => DynamicMemberLocator::FunctionParameter {
            function: if crate::graph_resource_index::normalize_resource_path(function.as_str())
                == source.as_str()
            {
                target.clone()
            } else {
                function.clone()
            },
            parameter: parameter.clone(),
        },
        DynamicMemberLocator::SchemaField { source, field } => DynamicMemberLocator::SchemaField {
            source: source.clone(),
            field: field.clone(),
        },
    }
}

fn remap_document_references(document: &mut GraphDocument, from: &str, to: &str) -> bool {
    let mut changed = false;
    for node in document.nodes.values_mut() {
        for value in node.parameters.values_mut() {
            if value.as_str() == Some(from) {
                *value = serde_json::Value::String(to.to_owned());
                changed = true;
            }
        }
    }
    changed
}

fn document_references(document: &GraphDocument, target: &str) -> bool {
    document.nodes.values().any(|node| {
        node.parameters
            .values()
            .any(|value| value.as_str() == Some(target))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::project_writers::ProjectProjectionStatus;
    use yss_graph_document::GraphResourceKind;
    use yss_project_history::ResourceDocumentPatch;
    use yss_project_model::ProjectData;

    #[test]
    fn graph_writers_reject_changed_revision_and_occupied_or_missing_paths() {
        let fixture =
            fixtures::TempProject::activate("graph-writer-preconditions", ProjectData::new());
        let state = fixture.state();
        let session = state.capture_project_session().unwrap();
        let path = GraphResourcePath::new("events/Event.yssbi-event").unwrap();
        let create = || {
            state.create_graph_resource(
                &session.instance_id,
                "Event",
                GraphResourceDocument::new("Event", GraphResourceKind::Event),
                yss_project_identity::OperationId::new(),
            )
        };
        create().unwrap();
        let file = session.root.as_path().join(path.as_str());
        let bytes = std::fs::read(&file).unwrap();
        // An unloaded graph still owns its disk path and committed revision.
        state.unload_graph_resource(&path).unwrap();
        assert!(matches!(
            create(),
            Err(ProjectFilesystemError::ResourceRevisionConflict { .. })
        ));
        let wrong_revision = ResourceRevision::INITIAL.checked_next().unwrap();
        assert!(matches!(
            state.remove_graph_resource(
                &session.instance_id,
                &path,
                wrong_revision,
                yss_project_identity::OperationId::new()
            ),
            Err(ProjectFilesystemError::ResourceRevisionConflict { .. })
        ));
        assert!(matches!(
            state.duplicate_graph_resource(
                &session.instance_id,
                &path,
                wrong_revision,
                yss_project_identity::OperationId::new()
            ),
            Err(ProjectFilesystemError::ResourceRevisionConflict { .. })
        ));
        assert_eq!(std::fs::read(&file).unwrap(), bytes);

        let snapshot = state.capture_writer_snapshot(&session.instance_id).unwrap();
        let context = context(
            state,
            session.clone(),
            yss_project_identity::OperationId::new(),
            [(
                yss_project_history::ResourceKey::Graph(path.clone()),
                ResourceRevision::INITIAL,
            )]
            .into(),
            Default::default(),
        );
        let _lease = state.filesystem().acquire(session.root.clone()).unwrap();
        state
            .validate_writer_context(&context, snapshot.authority_generation)
            .unwrap();
        std::fs::remove_file(&file).unwrap();
        assert!(matches!(
            state.validate_writer_context(&context, snapshot.authority_generation),
            Err(ProjectFilesystemError::ResourceRevisionConflict { .. })
        ));
    }

    #[test]
    fn graph_crud_publishes_ordered_deltas_before_any_chart_exists() {
        let fixture =
            fixtures::TempProject::activate("graph-crud-publications", ProjectData::new());
        let state = fixture.state();
        let project = ProjectInstanceId::from_existing(state.project_instance_id());
        let event = GraphResourcePath::new("events/Event.yssbi-event").unwrap();
        let function = GraphResourcePath::new("functions/Function.yssbi-function").unwrap();
        for (revision, name, kind) in [
            (1, "Event", GraphResourceKind::Event),
            (2, "Function", GraphResourceKind::Function),
        ] {
            let result = state
                .create_graph_resource(
                    &project,
                    name,
                    GraphResourceDocument::new(name, kind),
                    yss_project_identity::OperationId::new(),
                )
                .unwrap()
                .into_parts();
            assert_eq!(result.publication_revision, revision);
            assert_eq!(result.deltas.len(), 1);
            assert!(matches!(&result.deltas[0].payload,
                ResourceDocumentPatch::ResourceLifecycle(patch) if patch.before.is_none() && patch.after.is_some()));
            assert!(matches!(
                result.projection_status,
                ProjectProjectionStatus::Complete { .. }
            ));
        }
        let duplicated = state
            .duplicate_graph_resource(
                &project,
                &event,
                ResourceRevision::INITIAL,
                yss_project_identity::OperationId::new(),
            )
            .unwrap()
            .into_parts();
        assert_eq!(duplicated.publication_revision, 3);
        let ResourceDocumentPatch::ResourceLifecycle(patch) = &duplicated.deltas[0].payload else {
            panic!("missing creation delta")
        };
        let copy = GraphResourcePath::new(patch.after.as_ref().unwrap().path.as_ref()).unwrap();
        let removed = state
            .remove_graph_resource(
                &project,
                &copy,
                ResourceRevision::INITIAL,
                yss_project_identity::OperationId::new(),
            )
            .unwrap()
            .into_parts();
        assert_eq!(removed.publication_revision, 4);
        assert!(matches!(&removed.deltas[0].payload,
            ResourceDocumentPatch::ResourceLifecycle(patch) if patch.before.is_some() && patch.after.is_none()));
        let renamed = state
            .rename_graph_resource(
                &project,
                &function,
                ResourceRevision::INITIAL,
                "Renamed",
                1,
                yss_project_identity::OperationId::new(),
            )
            .unwrap()
            .into_parts();
        assert_eq!(renamed.publication_revision, 5);
        assert_eq!(renamed.moves.len(), 1);
        assert!(matches!(
            renamed.deltas[0].payload,
            ResourceDocumentPatch::ResourceMove(_)
        ));
        let index = state.read_project_index(&project).unwrap();
        assert_eq!(index.publication_revision, 5);
        assert!(index.charts.is_empty());
    }
}
