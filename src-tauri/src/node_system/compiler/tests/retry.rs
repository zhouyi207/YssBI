use super::*;

#[test]
fn effective_cache_policy_disables_every_effect_semantics_independently() {
    for effects in [EffectSemantics::Ordered, EffectSemantics::Exclusive] {
        assert_eq!(
            super::super::pipeline::effective_cache_policy(
                CachePolicy::PerRun,
                Determinism::Deterministic,
                Purity::Pure,
                effects,
            ),
            CachePolicy::Disabled
        );
    }
}

#[test]
fn retry_compiler_authority_retains_only_explicit_safe_protocol_policy() {
    let policy = RetryPolicy::new(
        std::num::NonZeroU32::new(3).unwrap(),
        std::time::Duration::from_millis(2),
        std::time::Duration::from_millis(8),
    )
    .unwrap();
    let native =
        super::super::pipeline::PendingKernel::Native(KernelHandle::new("test.retry").unwrap());
    let safe = super::super::pipeline::effective_retry_policy(
        true,
        Some(policy),
        Determinism::Deterministic,
        Purity::Pure,
        EffectSemantics::None,
        false,
        &native,
        &[],
    );
    assert_eq!(
        safe,
        PlannedRetry {
            idempotent: true,
            policy: Some(policy),
        }
    );

    let shared_resource = CompiledResourceRequirement {
        resource: ResourceId::new("database/read").unwrap(),
        kind: ResourceKind::DatabaseConnection,
        access: ResourceAccess::Shared,
        optional: false,
    };
    let unsafe_cases = [
        super::super::pipeline::effective_retry_policy(
            false,
            Some(policy),
            Determinism::Deterministic,
            Purity::Pure,
            EffectSemantics::None,
            false,
            &native,
            &[],
        ),
        super::super::pipeline::effective_retry_policy(
            true,
            Some(policy),
            Determinism::NonDeterministic,
            Purity::Pure,
            EffectSemantics::None,
            false,
            &native,
            &[],
        ),
        super::super::pipeline::effective_retry_policy(
            true,
            Some(policy),
            Determinism::Deterministic,
            Purity::Effectful,
            EffectSemantics::None,
            false,
            &native,
            &[],
        ),
        super::super::pipeline::effective_retry_policy(
            true,
            Some(policy),
            Determinism::Deterministic,
            Purity::Pure,
            EffectSemantics::Ordered,
            false,
            &native,
            &[],
        ),
        super::super::pipeline::effective_retry_policy(
            true,
            Some(policy),
            Determinism::Deterministic,
            Purity::Pure,
            EffectSemantics::None,
            true,
            &native,
            &[],
        ),
        super::super::pipeline::effective_retry_policy(
            true,
            Some(policy),
            Determinism::Deterministic,
            Purity::Pure,
            EffectSemantics::None,
            false,
            &native,
            std::slice::from_ref(&shared_resource),
        ),
        super::super::pipeline::effective_retry_policy(
            true,
            None,
            Determinism::Deterministic,
            Purity::Pure,
            EffectSemantics::None,
            false,
            &native,
            &[],
        ),
        super::super::pipeline::effective_retry_policy(
            true,
            Some(policy),
            Determinism::Deterministic,
            Purity::Pure,
            EffectSemantics::None,
            false,
            &super::super::pipeline::PendingKernel::Relational,
            &[],
        ),
    ];
    assert!(
        unsafe_cases
            .into_iter()
            .all(|retry| retry == PlannedRetry::default())
    );
}

#[test]
fn retry_unsafe_operation_matrix_forces_effect_call_relational_and_resource_native_off() {
    let policy = RetryPolicy::new(
        std::num::NonZeroU32::new(2).unwrap(),
        std::time::Duration::ZERO,
        std::time::Duration::ZERO,
    )
    .unwrap();
    let native =
        super::super::pipeline::PendingKernel::Native(KernelHandle::new("test.retry").unwrap());
    let resource = CompiledResourceRequirement {
        resource: ResourceId::new("database/read").unwrap(),
        kind: ResourceKind::DatabaseConnection,
        access: ResourceAccess::Shared,
        optional: false,
    };
    let cases = [
        (
            "native effect edge",
            super::super::pipeline::effective_retry_policy(
                true,
                Some(policy),
                Determinism::Deterministic,
                Purity::Effectful,
                EffectSemantics::Ordered,
                true,
                &native,
                &[],
            ),
        ),
        (
            "call",
            super::super::pipeline::effective_retry_policy(
                true,
                Some(policy),
                Determinism::Deterministic,
                Purity::Pure,
                EffectSemantics::None,
                true,
                &native,
                &[],
            ),
        ),
        (
            "relational",
            super::super::pipeline::effective_retry_policy(
                true,
                Some(policy),
                Determinism::Deterministic,
                Purity::Pure,
                EffectSemantics::None,
                false,
                &super::super::pipeline::PendingKernel::Relational,
                &[],
            ),
        ),
        (
            "resource-backed native",
            super::super::pipeline::effective_retry_policy(
                true,
                Some(policy),
                Determinism::Deterministic,
                Purity::Pure,
                EffectSemantics::None,
                false,
                &native,
                std::slice::from_ref(&resource),
            ),
        ),
    ];

    for (case, retry) in cases {
        assert_eq!(retry, PlannedRetry::default(), "{case}");
    }
}

#[test]
fn operation_stable_ids_include_canonical_graph_identity() {
    let protocol = test_protocol("stable_graph_identity", vec![], vec![], vec![]);
    let registry = TestRegistry::new(vec![protocol]);
    let graph = graph_with_nodes(&[(7, "stable_graph_identity")]);
    let compiler = GraphCompiler::new(&registry, &Resources);
    let compile = |path: &str| {
        compiler
            .compile_snapshot(
                &compiler.snapshot(GraphResourcePath::new(path).unwrap(), &graph),
                &CompileCancellationToken::new(),
            )
            .unwrap()
            .plan
            .unwrap()
            .operations[0]
            .stable_id
            .clone()
    };

    assert_ne!(
        compile("events/first.yssbi-event"),
        compile("events/second.yssbi-event")
    );
}

#[test]
fn execution_semantics_version_is_sensitive_to_registry_and_parameters() {
    let mut protocol = test_protocol("semantics_identity", vec![], vec![], vec![]);
    protocol.parameters = ParameterSchema::new(vec![ParameterSpec {
        key: ParameterKey::new("value").unwrap(),
        title_key: I18nKey::new("parameters.value.title").unwrap(),
        description_key: None,
        value_type: TypeExpr::Concrete(TypeId::new("core.int64").unwrap()),
        default_value: None,
        constraints: vec![ParameterConstraint::Required],
        editor: ParameterEditorSpec::Auto,
        presentation: ParameterPresentation::DetailPanel,
    }])
    .unwrap();
    let mut first_registry = TestRegistry::new(vec![protocol.clone()]);
    first_registry.fingerprint = RegistryFingerprint::from_bytes([1; 32]);
    let mut second_registry = TestRegistry::new(vec![protocol]);
    second_registry.fingerprint = RegistryFingerprint::from_bytes([2; 32]);
    let graph = |value| {
        let mut graph = graph_with_nodes(&[(7, "semantics_identity")]);
        graph.nodes.get_mut(&node_id(7)).unwrap().parameters = BTreeMap::from([(
            ParameterKey::new("value").unwrap(),
            serde_json::json!(value),
        )]);
        graph
    };
    let compile = |registry: &TestRegistry, graph: &GraphDocument| {
        GraphCompiler::new(registry, &Resources)
            .compile(graph)
            .plan
            .unwrap()
            .operations[0]
            .semantics_version
    };

    let baseline = compile(&first_registry, &graph(1));
    assert_ne!(baseline, compile(&second_registry, &graph(1)));
    assert_ne!(baseline, compile(&first_registry, &graph(2)));
    assert_ne!(baseline.as_bytes(), &[0; 32]);
}

#[test]
fn effective_cache_policy_matrix_is_carried_into_plans() {
    let deterministic = test_protocol("cache_deterministic", vec![], vec![], vec![]);
    let mut nondeterministic = test_protocol("cache_nondeterministic", vec![], vec![], vec![]);
    nondeterministic.execution.determinism = Determinism::NonDeterministic;
    let mut effectful = test_protocol("cache_effectful", vec![], vec![], vec![]);
    effectful.execution.purity = Purity::Effectful;
    effectful.execution.effects = EffectSemantics::Ordered;
    let registry = TestRegistry::new(vec![deterministic, nondeterministic, effectful]);

    let plan = GraphCompiler::new(&registry, &Resources)
        .compile(&graph_with_nodes(&[
            (1, "cache_deterministic"),
            (2, "cache_nondeterministic"),
            (3, "cache_effectful"),
        ]))
        .plan
        .expect("cache-policy matrix should compile");
    let operations = plan
        .operations
        .iter()
        .map(|operation| (operation.source_node_id, operation))
        .collect::<BTreeMap<_, _>>();

    assert_eq!(operations[&node_id(1)].cache_policy, CachePolicy::PerRun);
    assert_eq!(operations[&node_id(2)].cache_policy, CachePolicy::Disabled);
    assert_eq!(operations[&node_id(3)].cache_policy, CachePolicy::Disabled);
    assert_eq!(operations[&node_id(1)].workload, WorkloadClass::Cpu);
    assert_eq!(operations[&node_id(3)].workload, WorkloadClass::Exclusive);
    assert_ne!(
        operations[&node_id(1)].semantics_version.as_bytes(),
        &[0; 32]
    );
    assert_eq!(operations[&node_id(1)].retry, PlannedRetry::default());
    assert!(
        operations
            .values()
            .all(|operation| operation.resource_dependencies.is_empty())
    );
}

#[test]
fn effective_cache_policy_metadata_survives_demand_specialization() {
    let basis = compiled_demand_basis();
    let expected = basis
        .operations
        .iter()
        .map(|operation| {
            (
                operation.source_node_id,
                (
                    operation.stable_id.clone(),
                    operation.cache_policy,
                    operation.semantics_version,
                    operation.workload,
                    operation.retry.clone(),
                    operation.resource_dependencies.clone(),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let plan = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/main.yssbi-event", 4, "out")]),
            include_default_results: false,
        })
        .expect("selected chain should specialize");

    assert!(!plan.operations.is_empty());
    for operation in &plan.operations {
        if matches!(operation.kernel, PlannedKernel::Adapter(_)) {
            assert_eq!(operation.cache_policy, CachePolicy::Disabled);
            assert_eq!(operation.workload, WorkloadClass::AdapterIo);
            continue;
        }
        let metadata = &expected[&operation.source_node_id];
        assert_eq!(&operation.stable_id, &metadata.0);
        assert_eq!(operation.cache_policy, metadata.1);
        assert_eq!(operation.semantics_version, metadata.2);
        assert_eq!(operation.workload, metadata.3);
        assert_eq!(&operation.retry, &metadata.4);
        assert_eq!(&operation.resource_dependencies, &metadata.5);
    }
}
