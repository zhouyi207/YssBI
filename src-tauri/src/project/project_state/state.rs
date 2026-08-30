use super::*;

#[derive(Clone)]
pub struct ProjectState {
    pub project_data: Arc<RwLock<ProjectData>>,
    pub(in crate::project) project_path: Arc<RwLock<Option<String>>>,
    pub project_store: Arc<RwLock<ProjectStore>>,
    pub(in crate::project) history: Arc<RwLock<ProjectHistory>>,
    pub(in crate::project) project_activation: crate::project::ProjectActivationCoordinator,
    pub(in crate::project) mutation_publication: Arc<Mutex<MutationPublication>>,
    pub(in crate::project) filesystem: ProjectFilesystemCoordinator,
    pub(in crate::project) resource_lifecycle: ResourceLifecycleRegistry,
    pub(in crate::project) resource_operations:
        Arc<Mutex<crate::project::resource_mutations::ResourceOperationLedger>>,
    pub(in crate::project) recovery_marker: crate::project::ProjectRecoveryMarker,
    pub(in crate::project) activation_generation: Arc<std::sync::atomic::AtomicU64>,
    pub(in crate::project) activation_identity: Arc<RwLock<ProjectAuthorityExpectation>>,
    pub(in crate::project) graph_revisions: Arc<
        RwLock<std::collections::HashMap<GraphResourcePath, yss_graph_document::GraphRevision>>,
    >,
    pub(in crate::project) variable_revisions: Arc<
        RwLock<std::collections::HashMap<yss_variable_contract::VariableId, VariableRevisionEntry>>,
    >,
    pub(in crate::project) worksheet_revisions: Arc<
        RwLock<std::collections::HashMap<WorksheetResourcePath, crate::project::ResourceRevision>>,
    >,
    pub(in crate::project) database_authority_revisions:
        Arc<RwLock<std::collections::HashMap<String, u64>>>,

    #[cfg(test)]
    pub(in crate::project) test_hooks: Arc<ProjectStateTestHooks>,
}

#[cfg(test)]
impl Default for ProjectState {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectState {
    #[cfg(not(test))]
    pub fn new() -> Self {
        Self::from_store_and_filesystem(
            ProjectStore::new(),
            ProjectFilesystemCoordinator::default(),
        )
    }

    #[cfg(test)]
    pub fn try_new() -> Result<Self, crate::graph::catalog::BuiltinInitializationError> {
        Self::try_with_filesystem(ProjectFilesystemCoordinator::default())
    }

    #[cfg(test)]
    pub fn new() -> Self {
        Self::try_new().expect("test built-ins are valid")
    }

    #[cfg(test)]
    fn try_with_filesystem(
        filesystem: ProjectFilesystemCoordinator,
    ) -> Result<Self, crate::graph::catalog::BuiltinInitializationError> {
        let store = ProjectStore::try_new()?;
        Ok(Self::from_store_and_filesystem(store, filesystem))
    }

    fn from_store_and_filesystem(
        store: ProjectStore,
        filesystem: ProjectFilesystemCoordinator,
    ) -> Self {
        let publication = MutationPublication::default();
        let activation_identity = ProjectAuthorityExpectation {
            project_instance_id: ProjectInstanceId::from_existing(
                publication.project_instance_id.clone(),
            ),
            project_root: None,
            project_session_id: store.project_session_id.clone(),
        };
        Self {
            project_data: Arc::new(RwLock::new(ProjectData::new())),
            project_path: Arc::new(RwLock::new(None)),
            project_store: Arc::new(RwLock::new(store)),
            history: Arc::new(RwLock::new(ProjectHistory::default())),
            project_activation: crate::project::ProjectActivationCoordinator::default(),
            mutation_publication: Arc::new(Mutex::new(publication)),
            filesystem,
            resource_lifecycle: ResourceLifecycleRegistry::default(),
            resource_operations: Arc::new(Mutex::new(
                crate::project::resource_mutations::ResourceOperationLedger::new(
                    activation_identity.project_instance_id.clone(),
                ),
            )),
            recovery_marker: crate::project::ProjectRecoveryMarker::default(),
            activation_generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            activation_identity: Arc::new(RwLock::new(activation_identity)),
            graph_revisions: Arc::new(RwLock::new(std::collections::HashMap::new())),
            variable_revisions: Arc::new(RwLock::new(std::collections::HashMap::new())),
            worksheet_revisions: Arc::new(RwLock::new(std::collections::HashMap::new())),
            database_authority_revisions: Arc::new(RwLock::new(std::collections::HashMap::new())),

            #[cfg(test)]
            test_hooks: Arc::new(ProjectStateTestHooks::default()),
        }
    }
}
