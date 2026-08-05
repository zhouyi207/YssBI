use crate::database::DatabaseInstance;
use crate::node_system::analysis::{BoundedTraceSink, ProjectSessionId};
use crate::node_system::catalog::{
    BuiltinCatalog, BuiltinInitializationError, BuiltinNodeSystem, build_builtin_node_system,
};
use crate::node_system::registry::NodeRegistry;
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
    pub trace_sink: Arc<BoundedTraceSink>,
    pub project_session_id: ProjectSessionId,
    #[cfg(test)]
    drop_test_hook: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl ProjectStore {
    pub fn try_new() -> Result<Self, BuiltinInitializationError> {
        build_builtin_node_system().map(Self::from_builtin)
    }

    #[cfg(test)]
    pub fn new() -> Self {
        Self::try_new().expect("test built-ins are valid")
    }

    #[cfg(test)]
    fn try_with_builtin_factory_and_constructor(
        factory: impl FnOnce() -> Result<BuiltinNodeSystem, BuiltinInitializationError>,
        constructor: impl FnOnce(BuiltinNodeSystem) -> Self,
    ) -> Result<Self, BuiltinInitializationError> {
        let bundle = factory()?;
        Ok(constructor(bundle))
    }

    pub(crate) fn validation_scratch(&self) -> Self {
        let project_session_id = self.project_session_id.clone();
        let function_plans = Arc::new(FunctionPlanStore::new(project_session_id.clone(), 64));
        Self {
            databases: self.databases.clone(),
            variable_tabular: self.variable_tabular.clone(),
            node_registry: Arc::clone(&self.node_registry),
            catalog: Arc::clone(&self.catalog),
            kernels: Arc::clone(&self.kernels),
            compiled_parameters: Arc::new(RwLock::new(CompiledParameterStore::new())),
            function_plans,
            results: ResultStore::new(),
            runs: Arc::new(ProjectRunRegistry::new()),
            trace_sink: Arc::new(BoundedTraceSink::default()),
            project_session_id,
            #[cfg(test)]
            drop_test_hook: None,
        }
    }

    fn from_builtin(bundle: BuiltinNodeSystem) -> Self {
        let project_session_id = ProjectSessionId::new(uuid::Uuid::new_v4().to_string());
        let function_plans = Arc::new(FunctionPlanStore::new(project_session_id.clone(), 64));
        Self {
            databases: HashMap::new(),
            variable_tabular: HashMap::new(),
            node_registry: bundle.registry,
            catalog: bundle.catalog,
            kernels: Arc::new(build_builtin_kernel_registry()),
            compiled_parameters: Arc::new(RwLock::new(CompiledParameterStore::new())),
            function_plans,
            results: ResultStore::new(),
            runs: Arc::new(ProjectRunRegistry::new()),
            trace_sink: Arc::new(BoundedTraceSink::default()),
            project_session_id,
            #[cfg(test)]
            drop_test_hook: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn set_drop_test_hook(&mut self, hook: Arc<dyn Fn() + Send + Sync>) {
        self.drop_test_hook = Some(hook);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_store_has_no_production_default_escape_hatch() {
        let source = include_str!("project_store.rs");

        let forbidden = ["impl Default", " for ProjectStore"].concat();
        assert!(!source.contains(&forbidden));
    }

    #[test]
    fn validation_scratch_copies_runtime_maps_and_shares_validated_arcs() {
        let mut authoritative = ProjectStore::try_new().unwrap();
        authoritative.variable_tabular.insert(
            "var:test".into(),
            crate::tabular::VariableTabularCache {
                schema: crate::graph::node::DataSchema {
                    columns: Vec::new(),
                },
                dataframe: Arc::new(polars::prelude::DataFrame::default()),
            },
        );

        let mut scratch = authoritative.validation_scratch();

        assert!(Arc::ptr_eq(
            &scratch.node_registry,
            &authoritative.node_registry
        ));
        assert!(Arc::ptr_eq(&scratch.catalog, &authoritative.catalog));
        assert!(Arc::ptr_eq(&scratch.kernels, &authoritative.kernels));
        assert_eq!(scratch.databases.len(), authoritative.databases.len());
        assert_eq!(
            scratch.variable_tabular.len(),
            authoritative.variable_tabular.len()
        );
        scratch.variable_tabular.clear();
        assert_eq!(authoritative.variable_tabular.len(), 1);
    }

    #[test]
    fn project_store_stops_before_construction_on_builtin_failure() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let later_constructions = AtomicUsize::new(0);
        let source = crate::node_system::protocol::NodeTypeId::new("Bad Store ID").unwrap_err();
        let expected = BuiltinInitializationError::Assembly(
            crate::node_system::catalog::BuiltinAssemblyError::InvalidSemanticId {
                value: "Bad Store ID".into(),
                source,
            },
        );
        let result = ProjectStore::try_with_builtin_factory_and_constructor(
            || Err(expected.clone()),
            |_| {
                later_constructions.fetch_add(1, Ordering::SeqCst);
                unreachable!("store construction must not run after assembly failure")
            },
        );

        assert!(matches!(result, Err(error) if error == expected));
        assert_eq!(later_constructions.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn project_store_requires_validated_builtin_bundle() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let later_constructions = AtomicUsize::new(0);
        let (mut provider, catalog, alias_keys) =
            crate::node_system::catalog::builtin_bundle_parts_for_test().unwrap();
        provider.types[0].title_key = "missing.type.title".parse().unwrap();

        let result = ProjectStore::try_with_builtin_factory_and_constructor(
            || {
                crate::node_system::catalog::validate_builtin_bundle_for_test(
                    provider, catalog, alias_keys,
                )
            },
            |_| {
                later_constructions.fetch_add(1, Ordering::SeqCst);
                unreachable!("store construction must not run after registration failure")
            },
        );

        assert!(matches!(
            result,
            Err(BuiltinInitializationError::Assembly(
                crate::node_system::catalog::BuiltinAssemblyError::Registration(_)
            ))
        ));
        assert_eq!(later_constructions.load(Ordering::SeqCst), 0);
    }
}

#[cfg(test)]
impl Drop for ProjectStore {
    fn drop(&mut self) {
        if let Some(hook) = self.drop_test_hook.take() {
            hook();
        }
    }
}
