#[cfg(test)]
use crate::database::DatabaseInstance;
#[cfg(test)]
use crate::graph::catalog::BuiltinCatalog;
#[cfg(test)]
use crate::graph::catalog::{
    BuiltinInitializationError, BuiltinNodeSystem, build_builtin_node_system,
};
#[cfg(test)]
use crate::node_system::runtime::{
    CompiledParameterStore, FunctionPlanStore, KernelRegistry, ProjectRunRegistry, ResultStore,
    SessionMemoization, build_builtin_kernel_registry,
};
use crate::project::ProjectSessionId;
#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use std::sync::Arc;
#[cfg(test)]
use std::sync::RwLock;
#[cfg(test)]
use yss_graph_registry::NodeRegistry;

pub struct ProjectStore {
    #[cfg(test)]
    pub databases: HashMap<String, DatabaseInstance>,
    #[cfg(test)]
    pub node_registry: Arc<NodeRegistry>,
    #[cfg(test)]
    pub catalog: Arc<BuiltinCatalog>,
    #[cfg(test)]
    pub kernels: Arc<KernelRegistry>,
    #[cfg(test)]
    pub compiled_parameters: Arc<RwLock<CompiledParameterStore>>,
    #[cfg(test)]
    pub function_plans: Arc<FunctionPlanStore>,
    #[cfg(test)]
    pub results: ResultStore,
    #[cfg(test)]
    pub memoization: Arc<SessionMemoization>,
    #[cfg(test)]
    pub runs: Arc<ProjectRunRegistry>,
    pub project_session_id: ProjectSessionId,
    #[cfg(test)]
    drop_test_hook: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl ProjectStore {
    #[cfg(not(test))]
    pub fn new() -> Self {
        Self {
            project_session_id: ProjectSessionId::new(uuid::Uuid::new_v4().to_string()),
        }
    }

    #[cfg(test)]
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

    #[cfg(test)]
    fn from_builtin(bundle: BuiltinNodeSystem) -> Self {
        let project_session_id = ProjectSessionId::new(uuid::Uuid::new_v4().to_string());
        #[cfg(test)]
        let function_plans = Arc::new(FunctionPlanStore::new(project_session_id.clone(), 64));
        Self {
            #[cfg(test)]
            databases: HashMap::new(),
            node_registry: bundle.registry,
            catalog: bundle.catalog,
            #[cfg(test)]
            kernels: Arc::new(build_builtin_kernel_registry()),
            #[cfg(test)]
            compiled_parameters: Arc::new(RwLock::new(CompiledParameterStore::new())),
            #[cfg(test)]
            function_plans,
            #[cfg(test)]
            results: ResultStore::new(),
            #[cfg(test)]
            memoization: Arc::new(SessionMemoization::new()),
            #[cfg(test)]
            runs: Arc::new(ProjectRunRegistry::new()),
            project_session_id,
            #[cfg(test)]
            drop_test_hook: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn finalize_session(&self) {
        self.memoization.finalize();
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
    fn replacing_project_session_invalidates_all_memo_entries() {
        use crate::execution::plan::legacy::{
            ExecutionSemanticsVersion, OperationStableId, PlannedValueContract, ResultPresentation,
            ValueRef,
        };
        use crate::node_system::runtime::RunId;
        use crate::node_system::runtime::{
            ActivationId, ActivationProvenance, CancellationToken, DemandFingerprint,
            OperationMemoKey, PendingOutputDescriptor, ResultId, ResultUsage, StoredValue,
        };
        use yss_graph_analysis_contract::ResourceVersionSet;
        use yss_graph_document::{GraphResourcePath, GraphRevision, NodeId};
        use yss_graph_protocol::Value;

        let old = ProjectStore::new();
        let activation_id = ActivationId::next().unwrap();
        let group = old
            .results
            .create_pending_group(
                ActivationProvenance {
                    run_id: RunId::new(1),
                    activation_id,
                    graph_path: GraphResourcePath::new("events/replaced.yssbi-event").unwrap(),
                    graph_revision: GraphRevision::new(1),
                    node_id: NodeId::from_uuid(uuid::Uuid::nil()),
                    created_at_ms: 1,
                    usage: ResultUsage::Produced,
                },
                &[PendingOutputDescriptor {
                    value: ValueRef::new(0),
                    output: None,
                    presentation: ResultPresentation::Inspector,
                    contract: PlannedValueContract::opaque(),
                }],
            )
            .unwrap();
        old.results
            .complete_group(
                &group,
                vec![StoredValue::scalar(Value::Integer(1))].into_boxed_slice(),
            )
            .unwrap();
        let old_result_id = group.output_result_ids[0];
        let key = OperationMemoKey {
            operation: OperationStableId::new("project-session-replacement").unwrap(),
            input_fingerprints: Box::new([]),
            resource_versions: ResourceVersionSet::new(),
            semantics_version: ExecutionSemanticsVersion::from_bytes([1; 32]),
            computation_settings:
                crate::node_system::runtime::ComputationSettingsFingerprint::from_bytes([3; 32]),
            demand: DemandFingerprint::from_bytes([2; 32]),
        };
        old.memoization
            .get_or_produce(key.clone(), &CancellationToken::new(), || {
                Ok(vec![old_result_id].into_boxed_slice())
            })
            .unwrap();

        old.finalize_session();
        let replacement = ProjectStore::new();

        assert!(replacement.results.result(old_result_id).is_none());
        assert_eq!(
            old.memoization
                .get_or_produce(key.clone(), &CancellationToken::new(), || Ok(vec![
                    ResultId::new(2)
                ]
                .into_boxed_slice()),),
            Err(crate::node_system::runtime::RunError::Cancelled)
        );
        assert_eq!(
            replacement
                .memoization
                .get_or_produce(key, &CancellationToken::new(), || {
                    Ok(vec![ResultId::new(2)].into_boxed_slice())
                })
                .unwrap()
                .as_ref(),
            &[ResultId::new(2)]
        );
    }

    #[test]
    fn project_replacement_drains_memo_producer_and_waiter() {
        use crate::execution::plan::legacy::{ExecutionSemanticsVersion, OperationStableId};
        use crate::node_system::runtime::{
            CancellationToken, ComputationSettingsFingerprint, DemandFingerprint,
            MemoCommitCheckpoint, OperationMemoKey, ResultId, RunError,
        };
        use std::sync::{Arc, Barrier, mpsc};
        use std::time::Duration;
        use yss_graph_analysis_contract::ResourceVersionSet;

        let store = Arc::new(ProjectStore::new());
        let key = OperationMemoKey {
            operation: OperationStableId::new("project-replacement-drain").unwrap(),
            input_fingerprints: Box::new([]),
            resource_versions: ResourceVersionSet::new(),
            semantics_version: ExecutionSemanticsVersion::from_bytes([1; 32]),
            computation_settings: ComputationSettingsFingerprint::from_bytes([3; 32]),
            demand: DemandFingerprint::from_bytes([2; 32]),
        };
        let producer_started = Arc::new(Barrier::new(2));
        let release_producer = Arc::new(Barrier::new(2));
        let producer = {
            let memo = Arc::clone(&store.memoization);
            let key = key.clone();
            let producer_started = Arc::clone(&producer_started);
            let release_producer = Arc::clone(&release_producer);
            std::thread::spawn(move || {
                memo.get_or_produce(key, &CancellationToken::new(), || {
                    producer_started.wait();
                    release_producer.wait();
                    Ok(vec![ResultId::new(1)].into_boxed_slice())
                })
            })
        };
        producer_started.wait();
        let waiter_registered = Arc::new(Barrier::new(2));
        let (waiter_tx, waiter_rx) = mpsc::channel();
        let waiter = {
            let memo = Arc::clone(&store.memoization);
            let key = key.clone();
            let waiter_registered = Arc::clone(&waiter_registered);
            std::thread::spawn(move || {
                let result = memo.get_or_produce_with_commit_checkpoint(
                    key,
                    &CancellationToken::new(),
                    || panic!("finalized waiter must not produce"),
                    |checkpoint| {
                        if checkpoint == MemoCommitCheckpoint::WaiterRegistered {
                            waiter_registered.wait();
                        }
                    },
                );
                waiter_tx.send(result).unwrap();
            })
        };
        waiter_registered.wait();

        store.finalize_session();

        assert_eq!(
            waiter_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
            Err(RunError::Cancelled)
        );
        release_producer.wait();
        assert_eq!(producer.join().unwrap(), Err(RunError::Cancelled));
        waiter.join().unwrap();
    }

    #[test]
    fn project_store_requires_validated_builtin_bundle() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let later_constructions = AtomicUsize::new(0);
        let (mut provider, catalog, alias_keys) =
            crate::graph::catalog::builtin_bundle_parts_for_test().unwrap();
        provider.types[0].title_key = "missing.type.title".parse().unwrap();

        let result = ProjectStore::try_with_builtin_factory_and_constructor(
            || {
                crate::graph::catalog::validate_builtin_bundle_for_test(
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
                crate::graph::catalog::BuiltinAssemblyError::Registration(_)
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
