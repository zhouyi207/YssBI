use super::control::{
    ControlEdge as RegionControlEdge, ControlNode, build_control_region,
    validate_structural_contract,
};
use super::coordinator::{CompileCancellationToken, CompileCancelled};
use super::dependency::cyclic_value_dependencies;
use super::dynamic_interface::{
    DynamicInterfaceResolution, InterfaceResolverSet, ProjectedDynamicPortBinding,
    ValidatedInterfaceProjection, ValidatedNodeInterfaceProjection,
    materialize_dynamic_interface_with_resources,
};
use super::relational::{RelationalConnection, RelationalFragment};
use super::schema_analysis::{SchemaAnalyzer, SchemaResolverSet};
use super::specialization::{
    DemandPortFact, ExecutionPlanBasis, IntermediateKernel, IntermediateOperation,
};
use super::type_analysis::{TypeConstraintGraph, TypeEnvironment};
use super::{LoweredKernel, LoweringContext, NodeImplementation};
use crate::node_system::analysis::{
    AnalysisSnapshot, AnalyzedNode, CompilationBasis, CompileId, CompileProvenance, ControlEdge,
    CorrelationContext, DiagnosticCode, DiagnosticLocation, DiagnosticSeverity, NOOP_TRACE_SINK,
    NodeDiagnostic, ProjectSessionId, ResolvedInterface, ResolvedPort, ResourceVersionSet,
    SemanticDependency, SpanEvent, SpanKind, SpanStatus, TraceSink, ValidatedSemanticGraph,
    ValidatedSemanticNode, ValidatedSemanticPort, ValueEdge,
};
use crate::node_system::document::{
    ConnectionId, DynamicMemberLocator, DynamicPortBinding, FunctionDocument, FunctionParameterId,
    GraphDocument, GraphResourcePath, GraphRevision, NodeId, PortAddress, PortRef,
};
use crate::node_system::plan::{
    CompiledParameterHandle, CompiledResourceRequirement, ControlStep,
    EffectDependency as PlannedEffectDependency, ExecutionPlan, FunctionPlanAbi, GraphOutputRef,
    KernelHandle, OperationIndex, PlanResult, PlanValueSource, PlannedInput, PlannedOutput,
    RelationalBackendId, ResourceAccess, ResourceId, ResourceKind, StructuredControlRegion,
    ValueDependency, ValueRef,
};
use crate::node_system::protocol::{
    ConnectionsPerPort, EffectSemantics, EvaluationPolicy, I18nKey, InputConsumption,
    LiteralPolicy, NodeProtocol, NodeTypeId, OutputProduction, ParameterConstraint,
    ParameterEditorSpec, PortDirection, PortInstances, PortKind, PortSpec, Purity, TypeClassId,
    TypeConstructorId, TypeExpr, TypeId,
};
use crate::node_system::registry::{
    NodeRegistry, ProtocolFingerprint, RegistryFingerprint, StructuralNodeRole,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};

pub type CompilerAnalysis = AnalysisSnapshot<
    GraphRevision,
    NodeId,
    PortAddress,
    ConnectionId,
    Box<str>,
    serde_json::Value,
    TypeExpr,
    crate::node_system::protocol::SchemaExpr,
>;

pub type CompilerSemanticGraph = ValidatedSemanticGraph<
    GraphRevision,
    NodeId,
    PortAddress,
    ConnectionId,
    serde_json::Value,
    TypeExpr,
    crate::node_system::protocol::SchemaExpr,
>;

pub struct RegistryNode<'a> {
    pub protocol: &'a NodeProtocol,
    pub protocol_fingerprint: ProtocolFingerprint,
    pub behavior: RegistryNodeBehavior<'a>,
}

#[derive(Clone, Copy)]
pub enum RegistryNodeBehavior<'a> {
    Leaf(&'a NodeImplementation),
    Structural(StructuralNodeRole),
}

impl RegistryNode<'_> {
    fn implementation(&self) -> Option<&NodeImplementation> {
        match self.behavior {
            RegistryNodeBehavior::Leaf(implementation) => Some(implementation),
            RegistryNodeBehavior::Structural(_) => None,
        }
    }

    fn structural_role(&self) -> Option<StructuralNodeRole> {
        match self.behavior {
            RegistryNodeBehavior::Leaf(_) => None,
            RegistryNodeBehavior::Structural(role) => Some(role),
        }
    }
}

/// The compiler registry resolves nodes and supplies the type facts required by analysis.
pub trait CompilerRegistry: TypeEnvironment {
    fn fingerprint(&self) -> &RegistryFingerprint;
    fn resolve(&self, node_type: &NodeTypeId) -> Option<RegistryNode<'_>>;
}

impl TypeEnvironment for NodeRegistry {
    fn concrete_implements(&self, value_type: &TypeId, class: &TypeClassId) -> Option<bool> {
        self.types()
            .get(value_type)
            .map(|registration| registration.classes.contains(class))
    }

    fn constructor_arity(&self, constructor: &TypeConstructorId) -> Option<usize> {
        self.types()
            .constructor(constructor)
            .map(|registration| registration.arity as usize)
    }
}

impl CompilerRegistry for NodeRegistry {
    fn fingerprint(&self) -> &RegistryFingerprint {
        self.fingerprint()
    }

    fn resolve(&self, node_type: &NodeTypeId) -> Option<RegistryNode<'_>> {
        let registered = self.get(node_type)?;
        let behavior = match (&registered.implementation, registered.structural_role) {
            (Some(implementation), None) => RegistryNodeBehavior::Leaf(
                implementation
                    .as_any()
                    .downcast_ref::<NodeImplementation>()
                    .expect("registry freeze guarantees compiler lowering capability"),
            ),
            (None, Some(role)) => RegistryNodeBehavior::Structural(role),
            _ => unreachable!("registry freeze guarantees one validated node behavior"),
        };
        Some(RegistryNode {
            protocol: &registered.protocol,
            protocol_fingerprint: self
                .catalog_manifest()
                .node_protocols
                .get(node_type)?
                .clone(),
            behavior,
        })
    }
}

pub trait ResourceSnapshot {
    fn versions(&self) -> ResourceVersionSet;

    fn function_document(&self, _path: &GraphResourcePath) -> Option<&FunctionDocument> {
        None
    }

    fn function_graph_document(&self, _path: &GraphResourcePath) -> Option<&GraphDocument> {
        None
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompilationSnapshot {
    pub provenance: CompileProvenance,
    pub document: GraphDocument,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedCompileAnalysis {
    pub analysis: CompilerAnalysis,
    pub semantic: Option<CompilerSemanticGraph>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileResult {
    pub analysis: CompilerAnalysis,
    pub interface_projection: ValidatedInterfaceProjection,
    pub semantic: Option<CompilerSemanticGraph>,
    pub execution_basis: Option<ExecutionPlanBasis>,
    pub plan: Option<ExecutionPlan>,
    pub function_abi: Option<FunctionPlanAbi>,
}

pub struct GraphCompiler<'a, R, S> {
    registry: &'a R,
    resources: &'a S,
    schema_resolvers: SchemaResolverSet,
    interface_resolvers: InterfaceResolverSet,
    project_session_id: ProjectSessionId,
    trace: &'a dyn TraceSink,
}

static NEXT_ADHOC_COMPILE_ID: AtomicU64 = AtomicU64::new(1);
#[cfg(test)]
static COMPILE_SNAPSHOT_INVOCATIONS: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
pub(crate) fn compile_snapshot_invocations() -> u64 {
    COMPILE_SNAPSHOT_INVOCATIONS.load(Ordering::Relaxed)
}

impl<'a, R: CompilerRegistry, S: ResourceSnapshot> GraphCompiler<'a, R, S> {
    pub fn new(registry: &'a R, resources: &'a S) -> Self {
        Self {
            registry,
            resources,
            schema_resolvers: SchemaResolverSet::new(),
            interface_resolvers: InterfaceResolverSet::new(),
            project_session_id: ProjectSessionId::unknown(),
            trace: &NOOP_TRACE_SINK,
        }
    }

    pub fn with_schema_resolvers(
        registry: &'a R,
        resources: &'a S,
        schema_resolvers: SchemaResolverSet,
    ) -> Self {
        Self {
            registry,
            resources,
            schema_resolvers,
            interface_resolvers: InterfaceResolverSet::new(),
            project_session_id: ProjectSessionId::unknown(),
            trace: &NOOP_TRACE_SINK,
        }
    }

    pub fn with_interface_resolvers(
        registry: &'a R,
        resources: &'a S,
        interface_resolvers: InterfaceResolverSet,
    ) -> Self {
        Self {
            registry,
            resources,
            schema_resolvers: SchemaResolverSet::new(),
            interface_resolvers,
            project_session_id: ProjectSessionId::unknown(),
            trace: &NOOP_TRACE_SINK,
        }
    }

    pub fn with_resolvers(
        registry: &'a R,
        resources: &'a S,
        schema_resolvers: SchemaResolverSet,
        interface_resolvers: InterfaceResolverSet,
    ) -> Self {
        Self {
            registry,
            resources,
            schema_resolvers,
            interface_resolvers,
            project_session_id: ProjectSessionId::unknown(),
            trace: &NOOP_TRACE_SINK,
        }
    }

    pub fn with_observability(
        mut self,
        project_session_id: ProjectSessionId,
        trace: &'a dyn TraceSink,
    ) -> Self {
        self.project_session_id = project_session_id;
        self.trace = trace;
        self
    }

    /// Captures every input identity needed to decide whether a result is still current.
    /// Callers should invoke this while holding their project read transaction, then release
    /// that lock before calling `compile_snapshot`.
    pub fn snapshot(
        &self,
        graph_path: GraphResourcePath,
        document: &GraphDocument,
    ) -> CompilationSnapshot {
        let compile_id = CompileId::new(NEXT_ADHOC_COMPILE_ID.fetch_add(1, Ordering::Relaxed));
        self.snapshot_with_compile_id(compile_id, graph_path, document)
    }

    pub fn snapshot_with_compile_id(
        &self,
        compile_id: CompileId,
        graph_path: GraphResourcePath,
        document: &GraphDocument,
    ) -> CompilationSnapshot {
        let provenance = CompileProvenance {
            project_session_id: self.project_session_id.clone(),
            graph_path,
            basis: CompilationBasis {
                graph_revision: document.revision,
                registry_fingerprint: self.registry.fingerprint().clone(),
                resource_versions: self.resources.versions(),
            },
            compile_id,
        };
        let correlation = CorrelationContext::compile(&provenance);
        self.trace.record(SpanEvent::new(
            SpanKind::Snapshot,
            SpanStatus::Started,
            correlation.clone(),
        ));
        let snapshot = CompilationSnapshot {
            provenance,
            document: document.clone(),
        };
        self.trace.record(SpanEvent::new(
            SpanKind::Snapshot,
            SpanStatus::Succeeded,
            correlation,
        ));
        snapshot
    }

    pub fn compile_snapshot(
        &self,
        snapshot: &CompilationSnapshot,
        cancellation: &CompileCancellationToken,
    ) -> Result<CompileResult, CompileCancelled> {
        #[cfg(test)]
        COMPILE_SNAPSHOT_INVOCATIONS.fetch_add(1, Ordering::Relaxed);
        let correlation = CorrelationContext::compile(&snapshot.provenance);
        self.trace.record(SpanEvent::new(
            SpanKind::Analysis,
            SpanStatus::Started,
            correlation.clone(),
        ));
        if let Err(error) = cancellation.checkpoint() {
            self.trace.record(SpanEvent::new(
                SpanKind::Analysis,
                SpanStatus::Cancelled,
                correlation,
            ));
            return Err(error);
        }
        let mut state = AnalysisState::new(
            &snapshot.document,
            snapshot.provenance.graph_path.clone(),
            snapshot.provenance.basis.clone(),
        );
        if let Err(error) = state.analyze(
            self.registry,
            &self.schema_resolvers,
            &self.interface_resolvers,
            self.resources,
            cancellation,
        ) {
            self.trace.record(SpanEvent::new(
                SpanKind::Analysis,
                SpanStatus::Cancelled,
                correlation,
            ));
            return Err(error);
        }
        let mut analysis = state.snapshot();
        let interface_projection = state.interface_projection();
        if let Err(error) = cancellation.checkpoint() {
            self.trace.record(SpanEvent::new(
                SpanKind::Analysis,
                SpanStatus::Cancelled,
                correlation,
            ));
            return Err(error);
        }
        if analysis.has_blocking_errors() {
            self.trace.record(SpanEvent::new(
                SpanKind::Analysis,
                SpanStatus::Blocked,
                correlation,
            ));
            return Ok(CompileResult {
                analysis,
                interface_projection,
                semantic: None,
                execution_basis: None,
                plan: None,
                function_abi: None,
            });
        }

        let semantic = state.semantic_graph();
        let semantic = match analysis.validated(semantic) {
            Ok(graph) => graph,
            Err(error) => {
                analysis.diagnostics = append_diagnostic(
                    analysis.diagnostics,
                    diagnostic(
                        "compiler.semantic.invalid",
                        DiagnosticLocation::Graph,
                        error.to_string(),
                    ),
                );
                self.trace.record(SpanEvent::new(
                    SpanKind::Analysis,
                    SpanStatus::Failed,
                    correlation,
                ));
                return Ok(CompileResult {
                    analysis,
                    interface_projection,
                    semantic: None,
                    execution_basis: None,
                    plan: None,
                    function_abi: None,
                });
            }
        };
        self.trace.record(SpanEvent::new(
            SpanKind::Analysis,
            SpanStatus::Succeeded,
            correlation.clone(),
        ));
        self.trace.record(SpanEvent::new(
            SpanKind::Lowering,
            SpanStatus::Started,
            correlation.clone(),
        ));
        let function_abi = match derive_function_abi(
            self.registry,
            &semantic,
            &interface_projection,
            &snapshot.provenance,
        ) {
            Ok(abi) => abi,
            Err(diagnostic) => {
                analysis.diagnostics = append_diagnostic(analysis.diagnostics, diagnostic);
                return Ok(CompileResult {
                    analysis,
                    interface_projection,
                    semantic: None,
                    execution_basis: None,
                    plan: None,
                    function_abi: None,
                });
            }
        };
        let function_abis = match self.function_abis_for_calls(&semantic, cancellation) {
            Ok(abis) => abis,
            Err(LowerGraphFailure::Cancelled(error)) => return Err(error),
            Err(LowerGraphFailure::Diagnostic(diagnostic)) => {
                analysis.diagnostics = append_diagnostic(analysis.diagnostics, diagnostic);
                return Ok(CompileResult {
                    analysis,
                    interface_projection,
                    semantic: None,
                    execution_basis: None,
                    plan: None,
                    function_abi,
                });
            }
        };
        let (semantic, execution_basis, plan) = match lower_graph(
            self.registry,
            &semantic,
            &interface_projection,
            &function_abis,
            snapshot.provenance.clone(),
            cancellation,
        ) {
            Ok((basis, plan)) => {
                self.trace.record(SpanEvent::new(
                    SpanKind::Lowering,
                    SpanStatus::Succeeded,
                    correlation.clone(),
                ));
                (Some(semantic), Some(basis), Some(plan))
            }
            Err(LowerGraphFailure::Cancelled(error)) => {
                self.trace.record(SpanEvent::new(
                    SpanKind::Lowering,
                    SpanStatus::Cancelled,
                    correlation,
                ));
                return Err(error);
            }
            Err(LowerGraphFailure::Diagnostic(diagnostic)) => {
                analysis.diagnostics = append_diagnostic(analysis.diagnostics, diagnostic);
                self.trace.record(SpanEvent::new(
                    SpanKind::Lowering,
                    SpanStatus::Failed,
                    correlation.clone(),
                ));
                (None, None, None)
            }
        };
        Ok(CompileResult {
            analysis,
            interface_projection,
            semantic,
            execution_basis,
            plan,
            function_abi,
        })
    }

    fn function_abis_for_calls(
        &self,
        graph: &CompilerSemanticGraph,
        cancellation: &CompileCancellationToken,
    ) -> Result<BTreeMap<GraphResourcePath, FunctionPlanAbi>, LowerGraphFailure> {
        let mut targets = BTreeSet::new();
        for node in graph.nodes.iter() {
            let resolved = resolve_for_lowering(self.registry, node)?;
            if resolved.structural_role() != Some(StructuralNodeRole::Call) {
                continue;
            }
            let Some(target) = function_target(&node.normalized_parameters) else {
                continue;
            };
            targets.insert(GraphResourcePath(target.into()));
        }

        let mut abis = BTreeMap::new();
        for target in targets {
            cancellation.checkpoint()?;
            let document = self
                .resources
                .function_graph_document(&target)
                .ok_or_else(|| {
                    diagnostic(
                        "compiler.control.call.abi_missing",
                        DiagnosticLocation::Graph,
                        format!(
                            "target function '{}' has no graph in the compilation snapshot",
                            target.0
                        ),
                    )
                })?;
            let provenance = CompileProvenance {
                project_session_id: self.project_session_id.clone(),
                graph_path: target.clone(),
                basis: CompilationBasis {
                    graph_revision: document.revision,
                    registry_fingerprint: self.registry.fingerprint().clone(),
                    resource_versions: self.resources.versions(),
                },
                compile_id: CompileId::new(NEXT_ADHOC_COMPILE_ID.fetch_add(1, Ordering::Relaxed)),
            };
            let mut state = AnalysisState::new(document, target.clone(), provenance.basis.clone());
            state.analyze(
                self.registry,
                &self.schema_resolvers,
                &self.interface_resolvers,
                self.resources,
                cancellation,
            )?;
            let analysis = state.snapshot();
            if analysis.has_blocking_errors() {
                return Err(diagnostic(
                    "compiler.control.call.abi_invalid",
                    DiagnosticLocation::Graph,
                    format!(
                        "target function '{}' has blocking interface diagnostics",
                        target.0
                    ),
                )
                .into());
            }
            let interface_projection = state.interface_projection();
            let semantic = analysis
                .validated(state.semantic_graph())
                .map_err(|error| {
                    diagnostic(
                        "compiler.control.call.abi_invalid",
                        DiagnosticLocation::Graph,
                        format!("target function '{}': {error}", target.0),
                    )
                })?;
            let abi =
                derive_function_abi(self.registry, &semantic, &interface_projection, &provenance)?
                    .ok_or_else(|| {
                        diagnostic(
                            "compiler.control.call.abi_invalid",
                            DiagnosticLocation::Graph,
                            format!("target function '{}' has no Entry/Return ABI", target.0),
                        )
                    })?;
            abis.insert(target, abi);
        }
        Ok(abis)
    }

    pub fn compile(&self, document: &GraphDocument) -> CompileResult {
        let snapshot = self.snapshot(GraphResourcePath(Box::from("")), document);
        self.compile_snapshot(&snapshot, &CompileCancellationToken::new())
            .expect("a fresh cancellation token is not cancelled")
    }
}

struct ResolvedNode<'a> {
    registry: RegistryNode<'a>,
    parameters: BTreeMap<crate::node_system::protocol::ParameterKey, serde_json::Value>,
    ports: BTreeMap<PortAddress, ResolvedPort<PortAddress>>,
}

struct AnalysisState<'a> {
    document: &'a GraphDocument,
    graph_path: GraphResourcePath,
    basis: CompilationBasis<GraphRevision>,
    nodes: BTreeMap<NodeId, ResolvedNode<'a>>,
    diagnostics: Vec<NodeDiagnostic<NodeId, PortAddress, ConnectionId, Box<str>>>,
    type_facts: BTreeMap<PortAddress, TypeExpr>,
    schema_facts: BTreeMap<PortAddress, crate::node_system::protocol::SchemaExpr>,
    projection_only_ports: BTreeSet<PortAddress>,
    interface_projections: BTreeMap<NodeId, ValidatedNodeInterfaceProjection>,
}

impl<'a> AnalysisState<'a> {
    fn new(
        document: &'a GraphDocument,
        graph_path: GraphResourcePath,
        basis: CompilationBasis<GraphRevision>,
    ) -> Self {
        Self {
            document,
            graph_path,
            basis,
            nodes: BTreeMap::new(),
            diagnostics: Vec::new(),
            type_facts: BTreeMap::new(),
            schema_facts: BTreeMap::new(),
            projection_only_ports: BTreeSet::new(),
            interface_projections: BTreeMap::new(),
        }
    }

    fn analyze<R: CompilerRegistry>(
        &mut self,
        registry: &'a R,
        schema_resolvers: &SchemaResolverSet,
        interface_resolvers: &InterfaceResolverSet,
        resources: &dyn ResourceSnapshot,
        cancellation: &CompileCancellationToken,
    ) -> Result<(), CompileCancelled> {
        for (&node_id, node) in &self.document.nodes {
            cancellation.checkpoint()?;
            if node.id != node_id {
                self.push(
                    "compiler.document.node_id_mismatch",
                    DiagnosticLocation::Node(node_id),
                    node.id.to_string(),
                );
            }
            let Some(resolved) = registry.resolve(&node.node_type) else {
                self.push(
                    "compiler.node.unknown",
                    DiagnosticLocation::Node(node_id),
                    node.node_type.to_string(),
                );
                continue;
            };
            let path_scope = if self.graph_path.0.starts_with("events/") {
                crate::node_system::protocol::NodeScope::Event
            } else if self.graph_path.0.starts_with("functions/") {
                crate::node_system::protocol::NodeScope::Function
            } else {
                crate::node_system::protocol::NodeScope::Any
            };
            if resolved.protocol.scope != crate::node_system::protocol::NodeScope::Any
                && path_scope != crate::node_system::protocol::NodeScope::Any
                && resolved.protocol.scope != path_scope
            {
                self.push(
                    "compiler.node.scope_mismatch",
                    DiagnosticLocation::Node(node_id),
                    format!(
                        "node scope {:?} is invalid for graph '{}'",
                        resolved.protocol.scope, self.graph_path.0
                    ),
                );
            }
            if resolved.protocol.type_id != node.node_type {
                self.push(
                    "compiler.registry.type_mismatch",
                    DiagnosticLocation::Node(node_id),
                    node.node_type.to_string(),
                );
                continue;
            }
            let parameters = self.normalize_parameters(node_id, resolved.protocol);
            let ports =
                self.resolve_ports(node_id, resolved.protocol, resources, interface_resolvers);
            self.nodes.insert(
                node_id,
                ResolvedNode {
                    registry: resolved,
                    parameters,
                    ports,
                },
            );
        }
        cancellation.checkpoint()?;
        self.validate_function_abi_contract(resources);
        self.validate_call_abi_contract(resources);
        self.validate_structural_control();
        cancellation.checkpoint()?;
        self.validate_connections();
        cancellation.checkpoint()?;
        self.validate_input_bindings();
        cancellation.checkpoint()?;
        self.validate_value_cycles();
        cancellation.checkpoint()?;
        self.analyze_types(registry);
        cancellation.checkpoint()?;
        self.analyze_schemas(schema_resolvers);
        cancellation.checkpoint()?;
        self.diagnostics.sort_by_key(diagnostic_sort_key);
        Ok(())
    }

    fn validate_function_abi_contract(&mut self, resources: &dyn ResourceSnapshot) {
        if !self.graph_path.0.starts_with("functions/") {
            return;
        }
        let Some(function) = resources.function_document(&self.graph_path) else {
            self.push(
                "compiler.function.abi.signature_missing",
                DiagnosticLocation::Graph,
                format!(
                    "function '{}' has no authoritative signature",
                    self.graph_path.0
                ),
            );
            return;
        };
        let expected_parameters = function
            .signature
            .parameters
            .iter()
            .map(|parameter| parameter.id.clone())
            .collect::<BTreeSet<_>>();
        let expected_results = function
            .signature
            .return_type
            .as_ref()
            .map(|_| FunctionParameterId("return".into()))
            .into_iter()
            .collect::<BTreeSet<_>>();
        self.validate_function_abi_role(
            StructuralNodeRole::FunctionEntry,
            "parameters",
            PortDirection::Output,
            &expected_parameters,
        );
        self.validate_function_abi_role(
            StructuralNodeRole::FunctionReturn,
            "results",
            PortDirection::Input,
            &expected_results,
        );
    }

    fn validate_function_abi_role(
        &mut self,
        role: StructuralNodeRole,
        expected_template: &str,
        expected_direction: PortDirection,
        expected_ids: &BTreeSet<FunctionParameterId>,
    ) {
        let nodes = self
            .nodes
            .iter()
            .filter_map(|(node_id, node)| {
                (node.registry.structural_role() == Some(role)).then_some(*node_id)
            })
            .collect::<Vec<_>>();
        if nodes.len() != 1 {
            self.push(
                "compiler.function.abi.managed_role_invalid",
                DiagnosticLocation::Graph,
                format!("function ABI requires exactly one {role:?} node"),
            );
            return;
        }
        let node_id = nodes[0];
        let protocol = self.nodes[&node_id].registry.protocol;
        let mut counts = BTreeMap::<FunctionParameterId, usize>::new();
        let bindings = self
            .document
            .port_bindings
            .iter()
            .filter(|(address, _)| address.node_id == node_id)
            .map(|(address, binding)| (address.clone(), binding.clone()))
            .collect::<Vec<_>>();
        for (address, binding) in bindings {
            let origin = match binding {
                DynamicPortBinding::Resolved { origin, .. }
                | DynamicPortBinding::Orphan { origin, .. } => origin,
                DynamicPortBinding::UserCreated { .. } => continue,
            };
            let DynamicMemberLocator::FunctionParameter {
                function,
                parameter,
            } = origin
            else {
                self.push(
                    "compiler.function.abi.locator_invalid",
                    DiagnosticLocation::Port(address),
                    "function ABI endpoint requires a FunctionParameter locator".into(),
                );
                continue;
            };
            let template = port_template(&address);
            let spec = protocol
                .interface
                .ports
                .iter()
                .find(|spec| &spec.key == template);
            if template.as_str() != expected_template
                || spec.is_none_or(|spec| {
                    spec.kind != PortKind::Data || spec.direction != expected_direction
                })
            {
                self.push(
                    "compiler.function.abi.endpoint_invalid",
                    DiagnosticLocation::Port(address),
                    format!(
                        "{role:?} ABI endpoint must be {expected_template} Data {expected_direction:?}"
                    ),
                );
                continue;
            }
            if function != self.graph_path {
                self.push(
                    "compiler.function.abi.locator_target_mismatch",
                    DiagnosticLocation::Port(address),
                    format!("ABI locator targets '{}'", function.0),
                );
                continue;
            }
            if !expected_ids.contains(&parameter) {
                self.push(
                    "compiler.function.abi.member_unexpected",
                    DiagnosticLocation::Port(address),
                    format!("unexpected ABI member '{}'", parameter.0),
                );
                continue;
            }
            *counts.entry(parameter).or_default() += 1;
        }
        for expected in expected_ids {
            match counts.get(expected).copied().unwrap_or(0) {
                0 => self.push(
                    "compiler.function.abi.member_missing",
                    DiagnosticLocation::Node(node_id),
                    format!("missing ABI member '{}'", expected.0),
                ),
                1 => {}
                _ => self.push(
                    "compiler.function.abi.member_duplicate",
                    DiagnosticLocation::Node(node_id),
                    format!("duplicate ABI member '{}'", expected.0),
                ),
            }
        }
    }

    fn validate_call_abi_contract(&mut self, resources: &dyn ResourceSnapshot) {
        let call_nodes = self
            .nodes
            .iter()
            .filter_map(|(node_id, node)| {
                (node.registry.structural_role() == Some(StructuralNodeRole::Call))
                    .then_some(*node_id)
            })
            .collect::<Vec<_>>();
        for node_id in call_nodes {
            let Some(target) = function_target(&self.nodes[&node_id].parameters) else {
                continue;
            };
            let target = GraphResourcePath(target.into());
            let Some(function) = resources.function_document(&target) else {
                continue;
            };
            let expected_arguments = function
                .signature
                .parameters
                .iter()
                .map(|parameter| parameter.id.clone())
                .collect::<BTreeSet<_>>();
            let expected_results = function
                .signature
                .return_type
                .as_ref()
                .map(|_| FunctionParameterId("return".into()))
                .into_iter()
                .collect::<BTreeSet<_>>();
            self.validate_call_abi_role(
                node_id,
                &target,
                "arguments",
                PortDirection::Input,
                &expected_arguments,
            );
            self.validate_call_abi_role(
                node_id,
                &target,
                "results",
                PortDirection::Output,
                &expected_results,
            );
        }
    }

    fn validate_call_abi_role(
        &mut self,
        node_id: NodeId,
        target: &GraphResourcePath,
        expected_template: &str,
        expected_direction: PortDirection,
        expected_ids: &BTreeSet<FunctionParameterId>,
    ) {
        let protocol = self.nodes[&node_id].registry.protocol.clone();
        let bindings = self
            .document
            .port_bindings
            .iter()
            .filter(|(address, _)| {
                address.node_id == node_id && port_template(address).as_str() == expected_template
            })
            .map(|(address, binding)| (address.clone(), binding.clone()))
            .collect::<Vec<_>>();
        let mut counts = BTreeMap::<FunctionParameterId, usize>::new();
        for (address, binding) in bindings {
            let origin = match binding {
                DynamicPortBinding::Resolved { origin, .. }
                | DynamicPortBinding::Orphan { origin, .. } => origin,
                DynamicPortBinding::UserCreated { .. } => {
                    self.push(
                        "compiler.control.call.locator_invalid",
                        DiagnosticLocation::Port(address),
                        "Call ABI endpoint requires a FunctionParameter locator".into(),
                    );
                    continue;
                }
            };
            let spec = protocol
                .interface
                .ports
                .iter()
                .find(|spec| spec.key.as_str() == expected_template);
            if spec.is_none_or(|spec| {
                spec.kind != PortKind::Data || spec.direction != expected_direction
            }) {
                self.push(
                    "compiler.control.call.endpoint_invalid",
                    DiagnosticLocation::Port(address),
                    format!(
                        "Call {expected_template} endpoint must be Data {expected_direction:?}"
                    ),
                );
                continue;
            }
            let DynamicMemberLocator::FunctionParameter {
                function,
                parameter,
            } = origin
            else {
                self.push(
                    "compiler.control.call.locator_invalid",
                    DiagnosticLocation::Port(address),
                    "Call ABI endpoint requires a FunctionParameter locator".into(),
                );
                continue;
            };
            if &function != target {
                self.push(
                    "compiler.control.call.locator_target_mismatch",
                    DiagnosticLocation::Port(address),
                    format!("Call member locator targets '{}'", function.0),
                );
                continue;
            }
            if !expected_ids.contains(&parameter) {
                self.push(
                    "compiler.control.call.member_unexpected",
                    DiagnosticLocation::Port(address),
                    format!("unexpected Call member '{}'", parameter.0),
                );
                continue;
            }
            *counts.entry(parameter).or_default() += 1;
        }
        for expected in expected_ids {
            match counts.get(expected).copied().unwrap_or(0) {
                0 => self.push(
                    "compiler.control.call.member_missing",
                    DiagnosticLocation::Node(node_id),
                    format!("missing Call member '{}'", expected.0),
                ),
                1 => {}
                _ => self.push(
                    "compiler.control.call.locator_duplicate",
                    DiagnosticLocation::Node(node_id),
                    format!("duplicate Call member '{}'", expected.0),
                ),
            }
        }
    }

    fn validate_structural_control(&mut self) {
        use crate::node_system::protocol::ManagedNodeRole;
        for role in [
            ManagedNodeRole::EventBegin,
            ManagedNodeRole::FunctionEntry,
            ManagedNodeRole::FunctionReturn,
        ] {
            let nodes = self
                .nodes
                .iter()
                .filter_map(|(node_id, node)| {
                    (node.registry.protocol.managed_role == Some(role)).then_some(*node_id)
                })
                .collect::<Vec<_>>();
            if nodes.len() > 1 {
                for node_id in nodes {
                    self.push(
                        "compiler.node.managed_singleton",
                        DiagnosticLocation::Node(node_id),
                        format!("managed role {role:?} may occur only once per graph"),
                    );
                }
            }
        }
        let issues = self
            .nodes
            .iter()
            .flat_map(|(&node_id, node)| {
                node.registry
                    .structural_role()
                    .into_iter()
                    .flat_map(move |role| {
                        validate_structural_contract(
                            node_id,
                            role,
                            node.registry.protocol,
                            &node.parameters,
                        )
                    })
            })
            .collect::<Vec<_>>();
        for issue in issues {
            self.push(
                issue.code,
                issue
                    .node_id
                    .map(DiagnosticLocation::Node)
                    .unwrap_or(DiagnosticLocation::Graph),
                issue.detail,
            );
        }
    }

    fn normalize_parameters(
        &mut self,
        node_id: NodeId,
        protocol: &NodeProtocol,
    ) -> BTreeMap<crate::node_system::protocol::ParameterKey, serde_json::Value> {
        let supplied = &self.document.nodes[&node_id].parameters;
        let specs: BTreeMap<_, _> = protocol
            .parameters
            .parameters
            .iter()
            .map(|spec| (&spec.key, spec))
            .collect();
        let mut normalized = BTreeMap::new();
        for (key, value) in supplied {
            if specs.contains_key(key) {
                normalized.insert(key.clone(), value.clone());
            } else {
                self.push(
                    "compiler.parameter.unknown",
                    DiagnosticLocation::Parameter {
                        node_id,
                        key: key.clone(),
                    },
                    key.to_string(),
                );
            }
        }
        for spec in protocol.parameters.parameters.iter() {
            if !normalized.contains_key(&spec.key) {
                if let Some(default) = &spec.default_value {
                    if let Ok(value) = serde_json::to_value(default) {
                        normalized.insert(spec.key.clone(), value);
                    }
                } else if spec.constraints.contains(&ParameterConstraint::Required) {
                    self.push(
                        "compiler.parameter.required",
                        DiagnosticLocation::Parameter {
                            node_id,
                            key: spec.key.clone(),
                        },
                        spec.key.to_string(),
                    );
                }
            }
        }
        normalized
    }

    fn resolve_ports(
        &mut self,
        node_id: NodeId,
        protocol: &NodeProtocol,
        resources: &dyn ResourceSnapshot,
        resolvers: &InterfaceResolverSet,
    ) -> BTreeMap<PortAddress, ResolvedPort<PortAddress>> {
        self.validate_binding_templates(node_id, protocol);
        let DynamicInterfaceResolution {
            interface,
            projected_bindings,
            available_members,
            diagnostics,
        } = materialize_dynamic_interface_with_resources(
            &self.basis,
            node_id,
            protocol,
            self.document,
            resources,
            resolvers,
        );

        self.projection_only_ports.extend(
            available_members
                .iter()
                .filter(|member| member.bound_address().is_none())
                .map(|member| member.projection_address().clone()),
        );
        self.diagnostics.extend(diagnostics);
        self.interface_projections.insert(
            node_id,
            ValidatedNodeInterfaceProjection {
                projected_bindings,
                available_members,
            },
        );
        interface
            .ports
            .into_vec()
            .into_iter()
            .map(|port| (port.address.clone(), port))
            .collect()
    }

    fn validate_binding_templates(&mut self, node_id: NodeId, protocol: &NodeProtocol) {
        for address in self
            .document
            .port_bindings
            .keys()
            .filter(|address| address.node_id == node_id)
        {
            let PortRef::Instance { template, .. } = &address.port else {
                self.push(
                    "compiler.port.binding_not_instance",
                    DiagnosticLocation::Port(address.clone()),
                    address.to_string(),
                );
                continue;
            };
            let Some(spec) = protocol
                .interface
                .ports
                .iter()
                .find(|port| &port.key == template)
            else {
                self.push(
                    "compiler.port.unknown",
                    DiagnosticLocation::Port(address.clone()),
                    address.to_string(),
                );
                continue;
            };
            if spec.instances == PortInstances::Declared {
                self.push(
                    "compiler.port.instance_not_allowed",
                    DiagnosticLocation::Port(address.clone()),
                    address.to_string(),
                );
            }
        }
    }

    fn validate_connections(&mut self) {
        let mut counts: BTreeMap<PortAddress, usize> = BTreeMap::new();
        for (&connection_id, connection) in &self.document.connections {
            if connection.id != connection_id {
                self.push(
                    "compiler.document.connection_id_mismatch",
                    DiagnosticLocation::Connection(connection_id),
                    connection.id.to_string(),
                );
            }
            let output = self.lookup_document_port(&connection.output).cloned();
            let input = self.lookup_document_port(&connection.input).cloned();
            if output.is_none() {
                self.push(
                    "compiler.port.unknown",
                    DiagnosticLocation::Port(connection.output.clone()),
                    connection.output.to_string(),
                );
            }
            if input.is_none() {
                self.push(
                    "compiler.port.unknown",
                    DiagnosticLocation::Port(connection.input.clone()),
                    connection.input.to_string(),
                );
            }
            let (Some(output), Some(input)) = (output, input) else {
                continue;
            };
            if output.direction != PortDirection::Output {
                self.push(
                    "compiler.connection.output_direction",
                    DiagnosticLocation::Connection(connection_id),
                    connection.output.to_string(),
                );
            }
            if input.direction != PortDirection::Input {
                self.push(
                    "compiler.connection.input_direction",
                    DiagnosticLocation::Connection(connection_id),
                    connection.input.to_string(),
                );
            }
            if output.kind != input.kind {
                self.push(
                    "compiler.connection.kind_mismatch",
                    DiagnosticLocation::Connection(connection_id),
                    connection_id.to_string(),
                );
            }
            if let Some(spec) = self.port_spec(&connection.input, &input.template) {
                match spec.connections {
                    ConnectionsPerPort::Multiple { ordered: true, .. }
                        if connection.order.is_none() =>
                    {
                        self.push(
                            "compiler.connection.order_required",
                            DiagnosticLocation::Connection(connection_id),
                            connection.input.to_string(),
                        );
                    }
                    ConnectionsPerPort::Single
                    | ConnectionsPerPort::Multiple { ordered: false, .. }
                        if connection.order.is_some() =>
                    {
                        self.push(
                            "compiler.connection.order_forbidden",
                            DiagnosticLocation::Connection(connection_id),
                            connection.input.to_string(),
                        );
                    }
                    _ => {}
                }
            }
            *counts.entry(connection.output.clone()).or_default() += 1;
            *counts.entry(connection.input.clone()).or_default() += 1;
        }
        for (address, count) in counts {
            let Some(port) = self.lookup_document_port(&address) else {
                continue;
            };
            let spec = self.port_spec(&address, &port.template);
            if let Some(spec) = spec {
                let exceeded = match spec.connections {
                    ConnectionsPerPort::Single => count > 1,
                    ConnectionsPerPort::Multiple { max, .. } => {
                        max.is_some_and(|max| count > max as usize)
                    }
                };
                if exceeded {
                    self.push(
                        "compiler.connection.limit",
                        DiagnosticLocation::Port(address.clone()),
                        count.to_string(),
                    );
                }
            }
        }
    }

    fn validate_input_bindings(&mut self) {
        let addresses: Vec<_> = self
            .nodes
            .values()
            .flat_map(|node| node.ports.keys())
            .filter(|address| !self.projection_only_ports.contains(*address))
            .cloned()
            .collect();
        for address in addresses {
            let port = self
                .lookup_document_port(&address)
                .cloned()
                .expect("address came from resolved ports");
            if port.direction != PortDirection::Input {
                if self.document.input_states.contains_key(&address) {
                    self.push(
                        "compiler.input.not_input",
                        DiagnosticLocation::Port(address.clone()),
                        address.to_string(),
                    );
                }
                continue;
            }
            let connections = self
                .document
                .connections
                .values()
                .filter(|connection| connection.input == address)
                .count();
            let literal = self
                .document
                .input_states
                .get(&address)
                .and_then(|state| state.literal_override.as_ref());
            let spec = self
                .port_spec(&address, &port.template)
                .cloned()
                .expect("resolved port has protocol spec");
            if literal.is_some() && connections != 0 {
                self.push(
                    "compiler.input.conflicting_bindings",
                    DiagnosticLocation::Port(address.clone()),
                    address.to_string(),
                );
            }
            if literal.is_some()
                && spec
                    .input_binding
                    .as_ref()
                    .is_none_or(|binding| binding.literal_policy == LiteralPolicy::Forbidden)
            {
                self.push(
                    "compiler.input.literal_forbidden",
                    DiagnosticLocation::Port(address.clone()),
                    address.to_string(),
                );
            }
            let has_default = spec
                .input_binding
                .as_ref()
                .is_some_and(|binding| binding.default_value.is_some());
            if port.kind == PortKind::Data && connections == 0 && literal.is_none() && !has_default
            {
                self.push(
                    "compiler.input.unbound",
                    DiagnosticLocation::Port(address.clone()),
                    address.to_string(),
                );
            }
        }
        let stale: Vec<_> = self
            .document
            .input_states
            .keys()
            .filter(|address| self.lookup_document_port(address).is_none())
            .cloned()
            .collect();
        for address in stale {
            self.push(
                "compiler.input.unknown_port",
                DiagnosticLocation::Port(address.clone()),
                address.to_string(),
            );
        }
    }

    fn validate_value_cycles(&mut self) {
        let edges = self
            .document
            .connections
            .values()
            .filter(|connection| {
                let Some(output) = self.lookup_document_port(&connection.output) else {
                    return false;
                };
                let Some(input) = self.lookup_document_port(&connection.input) else {
                    return false;
                };
                let is_loop_condition_feedback = connection.output.node_id
                    == connection.input.node_id
                    && output.template.as_str() == "body_input"
                    && input.template.as_str() == "condition"
                    && self
                        .nodes
                        .get(&connection.output.node_id)
                        .is_some_and(|node| {
                            node.registry.structural_role() == Some(StructuralNodeRole::Loop)
                        });
                output.kind == PortKind::Data
                    && input.kind == PortKind::Data
                    && !is_loop_condition_feedback
            })
            .map(|connection| {
                (
                    connection.id,
                    connection.output.node_id,
                    connection.input.node_id,
                )
            })
            .collect::<Vec<_>>();
        for connection_id in cyclic_value_dependencies(&edges) {
            self.push(
                "compiler.dependency.value_cycle",
                DiagnosticLocation::Connection(connection_id),
                connection_id.to_string(),
            );
        }
    }

    fn analyze_types<R: CompilerRegistry>(&mut self, registry: &R) {
        let mut graph = TypeConstraintGraph::new();
        for (&node_id, node) in &self.nodes {
            graph.add_node(node_id, node.registry.protocol, node.ports.keys().cloned());
        }
        for connection in self.document.connections.values() {
            let is_value = self
                .lookup_document_port(&connection.output)
                .is_some_and(|port| port.kind == PortKind::Data)
                && self
                    .lookup_document_port(&connection.input)
                    .is_some_and(|port| port.kind == PortKind::Data);
            if is_value {
                graph.add_connection(connection.id, &connection.output, &connection.input);
            }
        }
        for (address, state) in &self.document.input_states {
            if let Some(value_type) = state
                .literal_override
                .as_ref()
                .and_then(|literal| literal.get("value_type"))
                .and_then(|value| serde_json::from_value::<TypeExpr>(value.clone()).ok())
            {
                graph.add_literal(address, &value_type);
            }
        }
        let (facts, issues) = graph.solve(registry);
        self.type_facts = facts;
        for issue in issues {
            self.push(issue.code, issue.location, issue.detail);
        }
    }

    fn analyze_schemas(&mut self, resolvers: &SchemaResolverSet) {
        let mut analyzer = SchemaAnalyzer::new(resolvers);
        for (&node_id, node) in &self.nodes {
            analyzer.add_node(
                node_id,
                node.registry.protocol,
                &node.parameters,
                node.ports.keys().cloned(),
            );
        }
        for connection in self.document.connections.values() {
            if self
                .lookup_document_port(&connection.output)
                .is_some_and(|port| port.kind == PortKind::Data)
                && self
                    .lookup_document_port(&connection.input)
                    .is_some_and(|port| port.kind == PortKind::Data)
            {
                analyzer.add_connection(connection.output.clone(), connection.input.clone());
            }
        }
        let (facts, issues) = analyzer.analyze();
        self.schema_facts = facts;
        for issue in issues {
            self.push(issue.code, issue.location, issue.detail);
        }
    }

    fn lookup_document_port(&self, address: &PortAddress) -> Option<&ResolvedPort<PortAddress>> {
        if self.projection_only_ports.contains(address) {
            return None;
        }
        self.nodes.get(&address.node_id)?.ports.get(address)
    }
    fn port_spec(
        &self,
        address: &PortAddress,
        key: &crate::node_system::protocol::PortKey,
    ) -> Option<&PortSpec> {
        self.nodes
            .get(&address.node_id)?
            .registry
            .protocol
            .interface
            .ports
            .iter()
            .find(|port| &port.key == key)
    }
    fn push(
        &mut self,
        code: &'static str,
        location: DiagnosticLocation<NodeId, PortAddress, ConnectionId, Box<str>>,
        detail: String,
    ) {
        self.diagnostics.push(diagnostic(code, location, detail));
    }

    fn interface_projection(&self) -> ValidatedInterfaceProjection {
        ValidatedInterfaceProjection {
            basis: self.basis.clone(),
            nodes: self.interface_projections.clone(),
        }
    }

    fn snapshot(&self) -> CompilerAnalysis {
        let nodes = self
            .nodes
            .iter()
            .map(|(&node_id, node)| AnalyzedNode {
                node_id,
                node_type_id: node.registry.protocol.type_id.clone(),
                protocol_fingerprint: node.registry.protocol_fingerprint.clone(),
                normalized_parameters: node.parameters.clone(),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let resolved_interfaces = self
            .nodes
            .iter()
            .map(|(&node_id, node)| ResolvedInterface {
                node_id,
                ports: node
                    .ports
                    .values()
                    .cloned()
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        AnalysisSnapshot {
            basis: self.basis.clone(),
            nodes,
            resolved_interfaces,
            partial_types: self.type_facts.clone(),
            partial_schemas: self.schema_facts.clone(),
            diagnostics: self.diagnostics.clone().into_boxed_slice(),
        }
    }

    fn semantic_graph(&self) -> CompilerSemanticGraph {
        let nodes = self
            .nodes
            .iter()
            .map(|(&node_id, node)| ValidatedSemanticNode {
                node_id,
                node_type_id: node.registry.protocol.type_id.clone(),
                protocol_fingerprint: node.registry.protocol_fingerprint.clone(),
                normalized_parameters: node.parameters.clone(),
                ports: node
                    .ports
                    .keys()
                    .filter(|address| !self.projection_only_ports.contains(*address))
                    .map(|address| ValidatedSemanticPort {
                        address: address.clone(),
                        resolved_type: self.type_facts.get(address).cloned(),
                        resolved_schema: self.schema_facts.get(address).cloned(),
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let dependencies = self
            .document
            .connections
            .values()
            .map(|connection| {
                let kind = self
                    .lookup_document_port(&connection.output)
                    .map(|port| port.kind)
                    .unwrap_or(PortKind::Data);
                match kind {
                    PortKind::Data => SemanticDependency::Value(ValueEdge {
                        connection_id: connection.id,
                        source: connection.output.clone(),
                        target: connection.input.clone(),
                    }),
                    PortKind::Control => SemanticDependency::Control(ControlEdge {
                        connection_id: connection.id,
                        source_node: connection.output.node_id,
                        source_port: connection.output.clone(),
                        target_node: connection.input.node_id,
                        target_port: connection.input.clone(),
                    }),
                    PortKind::Effect => {
                        SemanticDependency::Effect(crate::node_system::analysis::EffectDependency {
                            predecessor: connection.output.node_id,
                            successor: connection.input.node_id,
                            effect_key: connection.id.to_string().into(),
                        })
                    }
                }
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        ValidatedSemanticGraph {
            basis: self.basis.clone(),
            nodes,
            dependencies,
        }
    }
}

type CompilerDiagnostic = NodeDiagnostic<NodeId, PortAddress, ConnectionId, Box<str>>;

enum LowerGraphFailure {
    Cancelled(CompileCancelled),
    Diagnostic(CompilerDiagnostic),
}

impl From<CompileCancelled> for LowerGraphFailure {
    fn from(error: CompileCancelled) -> Self {
        Self::Cancelled(error)
    }
}

impl From<CompilerDiagnostic> for LowerGraphFailure {
    fn from(diagnostic: CompilerDiagnostic) -> Self {
        Self::Diagnostic(diagnostic)
    }
}

#[derive(Clone)]
struct PendingOperation {
    node_id: NodeId,
    node_type_id: NodeTypeId,
    has_control_or_effect_ports: bool,
    kernel: PendingKernel,
    input_ports: Box<[PortAddress]>,
    inputs: Box<[PlannedInput]>,
    output_ports: Box<[PortAddress]>,
    outputs: Box<[PlannedOutput]>,
    parameters: CompiledParameterHandle,
    evaluation: EvaluationPolicy,
    purity: Purity,
    effects: EffectSemantics,
    resources: Box<[CompiledResourceRequirement]>,
}

#[derive(Clone)]
enum PendingKernel {
    Native(KernelHandle),
    Relational,
}

#[derive(Clone)]
struct PendingRelationalFragment {
    backend: RelationalBackendId,
    fragment: RelationalFragment,
    inputs: BTreeMap<PortAddress, crate::node_system::plan::RelationalOperatorIndex>,
}

fn function_target(
    parameters: &BTreeMap<crate::node_system::protocol::ParameterKey, serde_json::Value>,
) -> Option<&str> {
    ["target", "function_plan", "function"]
        .into_iter()
        .find_map(|name| {
            parameters
                .iter()
                .find(|(key, _)| key.as_str() == name)
                .and_then(|(_, value)| value.as_str())
        })
        .filter(|target| !target.is_empty() && target.trim() == *target)
}

fn allocate_port_values<R: CompilerRegistry>(
    registry: &R,
    graph: &CompilerSemanticGraph,
) -> Result<(u32, BTreeMap<PortAddress, ValueRef>), CompilerDiagnostic> {
    let mut next_value = 0u32;
    let mut values = BTreeMap::new();
    for node in graph.nodes.iter() {
        let resolved = resolve_for_lowering(registry, node)?;
        for port in node.ports.iter() {
            if protocol_port(resolved.protocol, &port.address).kind == PortKind::Data {
                values.insert(port.address.clone(), ValueRef::new(next_value));
                next_value += 1;
            }
        }
    }
    Ok((next_value, values))
}

fn derive_function_abi<R: CompilerRegistry>(
    registry: &R,
    graph: &CompilerSemanticGraph,
    projection: &ValidatedInterfaceProjection,
    provenance: &CompileProvenance,
) -> Result<Option<FunctionPlanAbi>, CompilerDiagnostic> {
    if !provenance.graph_path.0.starts_with("functions/") {
        return Ok(None);
    }
    let (_, values) = allocate_port_values(registry, graph)?;
    let mut parameters = BTreeMap::new();
    let mut results = BTreeMap::new();
    for node in graph.nodes.iter() {
        let resolved = resolve_for_lowering(registry, node)?;
        let destination = match resolved.structural_role() {
            Some(StructuralNodeRole::FunctionEntry) => &mut parameters,
            Some(StructuralNodeRole::FunctionReturn) => &mut results,
            _ => continue,
        };
        let Some(node_projection) = projection.nodes.get(&node.node_id) else {
            continue;
        };
        for port in node.ports.iter() {
            let Some(ProjectedDynamicPortBinding::Resolved { origin, .. }) =
                node_projection.projected_bindings.get(&port.address)
            else {
                continue;
            };
            let DynamicMemberLocator::FunctionParameter {
                function,
                parameter,
            } = origin
            else {
                continue;
            };
            if function != &provenance.graph_path {
                return Err(diagnostic(
                    "compiler.function.abi_target_mismatch",
                    DiagnosticLocation::Port(port.address.clone()),
                    format!(
                        "function ABI member targets '{}' instead of '{}'",
                        function.0, provenance.graph_path.0
                    ),
                ));
            }
            let value = values[&port.address];
            if destination.insert(parameter.clone(), value).is_some() {
                return Err(diagnostic(
                    "compiler.function.abi_member_duplicate",
                    DiagnosticLocation::Port(port.address.clone()),
                    format!("duplicate function ABI member '{}'", parameter.0),
                ));
            }
        }
    }
    Ok(Some(FunctionPlanAbi {
        provenance: provenance.clone(),
        parameters,
        results,
    }))
}

fn lower_graph<R: CompilerRegistry>(
    registry: &R,
    graph: &CompilerSemanticGraph,
    interface_projection: &ValidatedInterfaceProjection,
    function_abis: &BTreeMap<GraphResourcePath, FunctionPlanAbi>,
    provenance: CompileProvenance,
    cancellation: &CompileCancellationToken,
) -> Result<(ExecutionPlanBasis, ExecutionPlan), LowerGraphFailure> {
    cancellation.checkpoint()?;
    let (next_value, port_values) = allocate_port_values(registry, graph)?;
    let mut production_by_port = BTreeMap::new();
    let mut consumption_by_port = BTreeMap::new();
    let mut structural_inputs = BTreeSet::new();
    let mut structural_outputs = BTreeSet::new();
    let mut value_sources = BTreeSet::new();
    for node in graph.nodes.iter() {
        cancellation.checkpoint()?;
        let resolved = resolve_for_lowering(registry, node)?;
        let structural_role = resolved.structural_role();
        for port in node.ports.iter() {
            let spec = protocol_port(resolved.protocol, &port.address);
            if spec.kind == PortKind::Data {
                let value = port_values[&port.address];
                if let Some(role) = structural_role {
                    match spec.direction {
                        PortDirection::Input => {
                            structural_inputs.insert(port.address.clone());
                        }
                        PortDirection::Output => {
                            structural_outputs.insert(port.address.clone());
                            if matches!(
                                role,
                                StructuralNodeRole::EventBegin | StructuralNodeRole::FunctionEntry
                            ) {
                                value_sources.insert(PlanValueSource::ExternalInput(value));
                            }
                        }
                    }
                }
                match spec.direction {
                    PortDirection::Output => {
                        production_by_port.insert(
                            port.address.clone(),
                            spec.production
                                .unwrap_or(OutputProduction::FullyMaterialized),
                        );
                    }
                    PortDirection::Input => {
                        consumption_by_port.insert(
                            port.address.clone(),
                            spec.consumption
                                .unwrap_or(InputConsumption::FullyMaterialized),
                        );
                    }
                }
            }
        }
    }

    let mut pending_operations = Vec::new();
    let mut operation_by_node = BTreeMap::new();
    let mut operation_inputs = BTreeMap::new();
    let mut operation_outputs = BTreeMap::new();
    let mut relational_by_node = BTreeMap::new();
    let mut resources = BTreeMap::<_, CompiledResourceRequirement>::new();
    let mut results = BTreeMap::<Box<str>, PlanResult>::new();
    for node in graph.nodes.iter() {
        cancellation.checkpoint()?;
        let resolved = resolve_for_lowering(registry, node)?;
        if resolved.structural_role().is_some() {
            continue;
        }
        let implementation = resolved.implementation().ok_or_else(|| {
            diagnostic(
                "compiler.lowering.implementation_missing",
                DiagnosticLocation::Node(node.node_id),
                node.node_type_id.to_string(),
            )
        })?;
        let mut inputs = Vec::new();
        let mut planned_inputs = Vec::new();
        let mut outputs = Vec::new();
        let mut planned_outputs = Vec::new();
        for port in node.ports.iter() {
            let spec = protocol_port(resolved.protocol, &port.address);
            if spec.kind != PortKind::Data {
                continue;
            }
            let value = port_values[&port.address];
            match spec.direction {
                PortDirection::Output => {
                    outputs.push((port.address.clone(), value));
                    planned_outputs.push(PlannedOutput {
                        value,
                        production: spec
                            .production
                            .unwrap_or(OutputProduction::FullyMaterialized),
                    });
                }
                PortDirection::Input => {
                    inputs.push((port.address.clone(), value));
                    planned_inputs.push(PlannedInput {
                        value,
                        consumption: spec
                            .consumption
                            .unwrap_or(InputConsumption::FullyMaterialized),
                    });
                }
            }
        }
        let context = LoweringContext {
            cancellation,
            node_id: node.node_id,
            protocol: resolved.protocol,
            parameters: &node.normalized_parameters,
            inputs: &inputs,
            outputs: &outputs,
        };
        let lowered = implementation.lowerer.lower(&context).map_err(|error| {
            diagnostic(
                "compiler.lowering.failed",
                DiagnosticLocation::Node(node.node_id),
                error.to_string(),
            )
        })?;
        let mut owned_resources = BTreeMap::<ResourceId, CompiledResourceRequirement>::new();
        if let Some(metadata) = lowered.kernel.metadata() {
            if metadata.effect != resolved.protocol.execution.effects {
                return Err(diagnostic(
                    "compiler.lowering.effect_contract",
                    DiagnosticLocation::Node(node.node_id),
                    "lowered fragment effect metadata differs from the node protocol".to_string(),
                )
                .into());
            }
            for requirement in &metadata.resources {
                owned_resources.insert(requirement.resource.clone(), requirement.clone());
                if let Some(previous) =
                    resources.insert(requirement.resource.clone(), requirement.clone())
                {
                    if previous != *requirement {
                        return Err(diagnostic(
                            "compiler.lowering.resource_conflict",
                            DiagnosticLocation::Node(node.node_id),
                            requirement.resource.as_str().to_string(),
                        )
                        .into());
                    }
                }
            }
            for result in &metadata.results {
                let Some(&value) = port_values.get(&result.output) else {
                    return Err(diagnostic(
                        "compiler.lowering.result_port",
                        DiagnosticLocation::Node(node.node_id),
                        "fragment result references an unknown data output".to_string(),
                    )
                    .into());
                };
                if !outputs.iter().any(|(address, _)| address == &result.output) {
                    return Err(diagnostic(
                        "compiler.lowering.result_port",
                        DiagnosticLocation::Node(node.node_id),
                        "fragment result must reference an output of the lowered node".to_string(),
                    )
                    .into());
                }
                if results
                    .insert(
                        result.name.clone(),
                        PlanResult {
                            name: result.name.clone(),
                            output: GraphOutputRef {
                                graph_path: provenance.graph_path.clone(),
                                port: result.output.clone(),
                            },
                            value,
                        },
                    )
                    .is_some()
                {
                    return Err(diagnostic(
                        "compiler.lowering.result_duplicate",
                        DiagnosticLocation::Node(node.node_id),
                        result.name.to_string(),
                    )
                    .into());
                }
            }
        }

        for parameter in resolved.protocol.parameters.parameters.iter() {
            if parameter.editor != ParameterEditorSpec::Resource {
                continue;
            }
            let Some(resource) = node
                .normalized_parameters
                .get(&parameter.key)
                .and_then(serde_json::Value::as_str)
            else {
                continue;
            };
            let kind = match parameter.key.as_str() {
                "dataframe" => ResourceKind::DatabaseConnection,
                "variable" => ResourceKind::ExternalArtifact,
                "function" => continue,
                _ => continue,
            };
            let access = if node.node_type_id.as_str() == "yssbi.project.variable.set" {
                ResourceAccess::Exclusive
            } else {
                ResourceAccess::Shared
            };
            let resource = ResourceId::new(resource).map_err(|error| {
                diagnostic(
                    "compiler.lowering.resource_id",
                    DiagnosticLocation::Node(node.node_id),
                    error.to_string(),
                )
            })?;
            let requirement = CompiledResourceRequirement {
                resource: resource.clone(),
                kind,
                access,
                optional: false,
            };
            owned_resources.insert(resource.clone(), requirement.clone());
            if let Some(previous) = resources.insert(resource, requirement.clone()) {
                if previous != requirement {
                    return Err(diagnostic(
                        "compiler.lowering.resource_conflict",
                        DiagnosticLocation::Node(node.node_id),
                        requirement.resource.as_str().to_string(),
                    )
                    .into());
                }
            }
        }

        let operation = OperationIndex::new(pending_operations.len() as u32);
        operation_by_node.insert(node.node_id, operation);
        for (address, _) in &inputs {
            operation_inputs.insert(address.clone(), operation);
        }
        for (address, _) in &outputs {
            operation_outputs.insert(address.clone(), operation);
        }
        let kernel = match lowered.kernel {
            LoweredKernel::Native(handle) => PendingKernel::Native(handle),
            LoweredKernel::Scalar(fragment) => PendingKernel::Native(fragment.kernel),
            LoweredKernel::Kernel(fragment) => PendingKernel::Native(fragment.kernel),
            LoweredKernel::Relational(fragment) => {
                if relational_by_node
                    .insert(
                        node.node_id,
                        PendingRelationalFragment {
                            backend: fragment.backend,
                            fragment: fragment.fragment,
                            inputs: fragment
                                .inputs
                                .into_vec()
                                .into_iter()
                                .map(|binding| (binding.port, binding.operator))
                                .collect(),
                        },
                    )
                    .is_some()
                {
                    unreachable!("one lowering result per semantic node");
                }
                PendingKernel::Relational
            }
        };
        pending_operations.push(PendingOperation {
            node_id: node.node_id,
            node_type_id: node.node_type_id.clone(),
            has_control_or_effect_ports: resolved
                .protocol
                .interface
                .ports
                .iter()
                .any(|port| port.kind != PortKind::Data),
            kernel,
            input_ports: inputs
                .into_iter()
                .map(|(address, _)| address)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            inputs: planned_inputs.into_boxed_slice(),
            output_ports: outputs
                .into_iter()
                .map(|(address, _)| address)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            outputs: planned_outputs.into_boxed_slice(),
            parameters: lowered.parameters,
            evaluation: resolved.protocol.execution.evaluation,
            purity: resolved.protocol.execution.purity,
            effects: resolved.protocol.execution.effects,
            resources: owned_resources
                .into_values()
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        });
    }

    cancellation.checkpoint()?;
    let mut value_dependencies = Vec::new();
    let mut effect_dependencies = Vec::new();
    let mut relational_connections = Vec::new();
    for dependency in graph.dependencies.iter() {
        match dependency {
            SemanticDependency::Value(edge) => {
                let Some(&source) = port_values.get(&edge.source) else {
                    return Err(diagnostic(
                        "compiler.plan.value_producer_missing",
                        DiagnosticLocation::Connection(edge.connection_id),
                        "semantic value producer has no plan value".to_string(),
                    )
                    .into());
                };
                let Some(&destination) = port_values.get(&edge.target) else {
                    return Err(diagnostic(
                        "compiler.plan.value_consumer_missing",
                        DiagnosticLocation::Connection(edge.connection_id),
                        "semantic value consumer has no plan value".to_string(),
                    )
                    .into());
                };
                if !operation_outputs.contains_key(&edge.source)
                    && !structural_outputs.contains(&edge.source)
                {
                    return Err(diagnostic(
                        "compiler.plan.value_producer_missing",
                        DiagnosticLocation::Connection(edge.connection_id),
                        "semantic value producer does not lower to an operation output".to_string(),
                    )
                    .into());
                }
                if !operation_inputs.contains_key(&edge.target)
                    && !structural_inputs.contains(&edge.target)
                {
                    return Err(diagnostic(
                        "compiler.plan.value_consumer_missing",
                        DiagnosticLocation::Connection(edge.connection_id),
                        "semantic value consumer does not lower to an operation input".to_string(),
                    )
                    .into());
                }
                value_dependencies.push(ValueDependency {
                    source,
                    destination,
                });

                if let (Some(producer), Some(consumer)) = (
                    relational_by_node.get(&edge.source.node_id),
                    relational_by_node.get(&edge.target.node_id),
                ) {
                    let Some(&consumer_input) = consumer.inputs.get(&edge.target) else {
                        return Err(diagnostic(
                            "compiler.relational.input_binding_missing",
                            DiagnosticLocation::Connection(edge.connection_id),
                            "relational consumer did not bind its semantic input port".to_string(),
                        )
                        .into());
                    };
                    relational_connections.push(RelationalConnection {
                        producer: producer.fragment.id.clone(),
                        consumer: consumer.fragment.id.clone(),
                        consumer_input,
                        production: production_by_port[&edge.source],
                        consumption: consumption_by_port[&edge.target],
                    });
                }
            }
            SemanticDependency::Effect(edge) => {
                let Some(&before) = operation_by_node.get(&edge.predecessor) else {
                    return Err(diagnostic(
                        "compiler.plan.effect_producer_missing",
                        DiagnosticLocation::Node(edge.predecessor),
                        edge.effect_key.to_string(),
                    )
                    .into());
                };
                let Some(&after) = operation_by_node.get(&edge.successor) else {
                    return Err(diagnostic(
                        "compiler.plan.effect_consumer_missing",
                        DiagnosticLocation::Node(edge.successor),
                        edge.effect_key.to_string(),
                    )
                    .into());
                };
                effect_dependencies.push(PlannedEffectDependency { before, after });
            }
            SemanticDependency::Control(_) => {}
        }
    }
    value_dependencies.sort_by_key(|dependency| (dependency.source, dependency.destination));
    value_dependencies.dedup();
    effect_dependencies.sort_by_key(|dependency| (dependency.before, dependency.after));
    effect_dependencies.dedup();

    cancellation.checkpoint()?;
    let mut port_facts = BTreeMap::new();
    let mut nodes = BTreeSet::new();
    let mut output_results = BTreeMap::new();
    for node in graph.nodes.iter() {
        nodes.insert(node.node_id);
        let resolved = resolve_for_lowering(registry, node)?;
        for port in node.ports.iter() {
            let spec = protocol_port(resolved.protocol, &port.address);
            port_facts.insert(
                port.address.clone(),
                DemandPortFact {
                    kind: spec.kind,
                    direction: spec.direction,
                },
            );
            if spec.kind == PortKind::Data && spec.direction == PortDirection::Output {
                let output = GraphOutputRef {
                    graph_path: provenance.graph_path.clone(),
                    port: port.address.clone(),
                };
                output_results.insert(
                    output.clone(),
                    PlanResult {
                        name: format!("requested.{}", port.address).into(),
                        output,
                        value: port_values[&port.address],
                    },
                );
            }
        }
    }
    let default_outputs = results
        .values()
        .map(|result| result.output.clone())
        .collect::<BTreeSet<_>>();
    for result in results.values() {
        output_results.insert(result.output.clone(), result.clone());
    }
    let control_nodes = graph
        .nodes
        .iter()
        .map(|node| {
            let resolved = resolve_for_lowering(registry, node)?;
            Ok((
                node.node_id,
                ControlNode {
                    node_id: node.node_id,
                    role: resolved.structural_role(),
                    protocol: resolved.protocol,
                    parameters: &node.normalized_parameters,
                    ports: node
                        .ports
                        .iter()
                        .map(|port| port.address.clone())
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    values: node
                        .ports
                        .iter()
                        .filter_map(|port| {
                            port_values
                                .get(&port.address)
                                .copied()
                                .map(|value| (port.address.clone(), value))
                        })
                        .collect(),
                    dynamic_members: interface_projection
                        .nodes
                        .get(&node.node_id)
                        .into_iter()
                        .flat_map(|projection| projection.projected_bindings.iter())
                        .filter_map(|(address, binding)| match binding {
                            ProjectedDynamicPortBinding::Resolved { origin, .. } => {
                                Some((address.clone(), origin.clone()))
                            }
                            ProjectedDynamicPortBinding::Orphan { .. } => None,
                        })
                        .collect(),
                    operation: operation_by_node.get(&node.node_id).copied(),
                },
            ))
        })
        .collect::<Result<BTreeMap<_, _>, NodeDiagnostic<_, _, _, _>>>()?;
    let control_edges = graph
        .dependencies
        .iter()
        .filter_map(|dependency| match dependency {
            SemanticDependency::Control(edge) => Some(RegionControlEdge {
                source: edge.source_port.clone(),
                target: edge.target_port.clone(),
            }),
            _ => None,
        })
        .collect();
    cancellation.checkpoint()?;
    let mut root_region = build_control_region(control_nodes, control_edges, function_abis)
        .map_err(|issue| {
            diagnostic(
                issue.code,
                issue
                    .node_id
                    .map(DiagnosticLocation::Node)
                    .unwrap_or(DiagnosticLocation::Graph),
                issue.detail,
            )
        })?;
    deduplicate_region_operations(&mut root_region);
    collect_control_value_sources(&root_region, &mut value_sources);
    debug_assert_eq!(provenance.basis, graph.basis);
    let operations = pending_operations
        .into_iter()
        .map(|pending| {
            let kernel = match pending.kernel {
                PendingKernel::Native(handle) => IntermediateKernel::Native(handle),
                PendingKernel::Relational => {
                    let relational = relational_by_node
                        .remove(&pending.node_id)
                        .expect("relational lowering fact belongs to its operation");
                    IntermediateKernel::Relational {
                        backend: relational.backend,
                        fragment: relational.fragment,
                        input_bindings: relational.inputs,
                    }
                }
            };
            IntermediateOperation {
                source_node_id: pending.node_id,
                source_node_type_id: pending.node_type_id,
                has_control_or_effect_ports: pending.has_control_or_effect_ports,
                kernel,
                input_ports: pending.input_ports,
                inputs: pending.inputs,
                output_ports: pending.output_ports,
                outputs: pending.outputs,
                params: pending.parameters,
                evaluation: pending.evaluation,
                purity: pending.purity,
                effects: pending.effects,
                resources: pending.resources,
            }
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let basis = ExecutionPlanBasis {
        provenance,
        value_count: next_value,
        operations,
        value_sources: value_sources
            .into_iter()
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        value_dependencies: value_dependencies.into_boxed_slice(),
        effect_dependencies: effect_dependencies
            .into_iter()
            .map(|dependency| (dependency.before.index(), dependency.after.index()))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        root_region,
        relational_connections: relational_connections.into_boxed_slice(),
        port_facts,
        nodes,
        output_results,
        default_outputs,
    };
    cancellation.checkpoint()?;
    let plan = basis.derive_full_plan().map_err(|error| {
        diagnostic(
            "compiler.plan.invalid",
            DiagnosticLocation::Graph,
            error.to_string(),
        )
    })?;
    Ok((basis, plan))
}

fn deduplicate_region_operations(region: &mut StructuredControlRegion) {
    match region {
        StructuredControlRegion::Sequence(steps) => {
            let mut seen = BTreeSet::new();
            let mut deduplicated = Vec::with_capacity(steps.len());
            for mut step in std::mem::take(steps).into_vec() {
                match &mut step {
                    ControlStep::Operation(operation) if !seen.insert(*operation) => continue,
                    ControlStep::Region(child) => deduplicate_region_operations(child),
                    ControlStep::Operation(_) => {}
                }
                deduplicated.push(step);
            }
            *steps = deduplicated.into_boxed_slice();
        }
        StructuredControlRegion::If {
            then_region,
            else_region,
            ..
        } => {
            deduplicate_region_operations(then_region);
            deduplicate_region_operations(else_region);
        }
        StructuredControlRegion::Loop { body, .. } => deduplicate_region_operations(body),
        StructuredControlRegion::Call { .. } => {}
    }
}

fn collect_control_value_sources(
    region: &StructuredControlRegion,
    sources: &mut BTreeSet<PlanValueSource>,
) {
    match region {
        StructuredControlRegion::Sequence(steps) => {
            for step in steps {
                if let crate::node_system::plan::ControlStep::Region(region) = step {
                    collect_control_value_sources(region, sources);
                }
            }
        }
        StructuredControlRegion::If {
            then_region,
            else_region,
            results,
            ..
        } => {
            sources.extend(
                results
                    .iter()
                    .map(|binding| PlanValueSource::ControlProduced(binding.destination)),
            );
            collect_control_value_sources(then_region, sources);
            collect_control_value_sources(else_region, sources);
        }
        StructuredControlRegion::Loop { body, carried, .. } => {
            for binding in carried {
                sources.insert(PlanValueSource::ControlProduced(binding.body_input));
                sources.insert(PlanValueSource::ControlProduced(binding.result));
            }
            collect_control_value_sources(body, sources);
        }
        StructuredControlRegion::Call { results, .. } => {
            sources.extend(
                results
                    .iter()
                    .map(|binding| PlanValueSource::ControlProduced(binding.caller_destination)),
            );
        }
    }
}

fn resolve_for_lowering<'a, R: CompilerRegistry>(
    registry: &'a R,
    node: &ValidatedSemanticNode<
        NodeId,
        PortAddress,
        serde_json::Value,
        TypeExpr,
        crate::node_system::protocol::SchemaExpr,
    >,
) -> Result<RegistryNode<'a>, NodeDiagnostic<NodeId, PortAddress, ConnectionId, Box<str>>> {
    registry.resolve(&node.node_type_id).ok_or_else(|| {
        diagnostic(
            "compiler.node.disappeared",
            DiagnosticLocation::Node(node.node_id),
            node.node_type_id.to_string(),
        )
    })
}

fn protocol_port<'a>(protocol: &'a NodeProtocol, address: &PortAddress) -> &'a PortSpec {
    protocol
        .interface
        .ports
        .iter()
        .find(|spec| port_template(address) == &spec.key)
        .expect("validated semantic port has protocol spec")
}

fn port_template(address: &PortAddress) -> &crate::node_system::protocol::PortKey {
    match &address.port {
        PortRef::Declared { key } => key,
        PortRef::Instance { template, .. } => template,
    }
}

fn append_diagnostic(
    diagnostics: Box<[NodeDiagnostic<NodeId, PortAddress, ConnectionId, Box<str>>]>,
    value: NodeDiagnostic<NodeId, PortAddress, ConnectionId, Box<str>>,
) -> Box<[NodeDiagnostic<NodeId, PortAddress, ConnectionId, Box<str>>]> {
    let mut values = diagnostics.into_vec();
    values.push(value);
    values.sort_by_key(diagnostic_sort_key);
    values.into_boxed_slice()
}
fn diagnostic(
    code: &'static str,
    primary: DiagnosticLocation<NodeId, PortAddress, ConnectionId, Box<str>>,
    detail: String,
) -> NodeDiagnostic<NodeId, PortAddress, ConnectionId, Box<str>> {
    NodeDiagnostic {
        code: DiagnosticCode::new(code),
        message_key: I18nKey::new(format!("diagnostics.{code}"))
            .expect("compiler diagnostic keys are valid"),
        arguments: BTreeMap::from([(Box::from("detail"), detail.into_boxed_str())]),
        severity: DiagnosticSeverity::Error,
        primary,
        related: Box::new([]),
    }
}
fn diagnostic_sort_key(
    value: &NodeDiagnostic<NodeId, PortAddress, ConnectionId, Box<str>>,
) -> (String, String) {
    (
        match &value.primary {
            DiagnosticLocation::Graph => "0".into(),
            DiagnosticLocation::Node(id) => format!("1:{id}"),
            DiagnosticLocation::Port(address) => format!("2:{address}"),
            DiagnosticLocation::Connection(id) => format!("3:{id}"),
            DiagnosticLocation::Parameter { node_id, key } => format!("4:{node_id}:{key}"),
            DiagnosticLocation::Resource(id) => format!("5:{id}"),
        },
        value.code.as_str().into(),
    )
}
