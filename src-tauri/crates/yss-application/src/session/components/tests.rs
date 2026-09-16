//! An ordinary numeric extension using only public composition and registration APIs.

use yss_node_kernel::KernelId;

use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};

use crate::ApplicationState;
use crate::session::{NodeComponents, NodeCompositionError};
use yss_graph_analysis_contract::GraphAnalysisBasis;
use yss_graph_document::{DocumentNode, GraphDocument, GraphResourcePath, NodeId, NodePosition};
use yss_graph_execution::package_preparation::PackagePreparationError;
use yss_graph_execution::plan::{
    PlanBasis, PlanExecutionDemand, PlanProjectSessionId, PlanRegistryFingerprint,
};
use yss_graph_execution::resource_preparation::RunResourceBindings;
use yss_graph_execution::state::{ExecutePreparedError, RunExecutionControl};
use yss_graph_resource_contract::{ResourceCatalogFingerprint, ResourceCatalogSnapshot};
use yss_node_catalog::{BuiltinCatalog, register_builtin_nodes};
use yss_node_kernel::KernelError;
use yss_node_kernel::RuntimeValue;
use yss_node_kernel::{
    KernelContract, KernelInvocation, KernelRegistrationError, KernelRegistryBuilder,
};
use yss_node_protocol::*;
use yss_node_registry::{
    LeafImplementation, NodeRegistry, NodeRegistryBuilder, ProviderRegistration, RegisteredNode,
    RegistryFingerprint,
};

const ID: &str = "example.numeric.increment";

fn definition() -> NodeProtocol {
    let integer = TypeExpr::Concrete("core.numeric".parse().unwrap());
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

fn increment(invocation: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let ([RuntimeValue::Integer(input)], Some(RuntimeValue::Integer(step))) =
        (invocation.inputs, invocation.parameter("step"))
    else {
        return Err(KernelError::InvalidNumericInput);
    };
    let result = input
        .checked_add(*step)
        .ok_or(KernelError::NonFiniteResult)?;
    Ok(vec![RuntimeValue::Integer(result)])
}

fn kernels(revision: Option<u32>) -> KernelRegistryBuilder {
    let mut builder = KernelRegistryBuilder::with_builtins();
    if let Some(revision) = revision {
        builder
            .register(
                KernelId::new(ID.into()).unwrap(),
                NonZeroU32::new(revision).unwrap(),
                KernelContract::new(
                    [yss_node_kernel::KernelParameterKey::new("step".into()).unwrap()],
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
    let basis = GraphAnalysisBasis {
        registry_fingerprint: RegistryFingerprint::from_bytes(
            session.graph().registry_fingerprint(),
        ),
        kernel_fingerprint: first_fingerprint.as_bytes(),
        resource_versions: BTreeMap::new(),
        resource_observations: BTreeMap::new(),
    };
    let resolve = |basis: &GraphAnalysisBasis, kernels: &yss_node_kernel::KernelRegistry| {
        let analysis = session.graph().resolve_graph_document(
            &graph,
            &document,
            basis,
            &resources,
            &[],
            "en-US",
        );
        let semantics = analysis
            .semantic_snapshot()
            .clone()
            .with_execution_kernel_support(&|id| kernels.supports(id));
        analysis.with_semantic_snapshot(semantics)
    };
    let analysis = resolve(&basis, session.execution().kernels());
    let plan_session =
        PlanProjectSessionId::from_existing(session.project_session_id().as_str().into());
    let plan_basis = |fingerprint| {
        PlanBasis::new(
            plan_session.clone(),
            PlanRegistryFingerprint::from_bytes(session.graph().registry_fingerprint()),
            fingerprint,
            BTreeMap::new(),
            BTreeMap::new(),
        )
    };
    let package = session
        .execution()
        .prepare_graph_package(&graph, &analysis, plan_basis(first_fingerprint))
        .unwrap();
    let prepared = session
        .execution()
        .prepare_package(package.clone(), session.runtime_generation())
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

    let updated = kernels(Some(2)).freeze();
    assert!(matches!(
        session.execution().prepare_graph_package(
            &graph,
            &analysis,
            plan_basis(updated.fingerprint())
        ),
        Err(yss_graph_execution::graph_preparation::GraphPlanError::KernelCapabilitiesMismatch)
    ));
    let mut updated_basis = basis.clone();
    updated_basis.kernel_fingerprint = updated.fingerprint().as_bytes();
    let revised = resolve(&updated_basis, &updated);
    assert_ne!(
        revised.semantic_input_hash(),
        analysis.semantic_input_hash()
    );

    let updated_runtime = yss_graph_execution::state::ExecutionRuntimeState::new(
        session.execution().session_id(),
        session.runtime_generation(),
        Arc::new(updated),
    );
    let revised_package = updated_runtime
        .prepare_graph_package(
            &graph,
            &revised,
            plan_basis(updated_runtime.kernels().fingerprint()),
        )
        .unwrap();
    assert!(!Arc::ptr_eq(package.plan(), revised_package.plan()));
    assert!(matches!(
        updated_runtime.prepare_package(package, session.runtime_generation()),
        Err(PackagePreparationError::KernelCapabilitiesChanged { .. })
    ));
    assert!(matches!(
        updated_runtime.execute_prepared_handoff(
            &prepared,
            RunResourceBindings::new(plan_session.clone(), [], []),
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
    let blocked = resolve(&missing_basis, &missing);
    let missing_runtime = yss_graph_execution::state::ExecutionRuntimeState::new(
        session.execution().session_id(),
        session.runtime_generation(),
        Arc::new(missing),
    );
    assert!(matches!(
        missing_runtime.prepare_graph_package(
            &graph,
            &blocked,
            plan_basis(missing_runtime.kernels().fingerprint())
        ),
        Err(yss_graph_execution::graph_preparation::GraphPlanError::NotReady)
    ));
    assert!(
        blocked
            .semantic_snapshot()
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code.as_str() == "graph.node.kernel_unavailable")
    );
}

#[test]
fn conflicting_registrations_and_node_kernel_contracts_are_rejected() {
    let mut builder = kernels(Some(1));
    let contract = KernelContract::new(
        [yss_node_kernel::KernelParameterKey::new("step".into()).unwrap()],
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
            [yss_node_kernel::KernelParameterKey::new("step".into()).unwrap()],
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
