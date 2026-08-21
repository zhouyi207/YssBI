use crate::database::DatabaseInstance;
use crate::node_system::ProjectSessionId;
use crate::node_system::catalog::{
    BuiltinCatalog, BuiltinInitializationError, BuiltinNodeSystem, build_builtin_node_system,
};
use crate::node_system::registry::NodeRegistry;
use crate::node_system::runtime::{
    CompiledParameterStore, FunctionPlanStore, KernelRegistry, ProjectRunRegistry, ResultStore,
    SessionMemoization, build_builtin_kernel_registry,
};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub struct ProjectStore {
    pub databases: HashMap<String, DatabaseInstance>,
    pub node_registry: Arc<NodeRegistry>,
    pub catalog: Arc<BuiltinCatalog>,
    pub kernels: Arc<KernelRegistry>,
    pub compiled_parameters: Arc<RwLock<CompiledParameterStore>>,
    pub function_plans: Arc<FunctionPlanStore>,
    pub results: ResultStore,
    pub memoization: Arc<SessionMemoization>,
    pub runs: Arc<ProjectRunRegistry>,
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

    fn from_builtin(bundle: BuiltinNodeSystem) -> Self {
        let project_session_id = ProjectSessionId::new(uuid::Uuid::new_v4().to_string());
        let function_plans = Arc::new(FunctionPlanStore::new(project_session_id.clone(), 64));
        Self {
            databases: HashMap::new(),
            node_registry: bundle.registry,
            catalog: bundle.catalog,
            kernels: Arc::new(build_builtin_kernel_registry()),
            compiled_parameters: Arc::new(RwLock::new(CompiledParameterStore::new())),
            function_plans,
            results: ResultStore::new(),
            memoization: Arc::new(SessionMemoization::new()),
            runs: Arc::new(ProjectRunRegistry::new()),
            project_session_id,
            #[cfg(test)]
            drop_test_hook: None,
        }
    }

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
        use crate::node_system::analysis::ResourceVersionSet;
        use crate::node_system::document::{GraphResourcePath, GraphRevision, NodeId};
        use crate::node_system::plan::{
            ExecutionSemanticsVersion, OperationStableId, PlannedValueContract, ResultPresentation,
            ValueRef,
        };
        use crate::node_system::protocol::Value;
        use crate::node_system::runtime::RunId;
        use crate::node_system::runtime::{
            ActivationId, ActivationProvenance, CancellationToken, DemandFingerprint,
            OperationMemoKey, PendingOutputDescriptor, ResultId, ResultUsage, StoredValue,
        };

        let old = ProjectStore::new();
        let activation_id = ActivationId::next().unwrap();
        let group = old
            .results
            .create_pending_group(
                ActivationProvenance {
                    run_id: RunId::new(1),
                    activation_id,
                    graph_path: GraphResourcePath("events/replaced".into()),
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
        use crate::node_system::analysis::ResourceVersionSet;
        use crate::node_system::plan::{ExecutionSemanticsVersion, OperationStableId};
        use crate::node_system::runtime::{
            CancellationToken, ComputationSettingsFingerprint, DemandFingerprint,
            MemoCommitCheckpoint, OperationMemoKey, ResultId, RunError,
        };
        use std::sync::{Arc, Barrier, mpsc};
        use std::time::Duration;

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
