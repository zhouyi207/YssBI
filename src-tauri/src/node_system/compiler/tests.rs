use super::*;
use crate::node_system::analysis::{
    CompileId, DiagnosticLocation, DiagnosticSeverity, NOOP_TRACE_SINK, ProjectSessionId,
    ResourceKey, ResourceVersion, SYSTEM_TRACE_CLOCK, SpanGuard, SpanKind, SpanSpec, TraceSink,
    TraceSpan,
};
use crate::node_system::document::{
    ConnectionId, DocumentConnection, DocumentNode, DynamicMemberLocator, DynamicPortBinding,
    FunctionDocument, FunctionParameter, FunctionParameterId, FunctionSignature, GraphDocument,
    GraphResourcePath, InputState, NodeId, NodePosition, OrderKey, PortAddress, PortInstanceId,
    PortRef,
};
use crate::node_system::plan::{
    BranchResultBinding, CallResultBinding, CompiledParameterHandle, CompiledResourceRequirement,
    ControlStep, ExecutionDemand, ExecutionPlan, ExecutionSemanticsVersion, FunctionPlanAbi,
    FunctionPlanHandle, GraphOutputRef, KernelHandle, LoopCarriedBinding, PlanResult,
    PlanValidationError, PlanValueSource, PlannedAdapter, PlannedKernel, PlannedOperation,
    PlannedPublication, PlannedRetry, RelationalBackendId, RelationalExpression,
    RelationalFragmentId, RelationalLiteral, RelationalOperator, RelationalOperatorIndex,
    RelationalPushdownHint, ResourceAccess, ResourceId, ResourceKind, ResultPresentation,
    ResultReportKind, StructuredControlRegion, ValueDependency, ValueRef, WorkloadClass,
};
use crate::node_system::protocol::*;
use crate::node_system::registry::{
    CategoryRegistration, I18nManifest, NodeRegistry, NodeRegistryBuilder, ProtocolFingerprint,
    ProviderRegistration, RegisteredNode, RegistryFingerprint, StructuralNodeRole,
    TypeRegistration,
};
use crate::node_system::testing::TestProtocolBuilder;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

struct ConstantLowerer;
impl NodeLowerer for ConstantLowerer {
    fn lower(&self, _: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        Ok(LoweredNode {
            kernel: LoweredKernel::Native(KernelHandle::new("test.constant").unwrap()),
            parameters: CompiledParameterHandle::new("test.params").unwrap(),
        })
    }
}

#[derive(Default)]
struct RecordingTrace(Mutex<Vec<TraceSpan>>);

impl TraceSink for RecordingTrace {
    fn start_span(&self, spec: SpanSpec) -> SpanGuard<'_> {
        SpanGuard::new(self, spec, &SYSTEM_TRACE_CLOCK)
    }

    fn complete_span(&self, span: TraceSpan) {
        self.0.lock().unwrap().push(span);
    }
}

struct CountingLowerer(Arc<AtomicUsize>);

impl NodeLowerer for CountingLowerer {
    fn lower(&self, _: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        self.0.fetch_add(1, AtomicOrdering::Relaxed);
        Ok(LoweredNode {
            kernel: LoweredKernel::Native(KernelHandle::new("test.counting").unwrap()),
            parameters: CompiledParameterHandle::new("test.counting.params").unwrap(),
        })
    }
}

fn assert_analysis_blocks_before_lowering(
    result: &CompileResult,
    trace: &RecordingTrace,
    calls: &AtomicUsize,
) {
    assert!(result.semantic.is_none());
    assert!(result.plan.is_none());
    assert_eq!(calls.load(AtomicOrdering::Relaxed), 0);
    assert!(
        trace
            .0
            .lock()
            .unwrap()
            .iter()
            .all(|span| span.kind != SpanKind::Lowering)
    );
}

struct Resources;
impl ResourceSnapshot for Resources {
    fn versions(&self) -> crate::node_system::analysis::ResourceVersionSet {
        BTreeMap::from([(ResourceKey::new("resource.test"), ResourceVersion::new("1"))])
    }
}

fn node_id(value: u128) -> NodeId {
    NodeId::from_uuid(Uuid::from_u128(value))
}
fn key(value: &str) -> PortKey {
    PortKey::new(value).unwrap()
}

fn protocol() -> NodeProtocol {
    TestProtocolBuilder::new("yssbi.test.constant", "test")
        .style("test")
        .ports(vec![PortSpec {
            key: PortKey::new("value").unwrap(),
            label_key: I18nKey::new("nodes.test.constant.value").unwrap(),
            direction: PortDirection::Output,
            kind: PortKind::Data,
            value_type: TypeExpr::Concrete(TypeId::new("core.int64").unwrap()),
            instances: PortInstances::Declared,
            connections: ConnectionsPerPort::Multiple {
                max: None,
                ordered: false,
            },
            input_binding: None,
            consumption: None,
            production: None,
            editor: PortEditorSpec::Default,
            schema: None,
        }])
        .build()
}

struct CorruptCompilerRegistrySnapshot {
    frozen: NodeRegistry,
}

impl TypeEnvironment for CorruptCompilerRegistrySnapshot {
    fn concrete_implements(&self, value_type: &TypeId, class: &TypeClassId) -> Option<bool> {
        self.frozen
            .types()
            .get(value_type)
            .map(|registration| registration.classes.contains(class))
    }

    fn constructor_arity(&self, constructor: &TypeConstructorId) -> Option<usize> {
        self.frozen
            .types()
            .constructor(constructor)
            .map(|registration| registration.arity as usize)
    }
}

impl CompilerRegistry for CorruptCompilerRegistrySnapshot {
    fn fingerprint(&self) -> &RegistryFingerprint {
        self.frozen.fingerprint()
    }

    fn resolve(&self, node_type: &NodeTypeId) -> Option<RegistryNode<'_>> {
        let registered = self.frozen.get(node_type)?;
        Some(RegistryNode {
            protocol: registered.protocol(),
            protocol_fingerprint: self
                .frozen
                .catalog_manifest()
                .node_protocols
                .get(node_type)?
                .clone(),
            behavior: RegistryNodeBehavior::ProtocolOnly,
        })
    }
}

struct AugmentedCompilerRegistry<'a> {
    frozen: &'a NodeRegistry,
    protocol: NodeProtocol,
    implementation: NodeImplementation,
}

impl TypeEnvironment for AugmentedCompilerRegistry<'_> {
    fn concrete_implements(&self, value_type: &TypeId, class: &TypeClassId) -> Option<bool> {
        self.frozen.concrete_implements(value_type, class)
    }

    fn constructor_arity(&self, constructor: &TypeConstructorId) -> Option<usize> {
        self.frozen.constructor_arity(constructor)
    }
}

impl CompilerRegistry for AugmentedCompilerRegistry<'_> {
    fn fingerprint(&self) -> &RegistryFingerprint {
        self.frozen.fingerprint()
    }

    fn resolve(&self, node_type: &NodeTypeId) -> Option<RegistryNode<'_>> {
        if node_type == &self.protocol.type_id {
            return Some(RegistryNode {
                protocol: &self.protocol,
                protocol_fingerprint: ProtocolFingerprint::from_bytes([0x8; 32]),
                behavior: RegistryNodeBehavior::Leaf(&self.implementation),
            });
        }
        self.frozen.resolve(node_type)
    }

    fn validate_nominal_parameter(
        &self,
        type_id: &TypeId,
        value: &serde_json::Value,
    ) -> Option<Result<(), String>> {
        self.frozen.validate_nominal_parameter(type_id, value)
    }
}

fn registry() -> NodeRegistry {
    let mut provider = ProviderRegistration::new(ProviderId::new("yssbi").unwrap());
    provider.types = vec![TypeRegistration {
        id: TypeId::new("core.int64").unwrap(),
        title_key: I18nKey::new("types.int64.title").unwrap(),
        classes: BTreeSet::new(),
    }]
    .into_boxed_slice();
    provider.categories = vec![CategoryRegistration {
        id: NodeCategoryId::new("test").unwrap(),
        title_key: I18nKey::new("categories.test.title").unwrap(),
        parent: None,
        order: 0,
    }]
    .into_boxed_slice();
    provider.i18n = I18nManifest {
        keys: BTreeSet::from([
            I18nKey::new("types.int64.title").unwrap(),
            I18nKey::new("categories.test.title").unwrap(),
            I18nKey::new("nodes.test.constant.title").unwrap(),
            I18nKey::new("nodes.test.constant.value").unwrap(),
        ]),
    };
    provider.nodes = vec![RegisteredNode::leaf(
        Arc::new(protocol()),
        Arc::new(NodeImplementation::new(ConstantLowerer)),
    )]
    .into_boxed_slice();
    let mut builder = NodeRegistryBuilder::new();
    builder.register_provider(provider).unwrap();
    builder.freeze().unwrap()
}
fn document(node_type: NodeTypeId) -> GraphDocument {
    let id = node_id(1);
    GraphDocument {
        revision: crate::node_system::document::GraphRevision::new(7),
        nodes: BTreeMap::from([(
            id,
            DocumentNode {
                id,
                node_type,
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: BTreeMap::new(),
                user_label: None,
            },
        )]),
        port_bindings: BTreeMap::new(),
        connections: BTreeMap::new(),
        input_states: BTreeMap::new(),
    }
}

#[test]
fn corrupt_registry_snapshot_produces_missing_lowering_blocking_diagnostic() {
    let registry = CorruptCompilerRegistrySnapshot { frozen: registry() };
    let result = GraphCompiler::new(&registry, &Resources)
        .compile(&document(NodeTypeId::new("yssbi.test.constant").unwrap()));

    assert!(result.semantic.is_some());
    assert!(result.plan.is_none());
    assert!(result.analysis.diagnostics.is_empty());
    assert!(matches!(
        result.outcome,
        CompilationOutcome::InternalFailure(ref failure)
            if failure.stage == CompilationStage::Lowering
                && failure.code.as_ref() == "compiler.lowering.implementation_missing"
                && failure.node_id == Some(node_id(1))
    ));
}

#[test]
fn valid_constant_graph_produces_plan_with_same_basis() {
    let registry = registry();
    let result = GraphCompiler::new(&registry, &Resources)
        .compile(&document(NodeTypeId::new("yssbi.test.constant").unwrap()));
    let semantic = result
        .semantic
        .expect("valid graph should retain its semantic graph");
    let plan = result.plan.expect("valid graph should lower");
    assert!(result.analysis.diagnostics.is_empty());
    assert_eq!(plan.operations.len(), 1);
    assert_eq!(semantic.basis, result.analysis.basis);
    assert_eq!(plan.provenance.basis, semantic.basis);
}

#[test]
fn compile_plan_and_trace_keep_the_exact_requested_correlation() {
    let registry = registry();
    let trace = RecordingTrace::default();
    let compiler = GraphCompiler::new(&registry, &Resources)
        .with_observability(ProjectSessionId::new("project-session-1"), &trace);
    let snapshot = compiler.snapshot_with_compile_id(
        CompileId::new(41),
        GraphResourcePath("events/main".into()),
        &document(NodeTypeId::new("yssbi.test.constant").unwrap()),
    );

    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let plan = result.plan.unwrap();

    assert_eq!(plan.provenance, snapshot.provenance);
    let spans = trace.0.lock().unwrap();
    let snapshot_span = spans
        .iter()
        .find(|span| span.kind == SpanKind::Snapshot)
        .unwrap();
    assert_eq!(snapshot_span.span_id, snapshot.trace_span_id);
    assert_eq!(snapshot_span.parent_span_id, None);
    for kind in [SpanKind::Analysis, SpanKind::Lowering] {
        let span = spans.iter().find(|span| span.kind == kind).unwrap();
        assert_eq!(span.parent_span_id, Some(snapshot_span.span_id));
        assert_eq!(
            span.outcome,
            crate::node_system::analysis::SpanOutcome::Success
        );
    }
    for span in spans.iter() {
        assert_eq!(
            span.correlation.project_session_id,
            snapshot.provenance.project_session_id
        );
        assert_eq!(span.correlation.graph_path, snapshot.provenance.graph_path);
        assert_eq!(
            span.correlation.graph_revision,
            snapshot.provenance.basis.graph_revision
        );
        assert_eq!(span.correlation.compile_id, snapshot.provenance.compile_id);
    }
}

#[test]
fn blocking_compile_emits_no_lowering_or_run_span() {
    let registry = registry();
    let trace = RecordingTrace::default();
    let result = GraphCompiler::new(&registry, &Resources)
        .with_observability(ProjectSessionId::new("project-session-1"), &trace)
        .compile(&document(NodeTypeId::new("yssbi.test.missing").unwrap()));

    assert!(result.plan.is_none());
    assert!(
        trace
            .0
            .lock()
            .unwrap()
            .iter()
            .all(|span| { !matches!(span.kind, SpanKind::Lowering | SpanKind::Run) })
    );
}

#[test]
fn lowerability_invalid_dataframe_parameters_block_in_analysis() {
    let builtins = crate::node_system::catalog::build_builtin_node_system().unwrap();
    for (node_type, parameter, value) in [
        ("yssbi.dataframe.limit", "rows", serde_json::json!(0)),
        ("yssbi.dataframe.limit", "rows", serde_json::json!("ten")),
        ("yssbi.dataframe.rename", "from", serde_json::json!(42)),
        (
            "yssbi.dataframe.source.get",
            "dataframe",
            serde_json::json!(" databases/main"),
        ),
    ] {
        let protocol = builtins
            .registry
            .protocol(&NodeTypeId::new(node_type).unwrap())
            .unwrap()
            .clone();
        let node_type_id = protocol.type_id.clone();
        let calls = Arc::new(AtomicUsize::new(0));
        let registry = TestRegistry::new(vec![protocol])
            .with_lowerer(&node_type_id, CountingLowerer(calls.clone()));
        let mut graph = graph_with_node_types([(1, node_type.to_owned())]);
        graph
            .nodes
            .get_mut(&node_id(1))
            .unwrap()
            .parameters
            .insert(ParameterKey::new(parameter).unwrap(), value);
        if node_type == "yssbi.dataframe.rename" {
            graph.nodes.get_mut(&node_id(1)).unwrap().parameters.insert(
                ParameterKey::new("to").unwrap(),
                serde_json::json!("renamed"),
            );
        }
        let trace = RecordingTrace::default();

        let result = GraphCompiler::new(&registry, &Resources)
            .with_observability(ProjectSessionId::new("lowerability"), &trace)
            .compile(&graph);

        assert_analysis_blocks_before_lowering(&result, &trace, &calls);
        assert!(
            result.analysis.diagnostics.iter().any(|diagnostic| {
                diagnostic.code.as_str() == "compiler.parameter.invalid"
                    && matches!(
                        &diagnostic.primary,
                        DiagnosticLocation::Parameter { node_id: actual, key }
                            if *actual == node_id(1) && key.as_str() == parameter
                    )
            }),
            "missing precise parameter diagnostic for {node_type}:{parameter}"
        );
    }
}

#[test]
fn lowerability_malformed_persisted_literal_blocks_at_port_in_analysis() {
    let protocol = test_protocol(
        "malformed_literal",
        vec![data_port(
            "value",
            PortDirection::Input,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let node_type = protocol.type_id.clone();
    let calls = Arc::new(AtomicUsize::new(0));
    let registry =
        TestRegistry::new(vec![protocol]).with_lowerer(&node_type, CountingLowerer(calls.clone()));
    let address = PortAddress::declared(node_id(1), key("value"));
    let mut graph = graph_with_nodes(&[(1, "malformed_literal")]);
    graph.input_states.insert(
        address.clone(),
        InputState {
            literal_override: Some(serde_json::json!({"value_type": "not-a-type"})),
        },
    );
    let trace = RecordingTrace::default();

    let result = GraphCompiler::new(&registry, &Resources)
        .with_observability(ProjectSessionId::new("lowerability"), &trace)
        .compile(&graph);

    assert_analysis_blocks_before_lowering(&result, &trace, &calls);
    assert!(result.analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "compiler.input.literal_invalid"
            && diagnostic.primary == DiagnosticLocation::Port(address.clone())
    }));
}

#[test]
fn lowerability_legal_literal_wire_with_wrong_port_type_blocks_at_exact_port() {
    let protocol = test_protocol(
        "literal_type_mismatch",
        vec![data_port(
            "value",
            PortDirection::Input,
            TypeExpr::Concrete(type_id("core.int64")),
            None,
        )],
        vec![],
        vec![],
    );
    let node_type = protocol.type_id.clone();
    let calls = Arc::new(AtomicUsize::new(0));
    let registry =
        TestRegistry::new(vec![protocol]).with_lowerer(&node_type, CountingLowerer(calls.clone()));
    let address = PortAddress::declared(node_id(1), key("value"));
    let mut graph = graph_with_nodes(&[(1, "literal_type_mismatch")]);
    graph.input_states.insert(
        address.clone(),
        InputState {
            literal_override: Some(
                serde_json::to_value(crate::node_system::protocol::TypedValue {
                    value_type: TypeExpr::Concrete(type_id("core.string")),
                    value: Value::String("legal-string-wire".into()),
                })
                .unwrap(),
            ),
        },
    );
    let trace = RecordingTrace::default();

    let result = GraphCompiler::new(&registry, &Resources)
        .with_observability(ProjectSessionId::new("literal-mismatch"), &trace)
        .compile(&graph);

    assert_analysis_blocks_before_lowering(&result, &trace, &calls);
    assert!(result.analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "compiler.input.literal_invalid"
            && diagnostic.primary == DiagnosticLocation::Port(address.clone())
    }));
}

#[test]
fn lowerability_nested_literal_mismatch_blocks_at_exact_port_before_lowering() {
    let series = TypeExpr::Applied {
        constructor: TypeConstructorId::new("core.data_series").unwrap(),
        arguments: vec![TypeExpr::Concrete(type_id("core.int64"))],
    };
    let protocol = test_protocol(
        "nested_literal_mismatch",
        vec![data_port(
            "value",
            PortDirection::Input,
            series.clone(),
            None,
        )],
        vec![],
        vec![],
    );
    let node_type = protocol.type_id.clone();
    let calls = Arc::new(AtomicUsize::new(0));
    let registry =
        TestRegistry::new(vec![protocol]).with_lowerer(&node_type, CountingLowerer(calls.clone()));
    let address = PortAddress::declared(node_id(1), key("value"));
    let mut graph = graph_with_nodes(&[(1, "nested_literal_mismatch")]);
    graph.input_states.insert(
        address.clone(),
        InputState {
            literal_override: Some(
                serde_json::to_value(crate::node_system::protocol::TypedValue {
                    value_type: series,
                    value: Value::List(vec![Value::Integer(1), Value::String("wrong".into())]),
                })
                .unwrap(),
            ),
        },
    );
    let trace = RecordingTrace::default();

    let result = GraphCompiler::new(&registry, &Resources)
        .with_observability(ProjectSessionId::new("nested-literal-mismatch"), &trace)
        .compile(&graph);

    assert_analysis_blocks_before_lowering(&result, &trace, &calls);
    assert!(result.analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "compiler.input.literal_invalid"
            && diagnostic.primary == DiagnosticLocation::Port(address.clone())
    }));
}

#[test]
fn lowerability_missing_and_blocking_callees_block_at_call_before_lowering() {
    struct FunctionResources {
        path: GraphResourcePath,
        function: FunctionDocument,
        graph: GraphDocument,
    }
    impl ResourceSnapshot for FunctionResources {
        fn versions(&self) -> crate::node_system::analysis::ResourceVersionSet {
            BTreeMap::from([(
                ResourceKey::new(self.path.0.as_ref()),
                ResourceVersion::new("callee-v1"),
            )])
        }

        fn function_name(&self, path: &GraphResourcePath) -> Option<&str> {
            self.function_document(path).map(|_| "Test function")
        }

        fn function_document(&self, path: &GraphResourcePath) -> Option<&FunctionDocument> {
            (path == &self.path).then_some(&self.function)
        }

        fn function_graph_document(&self, path: &GraphResourcePath) -> Option<&GraphDocument> {
            (path == &self.path).then_some(&self.graph)
        }
    }

    let builtins = crate::node_system::catalog::build_builtin_node_system().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = test_protocol("callee_guard", vec![], vec![], vec![]);
    let registry = AugmentedCompilerRegistry {
        frozen: &builtins.registry,
        protocol: counted,
        implementation: NodeImplementation::new(CountingLowerer(calls.clone())),
    };
    let function_path = GraphResourcePath("functions/blocking".into());
    let blocking_resources = FunctionResources {
        path: function_path.clone(),
        function: FunctionDocument::new(FunctionSignature {
            parameters: Vec::new(),
            return_type: None,
        }),
        graph: builtin_graph_with_nodes(&[(20, "yssbi.test.missing")]),
    };

    for (name, target, expected_code) in [
        (
            "missing",
            GraphResourcePath("functions/missing".into()),
            "compiler.resource.resolution_failed",
        ),
        (
            "blocking",
            function_path,
            "compiler.control.call.abi_invalid",
        ),
    ] {
        calls.store(0, AtomicOrdering::Relaxed);
        let mut graph = builtin_graph_with_nodes(&[
            (1, "yssbi.project.function.call"),
            (2, "yssbi.test.callee_guard"),
            (3, "yssbi.project.function.call"),
        ]);
        set_parameters(
            &mut graph,
            1,
            &[("target", serde_json::json!(target.0.as_ref()))],
        );
        set_parameters(
            &mut graph,
            3,
            &[("target", serde_json::json!(target.0.as_ref()))],
        );
        let trace = RecordingTrace::default();
        let compiler = GraphCompiler::with_interface_resolvers(
            &registry,
            &blocking_resources,
            build_builtin_interface_resolvers(),
        );

        let result = compiler
            .with_observability(ProjectSessionId::new("lowerability"), &trace)
            .compile(&graph);

        assert_analysis_blocks_before_lowering(&result, &trace, &calls);
        let locations = result
            .analysis
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code.as_str() == expected_code)
            .map(|diagnostic| diagnostic.primary.clone())
            .collect::<Vec<_>>();
        for expected in [node_id(1), node_id(3)] {
            assert!(
                locations.contains(&DiagnosticLocation::Node(expected)),
                "missing precise {name} callee diagnostic at {expected}: {:?}",
                result.analysis.diagnostics
            );
        }
    }
}

#[test]
fn nested_blocking_callee_projects_to_root_call_with_exact_basis() {
    struct FunctionResources {
        functions: BTreeMap<GraphResourcePath, FunctionDocument>,
        graphs: BTreeMap<GraphResourcePath, GraphDocument>,
        versions: crate::node_system::analysis::ResourceVersionSet,
    }

    impl ResourceSnapshot for FunctionResources {
        fn versions(&self) -> crate::node_system::analysis::ResourceVersionSet {
            self.versions.clone()
        }

        fn function_name(&self, path: &GraphResourcePath) -> Option<&str> {
            self.function_document(path).map(|_| "Test function")
        }

        fn function_document(&self, path: &GraphResourcePath) -> Option<&FunctionDocument> {
            self.functions.get(path)
        }

        fn function_graph_document(&self, path: &GraphResourcePath) -> Option<&GraphDocument> {
            self.graphs.get(path)
        }
    }

    let registry = std::sync::Arc::unwrap_or_clone(
        crate::node_system::catalog::build_builtin_node_system()
            .unwrap()
            .registry,
    );
    let outer_path = GraphResourcePath("functions/outer".into());
    let inner_path = GraphResourcePath("functions/inner".into());
    let signature = FunctionDocument::new(FunctionSignature {
        parameters: Vec::new(),
        return_type: None,
    });
    let mut outer = builtin_graph_with_nodes(&[
        (20, "yssbi.project.function.entry"),
        (21, "yssbi.project.function.call"),
        (22, "yssbi.project.function.return"),
    ]);
    set_parameters(
        &mut outer,
        20,
        &[("function", serde_json::json!(outer_path.0.as_ref()))],
    );
    set_parameters(
        &mut outer,
        21,
        &[("target", serde_json::json!(inner_path.0.as_ref()))],
    );
    set_parameters(
        &mut outer,
        22,
        &[("function", serde_json::json!(outer_path.0.as_ref()))],
    );
    connect(&mut outer, 100, 20, "then", 21, "enter");
    connect(&mut outer, 101, 21, "then", 22, "enter");

    let versions = BTreeMap::from([
        (
            ResourceKey::new(outer_path.0.as_ref()),
            ResourceVersion::new("outer-v1"),
        ),
        (
            ResourceKey::new(inner_path.0.as_ref()),
            ResourceVersion::new("inner-v1"),
        ),
    ]);
    let resources = FunctionResources {
        functions: BTreeMap::from([
            (outer_path.clone(), signature.clone()),
            (inner_path.clone(), signature),
        ]),
        graphs: BTreeMap::from([
            (outer_path.clone(), outer),
            (
                inner_path.clone(),
                builtin_graph_with_nodes(&[(30, "yssbi.test.missing")]),
            ),
        ]),
        versions: versions.clone(),
    };
    let mut caller = builtin_graph_with_nodes(&[(1, "yssbi.project.function.call")]);
    set_parameters(
        &mut caller,
        1,
        &[("target", serde_json::json!(outer_path.0.as_ref()))],
    );
    let trace = RecordingTrace::default();
    let calls = AtomicUsize::new(0);

    let result = GraphCompiler::with_interface_resolvers(
        &registry,
        &resources,
        build_builtin_interface_resolvers(),
    )
    .with_observability(ProjectSessionId::new("nested-call"), &trace)
    .compile(&caller);

    assert_analysis_blocks_before_lowering(&result, &trace, &calls);
    assert_eq!(result.analysis.basis.resource_versions, versions);
    assert!(result.analysis.basis.resource_observations.is_empty());
    assert!(result.analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "compiler.control.call.abi_invalid"
            && diagnostic.primary == DiagnosticLocation::Node(node_id(1))
    }));
    assert!(result.analysis.diagnostics.iter().all(|diagnostic| {
        matches!(diagnostic.primary, DiagnosticLocation::Node(id) if id == node_id(1))
    }));
}

#[test]
fn locally_invalid_callee_still_discovers_complete_outgoing_call_closure() {
    struct FunctionResources {
        functions: BTreeMap<GraphResourcePath, FunctionDocument>,
        graphs: BTreeMap<GraphResourcePath, GraphDocument>,
        versions: crate::node_system::analysis::ResourceVersionSet,
    }

    impl ResourceSnapshot for FunctionResources {
        fn versions(&self) -> crate::node_system::analysis::ResourceVersionSet {
            self.versions.clone()
        }

        fn function_name(&self, path: &GraphResourcePath) -> Option<&str> {
            self.function_document(path).map(|_| "Test function")
        }

        fn function_document(&self, path: &GraphResourcePath) -> Option<&FunctionDocument> {
            self.functions.get(path)
        }

        fn function_graph_document(&self, path: &GraphResourcePath) -> Option<&GraphDocument> {
            self.graphs.get(path)
        }
    }

    fn function_graph(
        own_path: &GraphResourcePath,
        calls: &[(u128, &GraphResourcePath)],
        locally_invalid: bool,
    ) -> GraphDocument {
        let mut nodes = vec![(20, "yssbi.project.function.entry")];
        nodes.extend(
            calls
                .iter()
                .map(|(id, _)| (*id, "yssbi.project.function.call")),
        );
        nodes.push((30, "yssbi.project.function.return"));
        if locally_invalid {
            nodes.push((40, "yssbi.test.missing"));
        }
        let mut graph = builtin_graph_with_nodes(&nodes);
        set_parameters(
            &mut graph,
            20,
            &[("function", serde_json::json!(own_path.0.as_ref()))],
        );
        set_parameters(
            &mut graph,
            30,
            &[("function", serde_json::json!(own_path.0.as_ref()))],
        );
        let mut previous = 20;
        for (index, (id, target)) in calls.iter().enumerate() {
            set_parameters(
                &mut graph,
                *id,
                &[("target", serde_json::json!(target.0.as_ref()))],
            );
            connect(
                &mut graph,
                100 + index as u128,
                previous,
                "then",
                *id,
                "enter",
            );
            previous = *id;
        }
        connect(&mut graph, 199, previous, "then", 30, "enter");
        graph
    }

    let registry = std::sync::Arc::unwrap_or_clone(
        crate::node_system::catalog::build_builtin_node_system()
            .unwrap()
            .registry,
    );
    let path_a = GraphResourcePath("functions/local-invalid-a".into());
    let path_b = GraphResourcePath("functions/present-b".into());
    let path_c = GraphResourcePath("functions/missing-c".into());
    let path_d = GraphResourcePath("functions/transitive-d".into());
    let signature = FunctionDocument::new(FunctionSignature {
        parameters: Vec::new(),
        return_type: None,
    });
    let versions = BTreeMap::from([
        (
            ResourceKey::new(path_a.0.as_ref()),
            ResourceVersion::new("a-v1"),
        ),
        (
            ResourceKey::new(path_b.0.as_ref()),
            ResourceVersion::new("b-v1"),
        ),
        (
            ResourceKey::new(path_d.0.as_ref()),
            ResourceVersion::new("d-v1"),
        ),
    ]);
    let resources = FunctionResources {
        functions: BTreeMap::from([
            (path_a.clone(), signature.clone()),
            (path_b.clone(), signature.clone()),
            (path_d.clone(), signature),
        ]),
        graphs: BTreeMap::from([
            (
                path_a.clone(),
                function_graph(&path_a, &[(21, &path_b), (22, &path_c)], true),
            ),
            (
                path_b.clone(),
                function_graph(&path_b, &[(23, &path_d)], false),
            ),
            (path_d.clone(), function_graph(&path_d, &[], false)),
        ]),
        versions: versions.clone(),
    };
    let mut caller = builtin_graph_with_nodes(&[(1, "yssbi.project.function.call")]);
    set_parameters(
        &mut caller,
        1,
        &[("target", serde_json::json!(path_a.0.as_ref()))],
    );

    let result = GraphCompiler::with_interface_resolvers(
        &registry,
        &resources,
        build_builtin_interface_resolvers(),
    )
    .compile(&caller);

    assert_eq!(result.analysis.basis.resource_versions, versions);
    assert_eq!(
        result
            .analysis
            .basis
            .resource_observations
            .keys()
            .map(ResourceKey::as_str)
            .collect::<Vec<_>>(),
        vec![path_c.0.as_ref()],
    );
    assert!(result.analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "compiler.control.call.abi_invalid"
            && diagnostic.primary == DiagnosticLocation::Node(node_id(1))
    }));
    assert!(result.analysis.diagnostics.iter().all(|diagnostic| {
        matches!(diagnostic.primary, DiagnosticLocation::Node(id) if id == node_id(1))
    }));
}

#[test]
fn mutual_call_scc_propagates_external_blocking_to_every_root_site() {
    struct FunctionResources {
        functions: BTreeMap<GraphResourcePath, FunctionDocument>,
        graphs: BTreeMap<GraphResourcePath, GraphDocument>,
        versions: crate::node_system::analysis::ResourceVersionSet,
    }

    impl ResourceSnapshot for FunctionResources {
        fn versions(&self) -> crate::node_system::analysis::ResourceVersionSet {
            self.versions.clone()
        }

        fn function_name(&self, path: &GraphResourcePath) -> Option<&str> {
            self.function_document(path).map(|_| "Test function")
        }

        fn function_document(&self, path: &GraphResourcePath) -> Option<&FunctionDocument> {
            self.functions.get(path)
        }

        fn function_graph_document(&self, path: &GraphResourcePath) -> Option<&GraphDocument> {
            self.graphs.get(path)
        }
    }

    fn function_with_calls(
        own_path: &GraphResourcePath,
        calls: &[(u128, &GraphResourcePath)],
    ) -> GraphDocument {
        let mut nodes = vec![(20, "yssbi.project.function.entry")];
        nodes.extend(
            calls
                .iter()
                .map(|(id, _)| (*id, "yssbi.project.function.call")),
        );
        nodes.push((30, "yssbi.project.function.return"));
        let mut graph = builtin_graph_with_nodes(&nodes);
        set_parameters(
            &mut graph,
            20,
            &[("function", serde_json::json!(own_path.0.as_ref()))],
        );
        set_parameters(
            &mut graph,
            30,
            &[("function", serde_json::json!(own_path.0.as_ref()))],
        );
        let mut previous = 20;
        for (index, (id, target)) in calls.iter().enumerate() {
            set_parameters(
                &mut graph,
                *id,
                &[("target", serde_json::json!(target.0.as_ref()))],
            );
            connect(
                &mut graph,
                100 + index as u128,
                previous,
                "then",
                *id,
                "enter",
            );
            previous = *id;
        }
        connect(&mut graph, 199, previous, "then", 30, "enter");
        graph
    }

    let registry = std::sync::Arc::unwrap_or_clone(
        crate::node_system::catalog::build_builtin_node_system()
            .unwrap()
            .registry,
    );
    let path_a = GraphResourcePath("functions/cycle-a".into());
    let path_b = GraphResourcePath("functions/cycle-b".into());
    let blocking_path = GraphResourcePath("functions/cycle-z-blocking".into());
    let signature = FunctionDocument::new(FunctionSignature {
        parameters: Vec::new(),
        return_type: None,
    });
    let versions = [
        (&path_a, "a-v1"),
        (&path_b, "b-v1"),
        (&blocking_path, "c-v1"),
    ]
    .into_iter()
    .map(|(path, version)| {
        (
            ResourceKey::new(path.0.as_ref()),
            ResourceVersion::new(version),
        )
    })
    .collect();
    let resources = FunctionResources {
        functions: BTreeMap::from([
            (path_a.clone(), signature.clone()),
            (path_b.clone(), signature.clone()),
            (blocking_path.clone(), signature),
        ]),
        graphs: BTreeMap::from([
            (
                path_a.clone(),
                function_with_calls(&path_a, &[(21, &path_b), (22, &blocking_path)]),
            ),
            (
                path_b.clone(),
                function_with_calls(&path_b, &[(23, &path_a)]),
            ),
            (
                blocking_path.clone(),
                builtin_graph_with_nodes(&[(40, "yssbi.test.missing")]),
            ),
        ]),
        versions,
    };
    let mut caller = builtin_graph_with_nodes(&[
        (1, "yssbi.project.function.call"),
        (2, "yssbi.project.function.call"),
        (3, "yssbi.project.function.call"),
        (4, "yssbi.project.function.call"),
    ]);
    for (id, target) in [(1, &path_a), (2, &path_a), (3, &path_b), (4, &path_b)] {
        set_parameters(
            &mut caller,
            id,
            &[("target", serde_json::json!(target.0.as_ref()))],
        );
    }

    let result = GraphCompiler::with_interface_resolvers(
        &registry,
        &resources,
        build_builtin_interface_resolvers(),
    )
    .compile(&caller);

    let invalid_sites = result
        .analysis
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code.as_str() == "compiler.control.call.abi_invalid")
        .filter_map(|diagnostic| match diagnostic.primary {
            DiagnosticLocation::Node(id) => Some(id),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        invalid_sites,
        BTreeSet::from([node_id(1), node_id(2), node_id(3), node_id(4)])
    );
    assert_eq!(result.analysis.basis.resource_versions, resources.versions);
    assert!(result.analysis.basis.resource_observations.is_empty());
    assert!(result.semantic.is_none());
    assert!(result.plan.is_none());
}

#[test]
fn unknown_node_returns_analysis_without_plan() {
    let registry = registry();
    let result = GraphCompiler::new(&registry, &Resources)
        .compile(&document(NodeTypeId::new("yssbi.test.missing").unwrap()));
    assert!(result.semantic.is_none());
    assert!(result.plan.is_none());
    assert_eq!(
        result.analysis.diagnostics[0].code.as_str(),
        "compiler.node.unknown"
    );
}

#[test]
fn unknown_port_and_wrong_direction_return_analysis_without_plan() {
    let registry = registry();
    let mut graph = document(NodeTypeId::new("yssbi.test.constant").unwrap());
    let node = node_id(1);
    let other = node_id(2);
    graph.nodes.insert(
        other,
        DocumentNode {
            id: other,
            node_type: NodeTypeId::new("yssbi.test.constant").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: BTreeMap::new(),
            user_label: None,
        },
    );
    let unknown = PortAddress::declared(other, key("missing"));
    let output_as_input = PortAddress::declared(other, key("value"));
    graph.connections.insert(
        crate::node_system::document::ConnectionId::from_uuid(Uuid::from_u128(3)),
        DocumentConnection {
            id: crate::node_system::document::ConnectionId::from_uuid(Uuid::from_u128(3)),
            output: PortAddress::declared(node, key("value")),
            input: unknown,
            order: None,
        },
    );
    graph.connections.insert(
        crate::node_system::document::ConnectionId::from_uuid(Uuid::from_u128(4)),
        DocumentConnection {
            id: crate::node_system::document::ConnectionId::from_uuid(Uuid::from_u128(4)),
            output: PortAddress::declared(node, key("value")),
            input: output_as_input,
            order: None,
        },
    );
    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);
    assert!(result.plan.is_none());
    let codes: Vec<_> = result
        .analysis
        .diagnostics
        .iter()
        .map(|item| item.code.as_str())
        .collect();
    assert!(codes.contains(&"compiler.port.unknown"));
    assert!(codes.contains(&"compiler.connection.input_direction"));
}

struct TestRegistry {
    fingerprint: RegistryFingerprint,
    nodes: BTreeMap<NodeTypeId, (NodeProtocol, NodeImplementation)>,
    structural_roles: BTreeMap<NodeTypeId, StructuralNodeRole>,
    type_classes: BTreeMap<TypeId, BTreeSet<TypeClassId>>,
    constructor_arities: BTreeMap<TypeConstructorId, usize>,
    constructor_classes: BTreeMap<TypeConstructorId, BTreeSet<TypeClassId>>,
    nominal_registry: Option<Arc<NodeRegistry>>,
}

impl TestRegistry {
    fn new(protocols: Vec<NodeProtocol>) -> Self {
        let nodes = protocols
            .into_iter()
            .enumerate()
            .map(|(index, protocol)| {
                (
                    protocol.type_id.clone(),
                    (protocol, NodeImplementation::new(TestLowerer(index as u32))),
                )
            })
            .collect();
        Self {
            fingerprint: RegistryFingerprint::from_bytes([9; 32]),
            nodes,
            structural_roles: BTreeMap::new(),
            type_classes: BTreeMap::new(),
            constructor_arities: BTreeMap::new(),
            constructor_classes: BTreeMap::new(),
            nominal_registry: None,
        }
    }

    fn with_type_class(mut self, value_type: TypeId, class: TypeClassId) -> Self {
        self.type_classes
            .entry(value_type)
            .or_default()
            .insert(class);
        self
    }

    fn with_constructor(
        mut self,
        constructor: TypeConstructorId,
        arity: usize,
        classes: impl IntoIterator<Item = TypeClassId>,
    ) -> Self {
        self.constructor_arities.insert(constructor.clone(), arity);
        self.constructor_classes
            .insert(constructor, classes.into_iter().collect());
        self
    }

    fn structural(mut self, node_type: &NodeTypeId, role: StructuralNodeRole) -> Self {
        self.structural_roles.insert(node_type.clone(), role);
        self
    }

    fn with_lowerer(mut self, node_type: &NodeTypeId, lowerer: impl NodeLowerer + 'static) -> Self {
        self.nodes.get_mut(node_type).expect("test node protocol").1 =
            NodeImplementation::new(lowerer);
        self
    }

    fn with_nominal_registry(mut self, registry: Arc<NodeRegistry>) -> Self {
        self.nominal_registry = Some(registry);
        self
    }
}

impl TypeEnvironment for TestRegistry {
    fn concrete_implements(&self, value_type: &TypeId, class: &TypeClassId) -> Option<bool> {
        Some(
            self.type_classes
                .get(value_type)
                .is_some_and(|classes| classes.contains(class)),
        )
    }

    fn constructor_arity(&self, constructor: &TypeConstructorId) -> Option<usize> {
        self.constructor_arities.get(constructor).copied()
    }

    fn applied_implements(
        &self,
        constructor: &TypeConstructorId,
        class: &TypeClassId,
    ) -> Option<bool> {
        Some(
            self.constructor_classes
                .get(constructor)
                .is_some_and(|classes| classes.contains(class)),
        )
    }
}

impl CompilerRegistry for TestRegistry {
    fn fingerprint(&self) -> &RegistryFingerprint {
        &self.fingerprint
    }

    fn resolve(&self, node_type: &NodeTypeId) -> Option<RegistryNode<'_>> {
        let (protocol, implementation) = self.nodes.get(node_type)?;
        let behavior = self
            .structural_roles
            .get(node_type)
            .copied()
            .map(RegistryNodeBehavior::Structural)
            .unwrap_or(RegistryNodeBehavior::Leaf(implementation));
        Some(RegistryNode {
            protocol,
            protocol_fingerprint: ProtocolFingerprint::from_bytes(
                [node_type.as_str().len() as u8; 32],
            ),
            behavior,
        })
    }

    fn validate_nominal_parameter(
        &self,
        type_id: &TypeId,
        value: &serde_json::Value,
    ) -> Option<Result<(), String>> {
        self.nominal_registry
            .as_ref()?
            .validate_nominal_parameter(type_id, value)
    }

    fn prepare_nominal_parameter(
        &self,
        type_id: &TypeId,
        value: &serde_json::Value,
    ) -> Option<Result<crate::node_system::registry::PreparedNominalValue, String>> {
        self.nominal_registry
            .as_ref()?
            .prepare_nominal_parameter(type_id, value)
    }
}

struct TestLowerer(u32);
impl NodeLowerer for TestLowerer {
    fn lower(&self, _: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        Ok(LoweredNode {
            kernel: LoweredKernel::Native(
                KernelHandle::new(format!("test.kernel.{}", self.0)).unwrap(),
            ),
            parameters: CompiledParameterHandle::new(format!("test.params.{}", self.0)).unwrap(),
        })
    }
}

fn type_id(value: &str) -> TypeId {
    TypeId::new(value).unwrap()
}

fn concrete(value: &str) -> TypeExpr {
    TypeExpr::Concrete(type_id(value))
}

#[test]
fn type_conformance_requires_every_source_union_member() {
    let source = TypeExpr::Union(vec![concrete("core.int64"), concrete("core.string")]);

    assert_eq!(
        type_exprs_compatibility(&source, &concrete("core.int64"), &[], &[]),
        TypeCompatibility::Incompatible
    );
}

#[test]
fn type_conformance_accepts_numeric_series_members() {
    let target = numeric_data_series_type();

    assert_eq!(
        type_exprs_compatibility(&data_series_type(concrete("core.int64")), &target, &[], &[],),
        TypeCompatibility::Compatible
    );
}

#[test]
fn type_conformance_reports_unknown_as_indeterminate() {
    assert_eq!(
        type_exprs_compatibility(
            &TypeExpr::Unknown,
            &data_series_type(concrete("core.float64")),
            &[],
            &[],
        ),
        TypeCompatibility::Indeterminate
    );
}

fn data_port(
    name: &str,
    direction: PortDirection,
    value_type: TypeExpr,
    schema: Option<SchemaExpr>,
) -> PortSpec {
    PortSpec {
        key: key(name),
        label_key: I18nKey::new(format!("ports.{name}.label")).unwrap(),
        direction,
        kind: PortKind::Data,
        value_type,
        instances: PortInstances::Declared,
        connections: if direction == PortDirection::Input {
            ConnectionsPerPort::Single
        } else {
            ConnectionsPerPort::Multiple {
                max: None,
                ordered: false,
            }
        },
        input_binding: (direction == PortDirection::Input).then_some(InputBindingSpec {
            literal_policy: LiteralPolicy::Allowed,
            default_value: None,
        }),
        consumption: None,
        production: None,
        editor: PortEditorSpec::Default,
        schema,
    }
}

fn effect_port(name: &str, direction: PortDirection) -> PortSpec {
    PortSpec {
        key: key(name),
        label_key: I18nKey::new(format!("ports.{name}.label")).unwrap(),
        direction,
        kind: PortKind::Effect,
        value_type: TypeExpr::Unknown,
        instances: PortInstances::Declared,
        connections: if direction == PortDirection::Input {
            ConnectionsPerPort::Single
        } else {
            ConnectionsPerPort::Multiple {
                max: None,
                ordered: false,
            }
        },
        input_binding: None,
        consumption: None,
        production: None,
        editor: PortEditorSpec::Default,
        schema: None,
    }
}

fn test_protocol(
    name: &str,
    mut ports: Vec<PortSpec>,
    type_parameters: Vec<TypeParameterId>,
    constraints: Vec<TypeConstraint>,
) -> NodeProtocol {
    for port in &mut ports {
        if port.kind == PortKind::Data && port.value_type == TypeExpr::Unknown {
            port.value_type = TypeExpr::Concrete(TypeId::new("core.object").unwrap());
        }
    }
    NodeProtocol {
        type_id: NodeTypeId::new(format!("yssbi.test.{name}")).unwrap(),
        catalog: NodeCatalogProtocol {
            title_key: I18nKey::new(format!("nodes.test.{name}.title")).unwrap(),
            description_key: None,
            documentation_key: None,
            aliases_key: None,
            category_id: NodeCategoryId::new("test").unwrap(),
            icon_id: IconId::new("test").unwrap(),
            style_id: NodeStyleId::new("test").unwrap(),
            hidden: false,
        },
        interface: NodeInterfaceProtocol::new(ports, type_parameters, constraints).unwrap(),
        parameters: ParameterSchema::default(),
        instance_display: NodeInstanceDisplaySpec::Static,
        execution: ExecutionSemantics {
            determinism: Determinism::Deterministic,
            purity: Purity::Pure,
            evaluation: EvaluationPolicy::DemandDriven,
            cache: CachePolicy::PerRun,
            effects: EffectSemantics::None,
            idempotent: false,
            retry: None,
        },
        scope: NodeScope::Any,
        managed_role: None,
    }
}

#[test]
fn semantic_graph_preserves_protocol_port_order_for_kernel_abi() {
    let protocol = test_protocol(
        "ordered_outputs",
        vec![
            data_port("z_result", PortDirection::Output, TypeExpr::Unknown, None),
            data_port("a_report", PortDirection::Output, TypeExpr::Unknown, None),
        ],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![protocol]);
    let result = GraphCompiler::new(&registry, &Resources).compile(&document(
        NodeTypeId::new("yssbi.test.ordered_outputs").unwrap(),
    ));
    let semantic = result.semantic.expect("ordered output graph must compile");
    let keys = semantic.nodes[0]
        .ports
        .iter()
        .map(|port| match &port.address.port {
            crate::node_system::document::PortRef::Declared { key } => key.as_str(),
            crate::node_system::document::PortRef::Instance { .. } => {
                panic!("expected declared output")
            }
        })
        .collect::<Vec<_>>();

    assert_eq!(keys, ["z_result", "a_report"]);
}

#[test]
fn planned_outputs_preserve_protocol_order_identity_and_presentation() {
    let mut protocol = test_protocol(
        "report",
        vec![
            data_port("z_result", PortDirection::Output, TypeExpr::Unknown, None),
            data_port("report", PortDirection::Output, TypeExpr::Unknown, None),
        ],
        vec![],
        vec![],
    );
    protocol.type_id = NodeTypeId::new("yssbi.statistics.ols.summary").unwrap();
    let registry = TestRegistry::new(vec![protocol]);
    let result = GraphCompiler::new(&registry, &Resources).compile(&document(
        NodeTypeId::new("yssbi.statistics.ols.summary").unwrap(),
    ));
    let plan = result.plan.expect("report fixture must lower");
    let operation = plan
        .operations
        .iter()
        .find(|operation| operation.source_node_type_id.as_str() == "yssbi.statistics.ols.summary")
        .unwrap();

    assert_eq!(
        operation
            .outputs
            .iter()
            .map(
                |output| match &output.public_output.as_ref().unwrap().port.port {
                    PortRef::Declared { key } => key.as_str(),
                    PortRef::Instance { template, .. } => template.as_str(),
                }
            )
            .collect::<Vec<_>>(),
        ["z_result", "report"],
    );
    assert_eq!(
        operation.outputs[0].presentation,
        ResultPresentation::Inspector
    );
    assert_eq!(
        operation.outputs[1].presentation,
        ResultPresentation::Report {
            report: ResultReportKind::OlsSummary,
        },
    );
}

#[test]
fn adapter_output_has_no_public_pin_and_inherits_presentation() {
    let mut report = test_protocol(
        "report",
        vec![data_port(
            "report",
            PortDirection::Output,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    report.type_id = NodeTypeId::new("yssbi.statistics.ols.summary").unwrap();
    report.interface.ports[0].production = Some(OutputProduction::Streaming);
    let mut consumer = test_protocol(
        "report_consumer",
        vec![data_port(
            "input",
            PortDirection::Input,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    consumer.interface.ports[0].consumption = Some(InputConsumption::FullyMaterialized);
    let registry = TestRegistry::new(vec![report, consumer]);
    let mut graph = graph_with_nodes(&[(1, "report"), (2, "report_consumer")]);
    graph.nodes.get_mut(&node_id(1)).unwrap().node_type =
        NodeTypeId::new("yssbi.statistics.ols.summary").unwrap();
    connect(&mut graph, 1, 1, "report", 2, "input");

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);
    let plan = result.plan.expect("report adapter fixture must lower");
    let adapter = plan
        .operations
        .iter()
        .find(|operation| matches!(operation.kernel, PlannedKernel::Adapter(_)))
        .unwrap();

    assert!(adapter.outputs[0].public_output.is_none());
    assert_eq!(
        adapter.outputs[0].presentation,
        ResultPresentation::Report {
            report: ResultReportKind::OlsSummary,
        },
    );
}

#[test]
fn effective_cache_policy_disables_every_effect_semantics_independently() {
    for effects in [EffectSemantics::Ordered, EffectSemantics::Exclusive] {
        assert_eq!(
            super::pipeline::effective_cache_policy(
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
    let native = super::pipeline::PendingKernel::Native(KernelHandle::new("test.retry").unwrap());
    let safe = super::pipeline::effective_retry_policy(
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
        super::pipeline::effective_retry_policy(
            false,
            Some(policy),
            Determinism::Deterministic,
            Purity::Pure,
            EffectSemantics::None,
            false,
            &native,
            &[],
        ),
        super::pipeline::effective_retry_policy(
            true,
            Some(policy),
            Determinism::NonDeterministic,
            Purity::Pure,
            EffectSemantics::None,
            false,
            &native,
            &[],
        ),
        super::pipeline::effective_retry_policy(
            true,
            Some(policy),
            Determinism::Deterministic,
            Purity::Effectful,
            EffectSemantics::None,
            false,
            &native,
            &[],
        ),
        super::pipeline::effective_retry_policy(
            true,
            Some(policy),
            Determinism::Deterministic,
            Purity::Pure,
            EffectSemantics::Ordered,
            false,
            &native,
            &[],
        ),
        super::pipeline::effective_retry_policy(
            true,
            Some(policy),
            Determinism::Deterministic,
            Purity::Pure,
            EffectSemantics::None,
            true,
            &native,
            &[],
        ),
        super::pipeline::effective_retry_policy(
            true,
            Some(policy),
            Determinism::Deterministic,
            Purity::Pure,
            EffectSemantics::None,
            false,
            &native,
            std::slice::from_ref(&shared_resource),
        ),
        super::pipeline::effective_retry_policy(
            true,
            None,
            Determinism::Deterministic,
            Purity::Pure,
            EffectSemantics::None,
            false,
            &native,
            &[],
        ),
        super::pipeline::effective_retry_policy(
            true,
            Some(policy),
            Determinism::Deterministic,
            Purity::Pure,
            EffectSemantics::None,
            false,
            &super::pipeline::PendingKernel::Relational,
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
    let native = super::pipeline::PendingKernel::Native(KernelHandle::new("test.retry").unwrap());
    let resource = CompiledResourceRequirement {
        resource: ResourceId::new("database/read").unwrap(),
        kind: ResourceKind::DatabaseConnection,
        access: ResourceAccess::Shared,
        optional: false,
    };
    let cases = [
        (
            "native effect edge",
            super::pipeline::effective_retry_policy(
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
            super::pipeline::effective_retry_policy(
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
            super::pipeline::effective_retry_policy(
                true,
                Some(policy),
                Determinism::Deterministic,
                Purity::Pure,
                EffectSemantics::None,
                false,
                &super::pipeline::PendingKernel::Relational,
                &[],
            ),
        ),
        (
            "resource-backed native",
            super::pipeline::effective_retry_policy(
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
fn concrete_resource_lowerers_force_database_variable_and_filesystem_retry_off() {
    let policy = RetryPolicy::new(
        std::num::NonZeroU32::new(2).unwrap(),
        std::time::Duration::ZERO,
        std::time::Duration::ZERO,
    )
    .unwrap();
    for (name, resource, kind) in [
        (
            "retry_database_write",
            "database/main",
            ResourceKind::DatabaseConnection,
        ),
        (
            "retry_variable_write",
            "variables/value",
            ResourceKind::ExternalArtifact,
        ),
        (
            "retry_filesystem_write",
            "filesystem/output",
            ResourceKind::TemporaryStorage,
        ),
    ] {
        let mut protocol = test_protocol(name, vec![], vec![], vec![]);
        protocol.execution.idempotent = true;
        protocol.execution.retry = Some(policy);
        let node_type = protocol.type_id.clone();
        let registry = TestRegistry::new(vec![protocol]).with_lowerer(
            &node_type,
            FragmentLowerer {
                fragment: kernel_fragment(
                    EffectSemantics::None,
                    FragmentMetadata {
                        resources: Box::new([CompiledResourceRequirement {
                            resource: ResourceId::new(resource).unwrap(),
                            kind,
                            access: ResourceAccess::Exclusive,
                            optional: false,
                        }]),
                        ..FragmentMetadata::default()
                    },
                ),
            },
        );
        let plan = GraphCompiler::new(&registry, &Resources)
            .compile(&graph_with_nodes(&[(1, name)]))
            .plan
            .unwrap();
        assert_eq!(plan.operations[0].retry, PlannedRetry::default(), "{name}");
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
                &compiler.snapshot(GraphResourcePath(path.into()), &graph),
                &CompileCancellationToken::new(),
            )
            .unwrap()
            .plan
            .unwrap()
            .operations[0]
            .stable_id
            .clone()
    };

    assert_ne!(compile("events/first"), compile("events/second"));
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
            outputs: Box::new([demand_output("events/main", 4, "out")]),
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

fn graph_with_nodes(nodes: &[(u128, &str)]) -> GraphDocument {
    graph_with_node_types(
        nodes
            .iter()
            .map(|(node, node_type)| (*node, format!("yssbi.test.{node_type}"))),
    )
}

fn builtin_graph_with_nodes(nodes: &[(u128, &str)]) -> GraphDocument {
    graph_with_node_types(
        nodes
            .iter()
            .map(|(node, node_type)| (*node, (*node_type).to_owned())),
    )
}

fn graph_with_node_types(nodes: impl IntoIterator<Item = (u128, String)>) -> GraphDocument {
    GraphDocument {
        revision: crate::node_system::document::GraphRevision::new(11),
        nodes: nodes
            .into_iter()
            .map(|(id, node_type)| {
                let id = node_id(id);
                (
                    id,
                    DocumentNode {
                        id,
                        node_type: NodeTypeId::new(node_type).unwrap(),
                        position: NodePosition { x: 0.0, y: 0.0 },
                        parameters: BTreeMap::new(),
                        user_label: None,
                    },
                )
            })
            .collect(),
        port_bindings: BTreeMap::new(),
        connections: BTreeMap::new(),
        input_states: BTreeMap::new(),
    }
}

fn connect(
    graph: &mut GraphDocument,
    id: u128,
    source_node: u128,
    source_port: &str,
    target_node: u128,
    target_port: &str,
) {
    connect_addresses(
        graph,
        id,
        PortAddress::declared(node_id(source_node), key(source_port)),
        PortAddress::declared(node_id(target_node), key(target_port)),
    );
}

fn connect_addresses(graph: &mut GraphDocument, id: u128, output: PortAddress, input: PortAddress) {
    let id = crate::node_system::document::ConnectionId::from_uuid(Uuid::from_u128(id));
    graph.connections.insert(
        id,
        DocumentConnection {
            id,
            output,
            input,
            order: None,
        },
    );
}

fn bind_resolved_function_port(
    graph: &mut GraphDocument,
    node: u128,
    template: &str,
    instance: u128,
    order: &str,
    function: &GraphResourcePath,
    parameter: &FunctionParameterId,
) -> PortAddress {
    let address = PortAddress::instance(
        node_id(node),
        key(template),
        PortInstanceId::from_uuid(Uuid::from_u128(instance)),
    );
    graph.port_bindings.insert(
        address.clone(),
        DynamicPortBinding::Resolved {
            origin: DynamicMemberLocator::FunctionParameter {
                function: function.clone(),
                parameter: parameter.clone(),
            },
            order: OrderKey(order.into()),
            last_known: crate::node_system::document::LastKnownPortMetadata::default(),
        },
    );
    address
}

fn bind_member_port(
    graph: &mut GraphDocument,
    node: u128,
    template: &str,
    instance: u128,
    order: &str,
) -> PortAddress {
    let address = PortAddress::instance(
        node_id(node),
        key(template),
        PortInstanceId::from_uuid(Uuid::from_u128(instance)),
    );
    graph.port_bindings.insert(
        address.clone(),
        DynamicPortBinding::UserCreated {
            order: OrderKey(order.into()),
        },
    );
    address
}

struct FailingLowerer;
struct CancelledLowerer;

impl NodeLowerer for CancelledLowerer {
    fn lower(&self, _: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        Err(LoweringError::Cancelled(CompileCancelled))
    }
}

impl NodeLowerer for FailingLowerer {
    fn lower(&self, _: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        Err(LoweringError::internal(
            LoweringInvariant::InvalidPreparedConfiguration,
        ))
    }
}

#[test]
fn non_concrete_parameter_shapes_block_when_they_cannot_be_prepared() {
    let shapes = [
        (
            "union",
            TypeExpr::Union(vec![TypeExpr::Concrete(type_id("core.int64"))]),
        ),
        (
            "applied",
            TypeExpr::Applied {
                constructor: TypeConstructorId::new("core.list").unwrap(),
                arguments: vec![TypeExpr::Concrete(type_id("core.int64"))],
            },
        ),
        (
            "generic",
            TypeExpr::Generic(TypeParameterId::new("t").unwrap()),
        ),
        ("unknown", TypeExpr::Unknown),
    ];

    for (name, value_type) in shapes {
        let mut protocol = test_protocol(name, vec![], vec![], vec![]);
        protocol.parameters = ParameterSchema::new(vec![ParameterSpec {
            key: ParameterKey::new("value").unwrap(),
            title_key: I18nKey::new("parameters.value.title").unwrap(),
            description_key: None,
            value_type,
            default_value: None,
            constraints: vec![ParameterConstraint::Required],
            editor: ParameterEditorSpec::Auto,
            presentation: ParameterPresentation::DetailPanel,
        }])
        .unwrap();
        let node_type = protocol.type_id.clone();
        let calls = Arc::new(AtomicUsize::new(0));
        let registry = TestRegistry::new(vec![protocol])
            .with_lowerer(&node_type, CountingLowerer(calls.clone()));
        let mut graph = graph_with_nodes(&[(1, name)]);
        set_parameters(&mut graph, 1, &[("value", serde_json::json!(7))]);
        let trace = RecordingTrace::default();

        let result = GraphCompiler::new(&registry, &Resources)
            .with_observability(ProjectSessionId::new("unpreparable"), &trace)
            .compile(&graph);

        assert_analysis_blocks_before_lowering(&result, &trace, &calls);
        assert!(result.analysis.diagnostics.iter().any(|diagnostic| {
            diagnostic.code.as_str() == "compiler.parameter.invalid"
                && matches!(
                    &diagnostic.primary,
                    DiagnosticLocation::Parameter { node_id: actual, key }
                        if *actual == node_id(1) && key.as_str() == "value"
                )
        }));
    }
}

#[test]
fn typed_lowering_cancellation_cancels_compilation() {
    let protocol = test_protocol("lowering_cancelled", vec![], vec![], vec![]);
    let node_type = protocol.type_id.clone();
    let registry = TestRegistry::new(vec![protocol]).with_lowerer(&node_type, CancelledLowerer);
    let compiler = GraphCompiler::new(&registry, &Resources);
    let graph = graph_with_nodes(&[(1, "lowering_cancelled")]);
    let snapshot = compiler.snapshot(GraphResourcePath("events/cancelled".into()), &graph);

    let result = compiler.compile_snapshot(&snapshot, &CompileCancellationToken::new());

    assert!(matches!(result, Err(CompileCancelled)));
}

#[test]
fn internal_lowering_failure_preserves_semantic_without_plan() {
    let protocol = test_protocol("lowering_failure", vec![], vec![], vec![]);
    let node_type = protocol.type_id.clone();
    let registry = TestRegistry::new(vec![protocol]).with_lowerer(&node_type, FailingLowerer);

    let result = GraphCompiler::new(&registry, &Resources)
        .compile(&graph_with_nodes(&[(1, "lowering_failure")]));

    assert!(result.semantic.is_some());
    assert!(result.plan.is_none());
    assert!(
        result.analysis.diagnostics.iter().all(|diagnostic| {
            diagnostic.code.as_str() != "compiler.lowering.internal_invariant"
        })
    );
    assert!(matches!(
        result.outcome,
        CompilationOutcome::InternalFailure(ref failure)
            if failure.stage == CompilationStage::Lowering
                && failure.code.as_ref() == "compiler.lowering.internal_invariant"
                && failure.node_id == Some(node_id(1))
    ));
}

#[test]
fn unbound_input_diagnostic_carries_the_exact_port() {
    let protocol = test_protocol(
        "unbound_input",
        vec![data_port(
            "value",
            PortDirection::Input,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![protocol]);
    let address = PortAddress::declared(node_id(1), key("value"));

    let result = GraphCompiler::new(&registry, &Resources)
        .compile(&graph_with_nodes(&[(1, "unbound_input")]));

    let diagnostic = result
        .analysis
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "compiler.input.unbound")
        .expect("unbound input diagnostic");
    assert_eq!(
        diagnostic.arguments,
        BTreeMap::from([(Box::from("port"), address.to_string().into())])
    );
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Warning);
    let basis = result.execution_basis.as_ref().expect("execution basis");
    assert!(result.plan.is_none());
    assert!(matches!(result.outcome, CompilationOutcome::Succeeded));
    let default_plan = basis
        .derive_plan(&ExecutionDemand::Default)
        .expect("default demand ignores an unbound orphan");
    assert!(default_plan.operations.is_empty());
}

struct FragmentLowerer {
    fragment: LoweredKernel,
}

impl NodeLowerer for FragmentLowerer {
    fn lower(&self, _: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        Ok(LoweredNode {
            kernel: self.fragment.clone(),
            parameters: CompiledParameterHandle::new("test.fragment.params").unwrap(),
        })
    }
}

fn kernel_fragment(effect: EffectSemantics, mut metadata: FragmentMetadata) -> LoweredKernel {
    metadata.effect = effect;
    LoweredKernel::Kernel(KernelFragment {
        kernel: KernelHandle::new("test.fragment.kernel").unwrap(),
        metadata,
    })
}

#[test]
fn compiler_maps_data_edges_into_plan_dependencies() {
    let source = test_protocol(
        "plan_data_source",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let sink = test_protocol(
        "plan_data_sink",
        vec![data_port(
            "in",
            PortDirection::Input,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![source, sink]);
    let mut graph = graph_with_nodes(&[(1, "plan_data_source"), (2, "plan_data_sink")]);
    connect(&mut graph, 10, 1, "out", 2, "in");

    let plan = GraphCompiler::new(&registry, &Resources)
        .compile(&graph)
        .plan
        .expect("data graph should lower");

    assert_eq!(plan.operations.len(), 2);
    assert_eq!(plan.value_dependencies.len(), 1);
    assert!(
        plan.operations
            .iter()
            .all(|operation| !matches!(operation.kernel, PlannedKernel::Adapter(_)))
    );
    assert_eq!(
        plan.value_dependencies[0],
        crate::node_system::plan::ValueDependency {
            source: plan.operations[0].outputs[0].value,
            destination: plan.operations[1].inputs[0].value,
        }
    );
}

#[test]
fn data_series_contract_survives_materialization_adapter_insertion() {
    use crate::node_system::plan::PlannedValueKind;
    use crate::node_system::protocol::data_series_type;

    let series = data_series_type(TypeExpr::Concrete(TypeId::new("core.int64").unwrap()));
    let mut source_output = data_port("out", PortDirection::Output, series.clone(), None);
    source_output.production = Some(OutputProduction::Streaming);
    let source = test_protocol(
        "series_contract_source",
        vec![source_output],
        vec![],
        vec![],
    );
    let mut sink_input = data_port("in", PortDirection::Input, series, None);
    sink_input.consumption = Some(InputConsumption::FullyMaterialized);
    let sink = test_protocol("series_contract_sink", vec![sink_input], vec![], vec![]);
    let registry = TestRegistry::new(vec![source, sink]).with_constructor(
        TypeConstructorId::new("core.data_series").unwrap(),
        1,
        [],
    );
    let mut graph = graph_with_nodes(&[(1, "series_contract_source"), (2, "series_contract_sink")]);
    connect(&mut graph, 10, 1, "out", 2, "in");

    let plan = GraphCompiler::new(&registry, &Resources)
        .compile(&graph)
        .plan
        .expect("connected canonical DataSeries graph should lower");
    let adapter = plan
        .operations
        .iter()
        .find(|operation| matches!(operation.kernel, PlannedKernel::Adapter(_)))
        .expect("streaming DataSeries edge should insert a materialization adapter");

    assert!(
        adapter
            .inputs
            .iter()
            .all(|input| input.contract.kind == PlannedValueKind::DataSeries)
    );
    assert!(
        adapter
            .outputs
            .iter()
            .all(|output| output.contract.kind == PlannedValueKind::DataSeries)
    );
}

#[test]
fn function_plan_store_rejects_data_series_kind_mismatch() {
    use crate::node_system::plan::{PlannedValueContract, PlannedValueKind};
    use crate::node_system::protocol::data_series_type;
    use crate::node_system::runtime::{FunctionPlanStore, FunctionPlanStoreError};

    let series = data_series_type(TypeExpr::Concrete(TypeId::new("core.int64").unwrap()));
    let mut source_output = data_port("out", PortDirection::Output, series.clone(), None);
    source_output.production = Some(OutputProduction::FullyMaterialized);
    let source = test_protocol(
        "series_function_source",
        vec![source_output],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![source]).with_constructor(
        TypeConstructorId::new("core.data_series").unwrap(),
        1,
        [],
    );
    let path = GraphResourcePath("functions/series-contract".into());
    let compiler = GraphCompiler::new(&registry, &Resources);
    let graph = graph_with_nodes(&[(1, "series_function_source")]);
    let mut plan = compiler
        .compile(&graph)
        .plan
        .expect("canonical DataSeries function body should lower");
    let version = ResourceVersion::new("1");
    let versions = BTreeMap::from([(ResourceKey::new(path.0.as_ref()), version.clone())]);
    plan.provenance.graph_path = path.clone();
    plan.provenance.basis.resource_versions = versions.clone();
    let output = &mut plan.operations[0].outputs[0];
    output
        .public_output
        .as_mut()
        .expect("compiled function output keeps its public identity")
        .graph_path = path.clone();
    let result_value = output.value;
    plan.operations[0].outputs[0].contract.kind = PlannedValueKind::Scalar;
    plan.value_contracts
        .get_mut(&result_value)
        .expect("compiled output has a plan-global value contract")
        .kind = PlannedValueKind::Scalar;
    let result = FunctionParameterId("return".into());
    let abi = FunctionPlanAbi {
        provenance: plan.provenance.clone(),
        parameters: BTreeMap::new(),
        parameter_contracts: BTreeMap::new(),
        results: BTreeMap::from([(result.clone(), result_value)]),
        result_productions: BTreeMap::from([(result.clone(), OutputProduction::FullyMaterialized)]),
        result_contracts: BTreeMap::from([(
            result,
            PlannedValueContract {
                kind: PlannedValueKind::DataSeries,
                type_expr: series,
            },
        )]),
    };
    let error = match FunctionPlanStore::new(plan.provenance.project_session_id.clone(), 64)
        .generation(
            registry.fingerprint.clone(),
            versions,
            vec![(path, version, Arc::new(plan), Arc::new(abi))],
        ) {
        Ok(_) => panic!("corrupt function ABI value contract must be rejected"),
        Err(error) => error,
    };

    assert!(
        matches!(
            error,
            FunctionPlanStoreError::AbiValueContractMismatch { .. }
        ),
        "unexpected function plan store error: {error:?}"
    );
}

#[test]
fn compiler_keeps_fully_materialized_ols_report_directly_connected_to_view_data() {
    let builtins = crate::node_system::catalog::build_builtin_node_system().unwrap();
    let mut ols = builtins
        .registry
        .protocol(&NodeTypeId::new("yssbi.statistics.ols.summary").unwrap())
        .unwrap()
        .clone();
    ols.interface.ports = ols
        .interface
        .ports
        .iter()
        .filter(|port| port.key.as_str() == "report")
        .cloned()
        .collect();
    let mut view = builtins
        .registry
        .protocol(&NodeTypeId::new("yssbi.debug.view").unwrap())
        .unwrap()
        .clone();
    view.interface.ports = view
        .interface
        .ports
        .iter()
        .filter(|port| port.key.as_str() == "data")
        .cloned()
        .collect();
    let registry = TestRegistry::new(vec![ols, view]);
    let mut graph =
        builtin_graph_with_nodes(&[(1, "yssbi.statistics.ols.summary"), (2, "yssbi.debug.view")]);
    connect(&mut graph, 10, 1, "report", 2, "data");

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);
    let plan = result.plan.unwrap_or_else(|| {
        panic!(
            "OLS report -> View Data diagnostics: {:?}",
            result.analysis.diagnostics
        )
    });
    let ols = &plan.operations[operation_index_for_node(&plan, 1).index()];
    let view = &plan.operations[operation_index_for_node(&plan, 2).index()];

    assert!(
        plan.operations
            .iter()
            .all(|operation| !matches!(operation.kernel, PlannedKernel::Adapter(_))),
        "an already fully-materialized boundary must not insert an adapter operation"
    );
    assert!(plan.value_dependencies.contains(&ValueDependency {
        source: ols.outputs[0].value,
        destination: view.inputs[0].value,
    }));
}

#[test]
fn compiler_materializes_stream_once_before_same_contract_fanout() {
    let mut source_output = data_port("out", PortDirection::Output, TypeExpr::Unknown, None);
    source_output.production = Some(OutputProduction::Streaming);
    let source = test_protocol("fanout_source", vec![source_output], vec![], vec![]);
    let mut sink_input = data_port("in", PortDirection::Input, TypeExpr::Unknown, None);
    sink_input.consumption = Some(InputConsumption::FullyMaterialized);
    let sink = test_protocol("fanout_sink", vec![sink_input], vec![], vec![]);
    let registry = TestRegistry::new(vec![source, sink]);
    let mut graph =
        graph_with_nodes(&[(1, "fanout_source"), (2, "fanout_sink"), (3, "fanout_sink")]);
    connect(&mut graph, 10, 1, "out", 2, "in");
    connect(&mut graph, 11, 1, "out", 3, "in");

    let plan = GraphCompiler::new(&registry, &Resources)
        .compile(&graph)
        .plan
        .expect("fanout graph should lower");
    let adapters = plan
        .operations
        .iter()
        .filter_map(|operation| match &operation.kernel {
            PlannedKernel::Adapter(adapter) => Some((operation, adapter)),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        adapters
            .iter()
            .filter(|(_, adapter)| matches!(adapter, PlannedAdapter::Collect { .. }))
            .count(),
        1
    );
    assert_eq!(
        adapters
            .iter()
            .filter(|(_, adapter)| matches!(adapter, PlannedAdapter::Identity))
            .count(),
        0
    );
    let shared = adapters
        .iter()
        .find(|(_, adapter)| matches!(adapter, PlannedAdapter::Collect { .. }))
        .unwrap()
        .0;
    assert_eq!(
        plan.value_dependencies
            .iter()
            .filter(|dependency| dependency.source == shared.outputs[0].value)
            .count(),
        2,
        "the stable collected artifact is the single fanout owner"
    );
}

#[test]
fn compiler_streaming_fanout_with_different_contracts_is_permutation_stable() {
    let mut source_output = data_port("out", PortDirection::Output, TypeExpr::Unknown, None);
    source_output.production = Some(OutputProduction::Streaming);
    let source = test_protocol("fanout_source_mixed", vec![source_output], vec![], vec![]);

    let sink = |name: &str, consumption| {
        let mut input = data_port("in", PortDirection::Input, TypeExpr::Unknown, None);
        input.consumption = Some(consumption);
        let output = data_port("out", PortDirection::Output, TypeExpr::Unknown, None);
        test_protocol(name, vec![input, output], vec![], vec![])
    };
    let registry = TestRegistry::new(vec![
        source,
        sink("fanout_sink_stream", InputConsumption::Streaming),
        sink(
            "fanout_sink_materialized",
            InputConsumption::FullyMaterialized,
        ),
    ]);

    let compile = |nodes: &[(u128, &str)], source: u128, streaming: u128, materialized: u128| {
        let mut graph = graph_with_nodes(nodes);
        connect(&mut graph, 10, source, "out", streaming, "in");
        connect(&mut graph, 11, source, "out", materialized, "in");
        GraphCompiler::new(&registry, &Resources)
            .compile(&graph)
            .plan
            .expect("mixed fanout graph should lower")
    };
    let forward = compile(
        &[
            (1, "fanout_source_mixed"),
            (2, "fanout_sink_stream"),
            (3, "fanout_sink_materialized"),
        ],
        1,
        2,
        3,
    );
    let permuted = compile(
        &[
            (103, "fanout_sink_materialized"),
            (101, "fanout_source_mixed"),
            (102, "fanout_sink_stream"),
        ],
        101,
        102,
        103,
    );

    let normalize = |plan: &ExecutionPlan| {
        let kind = |operation: &PlannedOperation| match &operation.kernel {
            PlannedKernel::Native(_) => format!("native:{}", operation.source_node_type_id),
            PlannedKernel::Relational(_) => "relational".to_owned(),
            PlannedKernel::Adapter(adapter) => format!("adapter:{adapter:?}"),
        };
        let owners = plan
            .operations
            .iter()
            .enumerate()
            .flat_map(|(index, operation)| {
                operation
                    .outputs
                    .iter()
                    .map(move |output| (output.value, index))
            })
            .collect::<BTreeMap<_, _>>();
        let consumers = plan
            .operations
            .iter()
            .enumerate()
            .flat_map(|(index, operation)| {
                operation
                    .inputs
                    .iter()
                    .map(move |input| (input.value, index))
            })
            .collect::<BTreeMap<_, _>>();
        let mut operations = plan
            .operations
            .iter()
            .map(|operation| {
                (
                    kind(operation),
                    operation
                        .inputs
                        .iter()
                        .map(|input| input.consumption)
                        .collect::<Vec<_>>(),
                    operation
                        .outputs
                        .iter()
                        .map(|output| output.production)
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        operations.sort_by(|left, right| format!("{left:?}").cmp(&format!("{right:?}")));
        let mut topology = plan
            .value_dependencies
            .iter()
            .filter_map(|dependency| {
                Some((
                    kind(&plan.operations[*owners.get(&dependency.source)?]),
                    kind(&plan.operations[*consumers.get(&dependency.destination)?]),
                ))
            })
            .collect::<Vec<_>>();
        topology.sort();
        (operations, topology)
    };

    assert_eq!(normalize(&forward), normalize(&permuted));
    assert_ne!(
        forward
            .operations
            .iter()
            .map(|operation| &operation.stable_id)
            .collect::<Vec<_>>(),
        permuted
            .operations
            .iter()
            .map(|operation| &operation.stable_id)
            .collect::<Vec<_>>(),
        "Task 10 permits stable IDs to follow real node UUIDs"
    );
    for plan in [&forward, &permuted] {
        assert_eq!(
            plan.operations
                .iter()
                .filter(|operation| matches!(
                    operation.kernel,
                    PlannedKernel::Adapter(PlannedAdapter::Collect { .. })
                ))
                .count(),
            1
        );
        assert_eq!(
            plan.operations
                .iter()
                .filter(|operation| matches!(
                    operation.kernel,
                    PlannedKernel::Adapter(PlannedAdapter::StreamBridge { .. })
                ))
                .count(),
            1
        );
        assert_eq!(
            plan.operations
                .iter()
                .filter(|operation| matches!(
                    operation.kernel,
                    PlannedKernel::Adapter(PlannedAdapter::Identity)
                ))
                .count(),
            0
        );
    }

    let mut demand_graph = graph_with_nodes(&[
        (1, "fanout_source_mixed"),
        (2, "fanout_sink_stream"),
        (3, "fanout_sink_materialized"),
    ]);
    connect(&mut demand_graph, 10, 1, "out", 2, "in");
    connect(&mut demand_graph, 11, 1, "out", 3, "in");
    let compiled = GraphCompiler::new(&registry, &Resources).compile(&demand_graph);
    let basis = compiled
        .execution_basis
        .expect("mixed fanout graph keeps a demand basis");
    let graph_path = basis.provenance.graph_path.0.clone();
    let specialized = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output(&graph_path, 3, "out")]),
            include_default_results: false,
        })
        .expect("materialized fanout consumer specializes");
    assert_eq!(
        specialized
            .operations
            .iter()
            .filter(|operation| matches!(operation.kernel, PlannedKernel::Adapter(_)))
            .count(),
        1,
        "demand specialization replans one retained boundary without duplicating shared fanout"
    );
    assert!(specialized.operations.iter().any(|operation| matches!(
        operation.kernel,
        PlannedKernel::Adapter(PlannedAdapter::Collect { .. })
    )));
}

#[test]
fn compiler_maps_effect_edges_into_plan_dependencies() {
    let mut before = test_protocol(
        "plan_effect_before",
        vec![effect_port("effect", PortDirection::Output)],
        vec![],
        vec![],
    );
    before.execution.purity = Purity::Effectful;
    before.execution.effects = EffectSemantics::Ordered;
    let mut after = test_protocol(
        "plan_effect_after",
        vec![effect_port("effect", PortDirection::Input)],
        vec![],
        vec![],
    );
    after.execution.purity = Purity::Effectful;
    after.execution.effects = EffectSemantics::Ordered;
    let before_type = before.type_id.clone();
    let after_type = after.type_id.clone();
    let registry = TestRegistry::new(vec![before, after])
        .with_lowerer(
            &before_type,
            FragmentLowerer {
                fragment: kernel_fragment(EffectSemantics::Ordered, FragmentMetadata::default()),
            },
        )
        .with_lowerer(
            &after_type,
            FragmentLowerer {
                fragment: kernel_fragment(EffectSemantics::Ordered, FragmentMetadata::default()),
            },
        );
    let mut graph = graph_with_nodes(&[(1, "plan_effect_before"), (2, "plan_effect_after")]);
    connect(&mut graph, 10, 1, "effect", 2, "effect");

    let plan = GraphCompiler::new(&registry, &Resources)
        .compile(&graph)
        .plan
        .expect("effect graph should lower");

    assert_eq!(plan.effect_dependencies.len(), 1);
    assert_eq!(plan.effect_dependencies[0].before.index(), 0);
    assert_eq!(plan.effect_dependencies[0].after.index(), 1);
}

#[test]
fn compiler_plans_relational_islands_with_valid_local_indices() {
    let mut source_output = data_port("out", PortDirection::Output, TypeExpr::Unknown, None);
    source_output.production = Some(OutputProduction::Streaming);
    let source = test_protocol("plan_relation_source", vec![source_output], vec![], vec![]);
    let mut sink_input = data_port("in", PortDirection::Input, TypeExpr::Unknown, None);
    sink_input.consumption = Some(InputConsumption::Streaming);
    let mut sink_output = data_port("out", PortDirection::Output, TypeExpr::Unknown, None);
    sink_output.production = Some(OutputProduction::Streaming);
    let sink = test_protocol(
        "plan_relation_sink",
        vec![sink_input, sink_output],
        vec![],
        vec![],
    );
    let source_type = source.type_id.clone();
    let sink_type = sink.type_id.clone();
    let backend = RelationalBackendId::new("test.relational").unwrap();
    let registry = TestRegistry::new(vec![source, sink])
        .with_lowerer(
            &source_type,
            FragmentLowerer {
                fragment: LoweredKernel::Relational(RelationalNodeFragment {
                    backend: backend.clone(),
                    fragment: relational::RelationalFragment {
                        id: RelationalFragmentId::new("source").unwrap(),
                        operators: Box::new([RelationalOperator::Source {
                            resource: ResourceId::new("database.source").unwrap(),
                            relation: "items".into(),
                        }]),
                        root: RelationalOperatorIndex::new(0),
                    },
                    inputs: Box::new([]),
                    metadata: FragmentMetadata::default(),
                }),
            },
        )
        .with_lowerer(
            &sink_type,
            FragmentLowerer {
                fragment: LoweredKernel::Relational(RelationalNodeFragment {
                    backend,
                    fragment: relational::RelationalFragment {
                        id: RelationalFragmentId::new("sink").unwrap(),
                        operators: Box::new([
                            RelationalOperator::Input {
                                name: "input".into(),
                            },
                            RelationalOperator::Limit {
                                input: RelationalOperatorIndex::new(0),
                                rows: 10,
                            },
                        ]),
                        root: RelationalOperatorIndex::new(1),
                    },
                    inputs: Box::new([RelationalInputBinding {
                        port: PortAddress::declared(node_id(2), key("in")),
                        operator: RelationalOperatorIndex::new(0),
                    }]),
                    metadata: FragmentMetadata::default(),
                }),
            },
        );
    let mut graph = graph_with_nodes(&[(1, "plan_relation_source"), (2, "plan_relation_sink")]);
    connect(&mut graph, 10, 1, "out", 2, "in");

    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/relational".into()), &graph);
    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let basis = result
        .execution_basis
        .expect("relational graph should retain pre-group facts");
    let plan = result.plan.expect("relational graph should lower");

    assert_eq!(plan.relational_subplans.len(), 1);
    let subplan = &plan.relational_subplans[0];
    assert_eq!(subplan.compiled_plan.fragment_order.len(), 2);
    assert_eq!(subplan.compiled_plan.roots.len(), 1);

    assert_eq!(
        plan.operations.len(),
        1,
        "one operation must own the island"
    );
    let operation = &plan.operations[0];
    assert!(matches!(
        operation.kernel,
        crate::node_system::plan::PlannedKernel::Relational(index) if index.index() == 0
    ));
    assert!(
        operation.inputs.is_empty(),
        "the source makes the island self-contained"
    );
    assert_eq!(operation.outputs.len(), 1, "only the sink root is exposed");
    assert_eq!(operation.outputs[0].production, OutputProduction::Streaming);
    assert!(
        plan.value_dependencies.is_empty(),
        "an internal fragment edge must not make the island depend on itself"
    );
    assert!(matches!(
        plan.root_region,
        crate::node_system::plan::StructuredControlRegion::Sequence(ref steps)
            if matches!(steps.as_ref(), [crate::node_system::plan::ControlStep::Operation(index)] if index.index() == 0)
    ));

    assert_eq!(basis.operations.len(), 2, "basis remains pre-group");
    assert_eq!(basis.relational_connections.len(), 1);
    assert!(matches!(
        &basis.operations[0].kernel,
        super::specialization::IntermediateKernel::Relational { fragment, .. }
            if fragment.id.as_str() == "source"
    ));
    assert!(matches!(
        &basis.operations[1].kernel,
        super::specialization::IntermediateKernel::Relational { fragment, input_bindings, .. }
            if fragment.id.as_str() == "sink" && input_bindings.len() == 1
    ));

    let requested_source = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/relational", 1, "out")]),
            include_default_results: false,
        })
        .expect("same-island intermediate output must be derivable");
    assert_eq!(requested_source.operations.len(), 1);
    assert_eq!(requested_source.operations[0].outputs.len(), 1);
    assert_ne!(
        requested_source.operations[0].stable_id, plan.operations[0].stable_id,
        "different demand/member combinations need different composite IDs"
    );
    assert_ne!(
        requested_source.operations[0].semantics_version, plan.operations[0].semantics_version,
        "different fused relational semantics need different versions"
    );
    assert!(basis.operations.iter().all(|member| {
        member.stable_id != plan.operations[0].stable_id
            && member.semantics_version != plan.operations[0].semantics_version
    }));
    let requested_source_again = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/relational", 1, "out")]),
            include_default_results: false,
        })
        .unwrap();
    assert_eq!(
        requested_source.operations[0].stable_id,
        requested_source_again.operations[0].stable_id
    );
    assert_eq!(
        requested_source.operations[0].semantics_version,
        requested_source_again.operations[0].semantics_version
    );
    assert_eq!(requested_source.relational_subplans.len(), 1);
    assert_eq!(
        requested_source.relational_subplans[0]
            .compiled_plan
            .fragment_order
            .as_ref(),
        &[RelationalFragmentId::new("source").unwrap()]
    );
    assert_eq!(
        requested_source.relational_subplans[0]
            .compiled_plan
            .roots
            .len(),
        requested_source.operations[0].outputs.len()
    );
    requested_source
        .validate()
        .expect("requested relational output plan validates");

    let mut retry_basis = basis.clone();
    let retry = PlannedRetry {
        idempotent: true,
        policy: Some(
            RetryPolicy::new(
                std::num::NonZeroU32::new(2).unwrap(),
                std::time::Duration::from_millis(1),
                std::time::Duration::from_millis(2),
            )
            .unwrap(),
        ),
    };
    retry_basis.operations[0].retry = retry.clone();
    retry_basis.operations[1].retry = retry.clone();
    retry_basis.operations[0].semantics_version = ExecutionSemanticsVersion::from_bytes([1; 32]);
    retry_basis.operations[1].semantics_version = ExecutionSemanticsVersion::from_bytes([2; 32]);
    assert!(matches!(
        retry_basis.derive_full_plan().unwrap_err(),
        DemandPlanError::InvalidDerivedPlan(message)
            if message.contains("InvalidRetryPolicy")
    ));

    retry_basis.operations[0].retry = PlannedRetry::default();
    retry_basis.operations[1].retry = PlannedRetry::default();
    let conservative_retry = retry_basis.derive_full_plan().unwrap();
    assert_eq!(
        conservative_retry.operations[0].retry,
        PlannedRetry::default()
    );
    assert_ne!(
        conservative_retry.operations[0].semantics_version,
        retry_basis.operations[0].semantics_version
    );
    assert_ne!(
        conservative_retry.operations[0].semantics_version,
        retry_basis.operations[1].semantics_version
    );

    let mut reversed = graph_with_nodes(&[(2, "plan_relation_sink"), (1, "plan_relation_source")]);
    connect(&mut reversed, 10, 1, "out", 2, "in");
    let reversed_compiler = GraphCompiler::new(&registry, &Resources);
    let reversed_plan = reversed_compiler
        .compile_snapshot(
            &reversed_compiler.snapshot(GraphResourcePath("events/relational".into()), &reversed),
            &CompileCancellationToken::new(),
        )
        .unwrap()
        .plan
        .expect("reordered relational graph should lower");
    assert_eq!(reversed_plan.operations, plan.operations);
    assert_eq!(
        reversed_plan.operations[0].stable_id, plan.operations[0].stable_id,
        "composite identity is insertion-order independent"
    );
    assert_eq!(reversed_plan.value_dependencies, plan.value_dependencies);
    assert_eq!(reversed_plan.root_region, plan.root_region);
    assert_eq!(reversed_plan.relational_subplans, plan.relational_subplans);
}

#[test]
fn compiler_aggregates_fragment_resources_and_results() {
    let protocol = test_protocol(
        "plan_metadata",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let node_type = protocol.type_id.clone();
    let resource = CompiledResourceRequirement {
        resource: ResourceId::new("database.main").unwrap(),
        kind: ResourceKind::DatabaseConnection,
        access: ResourceAccess::Shared,
        optional: false,
    };
    let registry = TestRegistry::new(vec![protocol]).with_lowerer(
        &node_type,
        FragmentLowerer {
            fragment: kernel_fragment(
                EffectSemantics::None,
                FragmentMetadata {
                    effect: EffectSemantics::None,
                    resources: Box::new([resource.clone()]),
                    results: Box::new([FragmentResult {
                        name: "answer".into(),
                        output: PortAddress::declared(node_id(1), key("out")),
                    }]),
                },
            ),
        },
    );

    let plan = GraphCompiler::new(&registry, &Resources)
        .compile(&graph_with_nodes(&[(1, "plan_metadata")]))
        .plan
        .expect("metadata graph should lower");

    assert_eq!(plan.resources.as_ref(), &[resource]);
    assert_eq!(
        plan.operations[0].resource_dependencies.as_ref(),
        &[crate::node_system::analysis::ResourceKey::new(
            "database.main"
        )]
    );
    assert_eq!(
        plan.results.as_ref(),
        &[PlanResult {
            name: "answer".into(),
            output: GraphOutputRef {
                graph_path: plan.provenance.graph_path.clone(),
                port: PortAddress::declared(node_id(1), key("out")),
            },
            value: plan.operations[0].outputs[0].value,
        }]
    );
}

#[test]
fn duplicate_lowering_result_emits_the_result_name() {
    let first = test_protocol(
        "duplicate_result_first",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let second = test_protocol(
        "duplicate_result_second",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let first_type = first.type_id.clone();
    let second_type = second.type_id.clone();
    let registry = TestRegistry::new(vec![first, second])
        .with_lowerer(
            &first_type,
            FragmentLowerer {
                fragment: kernel_fragment(
                    EffectSemantics::None,
                    FragmentMetadata {
                        effect: EffectSemantics::None,
                        resources: Box::new([]),
                        results: Box::new([FragmentResult {
                            name: "answer".into(),
                            output: PortAddress::declared(node_id(1), key("out")),
                        }]),
                    },
                ),
            },
        )
        .with_lowerer(
            &second_type,
            FragmentLowerer {
                fragment: kernel_fragment(
                    EffectSemantics::None,
                    FragmentMetadata {
                        effect: EffectSemantics::None,
                        resources: Box::new([]),
                        results: Box::new([FragmentResult {
                            name: "answer".into(),
                            output: PortAddress::declared(node_id(2), key("out")),
                        }]),
                    },
                ),
            },
        );

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph_with_nodes(&[
        (1, "duplicate_result_first"),
        (2, "duplicate_result_second"),
    ]));

    assert!(result.semantic.is_some());
    assert!(result.plan.is_none());
    assert!(result.analysis.diagnostics.is_empty());
    assert!(matches!(
        result.outcome,
        CompilationOutcome::InternalFailure(ref failure)
            if failure.stage == CompilationStage::Lowering
                && failure.code.as_ref() == "compiler.lowering.result_duplicate"
                && failure.node_id == Some(node_id(2))
    ));
}

fn demand_output(graph_path: &str, node: u128, port: &str) -> GraphOutputRef {
    GraphOutputRef {
        graph_path: GraphResourcePath(graph_path.into()),
        port: PortAddress::declared(node_id(node), key(port)),
    }
}

#[test]
fn finalization_rejects_public_output_that_conflicts_with_exact_port_facts() {
    let (registry, graph) = demand_fixture();
    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);
    let mut basis = result.execution_basis.expect("demand fixture must lower");
    let operation = basis
        .operations
        .iter_mut()
        .find(|operation| operation.source_node_id == node_id(2))
        .unwrap();
    operation.outputs[0].public_output.as_mut().unwrap().port =
        PortAddress::declared(node_id(2), key("in"));

    let error = basis
        .derive_full_plan()
        .expect_err("input/non-output public identity must be rejected");
    assert!(matches!(error, DemandPlanError::InvalidDerivedPlan(_)));
}

fn chain_protocol(name: &str) -> NodeProtocol {
    test_protocol(
        name,
        vec![
            data_port("in", PortDirection::Input, TypeExpr::Unknown, None),
            data_port("out", PortDirection::Output, TypeExpr::Unknown, None),
        ],
        vec![],
        vec![],
    )
}

fn demand_fixture() -> (TestRegistry, GraphDocument) {
    let mut entry = test_protocol(
        "demand_event_begin",
        vec![control_port("then", PortDirection::Output)],
        vec![],
        vec![],
    );
    entry.managed_role = Some(ManagedNodeRole::EventBegin);
    let entry_type = entry.type_id.clone();
    let source_a = test_protocol(
        "demand_source_a",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let sink_a = chain_protocol("demand_sink_a");
    let source_b = test_protocol(
        "demand_source_b",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let sink_b = chain_protocol("demand_sink_b");
    let sink_a_type = sink_a.type_id.clone();
    let sink_b_type = sink_b.type_id.clone();
    let registry = TestRegistry::new(vec![entry, source_a, sink_a, source_b, sink_b])
        .structural(&entry_type, StructuralNodeRole::EventBegin)
        .with_lowerer(
            &sink_a_type,
            FragmentLowerer {
                fragment: kernel_fragment(
                    EffectSemantics::None,
                    FragmentMetadata {
                        effect: EffectSemantics::None,
                        resources: Box::new([CompiledResourceRequirement {
                            resource: ResourceId::new("database.chain-a").unwrap(),
                            kind: ResourceKind::DatabaseConnection,
                            access: ResourceAccess::Shared,
                            optional: false,
                        }]),
                        results: Box::new([FragmentResult {
                            name: "chain-a".into(),
                            output: PortAddress::declared(node_id(2), key("out")),
                        }]),
                    },
                ),
            },
        )
        .with_lowerer(
            &sink_b_type,
            FragmentLowerer {
                fragment: kernel_fragment(
                    EffectSemantics::None,
                    FragmentMetadata {
                        effect: EffectSemantics::None,
                        resources: Box::new([CompiledResourceRequirement {
                            resource: ResourceId::new("database.chain-b").unwrap(),
                            kind: ResourceKind::DatabaseConnection,
                            access: ResourceAccess::Shared,
                            optional: false,
                        }]),
                        results: Box::new([FragmentResult {
                            name: "chain-b".into(),
                            output: PortAddress::declared(node_id(4), key("out")),
                        }]),
                    },
                ),
            },
        );
    let mut graph = graph_with_nodes(&[
        (100, "demand_event_begin"),
        (1, "demand_source_a"),
        (2, "demand_sink_a"),
        (3, "demand_source_b"),
        (4, "demand_sink_b"),
    ]);
    connect(&mut graph, 10, 1, "out", 2, "in");
    connect(&mut graph, 11, 3, "out", 4, "in");
    (registry, graph)
}

fn compiled_demand_basis() -> ExecutionPlanBasis {
    let (registry, graph) = demand_fixture();
    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/main".into()), &graph);
    compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap()
        .execution_basis
        .expect("demand fixture has a basis")
}

#[test]
fn demand_specialization_ignores_unbound_inputs_outside_the_retained_closure() {
    let (registry, mut graph) = demand_fixture();
    graph
        .connections
        .remove(&ConnectionId::from_uuid(Uuid::from_u128(11)));
    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/main".into()), &graph);

    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();

    assert!(
        matches!(result.outcome, CompilationOutcome::Succeeded),
        "unexpected outcome {:?} with diagnostics {:?}",
        result.outcome,
        result.analysis.diagnostics
    );
    assert!(result.analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "compiler.input.unbound"
            && diagnostic.primary
                == DiagnosticLocation::Port(PortAddress::declared(node_id(4), key("in")))
    }));
    let basis = result
        .execution_basis
        .expect("unbound orphan preserves basis");
    basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/main", 2, "out")]),
            include_default_results: false,
        })
        .expect("unbound input outside the retained closure must not block execution");
}

#[test]
fn pin_preview_ignores_unbound_inputs_outside_the_retained_closure() {
    let (registry, mut graph) = demand_fixture();
    graph
        .connections
        .remove(&ConnectionId::from_uuid(Uuid::from_u128(11)));
    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/main".into()), &graph);
    let basis = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap()
        .execution_basis
        .expect("unbound orphan preserves preview basis");

    basis
        .derive_plan(&ExecutionDemand::PinPreview {
            output: demand_output("events/main", 2, "out"),
            generation: 7,
        })
        .expect("preview ignores unbound inputs outside its retained closure");
}

#[test]
fn demand_specialization_rejects_a_required_unbound_input_with_its_port() {
    let (registry, mut graph) = demand_fixture();
    graph
        .connections
        .remove(&ConnectionId::from_uuid(Uuid::from_u128(11)));
    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/main".into()), &graph);
    let basis = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap()
        .execution_basis
        .expect("unbound input preserves demand-specializable basis");
    let expected = PortAddress::declared(node_id(4), key("in"));

    assert_eq!(
        basis
            .derive_plan(&ExecutionDemand::Outputs {
                outputs: Box::new([demand_output("events/main", 4, "out")]),
                include_default_results: false,
            })
            .unwrap_err(),
        DemandPlanError::UnboundInput(expected)
    );
}

fn append_control_region(basis: &mut ExecutionPlanBasis, region: StructuredControlRegion) {
    let StructuredControlRegion::Sequence(steps) = &mut basis.root_region else {
        panic!("demand fixture root is a sequence")
    };
    let mut projected = steps.to_vec();
    projected.push(ControlStep::Region(Box::new(region)));
    *steps = projected.into_boxed_slice();
}

fn declare_control_value(basis: &mut ExecutionPlanBasis) -> ValueRef {
    let value = ValueRef::new(basis.value_count);
    basis.value_count += 1;
    let mut sources = basis.value_sources.to_vec();
    sources.push(PlanValueSource::ControlProduced(
        value,
        OutputProduction::FullyMaterialized,
    ));
    sources.sort();
    basis.value_sources = sources.into_boxed_slice();
    value
}

#[test]
fn demand_specialization_deletes_disconnected_if_control_sources() {
    let mut basis = compiled_demand_basis();
    let destination = declare_control_value(&mut basis);
    let condition = basis.operations[0].outputs[0].value;
    let then_source = basis.operations[0].outputs[0].value;
    let else_source = basis.operations[2].outputs[0].value;
    append_control_region(
        &mut basis,
        StructuredControlRegion::If {
            condition,
            then_region: Box::new(StructuredControlRegion::Sequence(Box::new([]))),
            else_region: Box::new(StructuredControlRegion::Sequence(Box::new([]))),
            results: Box::new([BranchResultBinding {
                destination,
                then_source,
                else_source,
                production: Some(OutputProduction::FullyMaterialized),
            }]),
        },
    );

    let plan = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/main", 2, "out")]),
            include_default_results: false,
        })
        .expect("disconnected If declarations are projected out");

    assert!(!contains_region(&plan.root_region, &|region| matches!(
        region,
        StructuredControlRegion::If { .. }
    )));
    assert!(
        !plan
            .value_sources
            .contains(&PlanValueSource::ControlProduced(
                destination,
                OutputProduction::FullyMaterialized,
            ))
    );
    plan.validate().unwrap();
}

#[test]
fn demand_specialization_keeps_only_requested_if_result_declaration() {
    let mut basis = compiled_demand_basis();
    let retained_destination = declare_control_value(&mut basis);
    let deleted_destination = declare_control_value(&mut basis);
    let condition = basis.operations[0].outputs[0].value;
    let then_source = basis.operations[0].outputs[0].value;
    let else_source = basis.operations[2].outputs[0].value;
    append_control_region(
        &mut basis,
        StructuredControlRegion::If {
            condition,
            then_region: Box::new(StructuredControlRegion::Sequence(Box::new([]))),
            else_region: Box::new(StructuredControlRegion::Sequence(Box::new([]))),
            results: Box::new([
                BranchResultBinding {
                    destination: retained_destination,
                    then_source,
                    else_source,
                    production: Some(OutputProduction::FullyMaterialized),
                },
                BranchResultBinding {
                    destination: deleted_destination,
                    then_source,
                    else_source,
                    production: Some(OutputProduction::FullyMaterialized),
                },
            ]),
        },
    );
    let requested = demand_output("events/main", 99, "out");
    basis.nodes.insert(node_id(99));
    basis.port_facts.insert(
        requested.port.clone(),
        super::specialization::DemandPortFact {
            kind: PortKind::Data,
            direction: PortDirection::Output,
        },
    );
    basis.output_results.insert(
        requested.clone(),
        PlanResult {
            name: "requested.branch-result".into(),
            output: requested.clone(),
            value: retained_destination,
        },
    );

    let plan = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([requested]),
            include_default_results: false,
        })
        .expect("only the requested If result declaration remains");

    let retained_results = match &plan.root_region {
        region
            if contains_region(region, &|candidate| {
                matches!(candidate, StructuredControlRegion::If { .. })
            }) =>
        {
            fn find(region: &StructuredControlRegion) -> Option<&[BranchResultBinding]> {
                match region {
                    StructuredControlRegion::Sequence(steps) => {
                        steps.iter().find_map(|step| match step {
                            ControlStep::Operation(_) => None,
                            ControlStep::Region(region) => find(region),
                        })
                    }
                    StructuredControlRegion::If { results, .. } => Some(results),
                    StructuredControlRegion::Loop { body, .. } => find(body),
                    StructuredControlRegion::Call { .. } => None,
                }
            }
            find(region).unwrap()
        }
        _ => panic!("retained If region"),
    };
    assert_eq!(retained_results.len(), 1);
    assert_eq!(retained_results[0].destination, retained_destination);
    assert!(
        plan.value_sources
            .contains(&PlanValueSource::ControlProduced(
                retained_destination,
                OutputProduction::FullyMaterialized,
            ))
    );
    assert!(
        !plan
            .value_sources
            .contains(&PlanValueSource::ControlProduced(
                deleted_destination,
                OutputProduction::FullyMaterialized,
            ))
    );
    plan.validate().unwrap();
}

#[test]
fn demand_specialization_deletes_disconnected_loop_control_sources() {
    let mut basis = compiled_demand_basis();
    let body_input = declare_control_value(&mut basis);
    let result = declare_control_value(&mut basis);
    let initial_source = basis.operations[0].outputs[0].value;
    let next_source = basis.operations[2].outputs[0].value;
    let continue_condition = basis.operations[0].outputs[0].value;
    append_control_region(
        &mut basis,
        StructuredControlRegion::Loop {
            body: Box::new(StructuredControlRegion::Sequence(Box::new([]))),
            carried: Box::new([LoopCarriedBinding {
                body_input,
                initial_source,
                next_source,
                result,
                production: Some(OutputProduction::FullyMaterialized),
            }]),
            continue_condition,
            max_iterations: 3,
        },
    );

    let plan = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/main", 2, "out")]),
            include_default_results: false,
        })
        .expect("disconnected Loop declarations are projected out");

    assert!(!contains_region(&plan.root_region, &|region| matches!(
        region,
        StructuredControlRegion::Loop { .. }
    )));
    for value in [body_input, result] {
        assert!(
            !plan
                .value_sources
                .contains(&PlanValueSource::ControlProduced(
                    value,
                    OutputProduction::FullyMaterialized,
                ))
        );
    }
    plan.validate().unwrap();
}

#[test]
fn demand_specialization_prunes_independent_pure_chain_and_owned_resource() {
    let (registry, graph) = demand_fixture();
    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/main".into()), &graph);
    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let basis = result
        .execution_basis
        .expect("valid graph has lowering basis");

    let plan = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/main", 2, "out")]),
            include_default_results: false,
        })
        .unwrap();

    assert_eq!(
        plan.operations
            .iter()
            .filter(|operation| !matches!(operation.kernel, PlannedKernel::Adapter(_)))
            .map(|operation| operation.source_node_id)
            .collect::<Vec<_>>(),
        vec![node_id(1), node_id(2)]
    );
    assert_eq!(plan.results.len(), 1);
    assert_eq!(
        plan.results[0].output,
        demand_output("events/main", 2, "out")
    );
    assert_eq!(
        plan.resources
            .iter()
            .map(|requirement| requirement.resource.as_str())
            .collect::<Vec<_>>(),
        vec!["database.chain-a"]
    );
}

#[test]
fn demand_normalization_is_order_independent_and_default_modes_are_distinct() {
    let (registry, graph) = demand_fixture();
    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/main".into()), &graph);
    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let basis = result
        .execution_basis
        .expect("valid graph has lowering basis");
    let a = demand_output("events/main", 2, "out");
    let b = demand_output("events/main", 4, "out");

    let first = ExecutionDemand::Outputs {
        outputs: Box::new([a.clone(), b.clone()]),
        include_default_results: false,
    };
    let second = ExecutionDemand::Outputs {
        outputs: Box::new([b.clone(), a.clone(), a.clone()]),
        include_default_results: false,
    };
    let first_key = basis.normalize_demand(&first).unwrap();
    let second_key = basis.normalize_demand(&second).unwrap();
    assert_eq!(first_key, second_key);
    assert_eq!(first_key.digest().unwrap(), second_key.digest().unwrap());
    assert_eq!(
        basis.derive_plan(&first).unwrap(),
        basis.derive_plan(&second).unwrap()
    );
    let same_selection_different_mode = ExecutionDemand::Outputs {
        outputs: Box::new([]),
        include_default_results: true,
    };
    let default_key = basis.normalize_demand(&ExecutionDemand::Default).unwrap();
    let explicit_default_key = basis
        .normalize_demand(&same_selection_different_mode)
        .unwrap();
    assert_ne!(
        default_key, explicit_default_key,
        "normalized keys retain request mode even when selected outputs match"
    );
    assert_ne!(
        default_key.digest().unwrap(),
        explicit_default_key.digest().unwrap()
    );
    let without_defaults = basis
        .normalize_demand(&ExecutionDemand::Outputs {
            outputs: Box::new([a.clone(), b.clone()]),
            include_default_results: false,
        })
        .unwrap();
    let with_defaults = basis
        .normalize_demand(&ExecutionDemand::Outputs {
            outputs: Box::new([a.clone(), b.clone()]),
            include_default_results: true,
        })
        .unwrap();
    assert_ne!(without_defaults, with_defaults);
    assert_ne!(
        without_defaults.digest().unwrap(),
        with_defaults.digest().unwrap()
    );

    let defaults = basis.derive_plan(&ExecutionDemand::Default).unwrap();
    assert_eq!(defaults.results.len(), 2);
    let only_a = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([a.clone()]),
            include_default_results: false,
        })
        .unwrap();
    assert_eq!(only_a.results.len(), 1);
    let a_with_defaults = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([a]),
            include_default_results: true,
        })
        .unwrap();
    assert_eq!(a_with_defaults.results.len(), 2);
}

#[test]
fn demand_driven_publication_preview_has_independent_normalized_identity_and_generation() {
    let (registry, graph) = demand_fixture();
    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/main".into()), &graph);
    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let basis = result
        .execution_basis
        .expect("valid graph has lowering basis");
    let output = demand_output("events/main", 2, "out");
    let ordinary = ExecutionDemand::Outputs {
        outputs: Box::new([output.clone()]),
        include_default_results: false,
    };
    let preview: ExecutionDemand = serde_json::from_value(serde_json::json!({
        "PinPreview": {
            "output": serde_json::to_value(&output).unwrap(),
            "generation": 17
        }
    }))
    .expect("pin preview demand must have a dedicated wire variant");

    let ordinary_plan = basis.derive_plan(&ordinary).unwrap();
    let preview_plan = basis.derive_plan(&preview).unwrap();
    let ordinary = basis.normalize_demand(&ordinary).unwrap();
    let preview = basis.normalize_demand(&preview).unwrap();

    assert!(matches!(
        ordinary_plan.publications.as_ref(),
        [PlannedPublication::GraphResult { output: published, .. }] if published == &output
    ));
    assert!(matches!(
        preview_plan.publications.as_ref(),
        [PlannedPublication::PinPreview {
            output: published,
            generation: 17,
            ..
        }] if published == &output
    ));
    assert_ne!(ordinary, preview);
    assert_ne!(ordinary.digest().unwrap(), preview.digest().unwrap());
    assert_eq!(
        serde_json::to_value(preview).unwrap()["PinPreview"]["generation"],
        17
    );
}

#[test]
fn invalid_requested_outputs_are_rejected_before_plan_construction() {
    let source = test_protocol(
        "demand_validation_source",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let mut protocol = test_protocol(
        "demand_validation",
        vec![
            data_port("in", PortDirection::Input, TypeExpr::Unknown, None),
            data_port("out", PortDirection::Output, TypeExpr::Unknown, None),
            effect_port("effect", PortDirection::Output),
            control_port("control", PortDirection::Output),
        ],
        vec![],
        vec![],
    );
    protocol.execution.effects = EffectSemantics::Ordered;
    let registry = TestRegistry::new(vec![source, protocol]);
    let mut graph = graph_with_nodes(&[(1, "demand_validation_source"), (2, "demand_validation")]);
    connect(&mut graph, 10, 1, "out", 2, "in");
    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/main".into()), &graph);
    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let basis = result
        .execution_basis
        .expect("valid graph has lowering basis");
    let stale_instance = GraphOutputRef {
        graph_path: GraphResourcePath("events/main".into()),
        port: PortAddress::instance(
            node_id(2),
            key("out"),
            PortInstanceId::from_uuid(Uuid::from_u128(99)),
        ),
    };
    let invalid = [
        (
            demand_output("events/other", 2, "out"),
            "graph_path_mismatch",
        ),
        (demand_output("events/main", 99, "out"), "missing_node"),
        (demand_output("events/main", 2, "missing"), "missing_port"),
        (demand_output("events/main", 2, "in"), "input_port"),
        (demand_output("events/main", 2, "effect"), "effect_port"),
        (demand_output("events/main", 2, "control"), "control_port"),
        (stale_instance, "stale_instance"),
    ];

    for (output, expected) in invalid {
        let error = basis
            .derive_plan(&ExecutionDemand::Outputs {
                outputs: Box::new([output.clone()]),
                include_default_results: false,
            })
            .unwrap_err();
        let actual = match error {
            DemandPlanError::GraphPathMismatch(_) => "graph_path_mismatch",
            DemandPlanError::MissingNode(_) => "missing_node",
            DemandPlanError::MissingPort(_) => "missing_port",
            DemandPlanError::StalePortInstance(_) => "stale_instance",
            DemandPlanError::InputPort(_) => "input_port",
            DemandPlanError::ControlPort(_) => "control_port",
            DemandPlanError::EffectPort(_) => "effect_port",
            DemandPlanError::UnboundInput(_) => "unbound_input",
            DemandPlanError::InvalidDerivedPlan(_) => "invalid_derived_plan",
            DemandPlanError::CanonicalEncoding(_) => "canonical_encoding",
        };
        assert_eq!(actual, expected, "wrong error for {output:?}");
    }
}

#[test]
fn retained_operation_keeps_external_value_dependency_and_source() {
    let mut entry = test_protocol(
        "demand_external_entry",
        vec![
            control_port("then", PortDirection::Output),
            data_port("payload", PortDirection::Output, TypeExpr::Unknown, None),
        ],
        vec![],
        vec![],
    );
    entry.managed_role = Some(ManagedNodeRole::EventBegin);
    let entry_type = entry.type_id.clone();
    let sink = chain_protocol("demand_external_sink");
    let sink_type = sink.type_id.clone();
    let registry = TestRegistry::new(vec![entry, sink])
        .structural(&entry_type, StructuralNodeRole::EventBegin)
        .with_lowerer(
            &sink_type,
            FragmentLowerer {
                fragment: kernel_fragment(
                    EffectSemantics::None,
                    FragmentMetadata {
                        results: Box::new([FragmentResult {
                            name: "external-result".into(),
                            output: PortAddress::declared(node_id(2), key("out")),
                        }]),
                        ..FragmentMetadata::default()
                    },
                ),
            },
        );
    let mut graph = graph_with_nodes(&[(1, "demand_external_entry"), (2, "demand_external_sink")]);
    connect(&mut graph, 10, 1, "payload", 2, "in");
    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/external".into()), &graph);
    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let basis = result
        .execution_basis
        .expect("valid graph has lowering basis");

    let plan = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/external", 2, "out")]),
            include_default_results: false,
        })
        .expect("external source dependency remains valid");

    assert_eq!(plan.operations.len(), 1);
    assert_eq!(plan.value_dependencies.len(), 1);
    assert!(
        plan.operations
            .iter()
            .all(|operation| !matches!(operation.kernel, PlannedKernel::Adapter(_)))
    );
    assert!(plan.value_sources.iter().any(|source| {
        matches!(source, PlanValueSource::ExternalInput(value, _) if plan.value_dependencies.iter().any(|dependency| dependency.source == *value))
    }));
    assert!(!matches!(
        plan.validate(),
        Err(ref errors) if errors.0.iter().any(|error| matches!(error, crate::node_system::plan::PlanValidationError::MissingInputSource { .. }))
    ));
}

#[test]
fn valid_dynamic_output_derives_without_invalid_plan_fallback() {
    let dynamic_output = PortSpec {
        key: key("items"),
        label_key: I18nKey::new("ports.items.label").unwrap(),
        direction: PortDirection::Output,
        kind: PortKind::Data,
        value_type: TypeExpr::Unknown,
        instances: PortInstances::UserCreated { min: 0, max: None },
        connections: ConnectionsPerPort::Multiple {
            max: None,
            ordered: false,
        },
        input_binding: None,
        consumption: None,
        production: None,
        editor: PortEditorSpec::Default,
        schema: None,
    };
    let protocol = test_protocol(
        "demand_dynamic_output",
        vec![dynamic_output],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![protocol]);
    let mut graph = graph_with_nodes(&[(1, "demand_dynamic_output")]);
    let output = bind_member_port(&mut graph, 1, "items", 10, "a");
    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/dynamic".into()), &graph);
    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let basis = result
        .execution_basis
        .expect("valid dynamic graph has basis");
    let requested = GraphOutputRef {
        graph_path: GraphResourcePath("events/dynamic".into()),
        port: output,
    };

    let plan = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([requested.clone()]),
            include_default_results: false,
        })
        .unwrap_or_else(|error| panic!("accepted dynamic output must derive directly: {error:?}"));
    assert_eq!(plan.results[0].output, requested);
    plan.validate()
        .expect("dynamic requested-output plan validates");
}

#[test]
fn evaluation_policy_and_effect_predecessors_are_authoritative_roots() {
    let mut predecessor = test_protocol(
        "demand_effect_predecessor",
        vec![effect_port("effect", PortDirection::Output)],
        vec![],
        vec![],
    );
    predecessor.execution.purity = Purity::Effectful;
    predecessor.execution.effects = EffectSemantics::Ordered;
    let mut middle = test_protocol(
        "demand_effect_middle",
        vec![
            effect_port("before", PortDirection::Input),
            effect_port("after", PortDirection::Output),
        ],
        vec![],
        vec![],
    );
    middle.execution.purity = Purity::Effectful;
    middle.execution.effects = EffectSemantics::Ordered;
    let mut eager = test_protocol(
        "demand_eager_pure",
        vec![effect_port("effect", PortDirection::Input)],
        vec![],
        vec![],
    );
    eager.execution.purity = Purity::Pure;
    eager.execution.evaluation = EvaluationPolicy::EagerWhenRegionEntered;
    eager.execution.effects = EffectSemantics::Ordered;
    let mut demand_driven_effectful =
        test_protocol("demand_disconnected_effectful", vec![], vec![], vec![]);
    demand_driven_effectful.execution.purity = Purity::Effectful;
    demand_driven_effectful.execution.effects = EffectSemantics::Ordered;
    let types = [
        predecessor.type_id.clone(),
        middle.type_id.clone(),
        eager.type_id.clone(),
        demand_driven_effectful.type_id.clone(),
    ];
    let mut registry = TestRegistry::new(vec![predecessor, middle, eager, demand_driven_effectful]);
    for node_type in &types {
        registry = registry.with_lowerer(
            node_type,
            FragmentLowerer {
                fragment: kernel_fragment(EffectSemantics::Ordered, FragmentMetadata::default()),
            },
        );
    }
    let mut graph = graph_with_nodes(&[
        (1, "demand_effect_predecessor"),
        (2, "demand_effect_middle"),
        (3, "demand_eager_pure"),
        (4, "demand_disconnected_effectful"),
    ]);
    connect(&mut graph, 10, 1, "effect", 2, "before");
    connect(&mut graph, 11, 2, "after", 3, "effect");
    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/main".into()), &graph);
    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let basis = result
        .execution_basis
        .expect("valid graph has lowering basis");

    let plan = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([]),
            include_default_results: false,
        })
        .unwrap();

    assert_eq!(
        plan.operations
            .iter()
            .map(|operation| operation.source_node_id)
            .collect::<Vec<_>>(),
        vec![node_id(1), node_id(2), node_id(3)]
    );
    assert_eq!(plan.effect_dependencies.len(), 2);
}

#[test]
fn compiler_inserts_explicit_materialization_adapter_for_relational_boundary() {
    let mut source_output = data_port("out", PortDirection::Output, TypeExpr::Unknown, None);
    source_output.production = Some(OutputProduction::Streaming);
    let source = test_protocol("plan_bridge_source", vec![source_output], vec![], vec![]);
    let mut sink_input = data_port("in", PortDirection::Input, TypeExpr::Unknown, None);
    sink_input.consumption = Some(InputConsumption::FullyMaterialized);
    let sink_output = data_port("out", PortDirection::Output, TypeExpr::Unknown, None);
    let sink = test_protocol(
        "plan_bridge_sink",
        vec![sink_input, sink_output],
        vec![],
        vec![],
    );
    let condition = test_protocol(
        "pruned_bridge_condition",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let mut then_source = data_port("then_source", PortDirection::Input, TypeExpr::Unknown, None);
    then_source.instances = PortInstances::UserCreated { min: 0, max: None };
    let mut else_source = data_port("else_source", PortDirection::Input, TypeExpr::Unknown, None);
    else_source.instances = PortInstances::UserCreated { min: 0, max: None };
    let mut branch_result = data_port("result", PortDirection::Output, TypeExpr::Unknown, None);
    branch_result.instances = PortInstances::UserCreated { min: 0, max: None };
    let mut branch = structural_protocol(
        "pruned_bridge_branch",
        vec![
            control_port("enter", PortDirection::Input),
            data_port("condition", PortDirection::Input, TypeExpr::Unknown, None),
            then_source,
            else_source,
            control_port("true", PortDirection::Output),
            control_port("false", PortDirection::Output),
            branch_result,
        ],
        vec![],
    );
    branch.interface = branch
        .interface
        .with_member_groups(vec![PortMemberGroupSpec {
            templates: vec![key("then_source"), key("else_source"), key("result")]
                .into_boxed_slice(),
            min: 0,
            max: None,
        }])
        .unwrap();
    let source_type = source.type_id.clone();
    let sink_type = sink.type_id.clone();
    let branch_type = branch.type_id.clone();
    let backend = RelationalBackendId::new("test.relational").unwrap();
    let registry = TestRegistry::new(vec![source, sink, condition, branch])
        .structural(&branch_type, StructuralNodeRole::Branch)
        .with_lowerer(
            &source_type,
            FragmentLowerer {
                fragment: LoweredKernel::Relational(RelationalNodeFragment {
                    backend: backend.clone(),
                    fragment: relational::RelationalFragment {
                        id: RelationalFragmentId::new("bridge-source").unwrap(),
                        operators: Box::new([RelationalOperator::Source {
                            resource: ResourceId::new("database.source").unwrap(),
                            relation: "items".into(),
                        }]),
                        root: RelationalOperatorIndex::new(0),
                    },
                    inputs: Box::new([]),
                    metadata: FragmentMetadata::default(),
                }),
            },
        )
        .with_lowerer(
            &sink_type,
            FragmentLowerer {
                fragment: LoweredKernel::Relational(RelationalNodeFragment {
                    backend,
                    fragment: relational::RelationalFragment {
                        id: RelationalFragmentId::new("bridge-sink").unwrap(),
                        operators: Box::new([RelationalOperator::Input {
                            name: "input".into(),
                        }]),
                        root: RelationalOperatorIndex::new(0),
                    },
                    inputs: Box::new([RelationalInputBinding {
                        port: PortAddress::declared(node_id(2), key("in")),
                        operator: RelationalOperatorIndex::new(0),
                    }]),
                    metadata: FragmentMetadata::default(),
                }),
            },
        );
    let mut graph = graph_with_nodes(&[
        (1, "plan_bridge_source"),
        (2, "plan_bridge_sink"),
        (3, "pruned_bridge_condition"),
        (4, "pruned_bridge_branch"),
    ]);
    connect(&mut graph, 10, 1, "out", 2, "in");
    connect(&mut graph, 11, 3, "out", 4, "condition");

    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/bridge-demand".into()), &graph);
    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let specialized = result
        .execution_basis
        .as_ref()
        .unwrap_or_else(|| panic!("adapter diagnostics: {:?}", result.analysis.diagnostics))
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/bridge-demand", 2, "out")]),
            include_default_results: false,
        })
        .expect("retained relational adapter boundary specializes after structured pruning");
    assert_eq!(specialized.operations.len(), 3);
    assert_eq!(specialized.relational_subplans.len(), 2);

    let adapters = specialized
        .operations
        .iter()
        .filter_map(|operation| match &operation.kernel {
            PlannedKernel::Adapter(adapter) => Some((operation, adapter)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(adapters.len(), 1);
    assert!(matches!(adapters[0].1, PlannedAdapter::Collect { .. }));
    assert_eq!(adapters[0].0.workload, WorkloadClass::AdapterIo);
    assert_eq!(adapters[0].0.cache_policy, CachePolicy::Disabled);
    assert_eq!(adapters[0].0.inputs.len(), 1);
    assert_eq!(
        adapters[0].0.inputs[0].consumption,
        InputConsumption::Streaming
    );
    assert_eq!(adapters[0].0.outputs.len(), 1);
    assert_eq!(
        adapters[0].0.outputs[0].production,
        OutputProduction::FullyMaterialized
    );
    assert_eq!(specialized.value_dependencies.len(), 2);
    for operation in &specialized.operations {
        if let crate::node_system::plan::PlannedKernel::Relational(subplan) = operation.kernel {
            assert_eq!(
                operation.outputs.len(),
                specialized.relational_subplans[subplan.index()]
                    .compiled_plan
                    .roots
                    .len(),
                "relational owner outputs and compiled roots keep exact cardinality"
            );
        }
    }
    specialized.validate().unwrap();

    let mut reversed = graph_with_nodes(&[
        (4, "pruned_bridge_branch"),
        (3, "pruned_bridge_condition"),
        (2, "plan_bridge_sink"),
        (1, "plan_bridge_source"),
    ]);
    connect(&mut reversed, 11, 3, "out", 4, "condition");
    connect(&mut reversed, 10, 1, "out", 2, "in");
    let reversed_snapshot =
        compiler.snapshot(GraphResourcePath("events/bridge-demand".into()), &reversed);
    let reversed_result = compiler
        .compile_snapshot(&reversed_snapshot, &CompileCancellationToken::new())
        .unwrap();
    let reversed_specialized = reversed_result
        .execution_basis
        .expect("reversed bridge graph has a specialization basis")
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/bridge-demand", 2, "out")]),
            include_default_results: false,
        })
        .unwrap();
    assert_eq!(specialized.operations, reversed_specialized.operations);
    assert_eq!(
        specialized.relational_subplans,
        reversed_specialized.relational_subplans
    );
    assert_eq!(specialized.root_region, reversed_specialized.root_region);

    let plan = result.plan.expect("bridge graph should lower");

    assert_eq!(plan.relational_subplans.len(), 2);
    assert_eq!(
        plan.operations
            .iter()
            .filter(|operation| matches!(operation.kernel, PlannedKernel::Adapter(_)))
            .count(),
        1
    );

    let adapter_index = plan
        .operations
        .iter()
        .position(|operation| matches!(operation.kernel, PlannedKernel::Adapter(_)))
        .unwrap();
    let adapter = &plan.operations[adapter_index];
    let incoming = plan
        .value_dependencies
        .iter()
        .find(|dependency| dependency.destination == adapter.inputs[0].value)
        .copied()
        .unwrap();
    let outgoing = plan
        .value_dependencies
        .iter()
        .find(|dependency| dependency.source == adapter.outputs[0].value)
        .copied()
        .unwrap();

    let mut missing = plan.clone();
    missing.operations = missing
        .operations
        .into_vec()
        .into_iter()
        .enumerate()
        .filter_map(|(index, operation)| (index != adapter_index).then_some(operation))
        .collect();
    missing.value_dependencies = Box::new([crate::node_system::plan::ValueDependency {
        source: incoming.source,
        destination: outgoing.destination,
    }]);
    let relational_operations = missing
        .operations
        .iter()
        .enumerate()
        .filter_map(|(index, operation)| {
            matches!(operation.kernel, PlannedKernel::Relational(_)).then_some(
                ControlStep::Operation(crate::node_system::plan::OperationIndex::new(index as u32)),
            )
        })
        .collect::<Vec<_>>();
    missing.root_region =
        StructuredControlRegion::Sequence(relational_operations.into_boxed_slice());
    assert!(
        missing
            .validate()
            .unwrap_err()
            .0
            .iter()
            .any(|error| matches!(
                error,
                PlanValidationError::MissingMaterializationAdapter { .. }
            ))
    );

    let mut extra = plan.clone();
    let mut extra_adapter = extra.operations[adapter_index].clone();
    extra_adapter.stable_id =
        crate::node_system::plan::OperationStableId::new("test.extra.materialization.adapter")
            .unwrap();
    extra_adapter.inputs[0].value = ValueRef::new(extra.value_count);
    extra.value_count += 1;
    extra_adapter.outputs[0].value = ValueRef::new(extra.value_count);
    extra.value_count += 1;
    extra.operations = extra
        .operations
        .into_vec()
        .into_iter()
        .chain([extra_adapter])
        .collect();
    assert!(extra.validate().unwrap_err().0.iter().any(|error| matches!(
        error,
        PlanValidationError::ExtraMaterializationAdapter { .. }
    )));

    let mut incompatible = plan.clone();
    incompatible.operations[adapter_index].kernel =
        PlannedKernel::Adapter(PlannedAdapter::Identity);
    assert!(
        incompatible
            .validate()
            .unwrap_err()
            .0
            .iter()
            .any(|error| matches!(
                error,
                PlanValidationError::IncompatibleMaterializationAdapter { .. }
            ))
    );
}

#[derive(Clone, Copy)]
enum FixtureInsertionOrder {
    Forward,
    Reverse,
}

fn in_fixture_order<T>(mut values: Vec<T>, order: FixtureInsertionOrder) -> Vec<T> {
    match order {
        FixtureInsertionOrder::Forward => {}
        FixtureInsertionOrder::Reverse => values.reverse(),
    }
    values
}

fn insert_tracked<K: Clone + Ord, V>(
    map: &mut BTreeMap<K, V>,
    trace: &mut Vec<K>,
    key: K,
    value: V,
) {
    trace.push(key.clone());
    assert!(map.insert(key, value).is_none());
}

struct DeterminismFixture {
    document: GraphDocument,
    node_insertions: Vec<NodeId>,
    connection_insertions: Vec<ConnectionId>,
    parameter_insertions: Vec<ParameterKey>,
    port_binding_insertions: Vec<PortAddress>,
    input_state_insertions: Vec<PortAddress>,
}

fn determinism_protocols() -> Vec<NodeProtocol> {
    let mut source_output = data_port("out", PortDirection::Output, TypeExpr::Unknown, None);
    source_output.production = Some(OutputProduction::Streaming);
    let source = test_protocol(
        "determinism_relation_source",
        vec![source_output],
        vec![],
        vec![],
    );

    let mut middle_input = data_port("in", PortDirection::Input, TypeExpr::Unknown, None);
    middle_input.consumption = Some(InputConsumption::Streaming);
    let mut middle_output = data_port("out", PortDirection::Output, TypeExpr::Unknown, None);
    middle_output.production = Some(OutputProduction::Streaming);
    let middle = test_protocol(
        "determinism_relation_middle",
        vec![middle_input, middle_output],
        vec![],
        vec![],
    );

    let mut sink_input = data_port("in", PortDirection::Input, TypeExpr::Unknown, None);
    sink_input.consumption = Some(InputConsumption::FullyMaterialized);
    let sink_output = data_port("out", PortDirection::Output, TypeExpr::Unknown, None);
    let sink = test_protocol(
        "determinism_relation_sink",
        vec![sink_input, sink_output],
        vec![],
        vec![],
    );

    let mut dynamic_input = data_port("values", PortDirection::Input, TypeExpr::Unknown, None);
    dynamic_input.instances = PortInstances::UserCreated {
        min: 0,
        max: Some(2),
    };
    let mut inputs = test_protocol("determinism_inputs", vec![dynamic_input], vec![], vec![]);
    inputs.parameters = ParameterSchema::new(vec![
        ParameterSpec {
            key: ParameterKey::new("alpha").unwrap(),
            title_key: I18nKey::new("parameters.alpha.title").unwrap(),
            description_key: None,
            value_type: TypeExpr::Concrete(type_id("core.int64")),
            default_value: None,
            constraints: vec![ParameterConstraint::Required],
            editor: ParameterEditorSpec::Auto,
            presentation: ParameterPresentation::DetailPanel,
        },
        ParameterSpec {
            key: ParameterKey::new("beta").unwrap(),
            title_key: I18nKey::new("parameters.beta.title").unwrap(),
            description_key: None,
            value_type: TypeExpr::Concrete(type_id("core.int64")),
            default_value: None,
            constraints: vec![ParameterConstraint::Required],
            editor: ParameterEditorSpec::Auto,
            presentation: ParameterPresentation::DetailPanel,
        },
    ])
    .unwrap();

    vec![source, middle, sink, inputs]
}

fn determinism_registry(protocols: Vec<NodeProtocol>) -> TestRegistry {
    let source_type = protocols[0].type_id.clone();
    let middle_type = protocols[1].type_id.clone();
    let sink_type = protocols[2].type_id.clone();
    let backend = RelationalBackendId::new("test.relational").unwrap();

    TestRegistry::new(protocols)
        .with_lowerer(
            &source_type,
            FragmentLowerer {
                fragment: LoweredKernel::Relational(RelationalNodeFragment {
                    backend: backend.clone(),
                    fragment: relational::RelationalFragment {
                        id: RelationalFragmentId::new("determinism-source").unwrap(),
                        operators: Box::new([RelationalOperator::Source {
                            resource: ResourceId::new("database.source").unwrap(),
                            relation: "items".into(),
                        }]),
                        root: RelationalOperatorIndex::new(0),
                    },
                    inputs: Box::new([]),
                    metadata: FragmentMetadata::default(),
                }),
            },
        )
        .with_lowerer(
            &middle_type,
            FragmentLowerer {
                fragment: LoweredKernel::Relational(RelationalNodeFragment {
                    backend: backend.clone(),
                    fragment: relational::RelationalFragment {
                        id: RelationalFragmentId::new("determinism-middle").unwrap(),
                        operators: Box::new([
                            RelationalOperator::Input {
                                name: "input".into(),
                            },
                            RelationalOperator::Limit {
                                input: RelationalOperatorIndex::new(0),
                                rows: 10,
                            },
                        ]),
                        root: RelationalOperatorIndex::new(1),
                    },
                    inputs: Box::new([RelationalInputBinding {
                        port: PortAddress::declared(node_id(2), key("in")),
                        operator: RelationalOperatorIndex::new(0),
                    }]),
                    metadata: FragmentMetadata::default(),
                }),
            },
        )
        .with_lowerer(
            &sink_type,
            FragmentLowerer {
                fragment: LoweredKernel::Relational(RelationalNodeFragment {
                    backend,
                    fragment: relational::RelationalFragment {
                        id: RelationalFragmentId::new("determinism-sink").unwrap(),
                        operators: Box::new([RelationalOperator::Input {
                            name: "input".into(),
                        }]),
                        root: RelationalOperatorIndex::new(0),
                    },
                    inputs: Box::new([RelationalInputBinding {
                        port: PortAddress::declared(node_id(3), key("in")),
                        operator: RelationalOperatorIndex::new(0),
                    }]),
                    metadata: FragmentMetadata::default(),
                }),
            },
        )
}

fn determinism_fixture(order: FixtureInsertionOrder) -> DeterminismFixture {
    let mut parameters = BTreeMap::new();
    let mut parameter_insertions = Vec::new();
    for (key, value) in in_fixture_order(
        vec![
            (ParameterKey::new("alpha").unwrap(), serde_json::json!(1)),
            (ParameterKey::new("beta").unwrap(), serde_json::json!(2)),
        ],
        order,
    ) {
        insert_tracked(&mut parameters, &mut parameter_insertions, key, value);
    }

    let node_entries = vec![
        (1, "determinism_relation_source", BTreeMap::new()),
        (2, "determinism_relation_middle", BTreeMap::new()),
        (3, "determinism_relation_sink", BTreeMap::new()),
        (4, "determinism_inputs", parameters),
    ];
    let mut nodes = BTreeMap::new();
    let mut node_insertions = Vec::new();
    for (id, node_type, parameters) in in_fixture_order(node_entries, order) {
        let id = node_id(id);
        insert_tracked(
            &mut nodes,
            &mut node_insertions,
            id,
            DocumentNode {
                id,
                node_type: NodeTypeId::new(format!("yssbi.test.{node_type}")).unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters,
                user_label: None,
            },
        );
    }

    let dynamic_ports = vec![
        PortAddress::instance(
            node_id(4),
            key("values"),
            PortInstanceId::from_uuid(Uuid::from_u128(40)),
        ),
        PortAddress::instance(
            node_id(4),
            key("values"),
            PortInstanceId::from_uuid(Uuid::from_u128(41)),
        ),
    ];
    let connection_entries = vec![
        DocumentConnection {
            id: ConnectionId::from_uuid(Uuid::from_u128(10)),
            output: PortAddress::declared(node_id(1), key("out")),
            input: PortAddress::declared(node_id(2), key("in")),
            order: None,
        },
        DocumentConnection {
            id: ConnectionId::from_uuid(Uuid::from_u128(11)),
            output: PortAddress::declared(node_id(2), key("out")),
            input: PortAddress::declared(node_id(3), key("in")),
            order: None,
        },
        DocumentConnection {
            id: ConnectionId::from_uuid(Uuid::from_u128(12)),
            output: PortAddress::declared(node_id(1), key("out")),
            input: dynamic_ports[0].clone(),
            order: None,
        },
        DocumentConnection {
            id: ConnectionId::from_uuid(Uuid::from_u128(13)),
            output: PortAddress::declared(node_id(1), key("out")),
            input: dynamic_ports[1].clone(),
            order: None,
        },
    ];
    let mut connections = BTreeMap::new();
    let mut connection_insertions = Vec::new();
    for connection in in_fixture_order(connection_entries, order) {
        insert_tracked(
            &mut connections,
            &mut connection_insertions,
            connection.id,
            connection,
        );
    }

    let binding_entries = vec![
        (
            dynamic_ports[0].clone(),
            DynamicPortBinding::UserCreated {
                order: OrderKey("a".into()),
            },
        ),
        (
            dynamic_ports[1].clone(),
            DynamicPortBinding::UserCreated {
                order: OrderKey("b".into()),
            },
        ),
    ];
    let mut port_bindings = BTreeMap::new();
    let mut port_binding_insertions = Vec::new();
    for (address, binding) in in_fixture_order(binding_entries, order) {
        insert_tracked(
            &mut port_bindings,
            &mut port_binding_insertions,
            address,
            binding,
        );
    }

    let state_entries = vec![
        (
            dynamic_ports[0].clone(),
            InputState {
                literal_override: None,
            },
        ),
        (
            dynamic_ports[1].clone(),
            InputState {
                literal_override: None,
            },
        ),
    ];
    let mut input_states = BTreeMap::new();
    let mut input_state_insertions = Vec::new();
    for (address, state) in in_fixture_order(state_entries, order) {
        insert_tracked(
            &mut input_states,
            &mut input_state_insertions,
            address,
            state,
        );
    }

    DeterminismFixture {
        document: GraphDocument {
            revision: crate::node_system::document::GraphRevision::new(19),
            nodes,
            port_bindings,
            connections,
            input_states,
        },
        node_insertions,
        connection_insertions,
        parameter_insertions,
        port_binding_insertions,
        input_state_insertions,
    }
}

fn assert_reversed_insertions<T: std::fmt::Debug + PartialEq>(forward: &[T], reverse: &[T]) {
    assert!(
        forward.len() >= 2,
        "fixture container must have multiple entries"
    );
    assert_eq!(
        forward.iter().rev().collect::<Vec<_>>(),
        reverse.iter().collect::<Vec<_>>()
    );
}

#[test]
fn semantically_identical_documents_serialize_identically() {
    let forward = determinism_fixture(FixtureInsertionOrder::Forward);
    let reverse = determinism_fixture(FixtureInsertionOrder::Reverse);

    assert_reversed_insertions(&forward.node_insertions, &reverse.node_insertions);
    assert_reversed_insertions(
        &forward.connection_insertions,
        &reverse.connection_insertions,
    );
    assert_reversed_insertions(&forward.parameter_insertions, &reverse.parameter_insertions);
    assert_reversed_insertions(
        &forward.port_binding_insertions,
        &reverse.port_binding_insertions,
    );
    assert_reversed_insertions(
        &forward.input_state_insertions,
        &reverse.input_state_insertions,
    );
    assert_eq!(forward.document, reverse.document);

    let registry = determinism_registry(determinism_protocols());
    let compiler = GraphCompiler::new(&registry, &Resources).with_observability(
        ProjectSessionId::new("determinism-session"),
        &NOOP_TRACE_SINK,
    );
    let graph_path = GraphResourcePath("events/determinism".into());
    let compile_id = CompileId::new(73);
    let forward_snapshot =
        compiler.snapshot_with_compile_id(compile_id, graph_path.clone(), &forward.document);
    let reverse_snapshot =
        compiler.snapshot_with_compile_id(compile_id, graph_path, &reverse.document);

    assert_eq!(forward_snapshot.provenance, reverse_snapshot.provenance);
    let forward_result = compiler
        .compile_snapshot(&forward_snapshot, &CompileCancellationToken::new())
        .expect("forward fixture should compile");
    let reverse_result = compiler
        .compile_snapshot(&reverse_snapshot, &CompileCancellationToken::new())
        .expect("reverse fixture should compile");

    assert_eq!(
        forward_result.analysis.diagnostics.as_ref(),
        reverse_result.analysis.diagnostics.as_ref()
    );
    assert!(
        forward_result.analysis.diagnostics.is_empty(),
        "fixture diagnostics: {:#?}",
        forward_result.analysis.diagnostics
    );
    assert_eq!(forward_result.analysis.nodes.len(), 4);
    assert_eq!(
        forward_result.analysis.nodes.as_ref(),
        reverse_result.analysis.nodes.as_ref()
    );
    assert_eq!(forward_result.analysis.resolved_interfaces.len(), 4);
    assert_eq!(
        forward_result.analysis.resolved_interfaces.as_ref(),
        reverse_result.analysis.resolved_interfaces.as_ref()
    );
    assert_eq!(forward_result.analysis.partial_types.len(), 7);
    assert_eq!(
        forward_result.analysis.partial_types,
        reverse_result.analysis.partial_types
    );
    assert!(
        forward_result.analysis.partial_schemas.is_empty(),
        "fixture protocols intentionally declare no schema expressions"
    );
    assert_eq!(
        forward_result.analysis.partial_schemas,
        reverse_result.analysis.partial_schemas
    );
    assert_eq!(
        serde_json::to_vec(&forward_result.analysis).unwrap(),
        serde_json::to_vec(&reverse_result.analysis).unwrap()
    );

    let forward_semantic = forward_result
        .semantic
        .expect("forward fixture should produce a semantic graph");
    let reverse_semantic = reverse_result
        .semantic
        .expect("reverse fixture should produce a semantic graph");
    assert_eq!(
        forward_semantic.dependencies.as_ref(),
        reverse_semantic.dependencies.as_ref()
    );
    assert_eq!(forward_semantic.dependencies.len(), 4);
    assert_eq!(
        serde_json::to_vec(&forward_semantic).unwrap(),
        serde_json::to_vec(&reverse_semantic).unwrap()
    );

    let forward_plan = forward_result
        .plan
        .expect("forward fixture should produce an execution plan");
    let reverse_plan = reverse_result
        .plan
        .expect("reverse fixture should produce an execution plan");
    assert_eq!(forward_plan.operations.len(), 5);
    assert_eq!(
        forward_plan
            .operations
            .iter()
            .filter(|operation| matches!(operation.kernel, PlannedKernel::Adapter(_)))
            .count(),
        2
    );
    assert!(forward_plan.operations.iter().all(|operation| !matches!(
        operation.kernel,
        PlannedKernel::Adapter(PlannedAdapter::Identity)
    )));
    assert_eq!(
        forward_plan.operations.as_ref(),
        reverse_plan.operations.as_ref()
    );
    assert_eq!(
        forward_plan.relational_subplans.as_ref(),
        reverse_plan.relational_subplans.as_ref()
    );
    assert_eq!(forward_plan.relational_subplans.len(), 2);
    assert!(
        forward_plan
            .relational_subplans
            .iter()
            .any(|subplan| subplan.compiled_plan.fragment_order.len() > 1)
    );
    assert!(
        forward_plan
            .relational_subplans
            .iter()
            .all(|subplan| !subplan.compiled_plan.fragment_roots.is_empty())
    );

    for (forward_subplan, reverse_subplan) in forward_plan
        .relational_subplans
        .iter()
        .zip(reverse_plan.relational_subplans.iter())
    {
        assert_eq!(
            forward_subplan.compiled_plan.fragment_order,
            reverse_subplan.compiled_plan.fragment_order
        );
        assert_eq!(
            forward_subplan.compiled_plan.fragment_roots,
            reverse_subplan.compiled_plan.fragment_roots
        );
        assert_eq!(
            forward_subplan.compiled_plan.roots,
            reverse_subplan.compiled_plan.roots
        );
    }
    assert_eq!(
        serde_json::to_vec(&forward_plan).unwrap(),
        serde_json::to_vec(&reverse_plan).unwrap()
    );
}

#[test]
fn generic_binding_is_written_to_type_facts() {
    let generic = TypeParameterId::new("value").unwrap();
    let source = test_protocol(
        "int_source",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Concrete(type_id("core.int")),
            None,
        )],
        vec![],
        vec![],
    );
    let passthrough = test_protocol(
        "generic",
        vec![
            data_port(
                "in",
                PortDirection::Input,
                TypeExpr::Generic(generic.clone()),
                None,
            ),
            data_port(
                "out",
                PortDirection::Output,
                TypeExpr::Generic(generic),
                None,
            ),
        ],
        vec![TypeParameterId::new("value").unwrap()],
        vec![TypeConstraint::Equal(
            TypeTerm::Port(key("in")),
            TypeTerm::Port(key("out")),
        )],
    );
    let registry = TestRegistry::new(vec![source, passthrough]);
    let mut graph = graph_with_nodes(&[(1, "int_source"), (2, "generic")]);
    connect(&mut graph, 10, 1, "out", 2, "in");

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);

    assert!(result.plan.is_some());
    assert_eq!(
        result
            .analysis
            .partial_types
            .get(&PortAddress::declared(node_id(2), key("out"))),
        Some(&TypeExpr::Concrete(type_id("core.int")))
    );
}

#[test]
fn implements_and_element_of_solve_registered_type_shapes() {
    let numeric = TypeClassId::new("core.numeric").unwrap();
    let iterable = TypeClassId::new("core.iterable").unwrap();
    let integer = type_id("core.int");
    let float = type_id("core.float");
    let list = TypeConstructorId::new("core.list").unwrap();
    let pair = TypeConstructorId::new("core.pair").unwrap();
    let generic = TypeParameterId::new("item").unwrap();
    let protocol = test_protocol(
        "type_constraints",
        vec![
            data_port(
                "concrete",
                PortDirection::Output,
                TypeExpr::Concrete(integer.clone()),
                None,
            ),
            data_port(
                "generic",
                PortDirection::Output,
                TypeExpr::Generic(generic.clone()),
                None,
            ),
            data_port(
                "applied",
                PortDirection::Output,
                TypeExpr::Applied {
                    constructor: list.clone(),
                    arguments: vec![TypeExpr::Concrete(integer.clone())],
                },
                None,
            ),
            data_port(
                "union",
                PortDirection::Output,
                TypeExpr::Union(vec![
                    TypeExpr::Concrete(integer.clone()),
                    TypeExpr::Concrete(float.clone()),
                ]),
                None,
            ),
            data_port(
                "pair",
                PortDirection::Output,
                TypeExpr::Applied {
                    constructor: pair.clone(),
                    arguments: vec![
                        TypeExpr::Concrete(integer.clone()),
                        TypeExpr::Concrete(float.clone()),
                    ],
                },
                None,
            ),
        ],
        vec![generic.clone()],
        vec![
            TypeConstraint::Equal(
                TypeTerm::Expr(TypeExpr::Generic(generic)),
                TypeTerm::Expr(TypeExpr::Concrete(integer.clone())),
            ),
            TypeConstraint::Implements(TypeTerm::Port(key("concrete")), numeric.clone()),
            TypeConstraint::Implements(TypeTerm::Port(key("generic")), numeric.clone()),
            TypeConstraint::Implements(TypeTerm::Port(key("applied")), iterable.clone()),
            TypeConstraint::Implements(TypeTerm::Port(key("union")), numeric.clone()),
            TypeConstraint::ElementOf(
                TypeTerm::Port(key("concrete")),
                TypeTerm::Port(key("applied")),
            ),
            TypeConstraint::ElementOf(TypeTerm::Port(key("union")), TypeTerm::Port(key("pair"))),
        ],
    );
    let registry = TestRegistry::new(vec![protocol])
        .with_type_class(integer, numeric.clone())
        .with_type_class(float, numeric)
        .with_constructor(list, 1, [iterable])
        .with_constructor(pair, 2, []);

    let result = GraphCompiler::new(&registry, &Resources)
        .compile(&graph_with_nodes(&[(1, "type_constraints")]));

    assert!(result.plan.is_some(), "{:?}", result.analysis.diagnostics);
    assert!(result.analysis.diagnostics.is_empty());
}

#[test]
fn incompatible_types_are_blocking_and_produce_no_plan() {
    let source = test_protocol(
        "int_source",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Concrete(type_id("core.int")),
            None,
        )],
        vec![],
        vec![],
    );
    let sink = test_protocol(
        "string_sink",
        vec![data_port(
            "in",
            PortDirection::Input,
            TypeExpr::Concrete(type_id("core.string")),
            None,
        )],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![source, sink]);
    let mut graph = graph_with_nodes(&[(1, "int_source"), (2, "string_sink")]);
    connect(&mut graph, 10, 1, "out", 2, "in");

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);

    assert!(result.plan.is_none());
    assert!(
        result
            .analysis
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_str() == "compiler.type.incompatible")
    );
}

#[test]
fn value_dependency_cycle_is_blocking() {
    let relay = test_protocol(
        "relay",
        vec![
            data_port("in", PortDirection::Input, TypeExpr::Unknown, None),
            data_port("out", PortDirection::Output, TypeExpr::Unknown, None),
        ],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![relay]);
    let mut graph = graph_with_nodes(&[(1, "relay"), (2, "relay")]);
    connect(&mut graph, 10, 1, "out", 2, "in");
    connect(&mut graph, 11, 2, "out", 1, "in");

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);

    assert!(result.plan.is_none());
    assert!(
        result
            .analysis
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code.as_str() == "compiler.dependency.value_cycle" })
    );
}

struct DataframeDatabaseResources;

impl ResourceSnapshot for DataframeDatabaseResources {
    fn versions(&self) -> crate::node_system::analysis::ResourceVersionSet {
        BTreeMap::from([(
            ResourceKey::new("databases/main"),
            ResourceVersion::new("fixture-v1"),
        )])
    }

    fn database_name(&self, id: &str) -> Option<&str> {
        (id == "main").then_some("Main database")
    }

    fn database_schema(&self, id: &str) -> Option<&[crate::schema::ColumnInfoDTO]> {
        (id == "main").then_some(&[])
    }
}

struct SourceSchemaResolver;
impl SchemaResolver for SourceSchemaResolver {
    fn resolve(
        &self,
        _: &mut SchemaResolutionContext<'_, '_>,
    ) -> Result<SchemaFact, SchemaResolutionError> {
        Ok(SchemaFact::new(
            SchemaExpr::Input(key("raw")),
            [
                SchemaField {
                    name: SchemaColumnRef("a".into()),
                    scalar_type: RelationalScalarType::Int64,
                    lineage: None,
                },
                SchemaField {
                    name: SchemaColumnRef("b".into()),
                    scalar_type: RelationalScalarType::String,
                    lineage: None,
                },
            ],
        ))
    }
}

fn compile_builtin_rename(
    from: serde_json::Value,
    to: serde_json::Value,
    include_source_schema: bool,
) -> CompileResult {
    use crate::node_system::catalog::{
        DATAFRAME_RESOURCE_SCHEMA_RESOLVER, build_builtin_node_system,
    };

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.dataframe.source.get"),
        (2, "yssbi.dataframe.rename"),
    ]);
    graph.nodes.get_mut(&node_id(1)).unwrap().parameters = BTreeMap::from([(
        ParameterKey::new("dataframe").unwrap(),
        serde_json::json!("databases/main"),
    )]);
    graph.nodes.get_mut(&node_id(2)).unwrap().parameters = BTreeMap::from([
        (ParameterKey::new("from").unwrap(), from),
        (ParameterKey::new("to").unwrap(), to),
    ]);
    connect(&mut graph, 10, 1, "dataframe", 2, "source");

    let mut resolvers = SchemaResolverSet::new();
    if include_source_schema {
        resolvers.insert(
            SchemaResolverId::new(DATAFRAME_RESOURCE_SCHEMA_RESOLVER).unwrap(),
            SourceSchemaResolver,
        );
    }
    GraphCompiler::with_schema_resolvers(&registry, &DataframeDatabaseResources, resolvers)
        .compile(&graph)
}

fn rename_diagnostic_codes(result: &CompileResult) -> BTreeSet<&str> {
    result
        .analysis
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect()
}

#[test]
fn rename_dataframe_configured_builtin_reaches_relational_plan() {
    let result = compile_builtin_rename(serde_json::json!("a"), serde_json::json!("renamed"), true);

    assert!(
        result.semantic.is_some(),
        "{:?}",
        result.analysis.diagnostics
    );
    let plan = result.plan.expect("configured Rename must lower");
    assert!(result.analysis.diagnostics.is_empty());
    assert_eq!(
        result
            .analysis
            .partial_schemas
            .get(&PortAddress::declared(node_id(2), key("result"))),
        Some(&SchemaExpr::Rename {
            input: Box::new(SchemaExpr::Input(key("raw"))),
            mapping: RenameExpr::Explicit(vec![ColumnRename {
                from: SchemaColumnRef("a".into()),
                to: SchemaColumnRef("renamed".into()),
            }]),
        })
    );
    assert_eq!(plan.relational_subplans.len(), 1);
    assert_eq!(
        plan.relational_subplans[0].compiled_plan.operators.as_ref(),
        [
            RelationalOperator::Source {
                resource: ResourceId::new("databases/main").unwrap(),
                relation: "databases/main".into(),
            },
            RelationalOperator::Rename {
                input: RelationalOperatorIndex::new(0),
                columns: Box::new([crate::node_system::plan::RelationalRename {
                    from: "a".into(),
                    to: "renamed".into(),
                }]),
            },
        ]
    );
}

fn compile_builtin_relational_chain(graph_path: &str) -> CompileResult {
    use crate::node_system::catalog::{
        DATAFRAME_RESOURCE_SCHEMA_RESOLVER, build_builtin_node_system,
    };

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.dataframe.source.get"),
        (2, "yssbi.dataframe.filter.rows"),
        (3, "yssbi.dataframe.project"),
        (4, "yssbi.dataframe.rename"),
        (5, "yssbi.dataframe.limit"),
    ]);
    graph.nodes.get_mut(&node_id(1)).unwrap().parameters = BTreeMap::from([(
        ParameterKey::new("dataframe").unwrap(),
        serde_json::json!("databases/main"),
    )]);
    graph.nodes.get_mut(&node_id(2)).unwrap().parameters = BTreeMap::from([(
        ParameterKey::new("predicate").unwrap(),
        serde_json::json!({
            "column": "b",
            "operator": "equal",
            "value": { "type": "string", "value": "paid" }
        }),
    )]);
    graph.nodes.get_mut(&node_id(3)).unwrap().parameters = BTreeMap::from([(
        ParameterKey::new("columns").unwrap(),
        serde_json::json!(["a"]),
    )]);
    graph.nodes.get_mut(&node_id(4)).unwrap().parameters = BTreeMap::from([
        (ParameterKey::new("from").unwrap(), serde_json::json!("a")),
        (ParameterKey::new("to").unwrap(), serde_json::json!("total")),
    ]);
    graph.nodes.get_mut(&node_id(5)).unwrap().parameters =
        BTreeMap::from([(ParameterKey::new("rows").unwrap(), serde_json::json!(5))]);
    connect(&mut graph, 10, 1, "dataframe", 2, "source");
    connect(&mut graph, 11, 2, "result", 3, "source");
    connect(&mut graph, 12, 3, "result", 4, "source");
    connect(&mut graph, 13, 4, "result", 5, "source");

    let mut resolvers = SchemaResolverSet::new();
    resolvers.insert(
        SchemaResolverId::new(DATAFRAME_RESOURCE_SCHEMA_RESOLVER).unwrap(),
        SourceSchemaResolver,
    );
    let compiler =
        GraphCompiler::with_schema_resolvers(&registry, &DataframeDatabaseResources, resolvers);
    let snapshot = compiler.snapshot(GraphResourcePath(graph_path.into()), &graph);
    compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap()
}

#[test]
fn builtin_relational_chain_specializes_final_and_intermediate_demands() {
    let graph_path = "events/task-3-fix-round-1";
    let result = compile_builtin_relational_chain(graph_path);
    assert!(
        result.analysis.diagnostics.is_empty(),
        "{:?}",
        result.analysis.diagnostics
    );
    let basis = result.execution_basis.expect("chain keeps demand basis");
    assert_eq!(
        basis.provenance.graph_path,
        GraphResourcePath(graph_path.into())
    );
    assert_eq!(
        basis.provenance.basis.resource_versions,
        BTreeMap::from([(
            ResourceKey::new("databases/main"),
            ResourceVersion::new("fixture-v1"),
        )])
    );

    let final_plan = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output(graph_path, 5, "result")]),
            include_default_results: false,
        })
        .unwrap();
    assert_eq!(final_plan.operations.len(), 1);
    assert_eq!(final_plan.relational_subplans.len(), 1);

    let final_relational = &final_plan.relational_subplans[0].compiled_plan;
    assert_eq!(final_relational.operators.len(), 5);
    assert_eq!(
        final_relational.roots.as_ref(),
        [RelationalOperatorIndex::new(4)]
    );
    assert_eq!(
        final_relational.fragment_order.as_ref(),
        [1_u128, 2, 3, 4, 5]
            .map(|id| RelationalFragmentId::new(format!("node.{}", node_id(id))).unwrap())
            .as_slice()
    );
    assert_eq!(
        final_relational.pushdown_hints.as_ref(),
        [
            RelationalPushdownHint::Projection {
                source: RelationalOperatorIndex::new(0),
                columns: Box::new(["a".into(), "b".into()]),
            },
            RelationalPushdownHint::Predicate {
                source: RelationalOperatorIndex::new(0),
                predicate: RelationalExpression::Equal(
                    Box::new(RelationalExpression::Column("b".into())),
                    Box::new(RelationalExpression::Literal(RelationalLiteral::String(
                        "paid".into(),
                    ))),
                ),
            },
        ]
    );
    final_plan.validate().unwrap();

    for (node, expected_fragments, expected_operators) in [(2, 2, 2), (3, 3, 3)] {
        let selected_output = demand_output(graph_path, node, "result");
        let plan = basis
            .derive_plan(&ExecutionDemand::Outputs {
                outputs: Box::new([selected_output.clone()]),
                include_default_results: false,
            })
            .unwrap();
        assert_eq!(plan.operations.len(), 1);
        assert_eq!(plan.results.len(), 1);
        assert_eq!(plan.results[0].output, selected_output);
        assert_eq!(plan.results[0].output.graph_path.0.as_ref(), graph_path);
        assert_eq!(
            plan.results[0].output.port,
            PortAddress::declared(node_id(node), key("result"))
        );
        let relational = &plan.relational_subplans[0].compiled_plan;
        let expected_prefix = (1..=node)
            .map(|id| RelationalFragmentId::new(format!("node.{}", node_id(id))).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(relational.fragment_order.as_ref(), expected_prefix);
        assert_eq!(relational.operators.len(), expected_operators);
        assert_eq!(relational.roots.len(), 1);
        for suffix_node in (node + 1)..=5 {
            let suffix =
                RelationalFragmentId::new(format!("node.{}", node_id(suffix_node))).unwrap();
            assert!(!relational.fragment_order.contains(&suffix));
            assert!(
                relational
                    .fragment_roots
                    .iter()
                    .all(|root| root.fragment != suffix)
            );
        }
        assert_eq!(relational.fragment_order.len(), expected_fragments);
        plan.validate().unwrap();
    }
}

#[test]
fn rename_dataframe_rejects_invalid_scalar_parameter_types() {
    for (from, to, expected_key) in [
        (serde_json::json!(1), serde_json::json!("renamed"), "from"),
        (serde_json::json!("a"), serde_json::json!(false), "to"),
    ] {
        let result = compile_builtin_rename(from, to, true);
        assert!(result.plan.is_none());
        assert!(result.analysis.diagnostics.iter().any(|diagnostic| {
            matches!(
                &diagnostic.primary,
                crate::node_system::analysis::DiagnosticLocation::Parameter { node_id: id, key }
                    if *id == node_id(2) && key.as_str() == expected_key
            )
        }));
    }
}

#[test]
fn rename_dataframe_rejects_empty_or_whitespace_padded_names() {
    for (from, to) in [
        ("", "renamed"),
        ("a", ""),
        (" a", "renamed"),
        ("a", "renamed "),
    ] {
        let result = compile_builtin_rename(serde_json::json!(from), serde_json::json!(to), true);
        assert!(result.plan.is_none(), "{from:?} -> {to:?}");
        assert!(
            rename_diagnostic_codes(&result).contains("compiler.schema.parameter_invalid"),
            "{:?}",
            result.analysis.diagnostics
        );
    }
}

#[test]
fn rename_dataframe_rejects_missing_source_and_destination_collision() {
    for (from, to, code) in [
        ("missing", "renamed", "compiler.schema.rename_field_missing"),
        ("a", "b", "compiler.schema.rename_target_conflict"),
    ] {
        let result = compile_builtin_rename(serde_json::json!(from), serde_json::json!(to), true);
        assert!(result.plan.is_none());
        assert!(
            rename_diagnostic_codes(&result).contains(code),
            "{:?}",
            result.analysis.diagnostics
        );
    }
}

#[test]
fn rename_dataframe_same_name_is_schema_noop_but_still_lowers() {
    let result = compile_builtin_rename(serde_json::json!("a"), serde_json::json!("a"), true);

    assert!(result.plan.is_some(), "{:?}", result.analysis.diagnostics);
    assert!(result.analysis.diagnostics.is_empty());
    assert_eq!(
        result
            .analysis
            .partial_schemas
            .get(&PortAddress::declared(node_id(2), key("result"))),
        Some(&SchemaExpr::Input(key("raw")))
    );
}

#[test]
fn rename_dataframe_unknown_input_schema_blocks_with_existing_diagnostic() {
    let result =
        compile_builtin_rename(serde_json::json!("a"), serde_json::json!("renamed"), false);

    assert!(result.plan.is_none());
    assert!(
        rename_diagnostic_codes(&result).contains("compiler.schema.resolver_missing"),
        "{:?}",
        result.analysis.diagnostics
    );
}

#[test]
fn schema_filter_project_and_rename_are_evaluated_into_facts() {
    let resolver_id = SchemaResolverId::new("test.source_schema").unwrap();
    let source = test_protocol(
        "schema_source",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            Some(SchemaExpr::Derived {
                resolver: resolver_id.clone(),
                dependencies: vec![],
            }),
        )],
        vec![],
        vec![],
    );
    let mut filter = test_protocol(
        "schema_filter",
        vec![
            data_port("in", PortDirection::Input, TypeExpr::Unknown, None),
            data_port(
                "out",
                PortDirection::Output,
                TypeExpr::Unknown,
                Some(SchemaExpr::Filter {
                    input: Box::new(SchemaExpr::Input(key("in"))),
                    predicate: Some(ParameterKey::new("predicate").unwrap()),
                }),
            ),
        ],
        vec![],
        vec![],
    );
    filter.parameters = ParameterSchema::new(vec![ParameterSpec {
        key: ParameterKey::new("predicate").unwrap(),
        title_key: I18nKey::new("parameters.predicate.title").unwrap(),
        description_key: None,
        value_type: TypeExpr::Concrete(type_id(
            crate::node_system::protocol::dataframe::FILTER_PREDICATE_TYPE_ID,
        )),
        default_value: None,
        constraints: vec![ParameterConstraint::Required],
        editor: ParameterEditorSpec::Auto,
        presentation: ParameterPresentation::DetailPanel,
    }])
    .unwrap();
    let project = test_protocol(
        "schema_project",
        vec![
            data_port("in", PortDirection::Input, TypeExpr::Unknown, None),
            data_port(
                "out",
                PortDirection::Output,
                TypeExpr::Unknown,
                Some(SchemaExpr::Project {
                    input: Box::new(SchemaExpr::Input(key("in"))),
                    columns: ColumnSelectionExpr::Explicit(vec![SchemaColumnRef("a".into())]),
                }),
            ),
        ],
        vec![],
        vec![],
    );
    let rename = test_protocol(
        "schema_rename",
        vec![
            data_port("in", PortDirection::Input, TypeExpr::Unknown, None),
            data_port(
                "out",
                PortDirection::Output,
                TypeExpr::Unknown,
                Some(SchemaExpr::Rename {
                    input: Box::new(SchemaExpr::Input(key("in"))),
                    mapping: RenameExpr::Explicit(vec![ColumnRename {
                        from: SchemaColumnRef("a".into()),
                        to: SchemaColumnRef("renamed".into()),
                    }]),
                }),
            ),
        ],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![source, filter, project, rename]).with_nominal_registry(
        crate::node_system::catalog::build_builtin_node_system()
            .unwrap()
            .registry,
    );
    let mut resolvers = SchemaResolverSet::new();
    resolvers.insert(resolver_id, SourceSchemaResolver);
    let mut graph = graph_with_nodes(&[
        (1, "schema_source"),
        (2, "schema_filter"),
        (3, "schema_project"),
        (4, "schema_rename"),
    ]);
    graph.nodes.get_mut(&node_id(2)).unwrap().parameters.insert(
        ParameterKey::new("predicate").unwrap(),
        serde_json::json!({
            "column": "a",
            "operator": "greaterThan",
            "value": {"type": "integer", "value": "0"}
        }),
    );
    connect(&mut graph, 10, 1, "out", 2, "in");
    connect(&mut graph, 11, 2, "out", 3, "in");
    connect(&mut graph, 12, 3, "out", 4, "in");

    let result =
        GraphCompiler::with_schema_resolvers(&registry, &Resources, resolvers).compile(&graph);

    assert!(result.plan.is_some(), "{:?}", result.analysis.diagnostics);
    let source_fact = SchemaExpr::Input(key("raw"));
    let filtered = SchemaExpr::Filter {
        input: Box::new(source_fact),
        predicate: Some(ParameterKey::new("predicate").unwrap()),
    };
    assert_eq!(
        result
            .analysis
            .partial_schemas
            .get(&PortAddress::declared(node_id(2), key("out"))),
        Some(&filtered)
    );
    let projected = SchemaExpr::Project {
        input: Box::new(filtered),
        columns: ColumnSelectionExpr::Explicit(vec![SchemaColumnRef("a".into())]),
    };
    assert_eq!(
        result
            .analysis
            .partial_schemas
            .get(&PortAddress::declared(node_id(3), key("out"))),
        Some(&projected)
    );
    assert_eq!(
        result
            .analysis
            .partial_schemas
            .get(&PortAddress::declared(node_id(4), key("out"))),
        Some(&SchemaExpr::Rename {
            input: Box::new(projected),
            mapping: RenameExpr::Explicit(vec![ColumnRename {
                from: SchemaColumnRef("a".into()),
                to: SchemaColumnRef("renamed".into()),
            }]),
        })
    );
    let output = PortAddress::declared(node_id(4), key("out"));
    let expected_fields = vec![SchemaField {
        name: SchemaColumnRef("renamed".into()),
        scalar_type: RelationalScalarType::Int64,
        lineage: None,
    }];
    assert_eq!(
        result.analysis.resolved_schemas[&output].fields,
        expected_fields
    );
    assert_eq!(
        result.semantic.as_ref().unwrap().resolved_schemas[&output].fields,
        expected_fields
    );
}

#[test]
fn explicit_project_reports_missing_and_duplicate_columns() {
    let resolver_id = SchemaResolverId::new("test.project_source_schema").unwrap();
    let source = test_protocol(
        "project_schema_source",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            Some(SchemaExpr::Derived {
                resolver: resolver_id.clone(),
                dependencies: vec![],
            }),
        )],
        vec![],
        vec![],
    );
    let project = test_protocol(
        "invalid_schema_project",
        vec![
            data_port("in", PortDirection::Input, TypeExpr::Unknown, None),
            data_port(
                "out",
                PortDirection::Output,
                TypeExpr::Unknown,
                Some(SchemaExpr::Project {
                    input: Box::new(SchemaExpr::Input(key("in"))),
                    columns: ColumnSelectionExpr::Explicit(vec![
                        SchemaColumnRef("missing".into()),
                        SchemaColumnRef("a".into()),
                        SchemaColumnRef("a".into()),
                    ]),
                }),
            ),
        ],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![source, project]);
    let mut resolvers = SchemaResolverSet::new();
    resolvers.insert(resolver_id, SourceSchemaResolver);
    let mut graph =
        graph_with_nodes(&[(1, "project_schema_source"), (2, "invalid_schema_project")]);
    connect(&mut graph, 10, 1, "out", 2, "in");

    let result =
        GraphCompiler::with_schema_resolvers(&registry, &Resources, resolvers).compile(&graph);
    let codes = result
        .analysis
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect::<BTreeSet<_>>();

    assert!(result.plan.is_none());
    assert!(codes.contains("compiler.schema.project_field_missing"));
    assert!(codes.contains("compiler.schema.project_field_duplicate"));
}

#[test]
fn explicit_rename_reports_missing_duplicate_and_conflicting_fields() {
    let resolver_id = SchemaResolverId::new("test.rename_source_schema").unwrap();
    let source = test_protocol(
        "rename_schema_source",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            Some(SchemaExpr::Derived {
                resolver: resolver_id.clone(),
                dependencies: vec![],
            }),
        )],
        vec![],
        vec![],
    );
    let rename = test_protocol(
        "invalid_schema_rename",
        vec![
            data_port("in", PortDirection::Input, TypeExpr::Unknown, None),
            data_port(
                "out",
                PortDirection::Output,
                TypeExpr::Unknown,
                Some(SchemaExpr::Rename {
                    input: Box::new(SchemaExpr::Input(key("in"))),
                    mapping: RenameExpr::Explicit(vec![
                        ColumnRename {
                            from: SchemaColumnRef("missing".into()),
                            to: SchemaColumnRef("x".into()),
                        },
                        ColumnRename {
                            from: SchemaColumnRef("a".into()),
                            to: SchemaColumnRef("b".into()),
                        },
                        ColumnRename {
                            from: SchemaColumnRef("a".into()),
                            to: SchemaColumnRef("x".into()),
                        },
                    ]),
                }),
            ),
        ],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![source, rename]);
    let mut resolvers = SchemaResolverSet::new();
    resolvers.insert(resolver_id, SourceSchemaResolver);
    let mut graph = graph_with_nodes(&[(1, "rename_schema_source"), (2, "invalid_schema_rename")]);
    connect(&mut graph, 10, 1, "out", 2, "in");

    let result =
        GraphCompiler::with_schema_resolvers(&registry, &Resources, resolvers).compile(&graph);
    let codes = result
        .analysis
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect::<BTreeSet<_>>();

    assert!(result.plan.is_none());
    assert!(codes.contains("compiler.schema.rename_field_missing"));
    assert!(codes.contains("compiler.schema.rename_source_duplicate"));
    assert!(codes.contains("compiler.schema.rename_target_conflict"));
}

#[test]
fn resolver_error_analysis_has_no_plan() {
    let protocol = test_protocol(
        "unresolved_schema",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            Some(SchemaExpr::Derived {
                resolver: SchemaResolverId::new("test.missing_schema").unwrap(),
                dependencies: vec![],
            }),
        )],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![protocol]);
    let graph = graph_with_nodes(&[(1, "unresolved_schema")]);

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);

    assert!(result.plan.is_none());
    assert!(
        result
            .analysis
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code.as_str() == "compiler.schema.resolver_missing" })
    );
}

fn control_port(name: &str, direction: PortDirection) -> PortSpec {
    PortSpec {
        key: key(name),
        label_key: I18nKey::new(format!("ports.{name}.label")).unwrap(),
        direction,
        kind: PortKind::Control,
        value_type: TypeExpr::Unknown,
        instances: PortInstances::Declared,
        connections: if direction == PortDirection::Input {
            ConnectionsPerPort::Single
        } else {
            ConnectionsPerPort::Multiple {
                max: None,
                ordered: true,
            }
        },
        input_binding: None,
        consumption: None,
        production: None,
        editor: PortEditorSpec::Default,
        schema: None,
    }
}

fn structural_protocol(
    name: &str,
    ports: Vec<PortSpec>,
    parameters: Vec<ParameterSpec>,
) -> NodeProtocol {
    let mut protocol = test_protocol(name, ports, vec![], vec![]);
    protocol.parameters = ParameterSchema::new(parameters).unwrap();
    protocol
}

fn set_parameters(graph: &mut GraphDocument, node: u128, values: &[(&str, serde_json::Value)]) {
    graph.nodes.get_mut(&node_id(node)).unwrap().parameters = values
        .iter()
        .map(|(name, value)| (ParameterKey::new(*name).unwrap(), value.clone()))
        .collect();
}

fn contains_region(
    region: &crate::node_system::plan::StructuredControlRegion,
    predicate: &impl Fn(&crate::node_system::plan::StructuredControlRegion) -> bool,
) -> bool {
    use crate::node_system::plan::{ControlStep, StructuredControlRegion};
    if predicate(region) {
        return true;
    }
    match region {
        StructuredControlRegion::Sequence(steps) => steps.iter().any(|step| match step {
            ControlStep::Operation(_) => false,
            ControlStep::Region(region) => contains_region(region, predicate),
        }),
        StructuredControlRegion::If {
            then_region,
            else_region,
            ..
        } => contains_region(then_region, predicate) || contains_region(else_region, predicate),
        StructuredControlRegion::Loop { body, .. } => contains_region(body, predicate),
        StructuredControlRegion::Call { .. } => false,
    }
}

#[test]
fn sequence_builds_an_ordered_control_region() {
    let sequence = structural_protocol(
        "sequence",
        vec![
            control_port("enter", PortDirection::Input),
            control_port("then", PortDirection::Output),
        ],
        vec![],
    );
    let leaf = test_protocol(
        "controlled_leaf",
        vec![control_port("enter", PortDirection::Input)],
        vec![],
        vec![],
    );
    let sequence_type = sequence.type_id.clone();
    let registry = TestRegistry::new(vec![sequence, leaf])
        .structural(&sequence_type, StructuralNodeRole::Sequence);
    let mut graph = graph_with_nodes(&[(1, "sequence"), (2, "controlled_leaf")]);
    connect(&mut graph, 10, 1, "then", 2, "enter");

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);

    let plan = result.plan.expect("sequence should produce a plan");
    assert_eq!(plan.operations.len(), 1);
    assert!(contains_region(&plan.root_region, &|region| matches!(
        region,
        crate::node_system::plan::StructuredControlRegion::Sequence(steps)
            if steps.iter().any(|step| matches!(step, crate::node_system::plan::ControlStep::Operation(_)))
    )));
}

fn operation_index_for_node(
    plan: &crate::node_system::plan::ExecutionPlan,
    node: u128,
) -> crate::node_system::plan::OperationIndex {
    crate::node_system::plan::OperationIndex::new(
        plan.operations
            .iter()
            .position(|operation| operation.source_node_id == node_id(node))
            .expect("built-in operation must be present") as u32,
    )
}

fn region_contains_operation(
    region: &crate::node_system::plan::StructuredControlRegion,
    expected: crate::node_system::plan::OperationIndex,
) -> bool {
    use crate::node_system::plan::{ControlStep, StructuredControlRegion};
    match region {
        StructuredControlRegion::Sequence(steps) => steps.iter().any(|step| match step {
            ControlStep::Operation(operation) => *operation == expected,
            ControlStep::Region(region) => region_contains_operation(region, expected),
        }),
        StructuredControlRegion::If {
            then_region,
            else_region,
            ..
        } => {
            region_contains_operation(then_region, expected)
                || region_contains_operation(else_region, expected)
        }
        StructuredControlRegion::Loop { body, .. } => region_contains_operation(body, expected),
        StructuredControlRegion::Call { .. } => false,
    }
}

#[test]
fn builtin_multi_output_sequence_outside_branch_keeps_walker_order() {
    use crate::node_system::catalog::build_builtin_node_system;

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.control.sequence"),
        (2, "yssbi.control.do"),
        (3, "yssbi.control.do"),
    ]);
    let first = bind_member_port(&mut graph, 1, "then", 10, "a");
    let second = bind_member_port(&mut graph, 1, "then", 11, "z");
    connect_addresses(
        &mut graph,
        100,
        first,
        PortAddress::declared(node_id(2), key("enter")),
    );
    connect_addresses(
        &mut graph,
        101,
        second,
        PortAddress::declared(node_id(3), key("enter")),
    );

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);
    let plan = result
        .plan
        .unwrap_or_else(|| panic!("sequence diagnostics: {:?}", result.analysis.diagnostics));

    assert_eq!(plan.operations.len(), 2);
    let first = operation_index_for_node(&plan, 2);
    let second = operation_index_for_node(&plan, 3);
    let StructuredControlRegion::Sequence(steps) = &plan.root_region else {
        panic!("expected root sequence, got {:?}", plan.root_region);
    };
    let first_position = steps
        .iter()
        .position(|step| matches!(step, ControlStep::Operation(operation) if *operation == first))
        .expect("first Sequence output operation");
    let second_position = steps
        .iter()
        .position(|step| matches!(step, ControlStep::Operation(operation) if *operation == second))
        .expect("second Sequence output operation");
    assert!(first_position < second_position);
}

#[test]
fn print_protocol_default_lowers_to_effective_runtime_binding() {
    use crate::node_system::catalog::build_builtin_node_system;

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let graph = builtin_graph_with_nodes(&[(1, "yssbi.debug.print")]);

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);
    let plan = result
        .plan
        .unwrap_or_else(|| panic!("print diagnostics: {:?}", result.analysis.diagnostics));
    let print = &plan.operations[operation_index_for_node(&plan, 1).index()];

    assert_eq!(print.inputs.len(), 1);
    assert_eq!(
        print.inputs[0].bound_value,
        Some(Value::String("Hello, World!".into()))
    );
    plan.validate().unwrap();
}

#[test]
fn unconnected_branch_condition_uses_the_protocol_default() {
    use crate::node_system::catalog::build_builtin_node_system;

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.project.event.begin"),
        (2, "yssbi.control.branch"),
        (3, "yssbi.constant.string"),
        (4, "yssbi.debug.print"),
    ]);
    connect(&mut graph, 100, 1, "then", 2, "enter");
    connect(&mut graph, 101, 2, "true", 4, "enter");
    connect(&mut graph, 102, 3, "value", 4, "message");
    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);

    assert!(
        matches!(result.outcome, CompilationOutcome::Succeeded),
        "unexpected outcome {:?} with diagnostics {:?}",
        result.outcome,
        result.analysis.diagnostics
    );
    let plan = result
        .plan
        .expect("Branch protocol default makes a full plan");
    let StructuredControlRegion::Sequence(steps) = &plan.root_region else {
        panic!("event root is a sequence")
    };
    let condition = steps
        .iter()
        .find_map(|step| match step {
            ControlStep::Region(region) => match region.as_ref() {
                StructuredControlRegion::If { condition, .. } => Some(*condition),
                _ => None,
            },
            ControlStep::Operation(_) => None,
        })
        .expect("event plan contains Branch");
    assert_eq!(plan.bound_values.get(&condition), Some(&Value::Bool(true)));
}

#[test]
fn branch_builds_exclusive_true_and_false_regions() {
    use crate::node_system::catalog::build_builtin_node_system;
    use crate::node_system::plan::{ControlStep, StructuredControlRegion};

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.constant.bool"),
        (2, "yssbi.constant.int64"),
        (3, "yssbi.constant.int64"),
        (4, "yssbi.control.branch"),
        (5, "yssbi.control.do"),
        (6, "yssbi.control.do"),
        (7, "yssbi.control.merge"),
        (8, "yssbi.debug.view"),
        (9, "yssbi.constant.int64"),
    ]);
    let then_source = bind_member_port(&mut graph, 4, "then_source", 40, "z");
    let else_source = bind_member_port(&mut graph, 4, "else_source", 40, "a");
    let result_port = bind_member_port(&mut graph, 4, "result", 40, "m");
    let demanded_result = result_port.clone();
    let merge_true = bind_member_port(&mut graph, 7, "enter", 70, "z");
    let merge_false = bind_member_port(&mut graph, 7, "enter", 71, "a");

    connect(&mut graph, 100, 1, "value", 4, "condition");
    connect_addresses(
        &mut graph,
        101,
        PortAddress::declared(node_id(2), key("value")),
        then_source,
    );
    connect_addresses(
        &mut graph,
        102,
        PortAddress::declared(node_id(3), key("value")),
        else_source,
    );
    connect_addresses(
        &mut graph,
        103,
        result_port,
        PortAddress::declared(node_id(8), key("data")),
    );
    connect(&mut graph, 104, 4, "true", 5, "enter");
    connect_addresses(
        &mut graph,
        105,
        PortAddress::declared(node_id(5), key("then")),
        merge_true,
    );
    connect(&mut graph, 106, 4, "false", 6, "enter");
    connect_addresses(
        &mut graph,
        107,
        PortAddress::declared(node_id(6), key("then")),
        merge_false,
    );
    connect(&mut graph, 108, 7, "then", 8, "enter");

    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/branch-demand".into()), &graph);
    let mut result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let basis = result.execution_basis.as_mut().unwrap_or_else(|| {
        panic!(
            "branch diagnostics: {:?}; outcome: {:?}",
            result.analysis.diagnostics, result.outcome
        )
    });
    let branch_local_pure = basis
        .operations
        .iter_mut()
        .find(|operation| operation.source_node_id == node_id(5))
        .expect("true arm operation is present in the basis");
    branch_local_pure.evaluation = EvaluationPolicy::DemandDriven;
    branch_local_pure.purity = Purity::Pure;
    branch_local_pure.effects = EffectSemantics::None;

    fn branch_results_mut(
        region: &mut StructuredControlRegion,
    ) -> Option<&mut Box<[crate::node_system::plan::BranchResultBinding]>> {
        match region {
            StructuredControlRegion::Sequence(steps) => {
                steps.iter_mut().find_map(|step| match step {
                    ControlStep::Operation(_) => None,
                    ControlStep::Region(region) => branch_results_mut(region),
                })
            }
            StructuredControlRegion::If { results, .. } => Some(results),
            StructuredControlRegion::Loop { body, .. } => branch_results_mut(body),
            StructuredControlRegion::Call { .. } => None,
        }
    }
    let deleted_result_value = ValueRef::new(basis.value_count);
    basis.value_count += 1;
    let results = branch_results_mut(&mut basis.root_region).expect("Branch result bindings");
    let mut bindings = results.to_vec();
    let mut deleted_binding = bindings[0];
    deleted_binding.destination = deleted_result_value;
    bindings.push(deleted_binding);
    *results = bindings.into_boxed_slice();
    let mut value_sources = basis.value_sources.to_vec();
    value_sources.push(PlanValueSource::ControlProduced(
        deleted_result_value,
        OutputProduction::FullyMaterialized,
    ));
    basis.value_sources = value_sources.into_boxed_slice();

    let mut disconnected_basis = basis.clone();
    for operation in &mut disconnected_basis.operations {
        if [node_id(6), node_id(8)].contains(&operation.source_node_id) {
            operation.evaluation = EvaluationPolicy::DemandDriven;
            operation.purity = Purity::Pure;
            operation.effects = EffectSemantics::None;
        }
    }
    let disconnected = disconnected_basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/branch-demand", 9, "value")]),
            include_default_results: false,
        })
        .expect("a disconnected If with results is deleted without orphan declarations");
    assert!(!contains_region(
        &disconnected.root_region,
        &|region| matches!(region, StructuredControlRegion::If { .. })
    ));
    assert!(
        disconnected
            .value_sources
            .iter()
            .all(|source| !matches!(source, PlanValueSource::ControlProduced(..)))
    );
    disconnected.validate().unwrap();

    let specialized = result
        .execution_basis
        .as_ref()
        .unwrap()
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([GraphOutputRef {
                graph_path: snapshot.provenance.graph_path.clone(),
                port: demanded_result.clone(),
            }]),
            include_default_results: false,
        })
        .expect("demanded Branch result safely specializes");
    let specialized_nodes = specialized
        .operations
        .iter()
        .map(|operation| operation.source_node_id)
        .collect::<BTreeSet<_>>();
    for pruned in [9] {
        assert!(!specialized_nodes.contains(&node_id(pruned)));
    }
    assert!(
        !specialized_nodes.contains(&node_id(5)),
        "unrequested arm-local pure work is pruned"
    );
    for retained in [1, 2, 3, 6] {
        assert!(
            specialized_nodes.contains(&node_id(retained)),
            "Branch condition, both result sources, and arm-local eager work stay retained"
        );
    }
    let retained_else_eager = operation_index_for_node(&specialized, 6);
    assert!(contains_region(
        &specialized.root_region,
        &|region| matches!(
            region,
            StructuredControlRegion::If {
                then_region,
                else_region,
                results,
                ..
            } if results.len() == 1
                && matches!(then_region.as_ref(), StructuredControlRegion::Sequence(steps) if steps.is_empty())
                && region_contains_operation(else_region, retained_else_eager)
        )
    ));
    specialized.validate().unwrap();

    let basis = result.execution_basis.as_ref().unwrap();
    let retained_result_value = basis.output_results[&GraphOutputRef {
        graph_path: snapshot.provenance.graph_path.clone(),
        port: demanded_result,
    }]
        .value;

    assert!(
        specialized
            .value_sources
            .contains(&PlanValueSource::ControlProduced(
                retained_result_value,
                OutputProduction::FullyMaterialized,
            ))
    );
    assert!(
        !specialized
            .value_sources
            .contains(&PlanValueSource::ControlProduced(
                deleted_result_value,
                OutputProduction::FullyMaterialized,
            ))
    );

    let plan = result
        .plan
        .unwrap_or_else(|| panic!("branch diagnostics: {:?}", result.analysis.diagnostics));
    let then_operation = operation_index_for_node(&plan, 5);
    let else_operation = operation_index_for_node(&plan, 6);
    let continuation = operation_index_for_node(&plan, 8);
    let then_output = plan.operations[operation_index_for_node(&plan, 2).index()].outputs[0].value;
    let else_output = plan.operations[operation_index_for_node(&plan, 3).index()].outputs[0].value;
    let continuation_input = plan.operations[continuation.index()].inputs[0].value;
    let then_binding_source = plan
        .value_dependencies
        .iter()
        .find(|dependency| dependency.source == then_output)
        .expect("then constant must feed its exact member")
        .destination;
    let else_binding_source = plan
        .value_dependencies
        .iter()
        .find(|dependency| dependency.source == else_output)
        .expect("else constant must feed its exact member")
        .destination;
    let branch_destination = plan
        .value_dependencies
        .iter()
        .find(|dependency| dependency.destination == continuation_input)
        .expect("branch result must feed the continuation directly")
        .source;
    assert!(
        plan.operations
            .iter()
            .all(|operation| !matches!(operation.kernel, PlannedKernel::Adapter(_)))
    );

    let root_steps = match &plan.root_region {
        StructuredControlRegion::Sequence(steps) => steps,
        other => panic!("expected root sequence, got {other:?}"),
    };
    let branch_region = root_steps
        .iter()
        .find_map(|step| match step {
            ControlStep::Region(region)
                if matches!(region.as_ref(), StructuredControlRegion::If { .. }) =>
            {
                Some(region.as_ref())
            }
            _ => None,
        })
        .expect("root sequence must contain Branch");
    let StructuredControlRegion::If {
        then_region,
        else_region,
        results,
        ..
    } = branch_region
    else {
        unreachable!()
    };
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].destination, branch_destination);
    assert_eq!(results[0].then_source, then_binding_source);
    assert_eq!(results[0].else_source, else_binding_source);
    assert_eq!(
        results[0].production,
        Some(OutputProduction::FullyMaterialized)
    );
    assert!(region_contains_operation(then_region, then_operation));
    assert!(!region_contains_operation(then_region, else_operation));
    assert!(region_contains_operation(else_region, else_operation));
    assert!(!region_contains_operation(else_region, then_operation));
    assert!(!region_contains_operation(then_region, continuation));
    assert!(!region_contains_operation(else_region, continuation));
    assert!(root_steps.iter().any(
        |step| matches!(step, ControlStep::Operation(operation) if *operation == continuation)
    ));
}

#[test]
fn nested_branch_with_one_terminating_arm_blocks_unstructured_continuation() {
    use crate::node_system::catalog::build_builtin_node_system;

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.constant.bool"),
        (2, "yssbi.constant.bool"),
        (3, "yssbi.control.branch"),
        (4, "yssbi.control.branch"),
        (5, "yssbi.control.merge"),
        (6, "yssbi.control.do"),
    ]);
    let merge_inner_true = bind_member_port(&mut graph, 5, "enter", 50, "z");
    let merge_outer_false = bind_member_port(&mut graph, 5, "enter", 51, "a");
    connect(&mut graph, 100, 1, "value", 3, "condition");
    connect(&mut graph, 101, 2, "value", 4, "condition");
    connect(&mut graph, 102, 3, "true", 4, "enter");
    connect_addresses(
        &mut graph,
        103,
        PortAddress::declared(node_id(4), key("true")),
        merge_inner_true,
    );
    connect_addresses(
        &mut graph,
        104,
        PortAddress::declared(node_id(3), key("false")),
        merge_outer_false,
    );
    connect(&mut graph, 105, 5, "then", 6, "enter");

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);

    assert!(
        result.plan.is_none(),
        "must not unconditionally execute node A"
    );
    assert!(result.semantic.is_some());
    assert!(!result.analysis.has_blocking_errors());
    assert!(matches!(
        result.outcome,
        CompilationOutcome::InternalFailure(ref failure)
            if failure.stage == CompilationStage::Lowering
                && failure.code.as_ref() == "compiler.control.unstructured_continuation"
                && failure.node_id == Some(node_id(3))
    ));
}

#[test]
fn branch_postdom_uses_last_walker_successor_for_multi_output_sequence() {
    use crate::node_system::catalog::build_builtin_node_system;
    use crate::node_system::plan::{ControlStep, StructuredControlRegion};

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.constant.bool"),
        (2, "yssbi.control.branch"),
        (3, "yssbi.control.sequence"),
        (4, "yssbi.control.do"),
        (5, "yssbi.control.merge"),
        (6, "yssbi.control.do"),
    ]);
    let sequence_first = bind_member_port(&mut graph, 3, "then", 30, "a");
    let sequence_second = bind_member_port(&mut graph, 3, "then", 31, "z");
    let merge_true = bind_member_port(&mut graph, 5, "enter", 50, "a");
    let merge_false = bind_member_port(&mut graph, 5, "enter", 51, "z");
    connect(&mut graph, 100, 1, "value", 2, "condition");
    connect(&mut graph, 101, 2, "true", 3, "enter");
    connect_addresses(
        &mut graph,
        102,
        sequence_first,
        PortAddress::declared(node_id(4), key("enter")),
    );
    connect_addresses(&mut graph, 103, sequence_second, merge_true);
    connect_addresses(
        &mut graph,
        104,
        PortAddress::declared(node_id(2), key("false")),
        merge_false,
    );
    connect(&mut graph, 105, 5, "then", 6, "enter");

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);
    let plan = result
        .plan
        .unwrap_or_else(|| panic!("sequence diagnostics: {:?}", result.analysis.diagnostics));
    let first = operation_index_for_node(&plan, 4);
    let continuation = operation_index_for_node(&plan, 6);
    let root_steps = match &plan.root_region {
        StructuredControlRegion::Sequence(steps) => steps,
        other => panic!("expected root sequence, got {other:?}"),
    };
    let (branch_index, branch) = root_steps
        .iter()
        .enumerate()
        .find_map(|(index, step)| match step {
            ControlStep::Region(region)
                if matches!(region.as_ref(), StructuredControlRegion::If { .. }) =>
            {
                Some((index, region.as_ref()))
            }
            _ => None,
        })
        .expect("Branch region");
    let StructuredControlRegion::If {
        then_region,
        else_region,
        ..
    } = branch
    else {
        unreachable!()
    };
    assert!(region_contains_operation(then_region, first));
    assert!(!region_contains_operation(else_region, first));
    assert!(!region_contains_operation(then_region, continuation));
    assert!(!region_contains_operation(else_region, continuation));
    assert!(root_steps[branch_index + 1..].iter().any(
        |step| matches!(step, ControlStep::Operation(operation) if *operation == continuation)
    ));
}

#[test]
fn branch_continuation_allows_multi_output_sequence_suffix_after_merge() {
    use crate::node_system::catalog::build_builtin_node_system;
    use crate::node_system::plan::{ControlStep, StructuredControlRegion};

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.constant.bool"),
        (2, "yssbi.control.branch"),
        (3, "yssbi.control.do"),
        (4, "yssbi.control.do"),
        (5, "yssbi.control.merge"),
        (6, "yssbi.control.sequence"),
        (7, "yssbi.control.do"),
        (8, "yssbi.control.do"),
    ]);
    let merge_true = bind_member_port(&mut graph, 5, "enter", 50, "a");
    let merge_false = bind_member_port(&mut graph, 5, "enter", 51, "z");
    let suffix_first = bind_member_port(&mut graph, 6, "then", 60, "a");
    let suffix_second = bind_member_port(&mut graph, 6, "then", 61, "z");
    connect(&mut graph, 100, 1, "value", 2, "condition");
    connect(&mut graph, 101, 2, "true", 3, "enter");
    connect(&mut graph, 102, 2, "false", 4, "enter");
    connect_addresses(
        &mut graph,
        103,
        PortAddress::declared(node_id(3), key("then")),
        merge_true,
    );
    connect_addresses(
        &mut graph,
        104,
        PortAddress::declared(node_id(4), key("then")),
        merge_false,
    );
    connect(&mut graph, 105, 5, "then", 6, "enter");
    connect_addresses(
        &mut graph,
        106,
        suffix_first,
        PortAddress::declared(node_id(7), key("enter")),
    );
    connect_addresses(
        &mut graph,
        107,
        suffix_second,
        PortAddress::declared(node_id(8), key("enter")),
    );

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);
    let plan = result
        .plan
        .unwrap_or_else(|| panic!("suffix diagnostics: {:?}", result.analysis.diagnostics));
    let first = operation_index_for_node(&plan, 7);
    let second = operation_index_for_node(&plan, 8);
    let root_steps = match &plan.root_region {
        StructuredControlRegion::Sequence(steps) => steps,
        other => panic!("expected root sequence, got {other:?}"),
    };
    let branch_index = root_steps
        .iter()
        .position(|step| matches!(step, ControlStep::Region(region) if matches!(region.as_ref(), StructuredControlRegion::If { .. })))
        .expect("Branch region");
    let suffix = &root_steps[branch_index + 1..];
    let first_index = suffix
        .iter()
        .position(|step| matches!(step, ControlStep::Operation(operation) if *operation == first))
        .expect("first suffix operation");
    let second_index = suffix
        .iter()
        .position(|step| matches!(step, ControlStep::Operation(operation) if *operation == second))
        .expect("second suffix operation");
    assert!(first_index < second_index);
}

#[test]
fn nested_branch_postdom_stops_before_outer_multi_output_sequence_suffix() {
    use crate::node_system::catalog::build_builtin_node_system;
    use crate::node_system::plan::{ControlStep, StructuredControlRegion};

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.constant.bool"),
        (2, "yssbi.constant.bool"),
        (3, "yssbi.control.branch"),
        (4, "yssbi.control.branch"),
        (5, "yssbi.control.merge"),
        (6, "yssbi.control.merge"),
        (7, "yssbi.control.sequence"),
        (8, "yssbi.control.do"),
        (9, "yssbi.control.do"),
    ]);
    let inner_true = bind_member_port(&mut graph, 5, "enter", 50, "a");
    let inner_false = bind_member_port(&mut graph, 5, "enter", 51, "z");
    let outer_true = bind_member_port(&mut graph, 6, "enter", 60, "a");
    let outer_false = bind_member_port(&mut graph, 6, "enter", 61, "z");
    let suffix_first = bind_member_port(&mut graph, 7, "then", 70, "a");
    let suffix_second = bind_member_port(&mut graph, 7, "then", 71, "z");
    connect(&mut graph, 100, 1, "value", 3, "condition");
    connect(&mut graph, 101, 2, "value", 4, "condition");
    connect(&mut graph, 102, 3, "true", 4, "enter");
    connect_addresses(
        &mut graph,
        103,
        PortAddress::declared(node_id(4), key("true")),
        inner_true,
    );
    connect_addresses(
        &mut graph,
        104,
        PortAddress::declared(node_id(4), key("false")),
        inner_false,
    );
    connect_addresses(
        &mut graph,
        105,
        PortAddress::declared(node_id(5), key("then")),
        outer_true,
    );
    connect_addresses(
        &mut graph,
        106,
        PortAddress::declared(node_id(3), key("false")),
        outer_false,
    );
    connect(&mut graph, 107, 6, "then", 7, "enter");
    connect_addresses(
        &mut graph,
        108,
        suffix_first,
        PortAddress::declared(node_id(8), key("enter")),
    );
    connect_addresses(
        &mut graph,
        109,
        suffix_second,
        PortAddress::declared(node_id(9), key("enter")),
    );

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);
    let plan = result.plan.unwrap_or_else(|| {
        panic!(
            "nested suffix diagnostics: {:?}",
            result.analysis.diagnostics
        )
    });
    let first = operation_index_for_node(&plan, 8);
    let second = operation_index_for_node(&plan, 9);
    let root_steps = match &plan.root_region {
        StructuredControlRegion::Sequence(steps) => steps,
        other => panic!("expected root sequence, got {other:?}"),
    };
    let branch_index = root_steps
        .iter()
        .position(|step| matches!(step, ControlStep::Region(region) if matches!(region.as_ref(), StructuredControlRegion::If { .. })))
        .expect("outer Branch region");
    assert!(
        root_steps[branch_index + 1..]
            .iter()
            .any(|step| matches!(step, ControlStep::Operation(operation) if *operation == first))
    );
    assert!(
        root_steps[branch_index + 1..]
            .iter()
            .any(|step| matches!(step, ControlStep::Operation(operation) if *operation == second))
    );
}

#[test]
fn nested_complete_diamond_uses_the_true_common_continuation() {
    use crate::node_system::catalog::build_builtin_node_system;
    use crate::node_system::plan::{ControlStep, StructuredControlRegion};

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.constant.bool"),
        (2, "yssbi.constant.bool"),
        (3, "yssbi.control.branch"),
        (4, "yssbi.control.branch"),
        (5, "yssbi.control.merge"),
        (6, "yssbi.control.merge"),
        (7, "yssbi.control.do"),
    ]);
    let inner_true = bind_member_port(&mut graph, 5, "enter", 50, "z");
    let inner_false = bind_member_port(&mut graph, 5, "enter", 51, "a");
    let outer_true = bind_member_port(&mut graph, 6, "enter", 60, "z");
    let outer_false = bind_member_port(&mut graph, 6, "enter", 61, "a");
    connect(&mut graph, 100, 1, "value", 3, "condition");
    connect(&mut graph, 101, 2, "value", 4, "condition");
    connect(&mut graph, 102, 3, "true", 4, "enter");
    connect_addresses(
        &mut graph,
        103,
        PortAddress::declared(node_id(4), key("true")),
        inner_true,
    );
    connect_addresses(
        &mut graph,
        104,
        PortAddress::declared(node_id(4), key("false")),
        inner_false,
    );
    connect_addresses(
        &mut graph,
        105,
        PortAddress::declared(node_id(5), key("then")),
        outer_true,
    );
    connect_addresses(
        &mut graph,
        106,
        PortAddress::declared(node_id(3), key("false")),
        outer_false,
    );
    connect(&mut graph, 107, 6, "then", 7, "enter");

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);
    let plan = result.plan.unwrap_or_else(|| {
        panic!(
            "nested diamond diagnostics: {:?}",
            result.analysis.diagnostics
        )
    });
    let continuation = operation_index_for_node(&plan, 7);
    let root_steps = match &plan.root_region {
        StructuredControlRegion::Sequence(steps) => steps,
        other => panic!("expected root sequence, got {other:?}"),
    };
    let (outer_index, outer) = root_steps
        .iter()
        .enumerate()
        .find_map(|(index, step)| match step {
            ControlStep::Region(region)
                if matches!(region.as_ref(), StructuredControlRegion::If { .. }) =>
            {
                Some((index, region.as_ref()))
            }
            _ => None,
        })
        .expect("outer Branch must be top-level");
    let StructuredControlRegion::If {
        then_region,
        else_region,
        ..
    } = outer
    else {
        unreachable!()
    };
    assert!(contains_region(then_region, &|region| matches!(
        region,
        StructuredControlRegion::If { .. }
    )));
    assert!(!region_contains_operation(then_region, continuation));
    assert!(!region_contains_operation(else_region, continuation));
    assert!(root_steps[outer_index + 1..].iter().any(
        |step| matches!(step, ControlStep::Operation(operation) if *operation == continuation)
    ));
}

#[test]
fn malformed_builtin_control_members_emit_blocking_structured_diagnostics() {
    use crate::node_system::catalog::build_builtin_node_system;

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut branch = builtin_graph_with_nodes(&[
        (1, "yssbi.constant.bool"),
        (2, "yssbi.constant.int64"),
        (3, "yssbi.constant.int64"),
        (4, "yssbi.control.branch"),
    ]);
    let then_source = bind_member_port(&mut branch, 4, "then_source", 40, "z");
    let else_source = bind_member_port(&mut branch, 4, "else_source", 41, "a");
    bind_member_port(&mut branch, 4, "result", 41, "m");
    connect(&mut branch, 100, 1, "value", 4, "condition");
    connect_addresses(
        &mut branch,
        101,
        PortAddress::declared(node_id(2), key("value")),
        then_source,
    );
    connect_addresses(
        &mut branch,
        102,
        PortAddress::declared(node_id(3), key("value")),
        else_source,
    );

    let branch_result = GraphCompiler::new(&registry, &Resources).compile(&branch);
    assert!(branch_result.plan.is_none());
    assert!(branch_result.semantic.is_some());
    assert!(!branch_result.analysis.has_blocking_errors());
    assert!(matches!(
        branch_result.outcome,
        CompilationOutcome::InternalFailure(ref failure)
            if failure.stage == CompilationStage::Lowering
                && failure.code.as_ref() == "compiler.control.member_group_identity_ambiguous"
                && failure.node_id == Some(node_id(4))
    ));

    let mut incomplete_branch = branch.clone();
    incomplete_branch.port_bindings.retain(|address, _| {
        matches!(
            &address.port,
            crate::node_system::document::PortRef::Instance { template, .. }
                if template.as_str() == "then_source"
        )
    });
    incomplete_branch
        .connections
        .remove(&ConnectionId::from_uuid(Uuid::from_u128(102)));
    let incomplete_result = GraphCompiler::new(&registry, &Resources).compile(&incomplete_branch);
    assert!(incomplete_result.plan.is_none());
    assert!(incomplete_result.semantic.is_some());
    assert!(!incomplete_result.analysis.has_blocking_errors());
    assert!(matches!(
        incomplete_result.outcome,
        CompilationOutcome::InternalFailure(ref failure)
            if failure.stage == CompilationStage::Lowering
                && failure.code.as_ref() == "compiler.control.member_group_incomplete"
                && failure.node_id == Some(node_id(4))
    ));

    let mut loop_graph =
        builtin_graph_with_nodes(&[(5, "yssbi.constant.bool"), (6, "yssbi.control.loop")]);
    set_parameters(
        &mut loop_graph,
        6,
        &[("max_iterations", serde_json::json!(3))],
    );
    connect(&mut loop_graph, 103, 5, "value", 6, "condition");

    let loop_result = GraphCompiler::new(&registry, &Resources).compile(&loop_graph);
    assert!(loop_result.plan.is_none());
    assert!(loop_result.semantic.is_some());
    assert!(!loop_result.analysis.has_blocking_errors());
    assert!(matches!(
        loop_result.outcome,
        CompilationOutcome::InternalFailure(ref failure)
            if failure.stage == CompilationStage::Lowering
                && failure.code.as_ref() == "compiler.control.member_group_count_invalid"
                && failure.node_id == Some(node_id(6))
    ));
}

#[test]
fn loop_uses_explicit_condition_limit_and_carried_bindings() {
    use crate::node_system::catalog::build_builtin_node_system;
    use crate::node_system::plan::StructuredControlRegion;

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.constant.bool"),
        (2, "yssbi.constant.int64"),
        (3, "yssbi.constant.int64"),
        (4, "yssbi.constant.int64"),
        (5, "yssbi.constant.int64"),
        (6, "yssbi.control.loop"),
        (7, "yssbi.control.do"),
        (8, "yssbi.control.do"),
        (9, "yssbi.constant.int64"),
        (10, "yssbi.control.do"),
        (11, "yssbi.control.sequence"),
    ]);
    set_parameters(&mut graph, 6, &[("max_iterations", serde_json::json!(7))]);
    let mut members = BTreeMap::new();
    for (instance, order) in [(61, "z"), (60, "a")] {
        let initial = bind_member_port(&mut graph, 6, "initial_source", instance, order);
        let body_input = bind_member_port(&mut graph, 6, "body_input", instance, order);
        let next = bind_member_port(&mut graph, 6, "next_source", instance, order);
        let result = bind_member_port(&mut graph, 6, "result", instance, order);
        members.insert(instance, (initial, body_input, next, result));
    }
    connect(&mut graph, 100, 1, "value", 6, "condition");
    for (connection, source, instance) in [(101, 2, 60), (102, 3, 61)] {
        connect_addresses(
            &mut graph,
            connection,
            PortAddress::declared(node_id(source), key("value")),
            members[&instance].0.clone(),
        );
    }
    for (connection, source, instance) in [(103, 4, 60), (104, 5, 61)] {
        connect_addresses(
            &mut graph,
            connection,
            PortAddress::declared(node_id(source), key("value")),
            members[&instance].2.clone(),
        );
    }
    let body_pure = bind_member_port(&mut graph, 11, "then", 110, "a");
    let body_eager = bind_member_port(&mut graph, 11, "then", 111, "z");
    connect(&mut graph, 105, 6, "body", 11, "enter");
    connect(&mut graph, 106, 6, "then", 8, "enter");
    connect_addresses(
        &mut graph,
        107,
        body_pure,
        PortAddress::declared(node_id(7), key("enter")),
    );
    connect_addresses(
        &mut graph,
        108,
        body_eager,
        PortAddress::declared(node_id(10), key("enter")),
    );

    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/loop-demand".into()), &graph);
    let mut result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let body_local_pure = result
        .execution_basis
        .as_mut()
        .expect("loop graph has a specialization basis")
        .operations
        .iter_mut()
        .find(|operation| operation.source_node_id == node_id(7))
        .expect("first body operation is present in the basis");
    body_local_pure.evaluation = EvaluationPolicy::DemandDriven;
    body_local_pure.purity = Purity::Pure;
    body_local_pure.effects = EffectSemantics::None;
    let specialized = result
        .execution_basis
        .as_ref()
        .unwrap()
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([GraphOutputRef {
                graph_path: snapshot.provenance.graph_path.clone(),
                port: members[&60].3.clone(),
            }]),
            include_default_results: false,
        })
        .expect("demanded Loop result safely specializes");
    let specialized_nodes = specialized
        .operations
        .iter()
        .map(|operation| operation.source_node_id)
        .collect::<BTreeSet<_>>();
    assert!(!specialized_nodes.contains(&node_id(9)));
    assert!(
        !specialized_nodes.contains(&node_id(7)),
        "unrequested body-local pure work is pruned"
    );
    for retained in [1, 2, 3, 4, 5, 10] {
        assert!(
            specialized_nodes.contains(&node_id(retained)),
            "Loop condition, carried bindings, and body eager work stay retained"
        );
    }
    let retained_body_eager = operation_index_for_node(&specialized, 10);
    assert!(contains_region(
        &specialized.root_region,
        &|region| matches!(
            region,
            StructuredControlRegion::Loop {
                body,
                carried,
                max_iterations: 7,
                ..
            } if carried.len() == 2 && region_contains_operation(body, retained_body_eager)
        )
    ));
    specialized.validate().unwrap();

    let mut disconnected_basis = result.execution_basis.as_ref().unwrap().clone();
    let body_eager = disconnected_basis
        .operations
        .iter_mut()
        .find(|operation| operation.source_node_id == node_id(10))
        .expect("second body operation is present in the basis");
    body_eager.evaluation = EvaluationPolicy::DemandDriven;
    body_eager.purity = Purity::Pure;
    body_eager.effects = EffectSemantics::None;
    let disconnected = disconnected_basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/loop-demand", 9, "value")]),
            include_default_results: false,
        })
        .expect("a disconnected carried Loop is deleted without orphan declarations");
    assert!(!contains_region(
        &disconnected.root_region,
        &|region| matches!(region, StructuredControlRegion::Loop { .. })
    ));
    assert!(
        disconnected
            .value_sources
            .iter()
            .all(|source| !matches!(source, PlanValueSource::ControlProduced(..)))
    );
    disconnected.validate().unwrap();

    let plan = result
        .plan
        .unwrap_or_else(|| panic!("loop diagnostics: {:?}", result.analysis.diagnostics));
    let carried = match &plan.root_region {
        region
            if contains_region(region, &|candidate| {
                matches!(candidate, StructuredControlRegion::Loop { .. })
            }) =>
        {
            fn find(
                region: &StructuredControlRegion,
            ) -> Option<&[crate::node_system::plan::LoopCarriedBinding]> {
                use crate::node_system::plan::ControlStep;
                match region {
                    StructuredControlRegion::Loop { carried, .. } => Some(carried),
                    StructuredControlRegion::Sequence(steps) => {
                        steps.iter().find_map(|step| match step {
                            ControlStep::Region(region) => find(region),
                            ControlStep::Operation(_) => None,
                        })
                    }
                    StructuredControlRegion::If {
                        then_region,
                        else_region,
                        ..
                    } => find(then_region).or_else(|| find(else_region)),
                    StructuredControlRegion::Call { .. } => None,
                }
            }
            find(region).unwrap()
        }
        _ => panic!("plan must contain Loop"),
    };
    assert_eq!(carried.len(), 2);
    for (binding, initial_node, next_node) in [(carried[0], 2, 4), (carried[1], 3, 5)] {
        let initial_output =
            plan.operations[operation_index_for_node(&plan, initial_node).index()].outputs[0].value;
        let next_output =
            plan.operations[operation_index_for_node(&plan, next_node).index()].outputs[0].value;
        assert!(plan.value_dependencies.iter().any(|dependency| {
            dependency.source == initial_output && dependency.destination == binding.initial_source
        }));
        assert!(plan.value_dependencies.iter().any(|dependency| {
            dependency.source == next_output && dependency.destination == binding.next_source
        }));
        assert_ne!(binding.body_input, binding.result);
        assert_ne!(binding.initial_source, binding.next_source);
    }
    assert!(contains_region(&plan.root_region, &|region| matches!(
        region,
        StructuredControlRegion::Loop {
            max_iterations: 7,
            ..
        }
    )));
}

#[test]
fn production_diagnostics_emit_canonical_protocol_enum_facts() {
    let protocol = structural_protocol("managed_role_mismatch", vec![], vec![]);
    let issues = super::control::validate_structural_contract(
        node_id(1),
        StructuralNodeRole::FunctionEntry,
        &protocol,
        &BTreeMap::new(),
    );
    let managed_role = issues
        .iter()
        .find(|issue| {
            issue.diagnostic.definition().code == "compiler.control.managed_role_mismatch"
        })
        .expect("managed-role mismatch");
    let managed_role = managed_role
        .diagnostic
        .clone()
        .into_node(DiagnosticLocation::Node(node_id(1)));
    assert_eq!(
        managed_role.arguments,
        BTreeMap::from([
            (Box::from("actual_role"), Box::from("none")),
            (Box::from("expected_role"), Box::from("function_entry")),
        ])
    );

    let mut scoped = test_protocol("function_scoped", vec![], vec![], vec![]);
    scoped.scope = NodeScope::Function;
    let registry = TestRegistry::new(vec![scoped]);
    let scope_graph = graph_with_nodes(&[(2, "function_scoped")]);
    let scope_compiler = GraphCompiler::new(&registry, &Resources);
    let scope_result = scope_compiler
        .compile_snapshot(
            &scope_compiler.snapshot(
                GraphResourcePath("events/canonical-facts".into()),
                &scope_graph,
            ),
            &CompileCancellationToken::new(),
        )
        .unwrap();
    let scope = scope_result
        .analysis
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "compiler.node.scope_mismatch")
        .expect("scope mismatch");
    assert_eq!(
        scope.arguments,
        BTreeMap::from([
            (Box::from("actual_scope"), Box::from("function")),
            (Box::from("expected_scope"), Box::from("event")),
        ])
    );

    let source = test_protocol(
        "kind_source",
        vec![data_port(
            "value",
            PortDirection::Output,
            TypeExpr::Concrete(type_id("core.int64")),
            None,
        )],
        vec![],
        vec![],
    );
    let sink = test_protocol(
        "kind_sink",
        vec![effect_port("effect", PortDirection::Input)],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![source, sink]);
    let mut graph = graph_with_nodes(&[(3, "kind_source"), (4, "kind_sink")]);
    connect(&mut graph, 301, 3, "value", 4, "effect");
    let kind_result = GraphCompiler::new(&registry, &Resources).compile(&graph);
    let kind = kind_result
        .analysis
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "compiler.connection.kind_mismatch")
        .expect("connection-kind mismatch");
    assert_eq!(
        kind.arguments,
        BTreeMap::from([
            (Box::from("source_kind"), Box::from("data")),
            (Box::from("target_kind"), Box::from("effect")),
        ])
    );
}

#[test]
fn function_abi_managed_role_error_emits_expected_role_and_actual_count() {
    struct FunctionResources {
        path: GraphResourcePath,
        function: FunctionDocument,
        graph: GraphDocument,
    }
    impl ResourceSnapshot for FunctionResources {
        fn versions(&self) -> crate::node_system::analysis::ResourceVersionSet {
            BTreeMap::from([(
                ResourceKey::new(self.path.0.clone()),
                ResourceVersion::new("fixture-v1"),
            )])
        }

        fn function_name(&self, path: &GraphResourcePath) -> Option<&str> {
            self.function_document(path).map(|_| "Test function")
        }

        fn function_document(&self, path: &GraphResourcePath) -> Option<&FunctionDocument> {
            (path == &self.path).then_some(&self.function)
        }

        fn function_graph_document(&self, path: &GraphResourcePath) -> Option<&GraphDocument> {
            (path == &self.path).then_some(&self.graph)
        }
    }

    let mut entry = structural_protocol(
        "duplicate_function_entry",
        vec![control_port("then", PortDirection::Output)],
        vec![],
    );
    entry.managed_role = Some(ManagedNodeRole::FunctionEntry);
    entry.scope = NodeScope::Function;
    let entry_type = entry.type_id.clone();
    let mut return_node = structural_protocol(
        "single_function_return",
        vec![control_port("enter", PortDirection::Input)],
        vec![],
    );
    return_node.managed_role = Some(ManagedNodeRole::FunctionReturn);
    return_node.scope = NodeScope::Function;
    let return_type = return_node.type_id.clone();
    let registry = TestRegistry::new(vec![entry, return_node])
        .structural(&entry_type, StructuralNodeRole::FunctionEntry)
        .structural(&return_type, StructuralNodeRole::FunctionReturn);
    let path = GraphResourcePath("functions/duplicate-entry".into());
    let resources = FunctionResources {
        path: path.clone(),
        function: FunctionDocument::new(FunctionSignature {
            parameters: vec![],
            return_type: None,
        }),
        graph: GraphDocument::default(),
    };
    let graph = graph_with_nodes(&[
        (1, "duplicate_function_entry"),
        (2, "duplicate_function_entry"),
        (3, "single_function_return"),
    ]);
    let compiler = GraphCompiler::new(&registry, &resources);

    let products = compiler
        .compile_snapshot(
            &compiler.snapshot(path, &graph),
            &CompileCancellationToken::new(),
        )
        .unwrap();

    let diagnostic = products
        .analysis
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "compiler.function.abi.managed_role_invalid")
        .expect("managed role count diagnostic");
    assert_eq!(
        diagnostic.arguments,
        BTreeMap::from([
            (Box::from("actual_count"), Box::from("2")),
            (Box::from("expected_role"), Box::from("function_entry")),
        ])
    );
    let singleton = products
        .analysis
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "compiler.node.managed_singleton")
        .expect("managed singleton diagnostic");
    assert_eq!(
        singleton.arguments,
        BTreeMap::from([(Box::from("managed_role"), Box::from("function_entry"))])
    );
}

#[test]
fn function_abi_rejects_wrong_dynamic_member_direction() {
    struct FunctionResources {
        path: GraphResourcePath,
        function: FunctionDocument,
        graph: GraphDocument,
    }
    impl ResourceSnapshot for FunctionResources {
        fn versions(&self) -> crate::node_system::analysis::ResourceVersionSet {
            BTreeMap::from([(
                ResourceKey::new(self.path.0.clone()),
                ResourceVersion::new("fixture-v1"),
            )])
        }

        fn function_name(&self, path: &GraphResourcePath) -> Option<&str> {
            self.function_document(path).map(|_| "Test function")
        }

        fn function_document(&self, path: &GraphResourcePath) -> Option<&FunctionDocument> {
            (path == &self.path).then_some(&self.function)
        }

        fn function_graph_document(&self, path: &GraphResourcePath) -> Option<&GraphDocument> {
            (path == &self.path).then_some(&self.graph)
        }
    }

    let mut parameters = data_port("parameters", PortDirection::Input, TypeExpr::Unknown, None);
    parameters.instances = PortInstances::UserCreated { min: 0, max: None };
    let mut entry = structural_protocol(
        "wrong_direction_entry",
        vec![control_port("then", PortDirection::Output), parameters],
        vec![],
    );
    entry.managed_role = Some(ManagedNodeRole::FunctionEntry);
    entry.scope = NodeScope::Function;
    let entry_type = entry.type_id.clone();
    let mut return_node = structural_protocol(
        "wrong_direction_return",
        vec![control_port("enter", PortDirection::Input)],
        vec![],
    );
    return_node.managed_role = Some(ManagedNodeRole::FunctionReturn);
    return_node.scope = NodeScope::Function;
    let return_type = return_node.type_id.clone();
    let registry = TestRegistry::new(vec![entry, return_node])
        .structural(&entry_type, StructuralNodeRole::FunctionEntry)
        .structural(&return_type, StructuralNodeRole::FunctionReturn);
    let path = GraphResourcePath("functions/wrong-direction".into());
    let parameter = FunctionParameterId("amount".into());
    let resources = FunctionResources {
        path: path.clone(),
        function: FunctionDocument::new(FunctionSignature {
            parameters: vec![FunctionParameter {
                id: parameter.clone(),
                name: "Amount".into(),
                type_name: "Int64".into(),
            }],
            return_type: None,
        }),
        graph: GraphDocument::default(),
    };
    let mut graph =
        graph_with_nodes(&[(1, "wrong_direction_entry"), (2, "wrong_direction_return")]);
    bind_resolved_function_port(&mut graph, 1, "parameters", 10, "a", &path, &parameter);
    connect(&mut graph, 11, 1, "then", 2, "enter");

    let compiler = GraphCompiler::new(&registry, &resources);
    let products = compiler
        .compile_snapshot(
            &compiler.snapshot(path, &graph),
            &CompileCancellationToken::new(),
        )
        .unwrap();

    assert!(products.plan.is_none());
    assert!(products.analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "compiler.function.abi.endpoint_invalid"
    }));
}

#[test]
fn missing_call_resource_parameter_is_blocking() {
    let call = structural_protocol("call_missing_target", vec![], vec![]);
    let call_type = call.type_id.clone();
    let registry = TestRegistry::new(vec![call]).structural(&call_type, StructuralNodeRole::Call);
    let graph = graph_with_nodes(&[(1, "call_missing_target")]);

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);

    assert!(result.plan.is_none());
    assert!(result.analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "compiler.control.call.resource_parameter_missing"
    }));
}

struct PanicLowerer;
impl NodeLowerer for PanicLowerer {
    fn lower(&self, _: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        Err(LoweringError::internal(
            LoweringInvariant::StructuralNodeReachedLeafLowerer,
        ))
    }
}

#[test]
fn structural_nodes_never_invoke_leaf_lowerers() {
    let sequence = structural_protocol(
        "structural_only",
        vec![control_port("then", PortDirection::Output)],
        vec![],
    );
    let sequence_type = sequence.type_id.clone();
    let mut registry =
        TestRegistry::new(vec![sequence]).structural(&sequence_type, StructuralNodeRole::Sequence);
    registry.nodes.get_mut(&sequence_type).unwrap().1 = NodeImplementation::new(PanicLowerer);
    let graph = graph_with_nodes(&[(1, "structural_only")]);

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);

    let plan = result
        .plan
        .expect("structural-only graph should produce a plan");
    assert!(plan.operations.is_empty());
}

fn structured_function_plan(
    root_region: StructuredControlRegion,
    value_sources: Box<[PlanValueSource]>,
    value_dependencies: Box<[ValueDependency]>,
    value_count: u32,
    result_value: ValueRef,
) -> (ExecutionPlan, FunctionPlanAbi, FunctionParameterId) {
    let result = FunctionParameterId("return".into());
    let provenance = crate::node_system::analysis::CompileProvenance {
        project_session_id: ProjectSessionId::new("project-a"),
        graph_path: GraphResourcePath("functions/structured-result".into()),
        basis: crate::node_system::analysis::CompilationBasis {
            graph_revision: crate::node_system::document::GraphRevision::new(1),
            registry_fingerprint: RegistryFingerprint::from_bytes([7; 32]),
            resource_versions: BTreeMap::new(),
            resource_observations: BTreeMap::new(),
        },
        compile_id: CompileId::new(1),
    };
    let plan = ExecutionPlan {
        provenance: provenance.clone(),
        value_count,
        operations: Box::new([]),
        value_contracts: BTreeMap::new(),
        value_sources,
        bound_values: BTreeMap::new(),
        value_dependencies,
        root_region,
        effect_dependencies: Box::new([]),
        relational_subplans: Box::new([]),
        resources: Box::new([]),
        results: Box::new([]),
        publications: Box::new([]),
    };
    let abi = FunctionPlanAbi {
        provenance,
        parameters: BTreeMap::new(),
        parameter_contracts: BTreeMap::new(),
        results: BTreeMap::from([(result.clone(), result_value)]),
        result_productions: BTreeMap::from([(result.clone(), OutputProduction::FullyMaterialized)]),
        result_contracts: BTreeMap::new(),
    };
    (plan, abi, result)
}

#[test]
fn branch_function_abi_finalizes_from_lowered_result_source() {
    let (plan, mut abi, result) = structured_function_plan(
        StructuredControlRegion::If {
            condition: ValueRef::new(3),
            then_region: Box::new(StructuredControlRegion::Sequence(Box::new([]))),
            else_region: Box::new(StructuredControlRegion::Sequence(Box::new([]))),
            results: Box::new([BranchResultBinding {
                destination: ValueRef::new(2),
                then_source: ValueRef::new(0),
                else_source: ValueRef::new(1),
                production: Some(OutputProduction::Streaming),
            }]),
        },
        Box::new([
            PlanValueSource::ExternalInput(ValueRef::new(0), OutputProduction::Streaming),
            PlanValueSource::ExternalInput(ValueRef::new(1), OutputProduction::Streaming),
            PlanValueSource::ExternalInput(ValueRef::new(3), OutputProduction::FullyMaterialized),
            PlanValueSource::ControlProduced(ValueRef::new(2), OutputProduction::Streaming),
        ]),
        Box::new([ValueDependency {
            source: ValueRef::new(2),
            destination: ValueRef::new(4),
        }]),
        5,
        ValueRef::new(4),
    );

    pipeline::finalize_function_abi_productions(&plan, &mut abi).unwrap();

    assert_eq!(abi.result_productions[&result], OutputProduction::Streaming);
}

#[test]
fn loop_function_abi_finalizes_from_lowered_result_source() {
    let (plan, mut abi, result) = structured_function_plan(
        StructuredControlRegion::Loop {
            body: Box::new(StructuredControlRegion::Sequence(Box::new([]))),
            carried: Box::new([LoopCarriedBinding {
                body_input: ValueRef::new(2),
                initial_source: ValueRef::new(0),
                next_source: ValueRef::new(1),
                result: ValueRef::new(4),
                production: Some(OutputProduction::Streaming),
            }]),
            continue_condition: ValueRef::new(3),
            max_iterations: 2,
        },
        Box::new([
            PlanValueSource::ExternalInput(ValueRef::new(0), OutputProduction::Streaming),
            PlanValueSource::ExternalInput(ValueRef::new(1), OutputProduction::Streaming),
            PlanValueSource::ExternalInput(ValueRef::new(3), OutputProduction::FullyMaterialized),
            PlanValueSource::ControlProduced(ValueRef::new(2), OutputProduction::Streaming),
            PlanValueSource::ControlProduced(ValueRef::new(4), OutputProduction::Streaming),
        ]),
        Box::new([]),
        5,
        ValueRef::new(4),
    );

    pipeline::finalize_function_abi_productions(&plan, &mut abi).unwrap();

    assert_eq!(abi.result_productions[&result], OutputProduction::Streaming);
}

#[test]
fn call_function_abi_finalizes_from_lowered_result_source() {
    let (plan, mut abi, result) = structured_function_plan(
        StructuredControlRegion::Call {
            target: FunctionPlanHandle::new("functions/callee").unwrap(),
            arguments: Box::new([]),
            results: Box::new([CallResultBinding {
                callee_source: ValueRef::new(0),
                caller_destination: ValueRef::new(1),
                production: Some(OutputProduction::Streaming),
            }]),
            mandatory: true,
        },
        Box::new([PlanValueSource::ControlProduced(
            ValueRef::new(1),
            OutputProduction::Streaming,
        )]),
        Box::new([]),
        2,
        ValueRef::new(1),
    );

    pipeline::finalize_function_abi_productions(&plan, &mut abi).unwrap();

    assert_eq!(abi.result_productions[&result], OutputProduction::Streaming);
}
