use super::*;
use crate::node_system::analysis::{
    CompilationBasis, CompileId, CompileProvenance, ProjectSessionId, ResourceKey, ResourceVersion,
};
use crate::node_system::document::{GraphResourcePath, GraphRevision};
use crate::node_system::plan::{
    CompiledResourceRequirement, ExecutionPlan, FunctionPlanHandle, ResourceAccess, ResourceId,
    ResourceKind, StructuredControlRegion,
};
use crate::node_system::registry::RegistryFingerprint;
use polars::prelude::{Column, DataFrame};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn resource_id(value: &str) -> ResourceId {
    ResourceId::new(value).unwrap()
}

fn function_handle(value: &str) -> FunctionPlanHandle {
    FunctionPlanHandle::new(value).unwrap()
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
            },
            compile_id: CompileId::new(1),
        },
        value_count: 0,
        operations: Box::new([]),
        value_sources: Box::new([]),
        value_dependencies: Box::new([]),
        root_region: StructuredControlRegion::Sequence(Box::new([])),
        effect_dependencies: Box::new([]),
        relational_subplans: Box::new([]),
        resources: Box::new([]),
        results: Box::new([]),
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

struct NoFunctions;

impl FunctionPlanProvider for NoFunctions {
    fn get_plan(&self, _: &FunctionPlanHandle) -> Result<Option<Arc<ExecutionPlan>>, Box<str>> {
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

    let error = RunExecutor::new(&KernelRegistry::new(), &provider, &NoFunctions)
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
    let error = RunExecutor::new(&KernelRegistry::new(), &provider, &NoFunctions)
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
    let error = RunExecutor::new(&KernelRegistry::new(), &provider, &NoFunctions)
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
fn concurrent_function_plan_publication_and_calls_keep_run_local_generations() {
    let session = ProjectSessionId::new("project-a");
    let registry = RegistryFingerprint::from_bytes([7; 32]);
    let store = Arc::new(FunctionPlanStore::new(session.clone(), 12));
    let barrier = Arc::new(std::sync::Barrier::new(3));

    let spawn_run = |version: &'static str| {
        let session = session.clone();
        let registry = registry.clone();
        let store = Arc::clone(&store);
        let barrier = Arc::clone(&barrier);
        thread::spawn(move || {
            let resource_versions = versions(&[("functions/shared", version)]);
            barrier.wait();
            let generation = store
                .generation(
                    registry.clone(),
                    resource_versions.clone(),
                    vec![(
                        GraphResourcePath("functions/shared".into()),
                        ResourceVersion::new(version),
                        Arc::new(empty_plan(
                            &session,
                            "functions/shared",
                            &registry,
                            resource_versions,
                        )),
                    )],
                )
                .unwrap();
            for _ in 0..100 {
                let plan = generation
                    .get_plan(&function_handle("functions/shared"))
                    .unwrap()
                    .expect("the run-local generation stays complete");
                assert_eq!(
                    plan.provenance.basis.resource_versions[&ResourceKey::new("functions/shared")]
                        .as_str(),
                    version
                );
                thread::yield_now();
            }
            assert_eq!(generation.recursion_limit(), 12);
        })
    };

    let first = spawn_run("3");
    let second = spawn_run("4");
    barrier.wait();
    first.join().unwrap();
    second.join().unwrap();
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
