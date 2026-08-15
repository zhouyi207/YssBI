use super::*;
use crate::node_system::analysis::{
    CompilationBasis, CompileId, CompileProvenance, ProjectSessionId, ResourceKey, ResourceVersion,
};
use crate::node_system::document::{GraphResourcePath, GraphRevision};
use crate::node_system::plan::{
    CompiledResourceRequirement, ExecutionPlan, FunctionPlanAbi, FunctionPlanHandle,
    PlannedValueContract, ResourceAccess, ResourceId, ResourceKind, StructuredControlRegion,
    ValueRef,
};
use crate::node_system::protocol::OutputProduction;
use crate::node_system::registry::RegistryFingerprint;
use polars::prelude::{Column, DataFrame};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn resource_id(value: &str) -> ResourceId {
    ResourceId::new(value).unwrap()
}

fn versions(entries: &[(&str, &str)]) -> BTreeMap<ResourceKey, ResourceVersion> {
    entries
        .iter()
        .map(|(key, version)| (ResourceKey::new(*key), ResourceVersion::new(*version)))
        .collect()
}

fn empty_plan(
    session: &ProjectSessionId,
    path: &str,
    registry: &RegistryFingerprint,
    resource_versions: BTreeMap<ResourceKey, ResourceVersion>,
) -> ExecutionPlan {
    ExecutionPlan {
        provenance: CompileProvenance {
            project_session_id: session.clone(),
            graph_path: GraphResourcePath(path.into()),
            basis: CompilationBasis {
                graph_revision: GraphRevision::new(1),
                registry_fingerprint: registry.clone(),
                resource_versions,
                resource_observations: Default::default(),
            },
            compile_id: CompileId::new(1),
        },
        value_count: 0,
        operations: Box::new([]),
        value_contracts: BTreeMap::new(),
        value_sources: Box::new([]),
        bound_values: BTreeMap::new(),
        value_dependencies: Box::new([]),
        root_region: StructuredControlRegion::Sequence(Box::new([])),
        effect_dependencies: Box::new([]),
        relational_subplans: Box::new([]),
        resources: Box::new([]),
        results: Box::new([]),
        publications: Box::new([]),
    }
}

#[test]
fn project_resource_version_fingerprint_tracks_variables_and_databases() {
    let first = ProjectResourceSnapshot::new(
        ProjectSessionId::new("project-a"),
        versions(&[("variables/rate", "4"), ("databases/main", "9")]),
    );
    let same = ProjectResourceSnapshot::new(
        ProjectSessionId::new("project-a"),
        versions(&[("databases/main", "9"), ("variables/rate", "4")]),
    );
    let changed = ProjectResourceSnapshot::new(
        ProjectSessionId::new("project-a"),
        versions(&[("variables/rate", "5"), ("databases/main", "9")]),
    );

    assert_eq!(first.version_fingerprint(), same.version_fingerprint());
    assert_ne!(first.version_fingerprint(), changed.version_fingerprint());
}

#[test]
fn project_variable_exclusive_access_is_allowed_for_durable_commit_collection() {
    let variable_id = crate::variable::VariableId::new();
    let resource = resource_id(&format!("variables/{variable_id}"));
    let variable = Arc::new(crate::variable::VariableInstance {
        id: variable_id,
        name: "Rate".into(),
        data_type: crate::graph::value::DataType::Int64,
        data_value: crate::graph::value::DataValue::Int64(1),
        tabular: None,
        description: String::new(),
        scope: crate::variable::VariableScope::Global,
        tags: Vec::new(),
    });
    let session = ProjectSessionId::new("project-a");
    let resource_versions = versions(&[(resource.as_str(), "1")]);
    let snapshot = ProjectResourceSnapshot::new(session.clone(), resource_versions.clone())
        .with_variable(resource.clone(), variable);
    let provider = ProjectResourceProvider::new(snapshot);
    let provenance = empty_plan(
        &session,
        "events/main",
        &RegistryFingerprint::from_bytes([7; 32]),
        resource_versions,
    )
    .provenance;
    let shared = CompiledResourceRequirement {
        resource: resource.clone(),
        kind: ResourceKind::ExternalArtifact,
        access: ResourceAccess::Shared,
        optional: false,
    };

    assert!(
        provider
            .validate_plan(&provenance, std::slice::from_ref(&shared))
            .is_ok()
    );

    let exclusive = CompiledResourceRequirement {
        access: ResourceAccess::Exclusive,
        ..shared
    };
    assert!(provider.validate_plan(&provenance, &[exclusive]).is_ok());
}

#[test]
fn project_resource_provider_rejects_unsupported_access_during_validation() {
    let session = ProjectSessionId::new("project-a");
    let database = resource_id("databases/main");
    let resource_versions = versions(&[(database.as_str(), "1")]);
    let provider = ProjectResourceProvider::new(
        ProjectResourceSnapshot::new(session.clone(), resource_versions.clone()).with_database(
            database.clone(),
            Arc::new(polars::prelude::DataFrame::default()),
        ),
    );
    let provenance = empty_plan(
        &session,
        "events/main",
        &RegistryFingerprint::from_bytes([7; 32]),
        resource_versions,
    )
    .provenance;
    let requirement = CompiledResourceRequirement {
        resource: database,
        kind: ResourceKind::DatabaseConnection,
        access: ResourceAccess::Exclusive,
        optional: false,
    };

    let error = provider
        .validate_plan(&provenance, &[requirement])
        .unwrap_err();
    assert_eq!(
        error.kind(),
        crate::node_system::runtime::ResourceErrorKind::UnsupportedAccess
    );
}

struct UnsupportedAccessProvider;

impl ResourceProvider for UnsupportedAccessProvider {
    fn validate_plan(
        &self,
        _: &CompileProvenance,
        _: &[CompiledResourceRequirement],
    ) -> Result<(), ResourceError> {
        Err(ResourceError::unsupported_access(
            "resource access is unsupported",
        ))
    }

    fn acquire(
        &self,
        _: &CompiledResourceRequirement,
    ) -> Result<Box<dyn ResourceLease>, ResourceError> {
        unreachable!("validation errors must prevent resource acquisition")
    }
}

struct NoFunctions;

impl FunctionPlanProvider for NoFunctions {
    fn get_function(
        &self,
        _: &FunctionPlanHandle,
    ) -> Result<Option<Arc<PublishedFunctionPlan>>, Box<str>> {
        Ok(None)
    }
}

#[test]
fn run_executor_classifies_resource_plan_validation_errors() {
    let variable_id = crate::variable::VariableId::new();
    let resource = resource_id(&format!("variables/{variable_id}"));
    let variable = Arc::new(crate::variable::VariableInstance {
        id: variable_id,
        name: "Rate".into(),
        data_type: crate::graph::value::DataType::Int64,
        data_value: crate::graph::value::DataValue::Int64(1),
        tabular: None,
        description: String::new(),
        scope: crate::variable::VariableScope::Global,
        tags: Vec::new(),
    });
    let session = ProjectSessionId::new("project-a");
    let resource_versions = versions(&[(resource.as_str(), "1")]);
    let provider = ProjectResourceProvider::new(
        ProjectResourceSnapshot::new(session.clone(), resource_versions.clone())
            .with_variable(resource.clone(), variable),
    );
    let mut unsupported = empty_plan(
        &session,
        "events/main",
        &RegistryFingerprint::from_bytes([7; 32]),
        resource_versions,
    );
    unsupported.resources = Box::new([CompiledResourceRequirement {
        resource: resource.clone(),
        kind: ResourceKind::ExternalArtifact,
        access: ResourceAccess::Exclusive,
        optional: false,
    }]);

    let error = RunExecutor::new(
        &KernelRegistry::new(),
        &UnsupportedAccessProvider,
        &NoFunctions,
        ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&unsupported, CancellationToken::new())
    .unwrap_err();
    assert!(matches!(error, RunError::InvalidPlan(_)));
    assert_eq!(RunErrorCode::from(&error), RunErrorCode::InvalidPlan);

    let stale_session = empty_plan(
        &ProjectSessionId::new("project-b"),
        "events/main",
        &RegistryFingerprint::from_bytes([7; 32]),
        BTreeMap::new(),
    );
    let error = RunExecutor::new(
        &KernelRegistry::new(),
        &provider,
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&stale_session, CancellationToken::new())
    .unwrap_err();
    assert!(matches!(error, RunError::ResourceSnapshotMismatch(_)));
    assert_eq!(
        RunErrorCode::from(&error),
        RunErrorCode::ResourceSnapshotMismatch
    );

    let mut stale_version = empty_plan(
        &session,
        "events/main",
        &RegistryFingerprint::from_bytes([7; 32]),
        versions(&[(resource.as_str(), "2")]),
    );
    stale_version.resources = Box::new([CompiledResourceRequirement {
        resource,
        kind: ResourceKind::ExternalArtifact,
        access: ResourceAccess::Shared,
        optional: false,
    }]);
    let error = RunExecutor::new(
        &KernelRegistry::new(),
        &provider,
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&stale_version, CancellationToken::new())
    .unwrap_err();
    assert!(matches!(error, RunError::ResourceSnapshotMismatch(_)));
    assert_eq!(
        RunErrorCode::from(&error),
        RunErrorCode::ResourceSnapshotMismatch
    );
}

#[test]
fn variable_reads_stay_on_the_snapshot() {
    let variable_id = crate::variable::VariableId::new();
    let resource = resource_id(&format!("variables/{variable_id}"));
    let variable = Arc::new(crate::variable::VariableInstance {
        id: variable_id,
        name: "Rate".into(),
        data_type: crate::graph::value::DataType::Int64,
        data_value: crate::graph::value::DataValue::Int64(1),
        tabular: None,
        description: String::new(),
        scope: crate::variable::VariableScope::Global,
        tags: Vec::new(),
    });
    let snapshot = ProjectResourceSnapshot::new(
        ProjectSessionId::new("project-a"),
        versions(&[(resource.as_str(), "1")]),
    )
    .with_variable(resource.clone(), variable);
    let provider = ProjectResourceProvider::new(snapshot);
    let lease = provider
        .acquire(&CompiledResourceRequirement {
            resource: resource.clone(),
            kind: ResourceKind::ExternalArtifact,
            access: ResourceAccess::Shared,
            optional: false,
        })
        .unwrap();
    let access = lease
        .as_any()
        .downcast_ref::<ProjectResourceLease>()
        .unwrap()
        .variable_access()
        .unwrap();

    let cloned = access.read().unwrap();
    assert!(matches!(
        cloned.data_value,
        crate::graph::value::DataValue::Int64(1)
    ));
}

#[test]
fn project_database_scan_applies_limit_before_protocol_materialization() {
    let dataframe =
        DataFrame::new(4, vec![Column::new("value".into(), &[1_i64, 2, 3, 4])]).unwrap();
    let database = ProjectDatabaseSnapshot::Loaded(Arc::new(dataframe));

    let scan = database.load_bounded(Some(2)).unwrap();

    assert_eq!(scan.applied_limit, Some(2));
    assert_eq!(scan.dataframe.height(), 2);
}

#[test]
fn project_resource_lease_owns_data_until_cleanup() {
    let dataframe = Arc::new(DataFrame::default());
    let weak = Arc::downgrade(&dataframe);
    let snapshot = ProjectResourceSnapshot::new(
        ProjectSessionId::new("project-a"),
        versions(&[("databases/main", "1")]),
    )
    .with_database(resource_id("databases/main"), dataframe.clone());
    drop(dataframe);
    let provider = ProjectResourceProvider::new(snapshot);
    let requirement = CompiledResourceRequirement {
        resource: resource_id("databases/main"),
        kind: ResourceKind::DatabaseConnection,
        access: ResourceAccess::Shared,
        optional: false,
    };

    let lease = provider.acquire(&requirement).unwrap();
    drop(provider);
    assert!(weak.upgrade().is_some());
    drop(lease);
    assert!(weak.upgrade().is_none());
}

#[test]
fn function_plan_generation_rejects_stale_abi_provenance() {
    let session = ProjectSessionId::new("project-a");
    let registry = RegistryFingerprint::from_bytes([7; 32]);
    let resource_versions = versions(&[("functions/shared", "4")]);
    let plan = Arc::new(empty_plan(
        &session,
        "functions/shared",
        &registry,
        resource_versions.clone(),
    ));
    let mut stale_provenance = plan.provenance.clone();
    stale_provenance.basis.resource_versions = versions(&[("functions/shared", "3")]);
    let abi = Arc::new(FunctionPlanAbi {
        provenance: stale_provenance,
        parameters: BTreeMap::new(),
        parameter_contracts: BTreeMap::new(),
        results: BTreeMap::new(),
        result_productions: BTreeMap::new(),
        result_contracts: BTreeMap::new(),
    });

    let error = match FunctionPlanStore::new(session, 64).generation(
        registry,
        resource_versions,
        vec![(
            GraphResourcePath("functions/shared".into()),
            ResourceVersion::new("4"),
            plan,
            abi,
        )],
    ) {
        Ok(_) => panic!("stale ABI provenance must be rejected"),
        Err(error) => error,
    };

    assert!(matches!(error, FunctionPlanStoreError::InvalidBasis { .. }));
    assert!(error.to_string().contains("ABI provenance"));
}

#[test]
fn function_plan_generation_rejects_aliased_abi_members() {
    use crate::node_system::document::FunctionParameterId;

    let session = ProjectSessionId::new("project-a");
    let registry = RegistryFingerprint::from_bytes([7; 32]);
    let resource_versions = versions(&[("functions/shared", "4")]);
    let mut plan = empty_plan(
        &session,
        "functions/shared",
        &registry,
        resource_versions.clone(),
    );
    plan.value_count = 1;
    let plan = Arc::new(plan);
    let abi = Arc::new(FunctionPlanAbi {
        provenance: plan.provenance.clone(),
        parameters: BTreeMap::from([
            (FunctionParameterId("left".into()), ValueRef::new(0)),
            (FunctionParameterId("right".into()), ValueRef::new(0)),
        ]),
        parameter_contracts: BTreeMap::from([
            (
                FunctionParameterId("left".into()),
                PlannedValueContract::opaque(),
            ),
            (
                FunctionParameterId("right".into()),
                PlannedValueContract::opaque(),
            ),
        ]),
        results: BTreeMap::new(),
        result_productions: BTreeMap::new(),
        result_contracts: BTreeMap::new(),
    });

    let result = FunctionPlanStore::new(session, 64).generation(
        registry,
        resource_versions,
        vec![(
            GraphResourcePath("functions/shared".into()),
            ResourceVersion::new("4"),
            plan,
            abi,
        )],
    );

    assert!(matches!(
        result,
        Err(FunctionPlanStoreError::InvalidBasis { .. })
    ));
}

#[test]
fn function_plan_generation_requires_exact_result_production_keys() {
    use crate::node_system::document::FunctionParameterId;
    use crate::node_system::plan::PlanValueSource;

    let session = ProjectSessionId::new("project-a");
    let registry = RegistryFingerprint::from_bytes([7; 32]);
    let resource_versions = versions(&[("functions/shared", "4")]);
    let mut plan = empty_plan(
        &session,
        "functions/shared",
        &registry,
        resource_versions.clone(),
    );
    plan.value_count = 1;
    plan.value_sources = Box::new([PlanValueSource::ExternalInput(
        ValueRef::new(0),
        OutputProduction::Streaming,
    )]);
    let result = FunctionParameterId("return".into());
    let generate = |result_productions| {
        FunctionPlanStore::new(session.clone(), 64).generation(
            registry.clone(),
            resource_versions.clone(),
            vec![(
                GraphResourcePath("functions/shared".into()),
                ResourceVersion::new("4"),
                Arc::new(plan.clone()),
                Arc::new(FunctionPlanAbi {
                    provenance: plan.provenance.clone(),
                    parameters: BTreeMap::new(),
                    parameter_contracts: BTreeMap::new(),
                    results: BTreeMap::from([(result.clone(), ValueRef::new(0))]),
                    result_productions,
                    result_contracts: BTreeMap::from([(
                        result.clone(),
                        PlannedValueContract::opaque(),
                    )]),
                }),
            )],
        )
    };

    assert!(matches!(
        generate(BTreeMap::new()),
        Err(FunctionPlanStoreError::InvalidBasis { .. })
    ));
    assert!(matches!(
        generate(BTreeMap::from([
            (result.clone(), OutputProduction::Streaming),
            (
                FunctionParameterId("extra".into()),
                OutputProduction::FullyMaterialized,
            ),
        ])),
        Err(FunctionPlanStoreError::InvalidBasis { .. })
    ));
}

#[test]
fn function_plan_generation_rejects_stale_result_production_contract() {
    use crate::node_system::document::FunctionParameterId;
    use crate::node_system::plan::PlanValueSource;

    let session = ProjectSessionId::new("project-a");
    let registry = RegistryFingerprint::from_bytes([7; 32]);
    let resource_versions = versions(&[("functions/shared", "4")]);
    let mut plan = empty_plan(
        &session,
        "functions/shared",
        &registry,
        resource_versions.clone(),
    );
    plan.value_count = 1;
    plan.value_contracts = BTreeMap::from([(ValueRef::new(0), PlannedValueContract::opaque())]);
    plan.value_sources = Box::new([PlanValueSource::ExternalInput(
        ValueRef::new(0),
        OutputProduction::Streaming,
    )]);
    let result = FunctionParameterId("return".into());
    let abi = FunctionPlanAbi {
        provenance: plan.provenance.clone(),
        parameters: BTreeMap::new(),
        parameter_contracts: BTreeMap::new(),
        results: BTreeMap::from([(result.clone(), ValueRef::new(0))]),
        result_productions: BTreeMap::from([(result.clone(), OutputProduction::FullyMaterialized)]),
        result_contracts: BTreeMap::from([(result, PlannedValueContract::opaque())]),
    };

    let error = match FunctionPlanStore::new(session, 64).generation(
        registry,
        resource_versions,
        vec![(
            GraphResourcePath("functions/shared".into()),
            ResourceVersion::new("4"),
            Arc::new(plan),
            Arc::new(abi),
        )],
    ) {
        Ok(_) => panic!("stale ABI production must not be published"),
        Err(error) => error,
    };

    assert!(matches!(error, FunctionPlanStoreError::InvalidBasis { .. }));
    assert!(error.to_string().contains("result production"));
}

#[test]
fn function_plan_generation_requires_initializable_parameters_and_sourced_results() {
    use crate::node_system::document::FunctionParameterId;
    use crate::node_system::plan::PlanValueSource;

    let session = ProjectSessionId::new("project-a");
    let registry = RegistryFingerprint::from_bytes([7; 32]);
    let resource_versions = versions(&[("functions/shared", "4")]);
    let generate = |plan: ExecutionPlan, abi: FunctionPlanAbi| {
        FunctionPlanStore::new(session.clone(), 64).generation(
            registry.clone(),
            resource_versions.clone(),
            vec![(
                GraphResourcePath("functions/shared".into()),
                ResourceVersion::new("4"),
                Arc::new(plan),
                Arc::new(abi),
            )],
        )
    };

    let mut unsourced_parameter = empty_plan(
        &session,
        "functions/shared",
        &registry,
        resource_versions.clone(),
    );
    unsourced_parameter.value_count = 1;
    let parameter_abi = FunctionPlanAbi {
        provenance: unsourced_parameter.provenance.clone(),
        parameters: BTreeMap::from([(FunctionParameterId("amount".into()), ValueRef::new(0))]),
        parameter_contracts: BTreeMap::new(),
        results: BTreeMap::new(),
        result_productions: BTreeMap::new(),
        result_contracts: BTreeMap::new(),
    };
    assert!(matches!(
        generate(unsourced_parameter, parameter_abi),
        Err(FunctionPlanStoreError::InvalidBasis { .. })
    ));

    let mut unsourced_result = empty_plan(
        &session,
        "functions/shared",
        &registry,
        resource_versions.clone(),
    );
    unsourced_result.value_count = 1;
    let result_abi = FunctionPlanAbi {
        provenance: unsourced_result.provenance.clone(),
        parameters: BTreeMap::new(),
        parameter_contracts: BTreeMap::new(),
        results: BTreeMap::from([(FunctionParameterId("return".into()), ValueRef::new(0))]),
        result_productions: BTreeMap::from([(
            FunctionParameterId("return".into()),
            OutputProduction::FullyMaterialized,
        )]),
        result_contracts: BTreeMap::from([(
            FunctionParameterId("return".into()),
            PlannedValueContract::opaque(),
        )]),
    };
    assert!(matches!(
        generate(unsourced_result, result_abi),
        Err(FunctionPlanStoreError::InvalidBasis { .. })
    ));

    let mut sourced = empty_plan(
        &session,
        "functions/shared",
        &registry,
        resource_versions.clone(),
    );
    sourced.value_count = 1;
    sourced.value_contracts = BTreeMap::from([(ValueRef::new(0), PlannedValueContract::opaque())]);
    sourced.value_sources = Box::new([PlanValueSource::ExternalInput(
        ValueRef::new(0),
        OutputProduction::FullyMaterialized,
    )]);
    let sourced_abi = FunctionPlanAbi {
        provenance: sourced.provenance.clone(),
        parameters: BTreeMap::from([(FunctionParameterId("amount".into()), ValueRef::new(0))]),
        parameter_contracts: BTreeMap::from([(
            FunctionParameterId("amount".into()),
            PlannedValueContract::opaque(),
        )]),
        results: BTreeMap::from([(FunctionParameterId("return".into()), ValueRef::new(0))]),
        result_productions: BTreeMap::from([(
            FunctionParameterId("return".into()),
            OutputProduction::FullyMaterialized,
        )]),
        result_contracts: BTreeMap::from([(
            FunctionParameterId("return".into()),
            PlannedValueContract::opaque(),
        )]),
    };
    assert!(generate(sourced, sourced_abi).is_ok());
}

#[test]
fn project_drain_cancels_and_waits_for_scoped_runs() {
    let registry = Arc::new(ProjectRunRegistry::new());
    let project = ProjectSessionId::new("project-a");
    let token = CancellationToken::new();
    let run = registry
        .track(project.clone(), RunId::new(41), token.clone())
        .unwrap();
    let draining = registry.clone();
    let (finished_tx, finished_rx) = std::sync::mpsc::channel();
    let drain = thread::spawn(move || {
        draining.cancel_and_drain(&project);
        finished_tx.send(()).unwrap();
    });

    let deadline = Instant::now() + Duration::from_secs(1);
    while !token.is_cancelled() && Instant::now() < deadline {
        thread::yield_now();
    }
    assert!(token.is_cancelled());
    assert!(finished_rx.try_recv().is_err());
    drop(run);
    finished_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    drain.join().unwrap();
    assert_eq!(registry.active_run_count(), 0);
}
