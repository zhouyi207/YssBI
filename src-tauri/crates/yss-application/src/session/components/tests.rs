//! An ordinary numeric extension using only public composition and registration APIs.

use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};

use crate::ApplicationState;
use crate::graph::execution_package_from_graph;
use crate::session::{NodeComponents, NodeCompositionError};
use yss_graph_analysis_contract::CompilationBasis;
use yss_graph_document::{DocumentNode, GraphDocument, GraphResourcePath, NodeId, NodePosition};
use yss_graph_execution::kernels::{
    KernelContract, KernelInvocation, KernelRegistrationError, KernelRegistryBuilder,
};
use yss_graph_execution::package_preparation::PackagePreparationError;
use yss_graph_execution::plan::{
    KernelId, PlanCompilationBasis, PlanExecutionDemand, PlanParameterScalar, PlanParameterValue,
    PlanProjectSessionId, PlanRegistryFingerprint,
};
use yss_graph_execution::resource_preparation::RunResourceBindings;
use yss_graph_execution::state::{ExecutePreparedError, KernelExecutionError, RunExecutionControl};
use yss_graph_execution::value::RuntimeValue;
use yss_graph_resource_contract::{ResourceCatalogFingerprint, ResourceCatalogSnapshot};
use yss_node_catalog::{BuiltinCatalog, register_builtin_nodes};
use yss_node_protocol::*;
use yss_node_registry::{
    LeafImplementation, NodeRegistry, NodeRegistryBuilder, ProviderRegistration, RegisteredNode,
    RegistryFingerprint,
};

const ID: &str = "example.numeric.increment";

fn definition() -> NodeProtocol {
    let integer = TypeExpr::Concrete("core.int64".parse().unwrap());
    let ports = [
        ("input", PortDirection::Input),
        ("result", PortDirection::Output),
    ]
    .into_iter()
    .map(|(key, direction)| PortSpec {
        key: key.parse().unwrap(),
        title: key.into(),
        direction,
        value_type: integer.clone(),
        cardinality: PortCardinality::Declared,
        connections: if direction == PortDirection::Input {
            ConnectionsPerPort::Single
        } else {
            ConnectionsPerPort::Multiple {
                max: None,
                ordered: false,
            }
        },
        input_binding: (direction == PortDirection::Input).then(|| InputBindingSpec {
            literal_policy: LiteralPolicy::Allowed,
            default_value: Some(TypedValue {
                value_type: integer.clone(),
                value: Value::Integer(2),
            }),
        }),
        consumption: (direction == PortDirection::Input)
            .then_some(InputConsumption::FullyMaterialized),
        production: (direction == PortDirection::Output)
            .then_some(OutputProduction::FullyMaterialized),
        editor: PortEditorSpec::Default,
        schema: None,
    })
    .collect();
    NodeProtocol {
        type_id: ID.parse().unwrap(),
        catalog: NodeCatalogProtocol {
            title_key: "example.increment.title".parse().unwrap(),
            documentation_key: None,
            aliases_key: None,
            category_id: "numeric".parse().unwrap(),
            icon_id: "builtin.numeric".parse().unwrap(),
            style_id: "builtin.numeric".parse().unwrap(),
            hidden: false,
        },
        interface: NodeInterfaceProtocol::new(ports, vec![]).unwrap(),
        parameters: ParameterSchema::new(vec![ParameterSpec {
            key: "step".parse().unwrap(),
            title_key: "example.increment.step.title".parse().unwrap(),
            description_key: None,
            value_type: integer.clone(),
            default_value: Some(ParameterValue {
                value_type: integer,
                value: Value::Integer(1),
            }),
            constraints: vec![ParameterConstraint::Required],
            editor: ParameterEditorSpec::Number,
            presentation: ParameterPresentation::DetailPanel,
        }])
        .unwrap(),
        instance_display: Default::default(),
        execution: ExecutionSemantics {
            determinism: Determinism::Deterministic,
            cache: CachePolicy::PerSession,
        },
        typing: Default::default(),
        scope: NodeScope::Any,
        managed_role: None,
    }
}

fn definitions() -> (Arc<NodeRegistry>, Arc<BuiltinCatalog>) {
    let mut builder = NodeRegistryBuilder::new();
    let catalog = register_builtin_nodes(&mut builder).unwrap();
    let mut provider = ProviderRegistration::new("example.numeric".parse().unwrap());
    provider.i18n.keys.extend(
        ["example.increment.title", "example.increment.step.title"].map(|key| key.parse().unwrap()),
    );
    provider.nodes = vec![RegisteredNode::leaf(
        Arc::new(definition()),
        LeafImplementation::new(ID),
    )]
    .into_boxed_slice();
    builder.register_provider(provider).unwrap();
    (Arc::new(builder.freeze().unwrap()), Arc::new(catalog))
}

fn increment(
    invocation: &KernelInvocation<'_>,
) -> Result<BTreeMap<yss_graph_execution::plan::PlanOutputRef, RuntimeValue>, KernelExecutionError>
{
    let (
        [RuntimeValue::Integer(input)],
        Some(PlanParameterValue::Scalar(PlanParameterScalar::Integer(step))),
        [output],
    ) = (
        invocation.inputs,
        invocation.parameter("step"),
        invocation.outputs,
    )
    else {
        return Err(KernelExecutionError::InvalidNumericInput);
    };
    let result = input
        .checked_add(*step)
        .ok_or(KernelExecutionError::NonFiniteResult)?;
    Ok(BTreeMap::from([(
        output.output().clone(),
        RuntimeValue::Integer(result),
    )]))
}

fn kernels(revision: Option<u32>) -> KernelRegistryBuilder {
    let mut builder = KernelRegistryBuilder::with_builtins();
    if let Some(revision) = revision {
        builder
            .register(
                KernelId::new(ID.into()).unwrap(),
                NonZeroU32::new(revision).unwrap(),
                KernelContract::new(
                    [
                        yss_graph_execution::plan::PlanParameterFieldId::new("step".into())
                            .unwrap(),
                    ],
                    1..=1,
                )
                .unwrap(),
                increment,
            )
            .unwrap();
    }
    builder
}

fn application(revision: Option<u32>) -> ApplicationState {
    let (registry, catalog) = definitions();
    ApplicationState::initialize_with_nodes(
        NodeComponents::new(registry, catalog, kernels(revision)).unwrap(),
    )
    .unwrap()
}

#[test]
fn numeric_extension_uses_actual_capabilities_and_rejects_old_artifacts() {
    let application = application(Some(1));
    let first = application.capture_session().unwrap();
    let first_fingerprint = first.execution().kernels().fingerprint();
    // Session replacement must retain extension configuration instead of reinstalling defaults.
    application.clear_project_for_application().unwrap();
    let session = application.capture_session().unwrap();
    assert_eq!(
        session.execution().kernels().fingerprint(),
        first_fingerprint
    );
    assert_ne!(session.epoch(), first.epoch());

    let graph = GraphResourcePath::new("events/extension.yssbi-event").unwrap();
    let node = NodeId::new();
    let mut document = GraphDocument::default();
    document.nodes.insert(
        node,
        DocumentNode {
            id: node,
            node_type: ID.parse().unwrap(),
            position: NodePosition { x: 0., y: 0. },
            parameters: BTreeMap::new(),
            user_label: None,
        },
    );
    let resources = ResourceCatalogSnapshot::new(
        BTreeMap::new(),
        BTreeMap::new(),
        ResourceCatalogFingerprint::from_bytes([0; 32]),
    );
    let basis = CompilationBasis {
        registry_fingerprint: RegistryFingerprint::from_bytes(
            session.graph().registry_fingerprint(),
        ),
        kernel_fingerprint: first_fingerprint.as_bytes(),
        resource_versions: BTreeMap::new(),
        resource_observations: BTreeMap::new(),
    };
    let compiled = session
        .graph()
        .compile_draft(&document, graph.clone(), &resources, &basis, &|id| {
            session.execution().kernels().supports(id)
        })
        .unwrap();
    let artifact = *compiled
        .artifact_id()
        .expect("registered numeric extension must compile");
    let cached = session.graph().compiled_draft(&graph, &artifact).unwrap();
    let plan_session =
        PlanProjectSessionId::from_existing(session.project_session_id().as_str().into());
    let package = execution_package_from_graph(
        cached.package().clone(),
        PlanCompilationBasis::new(
            plan_session.clone(),
            PlanRegistryFingerprint::from_bytes(session.graph().registry_fingerprint()),
            first_fingerprint,
            BTreeMap::new(),
            BTreeMap::new(),
        ),
    )
    .unwrap();
    let prepared = session
        .execution()
        .prepare_compiled_package(package.clone(), session.runtime_generation())
        .unwrap();
    let control = RunExecutionControl::with_cancellation(
        Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_secs(10),
    );
    let executed = session
        .execution()
        .execute_prepared_handoff(
            &prepared,
            RunResourceBindings::new(plan_session.clone(), [], []),
            session.resource_provider_factory(),
            &control,
            &PlanExecutionDemand::Default,
            None,
            |_| {},
        )
        .unwrap();
    assert_eq!(
        executed.handoff().results()[0].value().value(),
        &RuntimeValue::Integer(3)
    );

    // Use one Graph cache to prove the capability version participates in its cache key.
    let updated = kernels(Some(2)).freeze();
    assert!(matches!(
        execution_package_from_graph(
            cached.package().clone(),
            PlanCompilationBasis::new(
                plan_session.clone(),
                PlanRegistryFingerprint::from_bytes(session.graph().registry_fingerprint()),
                updated.fingerprint(),
                BTreeMap::new(),
                BTreeMap::new(),
            )
        ),
        Err(crate::graph::GraphPackageMappingError::KernelCapabilitiesMismatch)
    ));
    let mut updated_basis = basis.clone();
    updated_basis.kernel_fingerprint = updated.fingerprint().as_bytes();
    let recompiled = session
        .graph()
        .compile_draft(
            &document,
            graph.clone(),
            &resources,
            &updated_basis,
            &|id| updated.supports(id),
        )
        .unwrap();
    assert_ne!(recompiled.artifact_id(), Some(&artifact));
    assert!(!recompiled.cache_hit());
    assert!(session.graph().compiled_draft(&graph, &artifact).is_none());

    let updated_runtime = yss_graph_execution::state::ExecutionRuntimeState::new(
        session.execution().session_id(),
        session.runtime_generation(),
        Arc::new(updated),
    );
    assert!(matches!(
        updated_runtime.prepare_compiled_package(package, session.runtime_generation()),
        Err(PackagePreparationError::KernelCapabilitiesChanged { .. })
    ));
    assert!(matches!(
        updated_runtime.execute_prepared_handoff(
            &prepared,
            RunResourceBindings::new(plan_session, [], []),
            session.resource_provider_factory(),
            &control,
            &PlanExecutionDemand::Default,
            None,
            |_| {}
        ),
        Err(ExecutePreparedError::KernelCapabilitiesChanged)
    ));

    let missing = kernels(None).freeze();
    let mut missing_basis = basis;
    missing_basis.kernel_fingerprint = missing.fingerprint().as_bytes();
    let blocked = session
        .graph()
        .compile_draft(&document, graph, &resources, &missing_basis, &|id| {
            missing.supports(id)
        })
        .unwrap();
    assert!(blocked.artifact_id().is_none());
    assert!(
        blocked
            .analysis()
            .semantic_snapshot()
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code.as_str() == "compiler.node.kernel_unavailable")
    );
}

#[test]
fn conflicting_registrations_and_node_kernel_contracts_are_rejected() {
    let mut builder = kernels(Some(1));
    let contract = KernelContract::new(
        [yss_graph_execution::plan::PlanParameterFieldId::new("step".into()).unwrap()],
        1..=1,
    )
    .unwrap();
    assert!(matches!(
        builder.register(
            KernelId::new(ID.into()).unwrap(),
            NonZeroU32::new(2).unwrap(),
            contract,
            increment
        ),
        Err(KernelRegistrationError::DuplicateKernel(_))
    ));
    for contract in [
        KernelContract::new([], 1..=1).unwrap(),
        KernelContract::new(
            [yss_graph_execution::plan::PlanParameterFieldId::new("step".into()).unwrap()],
            2..=2,
        )
        .unwrap(),
    ] {
        let mut builder = kernels(None);
        builder
            .register(
                KernelId::new(ID.into()).unwrap(),
                NonZeroU32::new(1).unwrap(),
                contract,
                increment,
            )
            .unwrap();
        let (registry, catalog) = definitions();
        assert!(matches!(
            NodeComponents::new(registry, catalog, builder),
            Err(NodeCompositionError::Binding { .. })
        ));
    }
}
