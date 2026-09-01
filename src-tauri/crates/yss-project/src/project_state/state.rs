use super::*;

#[derive(Clone)]
pub struct ProjectState {
    pub(crate) project_data: Arc<RwLock<ProjectData>>,
    pub(crate) project_path: Arc<RwLock<Option<String>>>,
    pub(crate) project_store: Arc<RwLock<ProjectStore>>,
    pub(crate) history: Arc<RwLock<ProjectHistory>>,
    pub(crate) project_activation: crate::ProjectActivationCoordinator,
    pub(crate) mutation_publication: Arc<Mutex<MutationPublication>>,
    pub(crate) filesystem: ProjectFilesystemCoordinator,
    pub(crate) resource_lifecycle: ResourceLifecycleRegistry,
    pub(crate) resource_operations: Arc<Mutex<yss_project_operation::ProjectOperationLedger>>,
    pub(crate) recovery_marker: yss_project_filesystem::ProjectRecoveryMarker,
    pub(crate) activation_generation: Arc<std::sync::atomic::AtomicU64>,
    pub(crate) activation_identity: Arc<RwLock<ProjectAuthorityExpectation>>,
    pub(crate) graph_revisions: Arc<
        RwLock<std::collections::HashMap<GraphResourcePath, yss_graph_document::GraphRevision>>,
    >,
    pub(crate) variable_revisions: Arc<
        RwLock<std::collections::HashMap<yss_variable_contract::VariableId, VariableRevisionEntry>>,
    >,
    pub(crate) chart_revisions: Arc<
        RwLock<
            std::collections::HashMap<ChartResourcePath, yss_project_identity::ResourceRevision>,
        >,
    >,
    pub(crate) database_authority_revisions: Arc<RwLock<std::collections::HashMap<String, u64>>>,

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) test_hooks: Arc<ProjectStateTestHooks>,
}

impl Default for ProjectState {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectState {
    pub fn new() -> Self {
        Self::from_store_and_filesystem(
            ProjectStore::new(),
            ProjectFilesystemCoordinator::default(),
        )
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
            project_activation: crate::ProjectActivationCoordinator::default(),
            mutation_publication: Arc::new(Mutex::new(publication)),
            filesystem,
            resource_lifecycle: ResourceLifecycleRegistry::default(),
            resource_operations: Arc::new(Mutex::new(
                yss_project_operation::ProjectOperationLedger::new(
                    activation_identity.project_instance_id.clone(),
                    activation_identity.project_session_id.clone(),
                ),
            )),
            recovery_marker: yss_project_filesystem::ProjectRecoveryMarker::default(),
            activation_generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            activation_identity: Arc::new(RwLock::new(activation_identity)),
            graph_revisions: Arc::new(RwLock::new(std::collections::HashMap::new())),
            variable_revisions: Arc::new(RwLock::new(std::collections::HashMap::new())),
            chart_revisions: Arc::new(RwLock::new(std::collections::HashMap::new())),
            database_authority_revisions: Arc::new(RwLock::new(std::collections::HashMap::new())),

            #[cfg(any(test, feature = "test-support"))]
            test_hooks: Arc::new(ProjectStateTestHooks::default()),
        }
    }
}
