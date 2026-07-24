use crate::database::DatabaseInstance;
use crate::node_system::analysis::ProjectSessionId;
use crate::node_system::catalog::{BuiltinCatalog, build_builtin_provider};
use crate::node_system::registry::{NodeRegistry, NodeRegistryBuilder};
use crate::node_system::runtime::{
    CompiledParameterStore, FunctionPlanStore, KernelRegistry, ProjectRunRegistry, ResultStore,
    build_builtin_kernel_registry,
};
use crate::tabular::VariableTabularCache;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub struct ProjectStore {
    pub databases: HashMap<String, DatabaseInstance>,
    pub variable_tabular: HashMap<String, VariableTabularCache>,
    pub node_registry: Arc<NodeRegistry>,
    pub catalog: Arc<BuiltinCatalog>,
    pub kernels: Arc<KernelRegistry>,
    pub compiled_parameters: Arc<RwLock<CompiledParameterStore>>,
    pub function_plans: Arc<FunctionPlanStore>,
    pub results: ResultStore,
    pub runs: Arc<ProjectRunRegistry>,
    pub project_session_id: ProjectSessionId,
}

impl Default for ProjectStore {
    fn default() -> Self {
        let (provider, catalog) = build_builtin_provider();
        let mut builder = NodeRegistryBuilder::new();
        builder
            .register_provider(provider)
            .expect("built-in node provider is unique");
        let node_registry = Arc::new(
            builder
                .freeze()
                .expect("built-in node provider must be valid"),
        );
        let project_session_id = ProjectSessionId::new(uuid::Uuid::new_v4().to_string());
        let function_plans = Arc::new(FunctionPlanStore::new(project_session_id.clone(), 64));

        Self {
            databases: HashMap::new(),
            variable_tabular: HashMap::new(),
            node_registry,
            catalog: Arc::new(catalog),
            kernels: Arc::new(build_builtin_kernel_registry()),
            compiled_parameters: Arc::new(RwLock::new(CompiledParameterStore::new())),
            function_plans,
            results: ResultStore::new(),
            runs: Arc::new(ProjectRunRegistry::new()),
            project_session_id,
        }
    }
}

impl ProjectStore {
    pub fn new() -> Self {
        Self::default()
    }
}
