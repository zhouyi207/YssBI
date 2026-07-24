//! Authoritative project state for normalized node-system graph documents.

use crate::application::database::bind_duckdb_instance;
use crate::database::{DatabaseEngine, DatabaseInstance, DatabaseState};
use crate::node_system::analysis::EditorGraphProjectionDto;
use crate::node_system::compiler::{GraphCompiler, ResourceSnapshot};
use crate::node_system::document::{
    GraphDeltaEvent, GraphDocumentPatch, GraphMutation, HistoryMutation, MutationConflict,
    MutationRequest, ProjectDocumentState, ProjectHistory, ProjectHistoryTransaction, ResourceKey,
    RevisionedGraphStore,
};
use crate::project::{
    GraphResourceDocument, GraphResourcePath, ProjectData, ProjectStore,
    load_project_graph_from_file, save_project_to_file,
};
use crate::tabular::is_variable_handle;
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct ProjectState {
    pub project_data: Arc<RwLock<ProjectData>>,
    pub project_path: Arc<RwLock<Option<String>>>,
    pub project_store: Arc<RwLock<ProjectStore>>,
    history: Arc<RwLock<ProjectHistory>>,
    pub(super) variable_revisions: Arc<
        RwLock<
            std::collections::HashMap<
                crate::variable::VariableId,
                crate::node_system::document::ResourceRevision,
            >,
        >,
    >,
    #[cfg(test)]
    function_load_checkpoint: Arc<
        RwLock<Option<Arc<dyn Fn(&crate::node_system::runtime::CancellationToken) + Send + Sync>>>,
    >,
}

impl Default for ProjectState {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectState {
    pub fn new() -> Self {
        Self {
            project_data: Arc::new(RwLock::new(ProjectData::new())),
            project_path: Arc::new(RwLock::new(None)),
            project_store: Arc::new(RwLock::new(ProjectStore::default())),
            history: Arc::new(RwLock::new(ProjectHistory::default())),
            variable_revisions: Arc::new(RwLock::new(std::collections::HashMap::new())),
            #[cfg(test)]
            function_load_checkpoint: Arc::new(RwLock::new(None)),
        }
    }

    pub fn get_data(&self) -> ProjectData {
        self.project_data.read().unwrap().clone()
    }

    pub fn set_data(&self, project_data: ProjectData) {
        let databases = project_data.databases.clone();
        let project_root = self
            .get_path()
            .map(|path| crate::project::project_root_from_path(&path));
        let mut store = ProjectStore::default();
        for (id, decl) in &databases {
            let instance = if matches!(decl.engine, DatabaseEngine::DuckDb { .. }) {
                bind_duckdb_instance(decl, project_root.as_deref())
            } else {
                DatabaseInstance {
                    decl: decl.clone(),
                    state: DatabaseState::Failed {
                        error: "Only DuckDb datasets are supported; re-import the data".into(),
                    },
                }
            };
            store.databases.insert(id.clone(), instance);
        }
        *self.variable_revisions.write().unwrap() = project_data
            .variables
            .keys()
            .copied()
            .map(|id| (id, crate::node_system::document::ResourceRevision::INITIAL))
            .collect();
        *self.project_data.write().unwrap() = project_data;
        *self.project_store.write().unwrap() = store;
        self.history.write().unwrap().clear();
        self.sync_all_variable_tabular();
    }

    pub fn get_path(&self) -> Option<String> {
        self.project_path.read().unwrap().clone()
    }

    pub fn set_path(&self, path: Option<String>) {
        *self.project_path.write().unwrap() = path;
    }

    pub fn clear(&self) {
        let store = self.project_store.read().unwrap();
        store.runs.cancel_and_drain(&store.project_session_id);
        drop(store);
        *self.project_data.write().unwrap() = ProjectData::default();
        *self.project_path.write().unwrap() = None;
        *self.project_store.write().unwrap() = ProjectStore::default();
        self.history.write().unwrap().clear();
        self.variable_revisions.write().unwrap().clear();
    }

    pub fn activate_loaded_project(&self, path: String, data: ProjectData) {
        self.clear();
        self.set_path(Some(path));
        self.set_data(data);
    }

    pub fn persist_current_project(&self) -> Result<(), String> {
        let Some(path) = self.get_path() else {
            return Ok(());
        };
        let snapshot = self.get_data();
        save_project_to_file(&snapshot, &path).map_err(|error| error.to_string())
    }

    pub fn invalidate_graph_runtime(&self) {}

    pub fn recompile_graphs_for_variable(&self, _variable_id: &crate::variable::VariableId) {}

    pub fn build_schema_provider(&self) -> crate::graph::core::SchemaProvider {
        let store = Arc::clone(&self.project_store);
        Arc::new(move |tabular_id: &str| {
            if is_variable_handle(tabular_id) {
                return store
                    .read()
                    .ok()?
                    .variable_tabular
                    .get(tabular_id)
                    .map(|entry| entry.schema.clone());
            }
            store
                .write()
                .ok()?
                .databases
                .get_mut(tabular_id)?
                .data_schema()
                .ok()
        })
    }

    pub fn insert_graph(
        &self,
        path: GraphResourcePath,
        resource: GraphResourceDocument,
    ) -> GraphResourceDocument {
        self.project_data
            .write()
            .unwrap()
            .graphs
            .insert(path, resource.clone());
        resource
    }

    fn allocate_graph_path(
        &self,
        name: &str,
        kind: crate::project::GraphDocumentKind,
    ) -> Result<(GraphResourcePath, String), String> {
        let persisted = if let Some(path) = self.get_path() {
            crate::project::read_project_index(&path)
                .map_err(|error| error.to_string())?
                .graphs
                .into_iter()
                .filter(|entry| entry.graph_type == kind)
                .map(|entry| (entry.path, entry.name))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let data = self.project_data.read().unwrap();
        let existing_names = data
            .graphs
            .values()
            .filter(|graph| graph.kind == kind)
            .map(|graph| graph.name.clone())
            .chain(persisted.iter().map(|(_, name)| name.clone()))
            .collect::<Vec<_>>();
        let unique_name = crate::project::unique_name::unique_name(name.trim(), existing_names);
        let stem = sanitize_graph_name(&unique_name);
        let (directory, extension) = match kind {
            crate::project::GraphDocumentKind::Event => {
                (crate::project::EVENTS_DIR, crate::project::EVENT_EXTENSION)
            }
            crate::project::GraphDocumentKind::Function => (
                crate::project::FUNCTIONS_DIR,
                crate::project::FUNCTION_EXTENSION,
            ),
        };
        let used = data
            .graphs
            .keys()
            .map(|path| path.as_str().to_string())
            .chain(persisted.into_iter().map(|(path, _)| path))
            .collect::<std::collections::HashSet<_>>();
        drop(data);
        for suffix in 0.. {
            let file_name = if suffix == 0 {
                format!("{stem}.{extension}")
            } else {
                format!("{stem} {suffix}.{extension}")
            };
            let candidate = format!("{directory}/{file_name}");
            if !used.contains(&candidate) {
                return Ok((
                    GraphResourcePath::new(candidate).map_err(|error| error.to_string())?,
                    unique_name,
                ));
            }
        }
        unreachable!("graph path allocation always finds a suffix")
    }

    pub fn create_graph_resource(
        &self,
        name: &str,
        kind: crate::project::GraphDocumentKind,
    ) -> Result<GraphResourcePath, String> {
        let project_path = self.get_path().ok_or_else(|| "项目尚未加载".to_string())?;
        let (graph_path, name) = self.allocate_graph_path(name, kind)?;
        let mut resource = GraphResourceDocument::new(name, kind);
        let shell_types: &[(&str, f64)] = match kind {
            crate::project::GraphDocumentKind::Event => &[("yssbi.project.event.begin", 120.0)],
            crate::project::GraphDocumentKind::Function => &[
                ("yssbi.project.function.entry", 120.0),
                ("yssbi.project.function.return", 560.0),
            ],
        };
        let mut shell_nodes = Vec::new();
        for (node_type, x) in shell_types {
            let node = crate::node_system::document::DocumentNode {
                id: crate::node_system::document::NodeId::new(),
                node_type: crate::node_system::protocol::NodeTypeId::new(*node_type)
                    .map_err(|error| error.to_string())?,
                position: crate::node_system::document::NodePosition { x: *x, y: 160.0 },
                parameters: if matches!(kind, crate::project::GraphDocumentKind::Function) {
                    [(
                        crate::node_system::protocol::ParameterKey::new("function")
                            .map_err(|error| error.to_string())?,
                        serde_json::Value::String(graph_path.as_str().to_string()),
                    )]
                    .into_iter()
                    .collect()
                } else {
                    crate::node_system::document::ParameterValues::new()
                },
                user_label: None,
            };
            shell_nodes.push(node.id);
            resource.document.nodes.insert(node.id, node);
        }
        if let [entry, returned] = shell_nodes.as_slice() {
            let connection_id = crate::node_system::document::ConnectionId::new();
            resource.document.connections.insert(
                connection_id,
                crate::node_system::document::DocumentConnection {
                    id: connection_id,
                    output: crate::node_system::document::PortAddress::declared(
                        *entry,
                        crate::node_system::protocol::PortKey::new("then")
                            .map_err(|error| error.to_string())?,
                    ),
                    input: crate::node_system::document::PortAddress::declared(
                        *returned,
                        crate::node_system::protocol::PortKey::new("enter")
                            .map_err(|error| error.to_string())?,
                    ),
                    order: None,
                },
            );
        }
        self.insert_graph(graph_path.clone(), resource);
        if let Err(error) = self.save_graph_resource_to(&project_path, &graph_path) {
            self.project_data
                .write()
                .unwrap()
                .graphs
                .remove(&graph_path);
            return Err(error);
        }
        self.unload_graph_resource(&graph_path);
        Ok(graph_path)
    }

    fn save_graph_resource_to(
        &self,
        project_path: &str,
        graph_path: &GraphResourcePath,
    ) -> Result<String, String> {
        let snapshot = self.get_data();
        crate::project::save_project_graph_to_file(&snapshot, project_path, graph_path)
            .map_err(|error| error.to_string())
    }

    pub fn save_graph_resource(&self, graph_path: &GraphResourcePath) -> Result<String, String> {
        let project_path = self.get_path().ok_or_else(|| "项目尚未加载".to_string())?;
        self.save_graph_resource_to(&project_path, graph_path)
    }

    pub fn unload_graph_resource(&self, graph_path: &GraphResourcePath) {
        let graph_path_text = graph_path.as_str();
        let mut data = self.project_data.write().unwrap();
        data.graphs.remove(graph_path);
        data.variables.retain(|_, variable| match &variable.scope {
            crate::variable::VariableScope::Global => true,
            crate::variable::VariableScope::Event { event_path } => event_path != graph_path_text,
            crate::variable::VariableScope::Function { function_path } => {
                function_path != graph_path_text
            }
        });
        drop(data);
        self.history.write().unwrap().clear();
    }

    pub fn remove_graph_resource(&self, graph_path: &GraphResourcePath) -> Result<(), String> {
        let loaded = self.project_data.write().unwrap().graphs.remove(graph_path);
        let removed = if let Some(project_path) = self.get_path() {
            crate::project::remove_project_graph_from_file(&project_path, graph_path)
                .map_err(|error| error.to_string())?
        } else {
            None
        };
        if loaded.is_none() && removed.is_none() {
            return Err(format!("graph '{}' not found", graph_path));
        }
        self.unload_graph_resource(graph_path);
        Ok(())
    }

    pub fn duplicate_graph_resource(
        &self,
        graph_path: &GraphResourcePath,
    ) -> Result<GraphResourcePath, String> {
        let project_path = self.get_path().ok_or_else(|| "项目尚未加载".to_string())?;
        let (path, resource) =
            crate::project::duplicate_project_graph_file(&project_path, graph_path)
                .map_err(|error| error.to_string())?;
        self.insert_graph(path.clone(), resource);
        self.unload_graph_resource(&path);
        Ok(path)
    }

    pub fn rename_graph_resource(
        &self,
        graph_path: &GraphResourcePath,
        new_name: &str,
    ) -> Result<GraphResourcePath, String> {
        let project_path = self.get_path().ok_or_else(|| "项目尚未加载".to_string())?;
        let was_loaded = self
            .project_data
            .read()
            .unwrap()
            .graphs
            .contains_key(graph_path);
        let mut resource = self.load_graph_from_current_project(graph_path)?;
        let (target, unique_name) = self.allocate_graph_path(new_name, resource.kind)?;
        resource.name = unique_name;
        crate::project::project_io::remap_graph_document_references(
            &mut resource.document,
            graph_path.as_str(),
            target.as_str(),
        );

        let mut staged = self.get_data();
        staged.graphs.remove(graph_path);
        staged.graphs.insert(target.clone(), resource);
        for graph in staged.graphs.values_mut() {
            crate::project::project_io::remap_graph_document_references(
                &mut graph.document,
                graph_path.as_str(),
                target.as_str(),
            );
        }
        for variable in staged.variables.values_mut() {
            crate::project::project_io::remap_variable_scope_path(
                &mut variable.scope,
                graph_path.as_str(),
                target.as_str(),
            );
        }

        crate::project::save_project_graph_to_file(&staged, &project_path, &target)
            .map_err(|error| error.to_string())?;
        let root = crate::project::project_root_from_path(&project_path);
        crate::project::cascade_graph_path_references_on_disk(
            &root,
            graph_path.as_str(),
            target.as_str(),
            Some(root.join(target.as_str()).as_path()),
        )
        .map_err(|error| error.to_string())?;
        crate::project::remove_project_graph_from_file(&project_path, graph_path)
            .map_err(|error| error.to_string())?;
        if !was_loaded {
            staged.graphs.remove(&target);
        }
        *self.project_data.write().unwrap() = staged;
        self.history.write().unwrap().clear();
        Ok(target)
    }

    pub fn load_graph_from_current_project(
        &self,
        graph_path: &GraphResourcePath,
    ) -> Result<GraphResourceDocument, String> {
        if let Some(graph) = self
            .project_data
            .read()
            .unwrap()
            .graphs
            .get(graph_path)
            .cloned()
        {
            return Ok(graph);
        }
        let path = self.get_path().ok_or_else(|| "项目尚未加载".to_string())?;
        let loaded = load_project_graph_from_file(&path, graph_path).map_err(|e| e.to_string())?;
        Ok(self.insert_graph(graph_path.clone(), loaded))
    }

    pub fn apply_graph_mutation(
        &self,
        graph_path: &GraphResourcePath,
        request: MutationRequest<GraphMutation>,
    ) -> Result<GraphDeltaEvent<GraphDocumentPatch>, MutationConflict> {
        let node_path = crate::node_system::document::GraphResourcePath(graph_path.as_str().into());
        let mut data = self.project_data.write().unwrap();
        let resource = data.graphs.get(graph_path).cloned().ok_or_else(|| {
            MutationConflict::ResourceMismatch {
                requested: request.resource.clone(),
                store: ResourceKey::Graph(node_path.clone()),
            }
        })?;
        let mut planner = RevisionedGraphStore::new(node_path.clone(), resource.document.clone());
        let event = planner.apply_mutation(request)?;
        let mut documents = ProjectDocumentState::new(
            data.graphs
                .iter()
                .map(|(path, graph)| {
                    (
                        crate::node_system::document::GraphResourcePath(path.as_str().into()),
                        graph.document.clone(),
                    )
                })
                .collect::<BTreeMap<_, _>>(),
            BTreeMap::new(),
            BTreeMap::new(),
        );
        let transaction = ProjectHistoryTransaction::graph(
            event
                .caused_by
                .expect("mutation events carry operation IDs"),
            node_path,
            event.from_revision,
            event.payload.clone(),
        );
        self.history
            .write()
            .unwrap()
            .apply_transaction(&mut documents, transaction)
            .map_err(|error| MutationConflict::History(error.to_string().into()))?;
        for (path, graph) in &mut data.graphs {
            let key = crate::node_system::document::GraphResourcePath(path.as_str().into());
            if let Some(document) = documents.graphs.remove(&key) {
                graph.document = document;
            }
        }
        Ok(event)
    }

    pub fn apply_graph_patch(
        &self,
        graph_path: &GraphResourcePath,
        request: MutationRequest<GraphDocumentPatch>,
    ) -> Result<GraphDeltaEvent<GraphDocumentPatch>, MutationConflict> {
        let node_path = crate::node_system::document::GraphResourcePath(graph_path.as_str().into());
        let expected_resource = ResourceKey::Graph(node_path.clone());
        if request.resource != expected_resource {
            return Err(MutationConflict::ResourceMismatch {
                requested: request.resource,
                store: expected_resource,
            });
        }
        let mut data = self.project_data.write().unwrap();
        let resource =
            data.graphs
                .get(graph_path)
                .ok_or_else(|| MutationConflict::ResourceMismatch {
                    requested: expected_resource.clone(),
                    store: expected_resource.clone(),
                })?;
        if resource.document.revision != request.base_revision {
            return Err(MutationConflict::StaleRevision {
                base_revision: request.base_revision,
                current_revision: resource.document.revision,
            });
        }
        let mut documents = ProjectDocumentState::new(
            data.graphs
                .iter()
                .map(|(path, graph)| {
                    (
                        crate::node_system::document::GraphResourcePath(path.as_str().into()),
                        graph.document.clone(),
                    )
                })
                .collect(),
            BTreeMap::new(),
            BTreeMap::new(),
        );
        let transaction = ProjectHistoryTransaction::graph(
            request.operation_id,
            node_path,
            request.base_revision,
            request.payload.clone(),
        );
        self.history
            .write()
            .unwrap()
            .apply_transaction(&mut documents, transaction)
            .map_err(|error| MutationConflict::History(error.to_string().into()))?;
        let updated = documents
            .graphs
            .remove(&crate::node_system::document::GraphResourcePath(
                graph_path.as_str().into(),
            ))
            .expect("patched graph remains present");
        let to_revision = updated.revision;
        data.graphs
            .get_mut(graph_path)
            .expect("graph remains loaded")
            .document = updated;
        Ok(GraphDeltaEvent {
            graph_path: crate::node_system::document::GraphResourcePath(graph_path.as_str().into()),
            from_revision: request.base_revision,
            to_revision,
            caused_by: Some(request.operation_id),
            payload: request.payload,
        })
    }

    pub fn update_function_signature(
        &self,
        graph_path: &GraphResourcePath,
        request: MutationRequest<crate::node_system::document::FunctionDocumentPatch>,
    ) -> Result<crate::node_system::document::ResourceDeltaEvent, MutationConflict> {
        let function_key =
            crate::node_system::document::FunctionResourceKey(graph_path.as_str().into());
        let expected_resource = ResourceKey::Function(function_key.clone());
        if request.resource != expected_resource {
            return Err(MutationConflict::ResourceMismatch {
                requested: request.resource,
                store: expected_resource,
            });
        }
        let mut data = self.project_data.write().unwrap();
        let function = data
            .graphs
            .get(graph_path)
            .and_then(|resource| resource.function.as_ref())
            .ok_or_else(|| MutationConflict::ResourceMismatch {
                requested: expected_resource.clone(),
                store: expected_resource.clone(),
            })?;
        if function.revision != request.base_revision {
            return Err(MutationConflict::StaleRevision {
                base_revision: request.base_revision,
                current_revision: function.revision,
            });
        }
        if function.signature != request.payload.before {
            return Err(MutationConflict::History(
                "function patch before-state does not match the current signature".into(),
            ));
        }
        let from_revision = function.revision;
        let mut revisions = self.variable_revisions.write().unwrap();
        let mut documents = project_documents(&data, &revisions);
        let transaction = crate::node_system::document::ProjectHistoryTransaction::new(
            request.operation_id,
            vec![crate::node_system::document::ResourcePatch::function(
                function_key,
                from_revision,
                request.payload.clone(),
            )],
        );
        self.history
            .write()
            .unwrap()
            .apply_transaction(&mut documents, transaction)
            .map_err(|error| MutationConflict::History(error.to_string().into()))?;
        let to_revision = documents.functions[match &expected_resource {
            ResourceKey::Function(key) => key,
            _ => unreachable!(),
        }]
        .revision;
        replace_project_documents(&mut data, &mut revisions, documents);
        Ok(crate::node_system::document::ResourceDeltaEvent {
            resource: expected_resource,
            from_revision,
            to_revision,
            caused_by: Some(request.operation_id),
            payload: crate::node_system::document::ResourceDocumentPatch::Function(request.payload),
        })
    }

    pub fn undo_last_transaction(
        &self,
        request: MutationRequest<HistoryMutation>,
    ) -> Result<Vec<crate::node_system::document::ResourceDeltaEvent>, MutationConflict> {
        self.apply_history_direction(true, request)
    }

    pub fn redo_last_transaction(
        &self,
        request: MutationRequest<HistoryMutation>,
    ) -> Result<Vec<crate::node_system::document::ResourceDeltaEvent>, MutationConflict> {
        self.apply_history_direction(false, request)
    }

    fn apply_history_direction(
        &self,
        undo: bool,
        request: MutationRequest<HistoryMutation>,
    ) -> Result<Vec<crate::node_system::document::ResourceDeltaEvent>, MutationConflict> {
        let mut data = self.project_data.write().unwrap();
        let mut revisions = self.variable_revisions.write().unwrap();
        let mut documents = project_documents(&data, &revisions);
        let current_revision = try_project_document_revision(&documents, &request.resource)
            .ok_or_else(|| {
                MutationConflict::History(
                    format!(
                        "history anchor resource {:?} was not found",
                        request.resource
                    )
                    .into(),
                )
            })?;
        if current_revision != request.base_revision {
            return Err(MutationConflict::StaleRevision {
                base_revision: request.base_revision,
                current_revision,
            });
        }

        let before = documents.clone();
        let transaction = if undo {
            self.history.write().unwrap().undo(&mut documents)
        } else {
            self.history.write().unwrap().redo(&mut documents)
        }
        .map_err(|error| MutationConflict::History(error.to_string().into()))?;
        let deltas = transaction
            .changes
            .iter()
            .map(|change| crate::node_system::document::ResourceDeltaEvent {
                resource: change.resource.clone(),
                from_revision: project_document_revision(&before, &change.resource),
                to_revision: project_document_revision(&documents, &change.resource),
                caused_by: Some(request.operation_id),
                payload: if undo {
                    change.inverse.clone()
                } else {
                    change.forward.clone()
                },
            })
            .collect();
        replace_project_documents(&mut data, &mut revisions, documents);
        Ok(deltas)
    }

    pub fn graph_projection(
        &self,
        graph_path: &GraphResourcePath,
        locale: &str,
    ) -> Result<EditorGraphProjectionDto, String> {
        let (document, variables, databases) = {
            let data = self.project_data.read().unwrap();
            let document = data
                .graphs
                .get(graph_path)
                .map(|graph| graph.document.clone())
                .ok_or_else(|| format!("graph '{}' not loaded", graph_path))?;
            (document, data.variables.clone(), data.databases.clone())
        };
        let (registry, catalog) = {
            let store = self.project_store.read().unwrap();
            (Arc::clone(&store.node_registry), Arc::clone(&store.catalog))
        };
        let resources = snapshot_project_resources(self, variables, databases)?.compile;
        let schema_resolvers = resources.schema_resolvers();
        let compiler = GraphCompiler::with_resolvers(
            registry.as_ref(),
            &resources,
            schema_resolvers,
            crate::node_system::compiler::build_builtin_interface_resolvers(),
        );
        let snapshot = compiler.snapshot(
            crate::node_system::document::GraphResourcePath(graph_path.as_str().into()),
            &document,
        );
        let analysis = compiler
            .compile_snapshot(
                &snapshot,
                &crate::node_system::compiler::CompileCancellationToken::new(),
            )
            .map_err(|error| error.to_string())?
            .analysis;
        EditorGraphProjectionDto::from_sources(
            graph_path.as_str(),
            &analysis,
            &document,
            registry.as_ref(),
            &catalog.localization(locale),
        )
        .map_err(|error| error.to_string())
    }

    #[cfg(test)]
    pub(super) fn set_function_load_checkpoint(
        &self,
        checkpoint: Arc<dyn Fn(&crate::node_system::runtime::CancellationToken) + Send + Sync>,
    ) {
        *self.function_load_checkpoint.write().unwrap() = Some(checkpoint);
    }

    fn load_function_resources(
        &self,
        project_path: &str,
        cancellation: &crate::node_system::runtime::CancellationToken,
    ) -> Result<(), String> {
        cancellation.check().map_err(|error| error.to_string())?;
        let function_paths = crate::project::read_project_index(&project_path)
            .map_err(|error| error.to_string())?
            .graphs
            .into_iter()
            .filter(|entry| entry.graph_type == crate::project::GraphDocumentKind::Function)
            .map(|entry| GraphResourcePath::new(entry.path).map_err(|error| error.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        for path in function_paths {
            cancellation.check().map_err(|error| error.to_string())?;
            let loaded = self.project_data.read().unwrap().graphs.contains_key(&path);
            if !loaded {
                let resource = crate::project::load_project_graph_from_file(project_path, &path)
                    .map_err(|error| error.to_string())?;
                #[cfg(test)]
                if let Some(checkpoint) = self.function_load_checkpoint.read().unwrap().clone() {
                    checkpoint(cancellation);
                }
                cancellation.check().map_err(|error| error.to_string())?;
                self.insert_graph(path, resource);
            }
        }
        Ok(())
    }

    pub fn execute_graph(
        &self,
        graph_path: &GraphResourcePath,
        events: &dyn crate::node_system::runtime::RunEventSink,
    ) -> Result<crate::node_system::runtime::RunResult, String> {
        let (registry, kernels, functions, results, runs, session_id) = {
            let store = self.project_store.read().unwrap();
            (
                Arc::clone(&store.node_registry),
                Arc::clone(&store.kernels),
                Arc::clone(&store.function_plans),
                store.results.clone(),
                Arc::clone(&store.runs),
                store.project_session_id.clone(),
            )
        };
        let cancellation = crate::node_system::runtime::CancellationToken::new();
        let _pre_run = runs
            .track_pre_run(session_id.clone(), cancellation.clone())
            .map_err(|error| error.to_string())?;
        let project_path = self.get_path().ok_or_else(|| "项目尚未加载".to_string())?;
        self.load_function_resources(&project_path, &cancellation)?;
        let (document, variables, databases) = {
            let data = self.project_data.read().unwrap();
            let document = data
                .graphs
                .get(graph_path)
                .map(|graph| graph.document.clone())
                .ok_or_else(|| format!("graph '{}' not loaded", graph_path))?;
            (document, data.variables.clone(), data.databases.clone())
        };
        let compile_cancellation =
            crate::node_system::compiler::CompileCancellationToken::from_shared(
                cancellation.shared_flag(),
            );
        let resource_snapshot = snapshot_project_resources(self, variables, databases)?;
        let mut compiled_parameters = crate::node_system::runtime::CompiledParameterStore::new();
        let function_generation = publish_function_plans(
            self,
            registry.as_ref(),
            functions.as_ref(),
            &resource_snapshot.compile,
            session_id.clone(),
            &compile_cancellation,
            &mut compiled_parameters,
        )?;
        let compiler = GraphCompiler::with_resolvers(
            registry.as_ref(),
            &resource_snapshot.compile,
            resource_snapshot.compile.schema_resolvers(),
            crate::node_system::compiler::build_builtin_interface_resolvers(),
        )
        .with_observability(
            session_id.clone(),
            &crate::node_system::analysis::NOOP_TRACE_SINK,
        );
        let snapshot = compiler.snapshot(
            crate::node_system::document::GraphResourcePath(graph_path.as_str().into()),
            &document,
        );
        let result = compiler
            .compile_snapshot(&snapshot, &compile_cancellation)
            .map_err(|error| error.to_string())?;
        cancellation.check().map_err(|error| error.to_string())?;
        let plan = result.plan.ok_or_else(|| {
            let codes = result
                .analysis
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!("execution refused because graph has blocking diagnostics: {codes}")
        })?;
        let (current_revision, current_variables, current_databases) = {
            let data = self.project_data.read().unwrap();
            let revision = data
                .graphs
                .get(graph_path)
                .map(|graph| graph.document.revision)
                .ok_or_else(|| format!("graph '{}' was unloaded during compilation", graph_path))?;
            (revision, data.variables.clone(), data.databases.clone())
        };
        if current_revision != plan.provenance.basis.graph_revision {
            return Err("execution refused because compiled plan is stale".into());
        }
        let (current_session_id, current_registry) = {
            let store = self.project_store.read().unwrap();
            (
                store.project_session_id.clone(),
                store.node_registry.fingerprint().clone(),
            )
        };
        if current_session_id != session_id {
            return Err(
                "execution refused because project session changed during compilation".into(),
            );
        }
        if current_registry != plan.provenance.basis.registry_fingerprint {
            return Err("execution refused because compiled registry is stale".into());
        }
        let current_resources =
            snapshot_project_resources(self, current_variables, current_databases)?;
        if current_resources.compile.versions != plan.provenance.basis.resource_versions {
            return Err("execution refused because compiled resources are stale".into());
        }
        let resources =
            crate::node_system::runtime::ProjectResourceProvider::new(resource_snapshot.runtime);
        build_run_parameters(&mut compiled_parameters, &document, &plan)?;
        let mut relational_backends = crate::node_system::runtime::RelationalBackendRegistry::new();
        relational_backends
            .register(
                crate::node_system::plan::RelationalBackendId::new("relational.default")
                    .map_err(|error| error.to_string())?,
                ProductionRelationalBackend,
            )
            .map_err(|error| error.to_string())?;
        let mut result = crate::node_system::runtime::RunExecutor::new(
            kernels.as_ref(),
            &resources,
            &function_generation,
        )
        .with_relational_backends(&relational_backends)
        .with_compiled_parameters(&compiled_parameters)
        .with_run_registry(runs.as_ref())
        .with_event_sink(events)
        .with_result_store(&results)
        .run(&plan, cancellation)
        .map_err(|error| error.to_string())?;
        let effects = resources.snapshot().variable_effects();
        let committed = self
            .commit_variable_effects(&session_id, effects)
            .map_err(|error| error.to_string())?;
        result.committed_variable_ids = committed.variable_ids;
        result.resource_deltas = committed.deltas;
        Ok(result)
    }

    pub(super) fn commit_variable_effects(
        &self,
        expected_session_id: &crate::node_system::analysis::ProjectSessionId,
        effects: Vec<crate::node_system::runtime::VariableWriteEffect>,
    ) -> Result<VariableEffectCommitResult, VariableEffectCommitError> {
        let current_session_id = self
            .project_store
            .read()
            .unwrap()
            .project_session_id
            .clone();
        if &current_session_id != expected_session_id {
            return Err(VariableEffectCommitError::SessionChanged {
                expected: expected_session_id.clone(),
                current: current_session_id,
            });
        }
        if effects.is_empty() {
            return Ok(VariableEffectCommitResult {
                variable_ids: Box::new([]),
                deltas: Vec::new(),
            });
        }
        let mut data = self.project_data.write().unwrap();
        let mut revisions = self.variable_revisions.write().unwrap();
        let mut documents = project_documents(&data, &revisions);
        let mut changes = Vec::with_capacity(effects.len());
        let mut ids = Vec::with_capacity(effects.len());
        for effect in effects {
            let id = effect
                .resource
                .as_str()
                .strip_prefix("variables/")
                .ok_or_else(|| VariableEffectCommitError::InvalidEffect {
                    message: format!("invalid variable resource '{}'", effect.resource.as_str())
                        .into(),
                })
                .and_then(|value| {
                    uuid::Uuid::parse_str(value).map_err(|error| {
                        VariableEffectCommitError::InvalidEffect {
                            message: error.to_string().into(),
                        }
                    })
                })
                .map(crate::variable::VariableId::from)?;
            let resource_key = crate::node_system::document::ResourceKey::Variable(
                crate::node_system::document::VariableResourceKey(effect.resource.as_str().into()),
            );
            let current =
                data.variables
                    .get(&id)
                    .ok_or_else(|| VariableEffectCommitError::Conflict {
                        resource: resource_key.clone(),
                        expected_revision: revisions
                            .get(&id)
                            .copied()
                            .unwrap_or(crate::node_system::document::ResourceRevision::INITIAL),
                        current_revision: None,
                    })?;
            let revision = revisions
                .get(&id)
                .copied()
                .unwrap_or(crate::node_system::document::ResourceRevision::INITIAL);
            if revision != effect.expected_revision {
                return Err(VariableEffectCommitError::Conflict {
                    resource: resource_key,
                    expected_revision: effect.expected_revision,
                    current_revision: Some(revision),
                });
            }
            let current_basis = serde_json::to_value(current).map_err(|error| {
                VariableEffectCommitError::InvalidEffect {
                    message: error.to_string().into(),
                }
            })?;
            let expected_basis = serde_json::to_value(&effect.before).map_err(|error| {
                VariableEffectCommitError::InvalidEffect {
                    message: error.to_string().into(),
                }
            })?;
            if current_basis != expected_basis {
                return Err(VariableEffectCommitError::Conflict {
                    resource: resource_key,
                    expected_revision: effect.expected_revision,
                    current_revision: Some(revision),
                });
            }
            changes.push(crate::node_system::document::ResourcePatch::variable(
                crate::node_system::document::VariableResourceKey(effect.resource.as_str().into()),
                revision,
                crate::node_system::document::VariableDocumentPatch::new(
                    serde_json::to_value(&effect.before.data_value).map_err(|error| {
                        VariableEffectCommitError::InvalidEffect {
                            message: error.to_string().into(),
                        }
                    })?,
                    serde_json::to_value(&effect.after).map_err(|error| {
                        VariableEffectCommitError::InvalidEffect {
                            message: error.to_string().into(),
                        }
                    })?,
                ),
            ));
            ids.push(id);
        }
        let transaction = crate::node_system::document::ProjectHistoryTransaction::new(
            crate::node_system::document::OperationId::new(),
            changes,
        );
        let deltas = transaction
            .changes
            .iter()
            .map(|change| crate::node_system::document::ResourceDeltaEvent {
                resource: change.resource.clone(),
                from_revision: change.before_revision,
                to_revision: change.after_revision,
                caused_by: Some(transaction.caused_by),
                payload: change.forward.clone(),
            })
            .collect();
        self.history
            .write()
            .unwrap()
            .apply_transaction(&mut documents, transaction)
            .map_err(|error| VariableEffectCommitError::History {
                message: error.to_string().into(),
            })?;
        replace_project_documents(&mut data, &mut revisions, documents);
        drop(revisions);
        drop(data);
        self.sync_all_variable_tabular();
        self.persist_current_project()
            .map_err(|error| VariableEffectCommitError::Persistence {
                message: error.into(),
            })?;
        Ok(VariableEffectCommitResult {
            variable_ids: ids.into_boxed_slice(),
            deltas,
        })
    }
}

#[derive(Debug)]
pub(super) struct VariableEffectCommitResult {
    pub variable_ids: Box<[crate::variable::VariableId]>,
    pub deltas: Vec<crate::node_system::document::ResourceDeltaEvent>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum VariableEffectCommitError {
    SessionChanged {
        expected: crate::node_system::analysis::ProjectSessionId,
        current: crate::node_system::analysis::ProjectSessionId,
    },
    Conflict {
        resource: crate::node_system::document::ResourceKey,
        expected_revision: crate::node_system::document::ResourceRevision,
        current_revision: Option<crate::node_system::document::ResourceRevision>,
    },
    InvalidEffect {
        message: Box<str>,
    },
    History {
        message: Box<str>,
    },
    Persistence {
        message: Box<str>,
    },
}

impl std::fmt::Display for VariableEffectCommitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SessionChanged { expected, current } => write!(
                formatter,
                "project session changed from '{}' to '{}' before variable effects committed",
                expected.as_str(),
                current.as_str()
            ),
            Self::Conflict {
                resource,
                expected_revision,
                current_revision,
            } => write!(
                formatter,
                "variable effect conflict for {resource:?}: expected revision {}, current revision {:?}",
                expected_revision.get(),
                current_revision.map(|revision| revision.get())
            ),
            Self::InvalidEffect { message }
            | Self::History { message }
            | Self::Persistence { message } => formatter.write_str(message),
        }
    }
}

#[derive(Clone)]
struct CompileResourceSnapshot {
    versions: crate::node_system::analysis::ResourceVersionSet,
    functions: BTreeMap<
        crate::node_system::document::GraphResourcePath,
        crate::node_system::document::FunctionDocument,
    >,
    database_schemas: BTreeMap<crate::node_system::plan::ResourceId, Vec<Box<str>>>,
}

impl CompileResourceSnapshot {
    fn schema_resolvers(&self) -> crate::node_system::compiler::SchemaResolverSet {
        let mut resolvers = crate::node_system::compiler::SchemaResolverSet::new();
        resolvers.insert(
            crate::node_system::protocol::SchemaResolverId::new(
                crate::node_system::catalog::DATAFRAME_RESOURCE_SCHEMA_RESOLVER,
            )
            .expect("built-in dataframe schema resolver ID is valid"),
            ProjectDatabaseSchemaResolver {
                schemas: self.database_schemas.clone(),
            },
        );
        resolvers
    }
}

struct ProjectDatabaseSchemaResolver {
    schemas: BTreeMap<crate::node_system::plan::ResourceId, Vec<Box<str>>>,
}

impl crate::node_system::compiler::SchemaResolver for ProjectDatabaseSchemaResolver {
    fn resolve(
        &self,
        context: &crate::node_system::compiler::SchemaResolutionContext<'_>,
    ) -> Result<
        crate::node_system::compiler::SchemaFact,
        crate::node_system::compiler::SchemaResolutionError,
    > {
        let resource = context
            .parameters
            .iter()
            .find(|(key, _)| key.as_str() == "dataframe")
            .and_then(|(_, value)| value.as_str())
            .ok_or_else(|| {
                crate::node_system::compiler::SchemaResolutionError::new(
                    "dataframe source requires a database resource",
                )
            })?;
        let resource = crate::node_system::plan::ResourceId::new(resource).map_err(|error| {
            crate::node_system::compiler::SchemaResolutionError::new(error.to_string())
        })?;
        let fields = self.schemas.get(&resource).ok_or_else(|| {
            crate::node_system::compiler::SchemaResolutionError::new(format!(
                "database resource '{}' has no compiled schema",
                resource.as_str()
            ))
        })?;
        Ok(crate::node_system::compiler::SchemaFact::new(
            crate::node_system::protocol::SchemaExpr::Input(
                crate::node_system::protocol::PortKey::new("dataframe").unwrap(),
            ),
            fields
                .iter()
                .cloned()
                .map(crate::node_system::protocol::SchemaColumnRef),
        ))
    }
}

impl ResourceSnapshot for CompileResourceSnapshot {
    fn versions(&self) -> crate::node_system::analysis::ResourceVersionSet {
        self.versions.clone()
    }

    fn function_document(
        &self,
        path: &crate::node_system::document::GraphResourcePath,
    ) -> Option<&crate::node_system::document::FunctionDocument> {
        self.functions.get(path)
    }
}

pub(super) struct ProductionRelationalBackend;

impl crate::node_system::runtime::RelationalBackend for ProductionRelationalBackend {
    fn execute(
        &self,
        context: &crate::node_system::runtime::RelationalContext<'_>,
        plan: &crate::node_system::plan::CompiledRelationalPlan,
        operation_inputs: &[crate::node_system::runtime::RuntimeValue],
        _bridge_inputs: &[crate::node_system::runtime::RelationalInput],
    ) -> Result<
        crate::node_system::runtime::RelationalExecution,
        crate::node_system::runtime::RelationalError,
    > {
        use crate::node_system::plan::RelationalOperator;
        use crate::node_system::runtime::RuntimeValue;
        let mut values = Vec::with_capacity(plan.operators.len());
        let mut next_input = 0;
        for operator in plan.operators.iter() {
            context
                .cancellation
                .check()
                .map_err(crate::node_system::runtime::RelationalError::from)?;
            let value = match operator {
                RelationalOperator::Input { .. } => {
                    let value = operation_inputs.get(next_input).cloned().ok_or_else(|| {
                        crate::node_system::runtime::RelationalError::new(
                            "relational input is missing",
                        )
                    })?;
                    next_input += 1;
                    value
                }
                RelationalOperator::Source { resource, .. } => {
                    let lease = context
                        .resources
                        .get(resource)
                        .and_then(|lease| {
                            lease
                                .as_any()
                                .downcast_ref::<crate::node_system::runtime::ProjectResourceLease>()
                        })
                        .ok_or_else(|| {
                            crate::node_system::runtime::RelationalError::new(format!(
                                "relational source '{}' is unavailable",
                                resource.as_str()
                            ))
                        })?;
                    let dataframe = lease
                        .load_dataframe()
                        .map_err(crate::node_system::runtime::RelationalError::new)?
                        .ok_or_else(|| {
                            crate::node_system::runtime::RelationalError::new(format!(
                                "relational source '{}' is unavailable",
                                resource.as_str()
                            ))
                        })?;
                    RuntimeValue::Scalar(
                        crate::node_system::runtime::dataframe_to_protocol_value(
                            dataframe.as_ref(),
                        )
                        .map_err(|error| {
                            crate::node_system::runtime::RelationalError::new(error.to_string())
                        })?,
                    )
                }
                RelationalOperator::Project { input, columns } => {
                    let source = relational_scalar(&values, input.index())?;
                    let mut projected = BTreeMap::new();
                    for column in columns.iter() {
                        projected.insert(
                            column.name.clone(),
                            relational_expression(&column.expression, source)?,
                        );
                    }
                    RuntimeValue::Scalar(crate::node_system::protocol::Value::Object(projected))
                }
                RelationalOperator::Filter { input, predicate } => {
                    let source = relational_scalar(&values, input.index())?;
                    let mask = relational_expression(predicate, source)?;
                    RuntimeValue::Scalar(relational_filter(source, &mask)?)
                }
                RelationalOperator::Rename { input, columns } => {
                    let mut source = relational_object(relational_scalar(&values, input.index())?)?;
                    for rename in columns.iter() {
                        if let Some(value) = source.remove(rename.from.as_ref()) {
                            source.insert(rename.to.clone(), value);
                        }
                    }
                    RuntimeValue::Scalar(crate::node_system::protocol::Value::Object(source))
                }
                RelationalOperator::Limit { input, rows } => {
                    let source = relational_object(relational_scalar(&values, input.index())?)?;
                    let limited = source
                        .into_iter()
                        .map(|(name, value)| {
                            let value = match value {
                                crate::node_system::protocol::Value::List(mut values) => {
                                    values.truncate(*rows as usize);
                                    crate::node_system::protocol::Value::List(values)
                                }
                                value => value,
                            };
                            (name, value)
                        })
                        .collect();
                    RuntimeValue::Scalar(crate::node_system::protocol::Value::Object(limited))
                }
                RelationalOperator::Union { inputs, all: _ } => {
                    let mut combined =
                        BTreeMap::<Box<str>, Vec<crate::node_system::protocol::Value>>::new();
                    for input in inputs.iter() {
                        for (name, value) in
                            relational_object(relational_scalar(&values, input.index())?)?
                        {
                            let crate::node_system::protocol::Value::List(column) = value else {
                                return Err(crate::node_system::runtime::RelationalError::new(
                                    "union expects dataframe columns",
                                ));
                            };
                            combined.entry(name).or_default().extend(column);
                        }
                    }
                    RuntimeValue::Scalar(crate::node_system::protocol::Value::Object(
                        combined
                            .into_iter()
                            .map(|(name, values)| {
                                (name, crate::node_system::protocol::Value::List(values))
                            })
                            .collect(),
                    ))
                }
            };
            values.push(value);
        }
        let outputs = plan
            .roots
            .iter()
            .map(|root| values[root.index()].clone())
            .collect();
        Ok(crate::node_system::runtime::RelationalExecution {
            outputs,
            fragment_outputs: BTreeMap::new(),
        })
    }
}

fn relational_scalar(
    values: &[crate::node_system::runtime::RuntimeValue],
    index: usize,
) -> Result<&crate::node_system::protocol::Value, crate::node_system::runtime::RelationalError> {
    match values.get(index) {
        Some(crate::node_system::runtime::RuntimeValue::Scalar(value)) => Ok(value),
        _ => Err(crate::node_system::runtime::RelationalError::new(
            "relational operator input is not materialized",
        )),
    }
}

fn relational_object(
    value: &crate::node_system::protocol::Value,
) -> Result<
    BTreeMap<Box<str>, crate::node_system::protocol::Value>,
    crate::node_system::runtime::RelationalError,
> {
    match value {
        crate::node_system::protocol::Value::Object(value) => Ok(value.clone()),
        _ => Err(crate::node_system::runtime::RelationalError::new(
            "relational value is not a dataframe",
        )),
    }
}

fn relational_expression(
    expression: &crate::node_system::plan::RelationalExpression,
    dataframe: &crate::node_system::protocol::Value,
) -> Result<crate::node_system::protocol::Value, crate::node_system::runtime::RelationalError> {
    use crate::node_system::plan::{RelationalExpression as Expr, RelationalLiteral as Literal};
    use crate::node_system::protocol::Value;
    match expression {
        Expr::Column(name) => relational_object(dataframe)?
            .remove(name.as_ref())
            .ok_or_else(|| {
                crate::node_system::runtime::RelationalError::new(format!(
                    "column '{name}' was not found"
                ))
            }),
        Expr::Literal(value) => Ok(match value {
            Literal::Null => Value::Null,
            Literal::Boolean(value) => Value::Bool(*value),
            Literal::Integer(value) => Value::Integer(*value),
            Literal::String(value) => Value::String(value.clone()),
        }),
        Expr::Equal(left, right) => relational_compare(left, right, dataframe, |a, b| a == b),
        Expr::NotEqual(left, right) => relational_compare(left, right, dataframe, |a, b| a != b),
        Expr::LessThan(left, right) => {
            relational_numeric_compare(left, right, dataframe, |a, b| a < b)
        }
        Expr::LessThanOrEqual(left, right) => {
            relational_numeric_compare(left, right, dataframe, |a, b| a <= b)
        }
        Expr::GreaterThan(left, right) => {
            relational_numeric_compare(left, right, dataframe, |a, b| a > b)
        }
        Expr::GreaterThanOrEqual(left, right) => {
            relational_numeric_compare(left, right, dataframe, |a, b| a >= b)
        }
        Expr::And(expressions) | Expr::Or(expressions) => {
            let is_and = matches!(expression, Expr::And(_));
            let mut masks = expressions
                .iter()
                .map(|expression| relational_expression(expression, dataframe));
            let first = masks.next().transpose()?.unwrap_or(Value::Bool(is_and));
            masks.try_fold(first, |left, right| {
                relational_bool_combine(&left, &right?, is_and)
            })
        }
        Expr::Not(expression) => match relational_expression(expression, dataframe)? {
            Value::Bool(value) => Ok(Value::Bool(!value)),
            Value::List(values) => Ok(Value::List(
                values
                    .into_iter()
                    .map(|value| match value {
                        Value::Bool(value) => Value::Bool(!value),
                        _ => Value::Bool(false),
                    })
                    .collect(),
            )),
            _ => Err(crate::node_system::runtime::RelationalError::new(
                "not expects boolean values",
            )),
        },
        Expr::IsNull(expression) => match relational_expression(expression, dataframe)? {
            Value::List(values) => Ok(Value::List(
                values
                    .into_iter()
                    .map(|value| Value::Bool(matches!(value, Value::Null)))
                    .collect(),
            )),
            value => Ok(Value::Bool(matches!(value, Value::Null))),
        },
    }
}

fn relational_expand(
    value: crate::node_system::protocol::Value,
    len: usize,
) -> Vec<crate::node_system::protocol::Value> {
    match value {
        crate::node_system::protocol::Value::List(values) => values,
        value => vec![value; len],
    }
}

fn relational_compare(
    left: &crate::node_system::plan::RelationalExpression,
    right: &crate::node_system::plan::RelationalExpression,
    dataframe: &crate::node_system::protocol::Value,
    compare: impl Fn(&crate::node_system::protocol::Value, &crate::node_system::protocol::Value) -> bool,
) -> Result<crate::node_system::protocol::Value, crate::node_system::runtime::RelationalError> {
    let left = relational_expression(left, dataframe)?;
    let right = relational_expression(right, dataframe)?;
    let len = match (&left, &right) {
        (crate::node_system::protocol::Value::List(values), _)
        | (_, crate::node_system::protocol::Value::List(values)) => values.len(),
        _ => 1,
    };
    Ok(crate::node_system::protocol::Value::List(
        relational_expand(left, len)
            .iter()
            .zip(relational_expand(right, len).iter())
            .map(|(left, right)| crate::node_system::protocol::Value::Bool(compare(left, right)))
            .collect(),
    ))
}

fn relational_number(value: &crate::node_system::protocol::Value) -> Option<f64> {
    match value {
        crate::node_system::protocol::Value::Integer(value) => Some(*value as f64),
        crate::node_system::protocol::Value::Unsigned(value) => Some(*value as f64),
        crate::node_system::protocol::Value::Decimal(value) => value.as_str().parse().ok(),
        _ => None,
    }
}

fn relational_numeric_compare(
    left: &crate::node_system::plan::RelationalExpression,
    right: &crate::node_system::plan::RelationalExpression,
    dataframe: &crate::node_system::protocol::Value,
    compare: impl Fn(f64, f64) -> bool,
) -> Result<crate::node_system::protocol::Value, crate::node_system::runtime::RelationalError> {
    relational_compare(left, right, dataframe, |left, right| {
        relational_number(left)
            .zip(relational_number(right))
            .is_some_and(|(left, right)| compare(left, right))
    })
}

fn relational_bool_combine(
    left: &crate::node_system::protocol::Value,
    right: &crate::node_system::protocol::Value,
    and: bool,
) -> Result<crate::node_system::protocol::Value, crate::node_system::runtime::RelationalError> {
    let values = |value: &crate::node_system::protocol::Value| match value {
        crate::node_system::protocol::Value::List(values) => values.clone(),
        value => vec![value.clone()],
    };
    let left = values(left);
    let right = values(right);
    let len = left.len().max(right.len());
    Ok(crate::node_system::protocol::Value::List(
        (0..len)
            .map(|index| {
                let left = matches!(
                    left.get(index % left.len()),
                    Some(crate::node_system::protocol::Value::Bool(true))
                );
                let right = matches!(
                    right.get(index % right.len()),
                    Some(crate::node_system::protocol::Value::Bool(true))
                );
                crate::node_system::protocol::Value::Bool(if and {
                    left && right
                } else {
                    left || right
                })
            })
            .collect(),
    ))
}

fn relational_filter(
    dataframe: &crate::node_system::protocol::Value,
    mask: &crate::node_system::protocol::Value,
) -> Result<crate::node_system::protocol::Value, crate::node_system::runtime::RelationalError> {
    let crate::node_system::protocol::Value::List(mask) = mask else {
        return Err(crate::node_system::runtime::RelationalError::new(
            "filter predicate is not a boolean series",
        ));
    };
    Ok(crate::node_system::protocol::Value::Object(
        relational_object(dataframe)?
            .into_iter()
            .map(|(name, value)| {
                let values = match value {
                    crate::node_system::protocol::Value::List(values) => values,
                    value => vec![value],
                };
                let filtered = values
                    .into_iter()
                    .zip(mask)
                    .filter_map(|(value, keep)| {
                        matches!(keep, crate::node_system::protocol::Value::Bool(true))
                            .then_some(value)
                    })
                    .collect();
                (name, crate::node_system::protocol::Value::List(filtered))
            })
            .collect(),
    ))
}

pub(super) struct ProductionPlotSink;

impl crate::node_system::runtime::PlotSink for ProductionPlotSink {
    fn publish(
        &self,
        _kind: crate::node_system::runtime::PlotKind,
        payload: &str,
    ) -> Result<Box<str>, crate::node_system::runtime::PlotPublishError> {
        Ok(payload.into())
    }
}

pub(super) struct ProductionResourceSnapshots {
    compile: CompileResourceSnapshot,
    pub(super) runtime: crate::node_system::runtime::ProjectResourceSnapshot,
}

pub(super) fn snapshot_project_resources(
    state: &ProjectState,
    variables: std::collections::HashMap<
        crate::variable::VariableId,
        crate::variable::VariableInstance,
    >,
    databases: std::collections::HashMap<String, crate::database::DatabaseDecl>,
) -> Result<ProductionResourceSnapshots, String> {
    use crate::node_system::analysis::{ResourceKey as AnalysisResourceKey, ResourceVersion};
    use crate::node_system::plan::ResourceId;

    let (session_id, loaded_databases) = {
        let store = state.project_store.read().unwrap();
        let loaded = store
            .databases
            .iter()
            .filter_map(|(id, database)| match &database.state {
                DatabaseState::Loaded { dataframe, .. } => Some((
                    id.clone(),
                    Arc::clone(dataframe),
                    dataframe
                        .get_column_names()
                        .into_iter()
                        .map(|name| Box::<str>::from(name.as_str()))
                        .collect::<Vec<_>>(),
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        (store.project_session_id.clone(), loaded)
    };

    let function_resources = state
        .project_data
        .read()
        .unwrap()
        .graphs
        .iter()
        .filter_map(|(path, graph)| {
            graph.function.clone().map(|function| {
                (
                    crate::node_system::document::GraphResourcePath(path.as_str().into()),
                    function,
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    let mut versions = crate::node_system::analysis::ResourceVersionSet::new();
    for (path, function) in &function_resources {
        let version = serde_json::to_string(function).map_err(|error| error.to_string())?;
        versions.insert(
            AnalysisResourceKey::new(path.0.as_ref()),
            ResourceVersion::new(version),
        );
    }
    for (id, variable) in &variables {
        let key = format!("variables/{id}");
        let version = serde_json::to_string(variable).map_err(|error| error.to_string())?;
        versions.insert(
            AnalysisResourceKey::new(key.as_str()),
            ResourceVersion::new(version),
        );
    }
    for (id, declaration) in &databases {
        let key = format!("databases/{id}");
        let version = serde_json::to_string(declaration).map_err(|error| error.to_string())?;
        versions.insert(
            AnalysisResourceKey::new(key.as_str()),
            ResourceVersion::new(version),
        );
    }

    let project_root = state
        .get_path()
        .map(|path| crate::project::project_root_from_path(&path));
    let mut database_schemas = BTreeMap::new();
    let variable_revisions = state.variable_revisions.read().unwrap().clone();
    let mut runtime =
        crate::node_system::runtime::ProjectResourceSnapshot::new(session_id, versions.clone())
            .with_plot_sink(Arc::new(ProductionPlotSink));
    for (id, variable) in variables {
        runtime = runtime.with_variable_revision(
            ResourceId::new(format!("variables/{id}")).map_err(|error| error.to_string())?,
            Arc::new(variable),
            variable_revisions
                .get(&id)
                .copied()
                .unwrap_or(crate::node_system::document::ResourceRevision::INITIAL),
        );
    }
    for (id, dataframe, columns) in loaded_databases {
        let resource =
            ResourceId::new(format!("databases/{id}")).map_err(|error| error.to_string())?;
        database_schemas.insert(resource.clone(), columns);
        runtime = runtime.with_database(resource, dataframe);
    }
    for (id, declaration) in databases {
        let crate::database::DatabaseEngine::DuckDb { path, table } = declaration.engine else {
            continue;
        };
        let root = project_root
            .as_ref()
            .ok_or_else(|| format!("database '{id}' requires an active project path"))?;
        let absolute = root.join(path);
        let metadata = crate::database::read_table_meta(&absolute, &table)?;
        let resource =
            ResourceId::new(format!("databases/{id}")).map_err(|error| error.to_string())?;
        database_schemas.insert(
            resource.clone(),
            metadata
                .columns
                .into_iter()
                .map(|column| column.name.into())
                .collect(),
        );
        runtime =
            runtime.with_duckdb_database(resource, absolute.to_string_lossy().into_owned(), table);
    }

    Ok(ProductionResourceSnapshots {
        compile: CompileResourceSnapshot {
            versions,
            functions: function_resources,
            database_schemas,
        },
        runtime,
    })
}

fn project_documents(
    data: &ProjectData,
    variable_revisions: &std::collections::HashMap<
        crate::variable::VariableId,
        crate::node_system::document::ResourceRevision,
    >,
) -> ProjectDocumentState {
    ProjectDocumentState::new(
        data.graphs
            .iter()
            .map(|(path, graph)| {
                (
                    crate::node_system::document::GraphResourcePath(path.as_str().into()),
                    graph.document.clone(),
                )
            })
            .collect(),
        data.graphs
            .iter()
            .filter_map(|(path, graph)| {
                graph.function.clone().map(|function| {
                    (
                        crate::node_system::document::FunctionResourceKey(path.as_str().into()),
                        function,
                    )
                })
            })
            .collect(),
        data.variables
            .iter()
            .map(|(id, variable)| {
                (
                    crate::node_system::document::VariableResourceKey(
                        format!("variables/{id}").into(),
                    ),
                    crate::node_system::document::VariableDocument {
                        revision: variable_revisions
                            .get(id)
                            .copied()
                            .unwrap_or(crate::node_system::document::ResourceRevision::INITIAL),
                        value: serde_json::to_value(&variable.data_value)
                            .expect("variable values are serializable"),
                    },
                )
            })
            .collect(),
    )
}

fn try_project_document_revision(
    documents: &ProjectDocumentState,
    resource: &ResourceKey,
) -> Option<crate::node_system::document::ResourceRevision> {
    match resource {
        ResourceKey::Graph(path) => documents.graphs.get(path).map(|document| document.revision),
        ResourceKey::Function(key) => documents
            .functions
            .get(key)
            .map(|document| document.revision),
        ResourceKey::Variable(key) => documents
            .variables
            .get(key)
            .map(|document| document.revision),
    }
}

fn project_document_revision(
    documents: &ProjectDocumentState,
    resource: &ResourceKey,
) -> crate::node_system::document::ResourceRevision {
    try_project_document_revision(documents, resource)
        .expect("history transaction resource remains present")
}

fn replace_project_documents(
    data: &mut ProjectData,
    variable_revisions: &mut std::collections::HashMap<
        crate::variable::VariableId,
        crate::node_system::document::ResourceRevision,
    >,
    mut documents: ProjectDocumentState,
) {
    for (path, graph) in &mut data.graphs {
        let key = crate::node_system::document::GraphResourcePath(path.as_str().into());
        if let Some(document) = documents.graphs.remove(&key) {
            graph.document = document;
        }
        let function_key = crate::node_system::document::FunctionResourceKey(path.as_str().into());
        if let Some(function) = documents.functions.remove(&function_key) {
            graph.function = Some(function);
        }
    }
    for (key, document) in documents.variables {
        let Some(id) = key.0.strip_prefix("variables/") else {
            continue;
        };
        let Ok(uuid) = uuid::Uuid::parse_str(id) else {
            continue;
        };
        let variable_id = crate::variable::VariableId::from(uuid);
        if let Some(variable) = data.variables.get_mut(&variable_id) {
            variable.data_value = serde_json::from_value(document.value)
                .expect("history retains valid variable values");
            variable_revisions.insert(variable_id, document.revision);
        }
    }
}

fn publish_function_plans(
    state: &ProjectState,
    registry: &crate::node_system::registry::NodeRegistry,
    store: &crate::node_system::runtime::FunctionPlanStore,
    resources: &CompileResourceSnapshot,
    session_id: crate::node_system::analysis::ProjectSessionId,
    cancellation: &crate::node_system::compiler::CompileCancellationToken,
    parameters: &mut crate::node_system::runtime::CompiledParameterStore,
) -> Result<crate::node_system::runtime::FunctionPlanGeneration, String> {
    let functions = state
        .project_data
        .read()
        .unwrap()
        .graphs
        .iter()
        .filter(|(_, graph)| graph.function.is_some())
        .map(|(path, graph)| (path.clone(), graph.document.clone()))
        .collect::<Vec<_>>();
    let compiler = GraphCompiler::with_resolvers(
        registry,
        resources,
        resources.schema_resolvers(),
        crate::node_system::compiler::build_builtin_interface_resolvers(),
    )
    .with_observability(session_id, &crate::node_system::analysis::NOOP_TRACE_SINK);
    let mut entries = Vec::with_capacity(functions.len());
    for (path, document) in functions {
        let document_path = crate::node_system::document::GraphResourcePath(path.as_str().into());
        let snapshot = compiler.snapshot(document_path.clone(), &document);
        let products = compiler
            .compile_snapshot(&snapshot, cancellation)
            .map_err(|error| error.to_string())?;
        let plan = products.plan.ok_or_else(|| {
            let diagnostics = products
                .analysis
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "function '{}' has blocking diagnostics and cannot be published: {}",
                path, diagnostics
            )
        })?;
        build_run_parameters(parameters, &document, &plan)?;
        let resource_key = crate::node_system::analysis::ResourceKey::new(path.as_str());
        let version = resources
            .versions
            .get(&resource_key)
            .cloned()
            .ok_or_else(|| format!("function '{}' has no resource version", path))?;
        entries.push((document_path, version, Arc::new(plan)));
    }
    store
        .generation(
            registry.fingerprint().clone(),
            resources.versions(),
            entries,
        )
        .map_err(|error| error.to_string())
}

fn sanitize_graph_name(name: &str) -> String {
    let sanitized = name
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
            {
                '_'
            } else {
                character
            }
        })
        .collect::<String>();
    let sanitized = sanitized.trim_matches([' ', '.']).trim();
    if sanitized.is_empty() {
        "Untitled".into()
    } else {
        sanitized.into()
    }
}

fn build_run_parameters(
    parameters: &mut crate::node_system::runtime::CompiledParameterStore,
    document: &crate::node_system::document::GraphDocument,
    plan: &crate::node_system::plan::ExecutionPlan,
) -> Result<(), String> {
    for operation in &plan.operations {
        let node_type = operation.source_node_type_id.as_str();
        let node = document
            .nodes
            .get(&operation.source_node_id)
            .ok_or_else(|| format!("plan source node '{}' is missing", operation.source_node_id))?;
        if matches!(
            node_type,
            "yssbi.project.variable.get" | "yssbi.project.variable.set"
        ) {
            let resource = node
                .parameters
                .iter()
                .find(|(key, _)| key.as_str() == "variable")
                .and_then(|(_, value)| value.as_str())
                .ok_or_else(|| format!("variable node '{}' has no binding", node.id))?;
            parameters
                .insert(
                    operation.params.clone(),
                    crate::node_system::runtime::BuiltinVariableParameters::new(
                        crate::node_system::plan::ResourceId::new(resource)
                            .map_err(|error| error.to_string())?,
                    ),
                )
                .map_err(|error| error.to_string())?;
            continue;
        }
        if node_type.starts_with("yssbi.statistics.") {
            let parameter = |name: &str| {
                node.parameters
                    .iter()
                    .find(|(key, _)| key.as_str() == name)
                    .map(|(_, value)| value)
            };
            let positive_integer = |name: &str| {
                parameter(name)
                    .and_then(serde_json::Value::as_u64)
                    .map(|value| value as usize)
            };
            parameters
                .insert(
                    operation.params.clone(),
                    crate::node_system::runtime::StatisticsKernelParameters {
                        lags: positive_integer("lags"),
                        max_lags: positive_integer("max_lags"),
                        rank: positive_integer("rank"),
                        trend: parameter("trend")
                            .and_then(serde_json::Value::as_str)
                            .map(Into::into),
                    },
                )
                .map_err(|error| error.to_string())?;
            continue;
        }
        if node_type.starts_with("yssbi.dataframe.") {
            let parameter = |name: &str| {
                node.parameters
                    .iter()
                    .find(|(key, _)| key.as_str() == name)
                    .map(|(_, value)| value)
            };
            let resource = parameter("dataframe")
                .and_then(serde_json::Value::as_str)
                .map(crate::node_system::plan::ResourceId::new)
                .transpose()
                .map_err(|error| error.to_string())?;
            let column = parameter("column")
                .and_then(serde_json::Value::as_str)
                .map(Into::into);
            let order = parameter("order")
                .or_else(|| parameter("window"))
                .and_then(serde_json::Value::as_u64)
                .map(|value| value as usize);
            parameters
                .insert(
                    operation.params.clone(),
                    crate::node_system::runtime::DataframeKernelParameters {
                        resource,
                        column,
                        order,
                    },
                )
                .map_err(|error| error.to_string())?;
            continue;
        }
        if !node_type.starts_with("yssbi.constant.") {
            continue;
        }
        let node = document
            .nodes
            .get(&operation.source_node_id)
            .ok_or_else(|| format!("plan source node '{}' is missing", operation.source_node_id))?;
        let value = node
            .parameters
            .iter()
            .find(|(key, _)| key.as_str() == "value")
            .map(|(_, value)| json_to_protocol_value(value))
            .transpose()?
            .unwrap_or(crate::node_system::protocol::Value::Null);
        parameters
            .insert(
                operation.params.clone(),
                crate::node_system::runtime::BuiltinConstantParameters::new(value),
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn json_to_protocol_value(
    value: &serde_json::Value,
) -> Result<crate::node_system::protocol::Value, String> {
    use crate::node_system::protocol::{CanonicalDecimal, Value};
    Ok(match value {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(value) => Value::Bool(*value),
        serde_json::Value::Number(value) if value.is_i64() => {
            Value::Integer(value.as_i64().expect("checked i64"))
        }
        serde_json::Value::Number(value) if value.is_u64() => {
            Value::Unsigned(value.as_u64().expect("checked u64"))
        }
        serde_json::Value::Number(value) => Value::Decimal(
            CanonicalDecimal::new(value.to_string()).map_err(|error| error.to_string())?,
        ),
        serde_json::Value::String(value) => Value::String(value.as_str().into()),
        serde_json::Value::Array(values) => Value::List(
            values
                .iter()
                .map(json_to_protocol_value)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        serde_json::Value::Object(values) => Value::Object(
            values
                .iter()
                .map(|(key, value)| Ok((key.as_str().into(), json_to_protocol_value(value)?)))
                .collect::<Result<_, String>>()?,
        ),
    })
}
