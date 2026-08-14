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
use super::reroute::collapse_transparent_nodes;
use super::schema_analysis::{SchemaAnalysisIssue, SchemaAnalyzer, SchemaResolverSet};
use super::specialization::{
    DemandPortFact, ExecutionPlanBasis, IntermediateKernel, IntermediateOperation,
};
use super::type_analysis::{TypeConstraintGraph, TypeEnvironment};
use super::{
    CompilerDiagnostic, CompilerDiagnosticLocation, CompilerNodeDiagnostic, FragmentMetadata,
    LoweredKernel, LoweringContext, LoweringError, NodeImplementation, ValidatedNodeConfig,
    compare_diagnostics, managed_node_role_name, node_scope_name, port_kind_name,
};
use crate::node_system::analysis::{
    AnalysisResourceReads, AnalysisResourceResolver, AnalysisSnapshot, AnalyzedNode,
    CompilationBasis, CompileId, CompileProvenance, ControlEdge, CorrelationContext,
    DiagnosticLocation, NOOP_TRACE_SINK, NodeDiagnostic, ProjectSessionId, ResolvedDatabase,
    ResolvedFunction, ResolvedFunctionValue, ResolvedInterface, ResolvedPort, ResolvedResource,
    ResolvedVariable, ResourceKey, ResourceObservationSet, ResourceObservedState,
    ResourceResolutionError, ResourceVersion, ResourceVersionSet, SemanticDependency, SpanId,
    SpanKind, SpanOutcome, SpanSpec, TraceSink, ValidatedSemanticGraph, ValidatedSemanticNode,
    ValidatedSemanticPort, ValueEdge, start_span_safely,
};
use crate::node_system::document::{
    ConnectionId, DynamicMemberLocator, DynamicPortBinding, FunctionDocument, FunctionParameterId,
    GraphDocument, GraphResourcePath, GraphRevision, NodeId, PortAddress, PortRef,
};
use crate::node_system::plan::{
    CompiledParameterHandle, CompiledResourceRequirement, ControlStep,
    EXECUTION_SEMANTICS_SCHEMA_VERSION, EffectDependency as PlannedEffectDependency, ExecutionPlan,
    ExecutionSemanticsVersion, FunctionPlanAbi, GraphOutputRef, KernelHandle, OperationIndex,
    OperationStableId, PlanResult, PlanValueSource, PlannedInput, PlannedOutput, PlannedRetry,
    RelationalBackendId, ResourceAccess, ResourceId, ResourceKind, StructuredControlRegion,
    ValueDependency, ValueRef, WorkloadClass,
};
use crate::node_system::protocol::{
    CachePolicy, ConnectionsPerPort, Determinism, EffectSemantics, EvaluationPolicy,
    InputConsumption, LiteralPolicy, NodeProtocol, NodeTypeId, OutputProduction,
    ParameterEditorSpec, ParameterIssueKind, PortDirection, PortInstances, PortKind, PortSpec,
    Purity, ResolvedSchemaFact, RetryPolicy, SchemaExpr, TypeClassId, TypeConstructorId, TypeExpr,
    TypeId, Value, validate_and_prepare_parameter_values,
};
use crate::node_system::registry::{
    NodeRegistry, PreparedNominalValue, ProtocolFingerprint, RegistryFingerprint,
    StructuralNodeRole, TransparentNodeRole, hash_canonical,
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
    ProtocolOnly,
    Structural(StructuralNodeRole),
    Transparent(TransparentNodeRole),
}

impl RegistryNode<'_> {
    fn implementation(&self) -> Option<&NodeImplementation> {
        match self.behavior {
            RegistryNodeBehavior::Leaf(implementation) => Some(implementation),
            RegistryNodeBehavior::ProtocolOnly
            | RegistryNodeBehavior::Structural(_)
            | RegistryNodeBehavior::Transparent(_) => None,
        }
    }

    fn structural_role(&self) -> Option<StructuralNodeRole> {
        match self.behavior {
            RegistryNodeBehavior::Leaf(_)
            | RegistryNodeBehavior::ProtocolOnly
            | RegistryNodeBehavior::Transparent(_) => None,
            RegistryNodeBehavior::Structural(role) => Some(role),
        }
    }
}

/// The compiler registry resolves nodes and supplies the type facts required by analysis.
struct CompilerNominalValidator<'a, R>(&'a R);

impl<R: CompilerRegistry> crate::node_system::protocol::NominalParameterValidator
    for CompilerNominalValidator<'_, R>
{
    fn validate_nominal_parameter(
        &self,
        type_id: &TypeId,
        value: &serde_json::Value,
    ) -> Option<Result<(), String>> {
        self.0.validate_nominal_parameter(type_id, value)
    }
}

pub trait CompilerRegistry: TypeEnvironment {
    fn fingerprint(&self) -> &RegistryFingerprint;
    fn resolve(&self, node_type: &NodeTypeId) -> Option<RegistryNode<'_>>;

    fn validate_nominal_parameter(
        &self,
        _type_id: &TypeId,
        _value: &serde_json::Value,
    ) -> Option<Result<(), String>> {
        None
    }

    fn prepare_nominal_parameter(
        &self,
        _type_id: &TypeId,
        _value: &serde_json::Value,
    ) -> Option<Result<crate::node_system::registry::PreparedNominalValue, String>> {
        None
    }
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
        let behavior = match (
            registered.implementation(),
            registered.structural_role(),
            registered.transparent_role(),
        ) {
            (Some(implementation), None, None) => RegistryNodeBehavior::Leaf(
                implementation
                    .as_any()
                    .downcast_ref::<NodeImplementation>()
                    .expect("registry freeze guarantees compiler lowering capability"),
            ),
            (None, None, None) => RegistryNodeBehavior::ProtocolOnly,
            (None, Some(role), None) => RegistryNodeBehavior::Structural(role),
            (None, None, Some(role)) => RegistryNodeBehavior::Transparent(role),
            _ => unreachable!("registry freeze guarantees one validated node behavior"),
        };
        Some(RegistryNode {
            protocol: registered.protocol(),
            protocol_fingerprint: self
                .catalog_manifest()
                .node_protocols
                .get(node_type)?
                .clone(),
            behavior,
        })
    }

    fn validate_nominal_parameter(
        &self,
        type_id: &TypeId,
        value: &serde_json::Value,
    ) -> Option<Result<(), String>> {
        NodeRegistry::validate_nominal_parameter(self, type_id, value)
    }

    fn prepare_nominal_parameter(
        &self,
        type_id: &TypeId,
        value: &serde_json::Value,
    ) -> Option<Result<crate::node_system::registry::PreparedNominalValue, String>> {
        NodeRegistry::prepare_nominal_parameter(self, type_id, value)
    }
}

pub trait ResourceSnapshot {
    fn versions(&self) -> ResourceVersionSet;

    fn version(&self, key: &ResourceKey) -> Option<ResourceVersion> {
        self.versions().remove(key)
    }

    fn observed_state(&self, key: &ResourceKey) -> ResourceObservedState {
        self.version(key)
            .map(ResourceObservedState::Present)
            .unwrap_or(ResourceObservedState::Absent(None))
    }

    fn function_document(&self, _path: &GraphResourcePath) -> Option<&FunctionDocument> {
        None
    }

    fn function_graph_document(&self, _path: &GraphResourcePath) -> Option<&GraphDocument> {
        None
    }

    fn variable(
        &self,
        _id: &crate::variable::VariableId,
    ) -> Option<&crate::variable::VariableInstance> {
        None
    }

    fn database_schema(&self, _id: &str) -> Option<&[crate::schema::ColumnInfoDTO]> {
        None
    }
}

struct TrackedResourceResolver<'a, S> {
    snapshot: &'a S,
    reads: AnalysisResourceReads,
    observations: ResourceObservationSet,
}

impl<'a, S> TrackedResourceResolver<'a, S> {
    fn new(snapshot: &'a S) -> Self {
        Self {
            snapshot,
            reads: AnalysisResourceReads::new(),
            observations: ResourceObservationSet::new(),
        }
    }
}

impl<S: ResourceSnapshot> TrackedResourceResolver<'_, S> {
    fn failure(
        &mut self,
        key: ResourceKey,
        state: ResourceObservedState,
        reason: impl Into<Box<str>>,
    ) -> ResourceResolutionError {
        let reason = reason.into();
        self.observations.insert(key.clone(), state.clone());
        ResourceResolutionError::new(key, state, reason)
    }

    fn successful(&mut self, key: ResourceKey, version: ResourceVersion) {
        self.observations.remove(&key);
        self.reads.insert(key, version);
    }
}

impl<S: ResourceSnapshot> AnalysisResourceResolver for TrackedResourceResolver<'_, S> {
    fn resolve_function(
        &mut self,
        path: &GraphResourcePath,
    ) -> Result<ResolvedFunction<'_>, ResourceResolutionError> {
        let key = ResourceKey::new(path.0.clone());
        let state = self.snapshot.observed_state(&key);
        let ResourceObservedState::Present(version) = state.clone() else {
            return Err(self.failure(
                key,
                state,
                format!("function resource '{}' is missing", path.0),
            ));
        };
        let Some(function) = self.snapshot.function_document(path) else {
            return Err(self.failure(
                key,
                state,
                format!("function resource '{}' has no signature", path.0),
            ));
        };
        let Some(graph) = self.snapshot.function_graph_document(path) else {
            return Err(self.failure(
                key,
                state,
                format!("function graph '{}' is missing", path.0),
            ));
        };
        self.successful(key.clone(), version.clone());
        Ok(ResolvedResource {
            key,
            version,
            value: ResolvedFunctionValue { function, graph },
        })
    }

    fn resolve_variable(
        &mut self,
        id: &crate::variable::VariableId,
    ) -> Result<ResolvedVariable<'_>, ResourceResolutionError> {
        let key = ResourceKey::new(format!("variables/{id}"));
        let state = self.snapshot.observed_state(&key);
        let ResourceObservedState::Present(version) = state.clone() else {
            return Err(self.failure(key, state, format!("variable resource '{id}' is missing")));
        };
        let Some(value) = self.snapshot.variable(id) else {
            return Err(self.failure(key, state, format!("variable resource '{id}' has no value")));
        };
        self.successful(key.clone(), version.clone());
        Ok(ResolvedResource {
            key,
            version,
            value,
        })
    }

    fn resolve_database(
        &mut self,
        id: &str,
    ) -> Result<ResolvedDatabase<'_>, ResourceResolutionError> {
        let key = ResourceKey::new(format!("databases/{id}"));
        let state = self.snapshot.observed_state(&key);
        let ResourceObservedState::Present(version) = state.clone() else {
            return Err(self.failure(key, state, format!("database resource '{id}' is missing")));
        };
        let Some(value) = self.snapshot.database_schema(id) else {
            return Err(self.failure(
                key,
                state,
                format!("database resource '{id}' has no schema"),
            ));
        };
        self.successful(key.clone(), version.clone());
        Ok(ResolvedResource {
            key,
            version,
            value,
        })
    }

    fn reads(&self) -> &AnalysisResourceReads {
        &self.reads
    }

    fn observations(&self) -> &ResourceObservationSet {
        &self.observations
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompilationSnapshot {
    pub provenance: CompileProvenance,
    pub document: GraphDocument,
    pub trace_span_id: SpanId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilationStage {
    Analysis,
    Lowering,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternalCompilationFailure {
    pub stage: CompilationStage,
    pub code: Box<str>,
    pub node_id: Option<NodeId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompilationOutcome {
    Succeeded,
    AnalysisBlocked,
    InternalFailure(InternalCompilationFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedCompileAnalysis {
    pub analysis: CompilerAnalysis,
    pub semantic: Option<CompilerSemanticGraph>,
    pub outcome: CompilationOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileResult {
    pub analysis: CompilerAnalysis,
    pub interface_projection: ValidatedInterfaceProjection,
    pub semantic: Option<CompilerSemanticGraph>,
    pub execution_basis: Option<ExecutionPlanBasis>,
    pub plan: Option<ExecutionPlan>,
    pub function_abi: Option<FunctionPlanAbi>,
    pub outcome: CompilationOutcome,
}

fn finalize_resource_basis(
    mut result: CompileResult,
    resources: &dyn AnalysisResourceResolver,
) -> CompileResult {
    let versions = resources.reads().clone();
    let observations = resources.observations().clone();
    let apply = |basis: &mut CompilationBasis<GraphRevision>| {
        basis.resource_versions = versions.clone();
        basis.resource_observations = observations.clone();
    };
    apply(&mut result.analysis.basis);
    apply(&mut result.interface_projection.basis);
    if let Some(semantic) = result.semantic.as_mut() {
        apply(&mut semantic.basis);
    }
    if let Some(execution_basis) = result.execution_basis.as_mut() {
        apply(&mut execution_basis.provenance.basis);
    }
    if let Some(plan) = result.plan.as_mut() {
        apply(&mut plan.provenance.basis);
    }
    if let Some(abi) = result.function_abi.as_mut() {
        apply(&mut abi.provenance.basis);
    }
    result
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
                resource_versions: ResourceVersionSet::new(),
                resource_observations: ResourceObservationSet::new(),
            },
            compile_id,
        };
        let correlation = CorrelationContext::compile(&provenance);
        let mut span = start_span_safely(
            self.trace,
            SpanSpec {
                parent_span_id: None,
                run_id: None,
                operation_id: None,
                activation_id: None,
                attempt_id: None,
                kind: SpanKind::Snapshot,
                correlation,
            },
        );
        let trace_span_id = span.span_id();
        let snapshot = CompilationSnapshot {
            provenance,
            document: document.clone(),
            trace_span_id,
        };
        span.finish(SpanOutcome::Success);
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
        let mut analysis_span = start_span_safely(
            self.trace,
            SpanSpec {
                parent_span_id: Some(snapshot.trace_span_id),
                run_id: None,
                operation_id: None,
                activation_id: None,
                attempt_id: None,
                kind: SpanKind::Analysis,
                correlation: correlation.clone(),
            },
        );
        if let Err(error) = cancellation.checkpoint() {
            analysis_span.finish(SpanOutcome::Cancellation);
            return Err(error);
        }
        let mut resources = TrackedResourceResolver::new(self.resources);
        let mut state = AnalysisState::new(
            &snapshot.document,
            snapshot.provenance.graph_path.clone(),
            snapshot.provenance.basis.clone(),
        );
        if let Err(error) = state.analyze(
            self.registry,
            &self.schema_resolvers,
            &self.interface_resolvers,
            &mut resources,
            cancellation,
        ) {
            analysis_span.finish(SpanOutcome::Cancellation);
            return Err(error);
        }
        let prepared_configs = state.prepared_configs();
        let provisional_semantic = state.semantic_graph();
        let interface_projection = state.interface_projection();
        let mut function_abi = match derive_function_abi(
            self.registry,
            &provisional_semantic,
            &interface_projection,
            &snapshot.provenance,
        ) {
            Ok(abi) => abi,
            Err(diagnostic) => {
                state.diagnostics.push(diagnostic);
                None
            }
        };
        let closure =
            match self.function_abis_for_calls(&provisional_semantic, &mut resources, cancellation)
            {
                Ok(closure) => closure,
                Err(error) => {
                    analysis_span.finish(SpanOutcome::Cancellation);
                    return Err(error);
                }
            };
        state.diagnostics.extend(closure.diagnostics);
        let mut function_abis = closure.abis;
        state.basis.resource_versions = resources.reads().clone();
        state.basis.resource_observations = resources.observations().clone();
        let decoded_literals = state.decoded_literals.clone();
        let mut analysis = state.snapshot();
        if let Err(error) = cancellation.checkpoint() {
            analysis_span.finish(SpanOutcome::Cancellation);
            return Err(error);
        }
        if analysis.has_blocking_errors() {
            analysis_span.finish(SpanOutcome::Error);
            return Ok(finalize_resource_basis(
                CompileResult {
                    analysis,
                    interface_projection,
                    semantic: None,
                    execution_basis: None,
                    plan: None,
                    function_abi: None,
                    outcome: CompilationOutcome::AnalysisBlocked,
                },
                &resources,
            ));
        }

        let semantic = state.semantic_graph();
        let semantic = match analysis.validated(semantic) {
            Ok(graph) => graph,
            Err(_) => {
                analysis.diagnostics = append_diagnostic(
                    analysis.diagnostics,
                    CompilerDiagnostic::SemanticInvalid {}.into_node(DiagnosticLocation::Graph),
                );
                analysis_span.finish(SpanOutcome::Error);
                return Ok(finalize_resource_basis(
                    CompileResult {
                        analysis,
                        interface_projection,
                        semantic: None,
                        execution_basis: None,
                        plan: None,
                        function_abi: None,
                        outcome: CompilationOutcome::InternalFailure(InternalCompilationFailure {
                            stage: CompilationStage::Analysis,
                            code: CompilerDiagnostic::SemanticInvalid {}
                                .definition()
                                .code
                                .into(),
                            node_id: None,
                        }),
                    },
                    &resources,
                ));
            }
        };
        let mut semantic = semantic_for_lowering(self.registry, semantic);
        analysis_span.finish(SpanOutcome::Success);
        let mut lowering_span = start_span_safely(
            self.trace,
            SpanSpec {
                parent_span_id: Some(snapshot.trace_span_id),
                run_id: None,
                operation_id: None,
                activation_id: None,
                attempt_id: None,
                kind: SpanKind::Lowering,
                correlation: correlation.clone(),
            },
        );

        let final_versions = resources.reads().clone();
        let final_observations = resources.observations().clone();
        analysis.basis.resource_versions = final_versions.clone();
        analysis.basis.resource_observations = final_observations.clone();
        semantic.basis.resource_versions = final_versions.clone();
        semantic.basis.resource_observations = final_observations.clone();
        let mut final_provenance = snapshot.provenance.clone();
        final_provenance.basis.resource_versions = final_versions;
        final_provenance.basis.resource_observations = final_observations;
        if let Some(abi) = function_abi.as_mut() {
            abi.provenance = final_provenance.clone();
        }
        for abi in function_abis.values_mut() {
            abi.provenance.basis.resource_versions =
                final_provenance.basis.resource_versions.clone();
            abi.provenance.basis.resource_observations =
                final_provenance.basis.resource_observations.clone();
        }
        let (semantic, execution_basis, plan, outcome) = match lower_graph(
            self.registry,
            &snapshot.document,
            &semantic,
            &prepared_configs,
            &decoded_literals,
            &interface_projection,
            &function_abis,
            final_provenance,
            cancellation,
        ) {
            Ok((basis, plan)) => {
                let abi_finalized = function_abi
                    .as_mut()
                    .map(|abi| finalize_function_abi_productions(&plan, abi))
                    .transpose();
                if abi_finalized.is_err() {
                    lowering_span.finish(SpanOutcome::Error);
                    (
                        Some(semantic),
                        None,
                        None,
                        CompilationOutcome::InternalFailure(InternalCompilationFailure {
                            stage: CompilationStage::Lowering,
                            code: CompilerDiagnostic::PlanInvalid {}.definition().code.into(),
                            node_id: None,
                        }),
                    )
                } else {
                    lowering_span.finish(SpanOutcome::Success);
                    (
                        Some(semantic),
                        Some(basis),
                        Some(plan),
                        CompilationOutcome::Succeeded,
                    )
                }
            }
            Err(LowerGraphFailure::Cancelled(error)) => {
                lowering_span.finish(SpanOutcome::Cancellation);
                return Err(error);
            }
            Err(LowerGraphFailure::Internal(failure)) => {
                lowering_span.finish(SpanOutcome::Error);
                (
                    Some(semantic),
                    None,
                    None,
                    CompilationOutcome::InternalFailure(failure),
                )
            }
        };
        Ok(finalize_resource_basis(
            CompileResult {
                analysis,
                interface_projection,
                semantic,
                execution_basis,
                plan,
                function_abi,
                outcome,
            },
            &resources,
        ))
    }

    fn function_abis_for_calls(
        &self,
        graph: &CompilerSemanticGraph,
        resources: &mut dyn AnalysisResourceResolver,
        cancellation: &CompileCancellationToken,
    ) -> Result<CallClosureAnalysis, CompileCancelled> {
        let mut closure = CallClosureAnalysis::default();
        let root_targets = self.call_targets(graph, &mut closure.diagnostics);
        let mut pending = root_targets.keys().cloned().collect::<Vec<_>>();
        let mut dependencies = BTreeMap::new();
        let mut direct_resolution_failures = BTreeSet::new();

        while let Some(target) = pending.pop() {
            cancellation.checkpoint()?;
            if dependencies.contains_key(&target) {
                continue;
            }
            let root_sites = root_targets.get(&target).map(Vec::as_slice);
            let (node, resolution_failed) = self.analyze_call_dependency(
                &target,
                root_sites,
                resources,
                cancellation,
                &mut closure.diagnostics,
            )?;
            if resolution_failed {
                direct_resolution_failures.insert(target.clone());
            }
            pending.extend(node.dependencies.iter().cloned());
            dependencies.insert(target, node);
        }

        let initially_invalid = invalid_call_targets(&dependencies);
        self.finalize_call_dependency_abis(
            &mut dependencies,
            &initially_invalid,
            resources,
            cancellation,
        )?;
        let invalid = invalid_call_targets(&dependencies);
        for (target, node) in dependencies {
            if !invalid.contains(&target)
                && let Some(abi) = node.abi
            {
                closure.abis.insert(target, abi);
            }
        }
        for (target, sites) in root_targets {
            if invalid.contains(&target) && !direct_resolution_failures.contains(&target) {
                self.push_call_abi_invalid(&target, &sites, &mut closure.diagnostics);
            }
        }
        Ok(closure)
    }

    fn finalize_call_dependency_abis(
        &self,
        dependencies: &mut BTreeMap<GraphResourcePath, CallDependencyNode>,
        initially_invalid: &BTreeSet<GraphResourcePath>,
        resources: &dyn AnalysisResourceResolver,
        cancellation: &CompileCancellationToken,
    ) -> Result<(), CompileCancelled> {
        let final_versions = resources.reads().clone();
        let final_observations = resources.observations().clone();
        for (target, node) in dependencies.iter_mut() {
            if initially_invalid.contains(target) {
                continue;
            }
            if let Some(abi) = node.abi.as_mut() {
                abi.provenance.basis.resource_versions = final_versions.clone();
                abi.provenance.basis.resource_observations = final_observations.clone();
            }
            if let Some(lowering) = node.lowering.as_mut() {
                lowering.provenance.basis.resource_versions = final_versions.clone();
                lowering.provenance.basis.resource_observations = final_observations.clone();
                lowering.semantic.basis.resource_versions = final_versions.clone();
                lowering.semantic.basis.resource_observations = final_observations.clone();
            }
        }

        let components = strongly_connected_call_components(dependencies);
        for component in components.into_iter().rev() {
            cancellation.checkpoint()?;
            let invalid = invalid_call_targets(dependencies);
            if component.iter().any(|target| invalid.contains(target)) {
                continue;
            }
            match self.finalize_call_component(dependencies, &component, cancellation) {
                Ok(()) => {}
                Err(CallAbiFinalizationFailure::Cancelled(error)) => return Err(error),
                Err(CallAbiFinalizationFailure::Invalid) => {
                    for target in component {
                        if let Some(node) = dependencies.get_mut(&target) {
                            node.abi = None;
                            node.locally_valid = false;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn finalize_call_component(
        &self,
        dependencies: &mut BTreeMap<GraphResourcePath, CallDependencyNode>,
        component: &[GraphResourcePath],
        cancellation: &CompileCancellationToken,
    ) -> Result<(), CallAbiFinalizationFailure> {
        const MAX_ITERATIONS: usize = 32;
        let mut seen = BTreeSet::new();
        for _ in 0..MAX_ITERATIONS {
            cancellation
                .checkpoint()
                .map_err(CallAbiFinalizationFailure::Cancelled)?;
            let state = component
                .iter()
                .map(|target| {
                    (
                        target.clone(),
                        dependencies[target]
                            .abi
                            .as_ref()
                            .map(|abi| abi.result_productions.clone()),
                    )
                })
                .collect::<Vec<_>>();
            if !seen.insert(state) {
                return Err(CallAbiFinalizationFailure::Invalid);
            }
            let call_abis = dependencies
                .iter()
                .filter_map(|(target, node)| {
                    node.abi.as_ref().map(|abi| (target.clone(), abi.clone()))
                })
                .collect::<BTreeMap<_, _>>();
            let mut finalized = Vec::with_capacity(component.len());
            for target in component {
                let node = &dependencies[target];
                let lowering = node
                    .lowering
                    .as_ref()
                    .ok_or(CallAbiFinalizationFailure::Invalid)?;
                let (_, plan) = lower_graph(
                    self.registry,
                    &lowering.document,
                    &lowering.semantic,
                    &lowering.prepared_configs,
                    &lowering.decoded_literals,
                    &lowering.interface_projection,
                    &call_abis,
                    lowering.provenance.clone(),
                    cancellation,
                )
                .map_err(|error| match error {
                    LowerGraphFailure::Cancelled(error) => {
                        CallAbiFinalizationFailure::Cancelled(error)
                    }
                    LowerGraphFailure::Internal(_) => CallAbiFinalizationFailure::Invalid,
                })?;
                let mut abi = node
                    .abi
                    .clone()
                    .ok_or(CallAbiFinalizationFailure::Invalid)?;
                finalize_function_abi_productions(&plan, &mut abi)
                    .map_err(|_| CallAbiFinalizationFailure::Invalid)?;
                finalized.push((target.clone(), abi));
            }
            let stable = finalized.iter().all(|(target, abi)| {
                dependencies[target]
                    .abi
                    .as_ref()
                    .is_some_and(|current| current.result_productions == abi.result_productions)
            });
            for (target, abi) in finalized {
                dependencies
                    .get_mut(&target)
                    .expect("call component target exists")
                    .abi = Some(abi);
            }
            if stable {
                return Ok(());
            }
        }
        Err(CallAbiFinalizationFailure::Invalid)
    }

    fn analyze_call_dependency(
        &self,
        target: &GraphResourcePath,
        root_sites: Option<&[NodeId]>,
        resources: &mut dyn AnalysisResourceResolver,
        cancellation: &CompileCancellationToken,
        diagnostics: &mut Vec<CompilerNodeDiagnostic>,
    ) -> Result<(CallDependencyNode, bool), CompileCancelled> {
        cancellation.checkpoint()?;
        let resolved = match resources.resolve_function(target) {
            Ok(resolved) => resolved,
            Err(error) => {
                if let Some(sites) = root_sites {
                    for &node_id in sites {
                        diagnostics.push(
                            CompilerDiagnostic::ResourceResolutionFailed {
                                resource_key: error.key().as_str().into(),
                                reason: error.reason().into(),
                            }
                            .into_node(DiagnosticLocation::Node(node_id)),
                        );
                    }
                }
                return Ok((CallDependencyNode::invalid(), true));
            }
        };
        let document = resolved.value.graph.clone();
        let provenance = CompileProvenance {
            project_session_id: self.project_session_id.clone(),
            graph_path: target.clone(),
            basis: CompilationBasis {
                graph_revision: document.revision,
                registry_fingerprint: self.registry.fingerprint().clone(),
                resource_versions: ResourceVersionSet::new(),
                resource_observations: ResourceObservationSet::new(),
            },
            compile_id: CompileId::new(NEXT_ADHOC_COMPILE_ID.fetch_add(1, Ordering::Relaxed)),
        };
        let mut state = AnalysisState::new(&document, target.clone(), provenance.basis.clone());
        state.analyze(
            self.registry,
            &self.schema_resolvers,
            &self.interface_resolvers,
            resources,
            cancellation,
        )?;
        let prepared_configs = state.prepared_configs();
        let decoded_literals = state.decoded_literals.clone();
        state.basis.resource_versions = resources.reads().clone();
        state.basis.resource_observations = resources.observations().clone();
        let provisional_semantic = state.semantic_graph();
        let mut nested_diagnostics = Vec::new();
        let dependencies = self
            .call_targets(&provisional_semantic, &mut nested_diagnostics)
            .into_keys()
            .collect();
        let analysis = state.snapshot();
        if analysis.has_blocking_errors() {
            return Ok((
                CallDependencyNode {
                    abi: None,
                    dependencies,
                    locally_valid: false,
                    lowering: None,
                },
                false,
            ));
        }
        let interface_projection = state.interface_projection();
        let semantic = match analysis.validated(provisional_semantic) {
            Ok(semantic) => semantic_for_lowering(self.registry, semantic),
            Err(_) => {
                return Ok((
                    CallDependencyNode {
                        abi: None,
                        dependencies,
                        locally_valid: false,
                        lowering: None,
                    },
                    false,
                ));
            }
        };
        let abi =
            match derive_function_abi(self.registry, &semantic, &interface_projection, &provenance)
            {
                Ok(Some(abi)) => abi,
                Ok(None) | Err(_) => return Ok((CallDependencyNode::invalid(), false)),
            };
        Ok((
            CallDependencyNode {
                abi: Some(abi),
                dependencies,
                locally_valid: nested_diagnostics.is_empty(),
                lowering: Some(CallDependencyLowering {
                    document,
                    semantic,
                    prepared_configs,
                    decoded_literals,
                    interface_projection,
                    provenance,
                }),
            },
            false,
        ))
    }

    fn call_targets(
        &self,
        graph: &CompilerSemanticGraph,
        diagnostics: &mut Vec<CompilerNodeDiagnostic>,
    ) -> BTreeMap<GraphResourcePath, Vec<NodeId>> {
        let mut targets = BTreeMap::<GraphResourcePath, Vec<NodeId>>::new();
        for node in graph.nodes.iter() {
            let resolved = match resolve_for_lowering(self.registry, node) {
                Ok(resolved) => resolved,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            if resolved.structural_role() != Some(StructuralNodeRole::Call) {
                continue;
            }
            let Some(target) = function_target(&node.normalized_parameters) else {
                continue;
            };
            targets
                .entry(GraphResourcePath(target.into()))
                .or_default()
                .push(node.node_id);
        }
        targets
    }

    fn push_call_abi_invalid(
        &self,
        target: &GraphResourcePath,
        sites: &[NodeId],
        diagnostics: &mut Vec<CompilerNodeDiagnostic>,
    ) {
        diagnostics.extend(sites.iter().map(|&node_id| {
            CompilerDiagnostic::ControlCallAbiInvalid {
                function_path: target.0.clone(),
            }
            .into_node(DiagnosticLocation::Node(node_id))
        }));
    }

    pub fn compile(&self, document: &GraphDocument) -> CompileResult {
        let snapshot = self.snapshot(GraphResourcePath(Box::from("")), document);
        self.compile_snapshot(&snapshot, &CompileCancellationToken::new())
            .expect("a fresh cancellation token is not cancelled")
    }
}

#[derive(Default)]
struct CallClosureAnalysis {
    abis: BTreeMap<GraphResourcePath, FunctionPlanAbi>,
    diagnostics: Vec<CompilerNodeDiagnostic>,
}

enum CallAbiFinalizationFailure {
    Cancelled(CompileCancelled),
    Invalid,
}

struct CallDependencyNode {
    abi: Option<FunctionPlanAbi>,
    dependencies: BTreeSet<GraphResourcePath>,
    locally_valid: bool,
    lowering: Option<CallDependencyLowering>,
}

struct CallDependencyLowering {
    document: GraphDocument,
    semantic: CompilerSemanticGraph,
    prepared_configs: BTreeMap<NodeId, ValidatedNodeConfig>,
    decoded_literals: BTreeMap<PortAddress, crate::node_system::protocol::TypedValue>,
    interface_projection: ValidatedInterfaceProjection,
    provenance: CompileProvenance,
}

impl CallDependencyNode {
    fn invalid() -> Self {
        Self {
            abi: None,
            dependencies: BTreeSet::new(),
            locally_valid: false,
            lowering: None,
        }
    }
}

fn invalid_call_targets(
    graph: &BTreeMap<GraphResourcePath, CallDependencyNode>,
) -> BTreeSet<GraphResourcePath> {
    let components = strongly_connected_call_components(graph);
    let component_by_target = components
        .iter()
        .enumerate()
        .flat_map(|(index, component)| component.iter().cloned().map(move |target| (target, index)))
        .collect::<BTreeMap<_, _>>();
    let mut invalid_components = components
        .iter()
        .map(|component| {
            component.iter().any(|target| {
                graph
                    .get(target)
                    .is_none_or(|node| !node.locally_valid || node.abi.is_none())
            })
        })
        .collect::<Vec<_>>();

    loop {
        let newly_invalid = graph
            .iter()
            .filter_map(|(target, node)| {
                let component = component_by_target[target];
                (!invalid_components[component]
                    && node.dependencies.iter().any(|dependency| {
                        component_by_target
                            .get(dependency)
                            .is_none_or(|dependency| invalid_components[*dependency])
                    }))
                .then_some(component)
            })
            .collect::<BTreeSet<_>>();
        if newly_invalid.is_empty() {
            break;
        }
        for component in newly_invalid {
            invalid_components[component] = true;
        }
    }

    components
        .into_iter()
        .enumerate()
        .filter(|(index, _)| invalid_components[*index])
        .flat_map(|(_, component)| component)
        .collect()
}

fn strongly_connected_call_components(
    graph: &BTreeMap<GraphResourcePath, CallDependencyNode>,
) -> Vec<Vec<GraphResourcePath>> {
    fn visit(
        target: &GraphResourcePath,
        graph: &BTreeMap<GraphResourcePath, CallDependencyNode>,
        visited: &mut BTreeSet<GraphResourcePath>,
        order: &mut Vec<GraphResourcePath>,
    ) {
        if !visited.insert(target.clone()) {
            return;
        }
        if let Some(node) = graph.get(target) {
            for dependency in &node.dependencies {
                if graph.contains_key(dependency) {
                    visit(dependency, graph, visited, order);
                }
            }
        }
        order.push(target.clone());
    }

    fn visit_reverse(
        target: &GraphResourcePath,
        reverse: &BTreeMap<GraphResourcePath, BTreeSet<GraphResourcePath>>,
        visited: &mut BTreeSet<GraphResourcePath>,
        component: &mut Vec<GraphResourcePath>,
    ) {
        if !visited.insert(target.clone()) {
            return;
        }
        component.push(target.clone());
        if let Some(dependents) = reverse.get(target) {
            for dependent in dependents {
                visit_reverse(dependent, reverse, visited, component);
            }
        }
    }

    let mut order = Vec::with_capacity(graph.len());
    let mut visited = BTreeSet::new();
    for target in graph.keys() {
        visit(target, graph, &mut visited, &mut order);
    }
    let mut reverse = graph
        .keys()
        .cloned()
        .map(|target| (target, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for (target, node) in graph {
        for dependency in &node.dependencies {
            if let Some(dependents) = reverse.get_mut(dependency) {
                dependents.insert(target.clone());
            }
        }
    }
    visited.clear();
    let mut components = Vec::new();
    while let Some(target) = order.pop() {
        if visited.contains(&target) {
            continue;
        }
        let mut component = Vec::new();
        visit_reverse(&target, &reverse, &mut visited, &mut component);
        components.push(component);
    }
    components
}

struct ResolvedNode<'a> {
    registry: RegistryNode<'a>,
    parameters: BTreeMap<crate::node_system::protocol::ParameterKey, serde_json::Value>,
    prepared_nominal: BTreeMap<crate::node_system::protocol::ParameterKey, PreparedNominalValue>,
    ports: BTreeMap<PortAddress, ResolvedPort<PortAddress>>,
    port_sequence: Vec<PortAddress>,
}

struct AnalysisState<'a> {
    document: &'a GraphDocument,
    graph_path: GraphResourcePath,
    basis: CompilationBasis<GraphRevision>,
    nodes: BTreeMap<NodeId, ResolvedNode<'a>>,
    diagnostics: Vec<NodeDiagnostic<NodeId, PortAddress, ConnectionId, Box<str>>>,
    type_facts: BTreeMap<PortAddress, TypeExpr>,
    schema_facts: BTreeMap<PortAddress, crate::node_system::protocol::SchemaExpr>,
    resolved_schema_facts: BTreeMap<PortAddress, crate::node_system::protocol::ResolvedSchemaFact>,
    projection_only_ports: BTreeSet<PortAddress>,
    interface_projections: BTreeMap<NodeId, ValidatedNodeInterfaceProjection>,
    decoded_literals: BTreeMap<PortAddress, crate::node_system::protocol::TypedValue>,
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
            resolved_schema_facts: BTreeMap::new(),
            projection_only_ports: BTreeSet::new(),
            interface_projections: BTreeMap::new(),
            decoded_literals: BTreeMap::new(),
        }
    }

    fn analyze<R: CompilerRegistry>(
        &mut self,
        registry: &'a R,
        schema_resolvers: &SchemaResolverSet,
        interface_resolvers: &InterfaceResolverSet,
        resources: &mut dyn AnalysisResourceResolver,
        cancellation: &CompileCancellationToken,
    ) -> Result<(), CompileCancelled> {
        let empty_schemas = BTreeMap::new();
        let mut deferred_nodes = BTreeSet::new();
        for (&node_id, node) in &self.document.nodes {
            cancellation.checkpoint()?;
            if node.id != node_id {
                self.push(
                    CompilerDiagnostic::DocumentNodeIdMismatch {
                        expected_id: node_id.to_string().into(),
                        actual_id: node.id.to_string().into(),
                    },
                    DiagnosticLocation::Node(node_id),
                );
            }
            let Some(resolved) = registry.resolve(&node.node_type) else {
                self.push(
                    CompilerDiagnostic::NodeUnknown {
                        node_type: node.node_type.to_string().into(),
                    },
                    DiagnosticLocation::Node(node_id),
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
                    CompilerDiagnostic::NodeScopeMismatch {
                        expected_scope: node_scope_name(path_scope).into(),
                        actual_scope: node_scope_name(resolved.protocol.scope).into(),
                    },
                    DiagnosticLocation::Node(node_id),
                );
            }
            if resolved.protocol.type_id != node.node_type {
                self.push(
                    CompilerDiagnostic::RegistryTypeMismatch {
                        expected_type: node.node_type.to_string().into(),
                        actual_type: resolved.protocol.type_id.to_string().into(),
                    },
                    DiagnosticLocation::Node(node_id),
                );
                continue;
            }
            let (parameters, prepared_nominal) =
                self.normalize_parameters(node_id, resolved.protocol, registry);
            if let Some(error) = track_variable_resource(&node.node_type, &parameters, resources) {
                self.push(
                    CompilerDiagnostic::ResourceResolutionFailed {
                        resource_key: error.key().as_str().into(),
                        reason: error.reason().into(),
                    },
                    DiagnosticLocation::Node(node_id),
                );
            }
            self.validate_binding_templates(node_id, resolved.protocol);
            let provisional_diagnostic_start = self.diagnostics.len();
            let (ports, port_sequence, deferred_for_schema) = self.resolve_ports(
                node_id,
                resolved.protocol,
                &empty_schemas,
                resources,
                interface_resolvers,
            );
            if deferred_for_schema {
                self.diagnostics.truncate(provisional_diagnostic_start);
                deferred_nodes.insert(node_id);
            }
            self.nodes.insert(
                node_id,
                ResolvedNode {
                    registry: resolved,
                    parameters,
                    prepared_nominal,
                    ports,
                    port_sequence,
                },
            );
        }
        cancellation.checkpoint()?;
        let (_, preliminary_schemas, _) = self.resolve_schema_facts(schema_resolvers, resources);
        self.complete_schema_dependent_interfaces(
            &deferred_nodes,
            &preliminary_schemas,
            resources,
            interface_resolvers,
        );
        cancellation.checkpoint()?;
        self.validate_function_abi_contract(resources);
        self.validate_call_abi_contract(resources);
        self.validate_structural_control();
        cancellation.checkpoint()?;
        self.validate_connections();
        cancellation.checkpoint()?;
        self.validate_input_bindings(registry);
        cancellation.checkpoint()?;
        self.validate_value_cycles();
        cancellation.checkpoint()?;
        self.analyze_types(registry);
        cancellation.checkpoint()?;
        self.analyze_schemas(schema_resolvers, resources);
        cancellation.checkpoint()?;
        self.diagnostics.sort_by(compare_diagnostics);
        Ok(())
    }

    fn validate_function_abi_contract(&mut self, resources: &mut dyn AnalysisResourceResolver) {
        if !self.graph_path.0.starts_with("functions/") {
            return;
        }
        let resolved = match resources.resolve_function(&self.graph_path) {
            Ok(resolved) => resolved,
            Err(error) => {
                self.push(
                    CompilerDiagnostic::ResourceResolutionFailed {
                        resource_key: error.key().as_str().into(),
                        reason: error.reason().into(),
                    },
                    DiagnosticLocation::Graph,
                );
                return;
            }
        };
        let function = resolved.value.function;
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
                CompilerDiagnostic::FunctionAbiManagedRoleInvalid {
                    expected_role: structural_role_name(role).into(),
                    actual_count: nodes.len().to_string().into(),
                },
                DiagnosticLocation::Graph,
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
                    CompilerDiagnostic::FunctionAbiLocatorInvalid {
                        port: address.to_string().into(),
                    },
                    DiagnosticLocation::Port(address),
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
                    CompilerDiagnostic::FunctionAbiEndpointInvalid {
                        port: address.to_string().into(),
                    },
                    DiagnosticLocation::Port(address),
                );
                continue;
            }
            if function != self.graph_path {
                self.push(
                    CompilerDiagnostic::FunctionAbiLocatorTargetMismatch {
                        function_path: function.0.clone(),
                    },
                    DiagnosticLocation::Port(address),
                );
                continue;
            }
            if !expected_ids.contains(&parameter) {
                self.push(
                    CompilerDiagnostic::FunctionAbiMemberUnexpected {
                        field_name: parameter.0.clone(),
                    },
                    DiagnosticLocation::Port(address),
                );
                continue;
            }
            *counts.entry(parameter).or_default() += 1;
        }
        for expected in expected_ids {
            match counts.get(expected).copied().unwrap_or(0) {
                0 => self.push(
                    CompilerDiagnostic::FunctionAbiMemberMissing {
                        field_name: expected.0.clone(),
                    },
                    DiagnosticLocation::Node(node_id),
                ),
                1 => {}
                _ => self.push(
                    CompilerDiagnostic::FunctionAbiMemberDuplicate {
                        field_name: expected.0.clone(),
                    },
                    DiagnosticLocation::Node(node_id),
                ),
            }
        }
    }

    fn validate_call_abi_contract(&mut self, resources: &mut dyn AnalysisResourceResolver) {
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
            let resolved = match resources.resolve_function(&target) {
                Ok(resolved) => resolved,
                Err(error) => {
                    self.push(
                        CompilerDiagnostic::ResourceResolutionFailed {
                            resource_key: error.key().as_str().into(),
                            reason: error.reason().into(),
                        },
                        DiagnosticLocation::Node(node_id),
                    );
                    continue;
                }
            };
            let function = resolved.value.function;
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
        let mut member_ports = BTreeMap::<FunctionParameterId, Vec<PortAddress>>::new();
        for (address, binding) in bindings {
            let origin = match binding {
                DynamicPortBinding::Resolved { origin, .. }
                | DynamicPortBinding::Orphan { origin, .. } => origin,
                DynamicPortBinding::UserCreated { .. } => {
                    self.push(
                        CompilerDiagnostic::ControlCallLocatorInvalid {
                            port: address.to_string().into(),
                        },
                        DiagnosticLocation::Port(address),
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
                    CompilerDiagnostic::ControlCallEndpointInvalid {
                        port: address.to_string().into(),
                    },
                    DiagnosticLocation::Port(address),
                );
                continue;
            }
            let DynamicMemberLocator::FunctionParameter {
                function,
                parameter,
            } = origin
            else {
                self.push(
                    CompilerDiagnostic::ControlCallLocatorInvalid {
                        port: address.to_string().into(),
                    },
                    DiagnosticLocation::Port(address),
                );
                continue;
            };
            if &function != target {
                self.push(
                    CompilerDiagnostic::ControlCallLocatorTargetMismatch {
                        function_path: function.0.clone(),
                    },
                    DiagnosticLocation::Port(address),
                );
                continue;
            }
            if !expected_ids.contains(&parameter) {
                self.push(
                    CompilerDiagnostic::ControlCallMemberUnexpected {
                        member_role: call_member_role(expected_template).into(),
                        member_id: parameter.0.clone(),
                    },
                    DiagnosticLocation::Port(address),
                );
                continue;
            }
            member_ports.entry(parameter).or_default().push(address);
        }
        for expected in expected_ids {
            match member_ports
                .get(expected)
                .map(Vec::as_slice)
                .unwrap_or_default()
            {
                [] => self.push(
                    CompilerDiagnostic::ControlCallMemberMissing {
                        member_role: call_member_role(expected_template).into(),
                        member_id: expected.0.clone(),
                    },
                    DiagnosticLocation::Node(node_id),
                ),
                [_] => {}
                [_, duplicate, ..] => self.push(
                    CompilerDiagnostic::ControlCallLocatorDuplicate {
                        function_path: target.0.clone(),
                        parameter_id: expected.0.clone(),
                        port: duplicate.to_string().into(),
                    },
                    DiagnosticLocation::Port((*duplicate).clone()),
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
                        CompilerDiagnostic::NodeManagedSingleton {
                            managed_role: managed_node_role_name(Some(role)).into(),
                        },
                        DiagnosticLocation::Node(node_id),
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
                issue.diagnostic,
                issue
                    .node_id
                    .map(DiagnosticLocation::Node)
                    .unwrap_or(DiagnosticLocation::Graph),
            );
        }
    }

    fn normalize_parameters<R: CompilerRegistry>(
        &mut self,
        node_id: NodeId,
        protocol: &NodeProtocol,
        registry: &R,
    ) -> (
        BTreeMap<crate::node_system::protocol::ParameterKey, serde_json::Value>,
        BTreeMap<crate::node_system::protocol::ParameterKey, PreparedNominalValue>,
    ) {
        let supplied = &self.document.nodes[&node_id].parameters;
        let mut values = supplied.clone();
        for spec in protocol.parameters.parameters.iter() {
            if !values.contains_key(&spec.key)
                && let Some(default) = &spec.default_value
            {
                values.insert(spec.key.clone(), protocol_value_to_json(&default.value));
            }
        }
        let validation =
            validate_and_prepare_parameter_values(protocol, &values, |type_id, value| {
                registry.prepare_nominal_parameter(type_id, value)
            });
        for issue in validation.issues {
            let diagnostic = match issue.kind {
                ParameterIssueKind::Unknown => CompilerDiagnostic::ParameterUnknown {
                    parameter_key: issue.key.to_string().into(),
                },
                ParameterIssueKind::Required => CompilerDiagnostic::ParameterRequired {
                    parameter_key: issue.key.to_string().into(),
                },
                ParameterIssueKind::InvalidType
                | ParameterIssueKind::Constraint
                | ParameterIssueKind::InvalidNominal(_)
                | ParameterIssueKind::InvalidResourceId => CompilerDiagnostic::ParameterInvalid {
                    parameter_key: issue.key.to_string().into(),
                },
            };
            self.push(
                diagnostic,
                DiagnosticLocation::Parameter {
                    node_id,
                    key: issue.key,
                },
            );
        }
        let known = protocol
            .parameters
            .parameters
            .iter()
            .map(|spec| &spec.key)
            .collect::<BTreeSet<_>>();
        let normalized = values
            .into_iter()
            .filter(|(key, _)| known.contains(key))
            .collect();
        (normalized, validation.prepared_nominal)
    }

    fn resolve_ports(
        &mut self,
        node_id: NodeId,
        protocol: &NodeProtocol,
        resolved_schemas: &BTreeMap<PortAddress, ResolvedSchemaFact>,
        resources: &mut dyn AnalysisResourceResolver,
        resolvers: &InterfaceResolverSet,
    ) -> (
        BTreeMap<PortAddress, ResolvedPort<PortAddress>>,
        Vec<PortAddress>,
        bool,
    ) {
        let DynamicInterfaceResolution {
            interface,
            projected_bindings,
            available_members,
            diagnostics,
            deferred_for_schema,
        } = materialize_dynamic_interface_with_resources(
            &self.basis,
            node_id,
            protocol,
            self.document,
            resolved_schemas,
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
        let port_sequence = interface
            .ports
            .iter()
            .map(|port| port.address.clone())
            .collect();
        let ports = interface
            .ports
            .into_vec()
            .into_iter()
            .map(|port| (port.address.clone(), port))
            .collect();
        (ports, port_sequence, deferred_for_schema)
    }

    fn complete_schema_dependent_interfaces(
        &mut self,
        deferred_nodes: &BTreeSet<NodeId>,
        resolved_schemas: &BTreeMap<PortAddress, ResolvedSchemaFact>,
        resources: &mut dyn AnalysisResourceResolver,
        resolvers: &InterfaceResolverSet,
    ) {
        for &node_id in deferred_nodes {
            let Some(protocol) = self.nodes.get(&node_id).map(|node| node.registry.protocol) else {
                continue;
            };
            self.projection_only_ports
                .retain(|address| address.node_id != node_id);
            self.interface_projections.remove(&node_id);
            let (ports, port_sequence, deferred_for_schema) =
                self.resolve_ports(node_id, protocol, resolved_schemas, resources, resolvers);
            if let Some(node) = self.nodes.get_mut(&node_id) {
                node.ports = ports;
                node.port_sequence = port_sequence;
            }
            if deferred_for_schema {
                self.push(
                    CompilerDiagnostic::InterfaceSchemaDependencyUnresolved {},
                    DiagnosticLocation::Node(node_id),
                );
            }
        }
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
                    CompilerDiagnostic::PortBindingNotInstance {
                        port: address.to_string().into(),
                    },
                    DiagnosticLocation::Port(address.clone()),
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
                    CompilerDiagnostic::PortUnknown {
                        port: address.to_string().into(),
                    },
                    DiagnosticLocation::Port(address.clone()),
                );
                continue;
            };
            if spec.instances == PortInstances::Declared {
                self.push(
                    CompilerDiagnostic::PortInstanceNotAllowed {
                        port: address.to_string().into(),
                    },
                    DiagnosticLocation::Port(address.clone()),
                );
            }
        }
    }

    fn validate_connections(&mut self) {
        let mut counts: BTreeMap<PortAddress, usize> = BTreeMap::new();
        for (&connection_id, connection) in &self.document.connections {
            if connection.id != connection_id {
                self.push(
                    CompilerDiagnostic::DocumentConnectionIdMismatch {
                        expected_id: connection_id.to_string().into(),
                        actual_id: connection.id.to_string().into(),
                    },
                    DiagnosticLocation::Connection(connection_id),
                );
            }
            let output = self.lookup_document_port(&connection.output).cloned();
            let input = self.lookup_document_port(&connection.input).cloned();
            if output.is_none() {
                self.push(
                    CompilerDiagnostic::PortUnknown {
                        port: connection.output.to_string().into(),
                    },
                    DiagnosticLocation::Port(connection.output.clone()),
                );
            }
            if input.is_none() {
                self.push(
                    CompilerDiagnostic::PortUnknown {
                        port: connection.input.to_string().into(),
                    },
                    DiagnosticLocation::Port(connection.input.clone()),
                );
            }
            let (Some(output), Some(input)) = (output, input) else {
                continue;
            };
            if output.direction != PortDirection::Output {
                self.push(
                    CompilerDiagnostic::ConnectionOutputDirection {
                        port: connection.output.to_string().into(),
                    },
                    DiagnosticLocation::Connection(connection_id),
                );
            }
            if input.direction != PortDirection::Input {
                self.push(
                    CompilerDiagnostic::ConnectionInputDirection {
                        port: connection.input.to_string().into(),
                    },
                    DiagnosticLocation::Connection(connection_id),
                );
            }
            if output.kind != input.kind {
                self.push(
                    CompilerDiagnostic::ConnectionKindMismatch {
                        source_kind: port_kind_name(output.kind).into(),
                        target_kind: port_kind_name(input.kind).into(),
                    },
                    DiagnosticLocation::Connection(connection_id),
                );
            }
            if let Some(spec) = self.port_spec(&connection.input, &input.template) {
                match spec.connections {
                    ConnectionsPerPort::Multiple { ordered: true, .. }
                        if connection.order.is_none() =>
                    {
                        self.push(
                            CompilerDiagnostic::ConnectionOrderRequired {
                                port: connection.input.to_string().into(),
                            },
                            DiagnosticLocation::Connection(connection_id),
                        );
                    }
                    ConnectionsPerPort::Single
                    | ConnectionsPerPort::Multiple { ordered: false, .. }
                        if connection.order.is_some() =>
                    {
                        self.push(
                            CompilerDiagnostic::ConnectionOrderForbidden {
                                port: connection.input.to_string().into(),
                            },
                            DiagnosticLocation::Connection(connection_id),
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
                        CompilerDiagnostic::ConnectionLimit {
                            port: address.to_string().into(),
                        },
                        DiagnosticLocation::Port(address.clone()),
                    );
                }
            }
        }
    }

    fn validate_input_bindings<R: CompilerRegistry>(&mut self, registry: &R) {
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
                        CompilerDiagnostic::InputNotInput {
                            port: address.to_string().into(),
                        },
                        DiagnosticLocation::Port(address.clone()),
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
            if let Some(literal) = literal {
                match crate::node_system::protocol::validate_typed_literal(
                    literal,
                    &spec.value_type,
                    &CompilerNominalValidator(registry),
                ) {
                    Ok(decoded) => {
                        self.decoded_literals.insert(address.clone(), decoded);
                    }
                    Err(_) => self.push(
                        CompilerDiagnostic::InputLiteralInvalid {
                            port: address.to_string().into(),
                        },
                        DiagnosticLocation::Port(address.clone()),
                    ),
                }
            }
            if literal.is_some() && connections != 0 {
                self.push(
                    CompilerDiagnostic::InputConflictingBindings {
                        port: address.to_string().into(),
                    },
                    DiagnosticLocation::Port(address.clone()),
                );
            }
            if literal.is_some()
                && spec
                    .input_binding
                    .as_ref()
                    .is_none_or(|binding| binding.literal_policy == LiteralPolicy::Forbidden)
            {
                self.push(
                    CompilerDiagnostic::InputLiteralForbidden {
                        port: address.to_string().into(),
                    },
                    DiagnosticLocation::Port(address.clone()),
                );
            }
            let has_default = spec
                .input_binding
                .as_ref()
                .is_some_and(|binding| binding.default_value.is_some());
            if port.kind == PortKind::Data && connections == 0 && literal.is_none() && !has_default
            {
                self.push(
                    CompilerDiagnostic::InputUnbound {
                        port: address.to_string().into(),
                    },
                    DiagnosticLocation::Port(address.clone()),
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
                CompilerDiagnostic::InputUnknownPort {
                    port: address.to_string().into(),
                },
                DiagnosticLocation::Port(address.clone()),
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
                CompilerDiagnostic::DependencyValueCycle {},
                DiagnosticLocation::Connection(connection_id),
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
        for (address, literal) in &self.decoded_literals {
            graph.add_literal(address, &literal.value_type);
        }
        let (facts, issues) = graph.solve(registry);
        self.type_facts = facts;
        for issue in issues {
            self.push(issue.diagnostic, issue.location);
        }
    }

    fn resolve_schema_facts(
        &self,
        resolvers: &SchemaResolverSet,
        resources: &mut dyn AnalysisResourceResolver,
    ) -> (
        BTreeMap<PortAddress, SchemaExpr>,
        BTreeMap<PortAddress, ResolvedSchemaFact>,
        Vec<SchemaAnalysisIssue>,
    ) {
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
        analyzer.analyze_with_resources(resources)
    }

    fn analyze_schemas(
        &mut self,
        resolvers: &SchemaResolverSet,
        resources: &mut dyn AnalysisResourceResolver,
    ) {
        let (expressions, facts, issues) = self.resolve_schema_facts(resolvers, resources);
        self.schema_facts = expressions;
        self.resolved_schema_facts = facts;
        for issue in issues {
            self.push(issue.diagnostic, issue.location);
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
    fn push(&mut self, diagnostic: CompilerDiagnostic, location: CompilerDiagnosticLocation) {
        self.diagnostics.push(diagnostic.into_node(location));
    }

    fn interface_projection(&self) -> ValidatedInterfaceProjection {
        ValidatedInterfaceProjection {
            basis: self.basis.clone(),
            nodes: self.interface_projections.clone(),
        }
    }

    fn prepared_configs(&mut self) -> BTreeMap<NodeId, ValidatedNodeConfig> {
        let attempts = self
            .nodes
            .iter()
            .map(|(&node_id, node)| {
                (
                    node_id,
                    ValidatedNodeConfig::from_analysis(
                        node.registry.protocol,
                        node.parameters.clone(),
                        &node.prepared_nominal,
                    ),
                )
            })
            .collect::<Vec<_>>();
        let mut prepared = BTreeMap::new();
        for (node_id, attempt) in attempts {
            match attempt {
                Ok(config) => {
                    prepared.insert(node_id, config);
                }
                Err(keys) => {
                    for key in keys {
                        self.push(
                            CompilerDiagnostic::ParameterInvalid {
                                parameter_key: key.to_string().into(),
                            },
                            DiagnosticLocation::Parameter { node_id, key },
                        );
                    }
                }
            }
        }
        self.diagnostics.sort_by(compare_diagnostics);
        prepared
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
                    .port_sequence
                    .iter()
                    .filter_map(|address| node.ports.get(address).cloned())
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
            resolved_schemas: self.resolved_schema_facts.clone(),
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
            resolved_schemas: self.resolved_schema_facts.clone(),
        }
    }
}

fn semantic_for_lowering<R: CompilerRegistry>(
    registry: &R,
    semantic: CompilerSemanticGraph,
) -> CompilerSemanticGraph {
    collapse_transparent_nodes(registry, semantic)
}

enum LowerGraphFailure {
    Cancelled(CompileCancelled),
    Internal(InternalCompilationFailure),
}

impl From<CompileCancelled> for LowerGraphFailure {
    fn from(error: CompileCancelled) -> Self {
        Self::Cancelled(error)
    }
}

impl From<CompilerNodeDiagnostic> for LowerGraphFailure {
    fn from(diagnostic: CompilerNodeDiagnostic) -> Self {
        let node_id = match &diagnostic.primary {
            DiagnosticLocation::Node(node_id) => Some(*node_id),
            DiagnosticLocation::Port(port) => Some(port.node_id),
            DiagnosticLocation::Parameter { node_id, .. } => Some(*node_id),
            DiagnosticLocation::Graph
            | DiagnosticLocation::Connection(_)
            | DiagnosticLocation::Resource(_) => None,
        };
        Self::Internal(InternalCompilationFailure {
            stage: CompilationStage::Lowering,
            code: diagnostic.code.as_str().into(),
            node_id,
        })
    }
}

#[derive(Clone)]
struct PendingOperation {
    stable_id: OperationStableId,
    node_id: NodeId,
    node_type_id: NodeTypeId,
    has_control_or_effect_ports: bool,
    kernel: PendingKernel,
    input_ports: Box<[PortAddress]>,
    inputs: Box<[PlannedInput]>,
    output_ports: Box<[PortAddress]>,
    outputs: Box<[PlannedOutput]>,
    parameters: CompiledParameterHandle,
    resource_dependencies: Box<[ResourceKey]>,
    cache_policy: CachePolicy,
    semantics_version: ExecutionSemanticsVersion,
    workload: WorkloadClass,
    retry: PlannedRetry,
    evaluation: EvaluationPolicy,
    purity: Purity,
    effects: EffectSemantics,
    resources: Box<[CompiledResourceRequirement]>,
}

#[derive(Clone)]
pub(crate) enum PendingKernel {
    Native(KernelHandle),
    Relational,
}

#[derive(Clone)]
struct PendingRelationalFragment {
    backend: RelationalBackendId,
    fragment: RelationalFragment,
    inputs: BTreeMap<PortAddress, crate::node_system::plan::RelationalOperatorIndex>,
}

pub(crate) fn effective_cache_policy(
    requested: CachePolicy,
    determinism: Determinism,
    purity: Purity,
    effects: EffectSemantics,
) -> CachePolicy {
    if determinism == Determinism::Deterministic
        && purity == Purity::Pure
        && effects == EffectSemantics::None
    {
        requested
    } else {
        CachePolicy::Disabled
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn effective_retry_policy(
    idempotent: bool,
    policy: Option<RetryPolicy>,
    determinism: Determinism,
    purity: Purity,
    effects: EffectSemantics,
    has_control_or_effect_ports: bool,
    kernel: &PendingKernel,
    resources: &[CompiledResourceRequirement],
) -> PlannedRetry {
    let compiler_approved = idempotent
        && policy.is_some()
        && determinism == Determinism::Deterministic
        && purity == Purity::Pure
        && effects == EffectSemantics::None
        && !has_control_or_effect_ports
        && matches!(kernel, PendingKernel::Native(_))
        && resources.is_empty();
    if compiler_approved {
        PlannedRetry { idempotent, policy }
    } else {
        PlannedRetry::default()
    }
}

fn fragment_metadata_identity(metadata: &FragmentMetadata) -> serde_json::Value {
    serde_json::json!({
        "effect": &metadata.effect,
        "resources": &metadata.resources,
        "results": &metadata.results.iter().map(|result| serde_json::json!({
            "name": &result.name,
            "output": &result.output,
        })).collect::<Vec<_>>(),
    })
}

fn lowered_kernel_identity(kernel: &LoweredKernel) -> serde_json::Value {
    match kernel {
        LoweredKernel::Native(handle) => serde_json::json!({
            "kind": "native",
            "handle": handle,
        }),
        LoweredKernel::Scalar(fragment) => serde_json::json!({
            "kind": "scalar",
            "handle": &fragment.kernel,
            "metadata": fragment_metadata_identity(&fragment.metadata),
        }),
        LoweredKernel::Kernel(fragment) => serde_json::json!({
            "kind": "kernel",
            "handle": &fragment.kernel,
            "metadata": fragment_metadata_identity(&fragment.metadata),
        }),
        LoweredKernel::Relational(fragment) => serde_json::json!({
            "kind": "relational",
            "backend": &fragment.backend,
            "fragment": {
                "id": &fragment.fragment.id,
                "operators": &fragment.fragment.operators,
                "root": fragment.fragment.root,
            },
            "inputs": fragment.inputs.iter().map(|input| serde_json::json!({
                "port": &input.port,
                "operator": input.operator,
            })).collect::<Vec<_>>(),
            "metadata": fragment_metadata_identity(&fragment.metadata),
        }),
    }
}

fn lowering_identity_failure(node_id: NodeId) -> LowerGraphFailure {
    LowerGraphFailure::Internal(InternalCompilationFailure {
        stage: CompilationStage::Lowering,
        code: CompilerDiagnostic::LoweringExecutionIdentity {}
            .definition()
            .code
            .into(),
        node_id: Some(node_id),
    })
}

fn effective_workload_class(
    kernel: &PendingKernel,
    purity: Purity,
    effects: EffectSemantics,
    resources: &BTreeMap<ResourceId, CompiledResourceRequirement>,
) -> WorkloadClass {
    if purity == Purity::Effectful
        || effects != EffectSemantics::None
        || resources
            .values()
            .any(|requirement| requirement.access == ResourceAccess::Exclusive)
    {
        WorkloadClass::Exclusive
    } else if matches!(kernel, PendingKernel::Relational) || !resources.is_empty() {
        WorkloadClass::Io
    } else {
        WorkloadClass::Cpu
    }
}

fn structural_role_name(role: StructuralNodeRole) -> &'static str {
    match role {
        StructuralNodeRole::EventBegin => "event_begin",
        StructuralNodeRole::FunctionEntry => "function_entry",
        StructuralNodeRole::FunctionReturn => "function_return",
        StructuralNodeRole::Branch => "branch",
        StructuralNodeRole::Loop => "loop",
        StructuralNodeRole::Sequence => "sequence",
        StructuralNodeRole::Call => "call",
    }
}

fn call_member_role(template: &str) -> &'static str {
    match template {
        "arguments" => "argument",
        "results" => "result",
        _ => "member",
    }
}

fn track_variable_resource(
    node_type: &NodeTypeId,
    parameters: &BTreeMap<crate::node_system::protocol::ParameterKey, serde_json::Value>,
    resources: &mut dyn AnalysisResourceResolver,
) -> Option<ResourceResolutionError> {
    if !matches!(
        node_type.as_str(),
        "yssbi.project.variable.get" | "yssbi.project.variable.set"
    ) {
        return None;
    }
    let Some(path) = parameters
        .iter()
        .find(|(key, _)| key.as_str() == "variable")
        .and_then(|(_, value)| value.as_str())
    else {
        return None;
    };
    let Some(id) = path.strip_prefix("variables/") else {
        return None;
    };
    let Ok(id) = uuid::Uuid::parse_str(id) else {
        return None;
    };
    resources
        .resolve_variable(&crate::variable::VariableId::from(id))
        .err()
}

fn protocol_value_to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Null => serde_json::Value::Null,
        Value::Bool(value) => serde_json::Value::Bool(*value),
        Value::Integer(value) => serde_json::Value::Number((*value).into()),
        Value::Unsigned(value) => serde_json::Value::Number((*value).into()),
        Value::Decimal(value) => serde_json::Value::String(value.as_str().to_owned()),
        Value::String(value) => serde_json::Value::String(value.as_ref().to_owned()),
        Value::Bytes(value) => serde_json::Value::Array(
            value
                .iter()
                .map(|value| serde_json::Value::Number(u64::from(*value).into()))
                .collect(),
        ),
        Value::List(values) => {
            serde_json::Value::Array(values.iter().map(protocol_value_to_json).collect())
        }
        Value::Object(values) => serde_json::Value::Object(
            values
                .iter()
                .map(|(key, value)| (key.to_string(), protocol_value_to_json(value)))
                .collect(),
        ),
    }
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
) -> Result<(u32, BTreeMap<PortAddress, ValueRef>), CompilerNodeDiagnostic> {
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

#[derive(Debug)]
pub(super) enum FinalizeFunctionAbiError {
    InvalidPlan,
    MissingResultProduction,
}

pub(super) fn finalize_function_abi_productions(
    plan: &ExecutionPlan,
    abi: &mut FunctionPlanAbi,
) -> Result<(), FinalizeFunctionAbiError> {
    let source_facts = plan
        .validate_with_source_facts()
        .map_err(|_| FinalizeFunctionAbiError::InvalidPlan)?;
    let mut productions = BTreeMap::new();
    for (result, value) in &abi.results {
        let production = source_facts
            .production(*value)
            .ok_or(FinalizeFunctionAbiError::MissingResultProduction)?;
        productions.insert(result.clone(), production);
    }
    abi.result_productions = productions;
    Ok(())
}

fn derive_function_abi<R: CompilerRegistry>(
    registry: &R,
    graph: &CompilerSemanticGraph,
    projection: &ValidatedInterfaceProjection,
    provenance: &CompileProvenance,
) -> Result<Option<FunctionPlanAbi>, CompilerNodeDiagnostic> {
    if !provenance.graph_path.0.starts_with("functions/") {
        return Ok(None);
    }
    let (_, values) = allocate_port_values(registry, graph)?;
    let mut parameters = BTreeMap::new();
    let mut results = BTreeMap::new();
    let mut result_productions = BTreeMap::new();
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
                return Err(CompilerDiagnostic::FunctionAbiTargetMismatch {
                    function_path: function.0.clone(),
                }
                .into_node(DiagnosticLocation::Port(port.address.clone())));
            }
            let value = values[&port.address];
            if resolved.structural_role() == Some(StructuralNodeRole::FunctionReturn) {
                let production = graph
                    .dependencies
                    .iter()
                    .find_map(|dependency| match dependency {
                        SemanticDependency::Value(edge) if edge.target == port.address => {
                            let source_node = graph
                                .nodes
                                .iter()
                                .find(|candidate| candidate.node_id == edge.source.node_id)?;
                            let source = resolve_for_lowering(registry, source_node).ok()?;
                            protocol_port(source.protocol, &edge.source).production
                        }
                        _ => None,
                    })
                    .ok_or_else(|| {
                        CompilerDiagnostic::PlanInvalid {}
                            .into_node(DiagnosticLocation::Port(port.address.clone()))
                    })?;
                result_productions.insert(parameter.clone(), production);
            }
            if destination.insert(parameter.clone(), value).is_some() {
                return Err(CompilerDiagnostic::FunctionAbiMemberDuplicate {
                    field_name: parameter.0.clone(),
                }
                .into_node(DiagnosticLocation::Port(port.address.clone())));
            }
        }
    }
    Ok(Some(FunctionPlanAbi {
        provenance: provenance.clone(),
        parameters,
        results,
        result_productions,
    }))
}

fn lower_graph<R: CompilerRegistry>(
    registry: &R,
    document: &GraphDocument,
    graph: &CompilerSemanticGraph,
    prepared_configs: &BTreeMap<NodeId, ValidatedNodeConfig>,
    decoded_literals: &BTreeMap<PortAddress, crate::node_system::protocol::TypedValue>,
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
                                value_sources.insert(PlanValueSource::ExternalInput(
                                    value,
                                    spec.production
                                        .unwrap_or(OutputProduction::FullyMaterialized),
                                ));
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
            CompilerDiagnostic::LoweringImplementationMissing {
                node_type: node.node_type_id.to_string().into(),
            }
            .into_node(DiagnosticLocation::Node(node.node_id))
        })?;
        let prepared_config = prepared_configs.get(&node.node_id).ok_or_else(|| {
            CompilerDiagnostic::LoweringInternalInvariant {
                node_type: node.node_type_id.to_string().into(),
            }
            .into_node(DiagnosticLocation::Node(node.node_id))
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
                    let has_connection = document
                        .connections
                        .values()
                        .any(|connection| connection.input == port.address);
                    let bound_value = if has_connection {
                        None
                    } else if let Some(literal) = decoded_literals.get(&port.address) {
                        Some(literal.value.clone())
                    } else {
                        spec.input_binding
                            .as_ref()
                            .and_then(|binding| binding.default_value.as_ref())
                            .map(|default| default.value.clone())
                    };
                    planned_inputs.push(PlannedInput {
                        value,
                        consumption: spec
                            .consumption
                            .unwrap_or(InputConsumption::FullyMaterialized),
                        bound_value,
                    });
                }
            }
        }
        let context = LoweringContext {
            cancellation,
            node_id: node.node_id,
            protocol: resolved.protocol,
            parameters: prepared_config,
            inputs: &inputs,
            outputs: &outputs,
        };
        let lowered = match implementation.lowerer.lower(&context) {
            Ok(lowered) => lowered,
            Err(LoweringError::Cancelled(error)) => {
                return Err(LowerGraphFailure::Cancelled(error));
            }
            Err(error) => {
                let node_type = node.node_type_id.to_string().into();
                let diagnostic = match error {
                    LoweringError::InternalInvariant(_) => {
                        CompilerDiagnostic::LoweringInternalInvariant { node_type }
                    }
                    LoweringError::DeadlineExceeded => {
                        CompilerDiagnostic::LoweringDeadlineExceeded { node_type }
                    }
                    LoweringError::ResourceExhausted => {
                        CompilerDiagnostic::LoweringResourceExhausted { node_type }
                    }
                    LoweringError::Cancelled(_) => unreachable!("handled above"),
                };
                return Err(diagnostic
                    .into_node(DiagnosticLocation::Node(node.node_id))
                    .into());
            }
        };
        let mut owned_resources = BTreeMap::<ResourceId, CompiledResourceRequirement>::new();
        if let Some(metadata) = lowered.kernel.metadata() {
            if metadata.effect != resolved.protocol.execution.effects {
                return Err(CompilerDiagnostic::LoweringEffectContract {}
                    .into_node(DiagnosticLocation::Node(node.node_id))
                    .into());
            }
            for requirement in &metadata.resources {
                owned_resources.insert(requirement.resource.clone(), requirement.clone());
                if let Some(previous) =
                    resources.insert(requirement.resource.clone(), requirement.clone())
                {
                    if previous != *requirement {
                        return Err(CompilerDiagnostic::LoweringResourceConflict {
                            resource_id: requirement.resource.as_str().into(),
                        }
                        .into_node(DiagnosticLocation::Node(node.node_id))
                        .into());
                    }
                }
            }
            for result in &metadata.results {
                let Some(&value) = port_values.get(&result.output) else {
                    return Err(CompilerDiagnostic::LoweringResultPort {
                        port: result.output.to_string().into(),
                    }
                    .into_node(DiagnosticLocation::Node(node.node_id))
                    .into());
                };
                if !outputs.iter().any(|(address, _)| address == &result.output) {
                    return Err(CompilerDiagnostic::LoweringResultPort {
                        port: result.output.to_string().into(),
                    }
                    .into_node(DiagnosticLocation::Node(node.node_id))
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
                    return Err(CompilerDiagnostic::LoweringResultDuplicate {
                        result_name: result.name.clone(),
                    }
                    .into_node(DiagnosticLocation::Node(node.node_id))
                    .into());
                }
            }
        }

        for parameter in resolved.protocol.parameters.parameters.iter() {
            if parameter.editor != ParameterEditorSpec::Resource {
                continue;
            }
            let Some(resource) = prepared_config.resource(&parameter.key).cloned() else {
                if node.normalized_parameters.contains_key(&parameter.key) {
                    return Err(CompilerDiagnostic::LoweringInternalInvariant {
                        node_type: node.node_type_id.to_string().into(),
                    }
                    .into_node(DiagnosticLocation::Node(node.node_id))
                    .into());
                }
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
            let requirement = CompiledResourceRequirement {
                resource: resource.clone(),
                kind,
                access,
                optional: false,
            };
            owned_resources.insert(resource.clone(), requirement.clone());
            if let Some(previous) = resources.insert(resource, requirement.clone()) {
                if previous != requirement {
                    return Err(CompilerDiagnostic::LoweringResourceConflict {
                        resource_id: requirement.resource.as_str().into(),
                    }
                    .into_node(DiagnosticLocation::Node(node.node_id))
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
        let stable_id = OperationStableId::from_digest(
            hash_canonical(
                "yssbi.operation-stable-id.node.v2",
                &serde_json::json!({
                    "graphPath": &provenance.graph_path,
                    "nodeId": node.node_id,
                }),
            )
            .map_err(|_| lowering_identity_failure(node.node_id))?,
        );
        let semantics_version = ExecutionSemanticsVersion::from_bytes(
            hash_canonical(
                "yssbi.execution-semantics.native.v2",
                &serde_json::json!({
                    "schemaVersion": EXECUTION_SEMANTICS_SCHEMA_VERSION,
                    "registryFingerprint": &provenance.basis.registry_fingerprint,
                    "protocolFingerprint": &node.protocol_fingerprint,
                    "nodeTypeId": &node.node_type_id,
                    "execution": &resolved.protocol.execution,
                    "kernel": lowered_kernel_identity(&lowered.kernel),
                    "compiledParameters": &lowered.parameters,
                    "normalizedParameters": &node.normalized_parameters,
                    "inputPorts": &inputs,
                    "inputs": &planned_inputs,
                    "outputPorts": &outputs,
                    "outputs": &planned_outputs,
                }),
            )
            .map_err(|_| lowering_identity_failure(node.node_id))?,
        );
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
        let execution = resolved.protocol.execution;
        let cache_policy = effective_cache_policy(
            execution.cache,
            execution.determinism,
            execution.purity,
            execution.effects,
        );
        let workload = effective_workload_class(
            &kernel,
            execution.purity,
            execution.effects,
            &owned_resources,
        );
        let has_control_or_effect_ports = resolved
            .protocol
            .interface
            .ports
            .iter()
            .any(|port| port.kind != PortKind::Data);
        let retry = effective_retry_policy(
            execution.idempotent,
            execution.retry,
            execution.determinism,
            execution.purity,
            execution.effects,
            has_control_or_effect_ports,
            &kernel,
            owned_resources
                .values()
                .cloned()
                .collect::<Vec<_>>()
                .as_slice(),
        );
        let resource_dependencies = owned_resources
            .keys()
            .map(|resource| ResourceKey::new(resource.as_str()))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        pending_operations.push(PendingOperation {
            stable_id,
            node_id: node.node_id,
            node_type_id: node.node_type_id.clone(),
            has_control_or_effect_ports,
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
            resource_dependencies,
            cache_policy,
            semantics_version,
            workload,
            retry,
            evaluation: execution.evaluation,
            purity: execution.purity,
            effects: execution.effects,
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
                    return Err(CompilerDiagnostic::PlanValueProducerMissing {
                        port: edge.source.to_string().into(),
                    }
                    .into_node(DiagnosticLocation::Connection(edge.connection_id))
                    .into());
                };
                let Some(&destination) = port_values.get(&edge.target) else {
                    return Err(CompilerDiagnostic::PlanValueConsumerMissing {
                        port: edge.target.to_string().into(),
                    }
                    .into_node(DiagnosticLocation::Connection(edge.connection_id))
                    .into());
                };
                if !operation_outputs.contains_key(&edge.source)
                    && !structural_outputs.contains(&edge.source)
                {
                    return Err(CompilerDiagnostic::PlanValueProducerMissing {
                        port: edge.source.to_string().into(),
                    }
                    .into_node(DiagnosticLocation::Connection(edge.connection_id))
                    .into());
                }
                if !operation_inputs.contains_key(&edge.target)
                    && !structural_inputs.contains(&edge.target)
                {
                    return Err(CompilerDiagnostic::PlanValueConsumerMissing {
                        port: edge.target.to_string().into(),
                    }
                    .into_node(DiagnosticLocation::Connection(edge.connection_id))
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
                        return Err(CompilerDiagnostic::RelationalInputBindingMissing {
                            port: edge.target.to_string().into(),
                        }
                        .into_node(DiagnosticLocation::Connection(edge.connection_id))
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
                    return Err(CompilerDiagnostic::PlanEffectProducerMissing {
                        port: edge.effect_key.clone(),
                    }
                    .into_node(DiagnosticLocation::Node(edge.predecessor))
                    .into());
                };
                let Some(&after) = operation_by_node.get(&edge.successor) else {
                    return Err(CompilerDiagnostic::PlanEffectConsumerMissing {
                        port: edge.effect_key.clone(),
                    }
                    .into_node(DiagnosticLocation::Node(edge.successor))
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
                    parameters: prepared_configs.get(&node.node_id).ok_or_else(|| {
                        CompilerDiagnostic::LoweringInternalInvariant {
                            node_type: node.node_type_id.to_string().into(),
                        }
                        .into_node(DiagnosticLocation::Node(node.node_id))
                    })?,
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
            issue.diagnostic.into_node(
                issue
                    .node_id
                    .map(DiagnosticLocation::Node)
                    .unwrap_or(DiagnosticLocation::Graph),
            )
        })?;
    deduplicate_region_operations(&mut root_region);
    let mut production_by_value = production_by_port
        .iter()
        .map(|(port, production)| (port_values[port], *production))
        .collect::<BTreeMap<_, _>>();
    for dependency in &value_dependencies {
        if let Some(production) = production_by_value.get(&dependency.source).copied() {
            production_by_value.insert(dependency.destination, production);
        }
    }
    collect_control_value_sources(
        &mut root_region,
        &mut value_sources,
        &mut production_by_value,
        function_abis,
    )?;
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
                stable_id: pending.stable_id,
                source_node_id: pending.node_id,
                source_node_type_id: pending.node_type_id,
                has_control_or_effect_ports: pending.has_control_or_effect_ports,
                kernel,
                input_ports: pending.input_ports,
                inputs: pending.inputs,
                output_ports: pending.output_ports,
                outputs: pending.outputs,
                params: pending.parameters,
                resource_dependencies: pending.resource_dependencies,
                cache_policy: pending.cache_policy,
                semantics_version: pending.semantics_version,
                workload: pending.workload,
                retry: pending.retry,
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
    let plan = basis
        .derive_full_plan()
        .map_err(|_| CompilerDiagnostic::PlanInvalid {}.into_node(DiagnosticLocation::Graph))?;
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
    region: &mut StructuredControlRegion,
    sources: &mut BTreeSet<PlanValueSource>,
    productions: &mut BTreeMap<ValueRef, OutputProduction>,
    function_abis: &BTreeMap<GraphResourcePath, FunctionPlanAbi>,
) -> Result<(), LowerGraphFailure> {
    match region {
        StructuredControlRegion::Sequence(steps) => {
            for step in steps {
                if let crate::node_system::plan::ControlStep::Region(region) = step {
                    collect_control_value_sources(region, sources, productions, function_abis)?;
                }
            }
        }
        StructuredControlRegion::If {
            then_region,
            else_region,
            results,
            ..
        } => {
            collect_control_value_sources(then_region, sources, productions, function_abis)?;
            collect_control_value_sources(else_region, sources, productions, function_abis)?;
            for binding in results {
                let then_production =
                    productions
                        .get(&binding.then_source)
                        .copied()
                        .ok_or_else(|| {
                            CompilerDiagnostic::PlanInvalid {}.into_node(DiagnosticLocation::Graph)
                        })?;
                let else_production =
                    productions
                        .get(&binding.else_source)
                        .copied()
                        .ok_or_else(|| {
                            CompilerDiagnostic::PlanInvalid {}.into_node(DiagnosticLocation::Graph)
                        })?;
                if then_production != else_production {
                    return Err(CompilerDiagnostic::PlanInvalid {}
                        .into_node(DiagnosticLocation::Graph)
                        .into());
                }
                binding.production = Some(then_production);
                productions.insert(binding.destination, then_production);
                sources.insert(PlanValueSource::ControlProduced(
                    binding.destination,
                    then_production,
                ));
            }
        }
        StructuredControlRegion::Loop { body, carried, .. } => {
            for binding in carried.iter_mut() {
                let initial = productions
                    .get(&binding.initial_source)
                    .copied()
                    .ok_or_else(|| {
                        CompilerDiagnostic::PlanInvalid {}.into_node(DiagnosticLocation::Graph)
                    })?;
                binding.production = Some(initial);
                productions.insert(binding.body_input, initial);
                sources.insert(PlanValueSource::ControlProduced(
                    binding.body_input,
                    initial,
                ));
            }
            collect_control_value_sources(body, sources, productions, function_abis)?;
            for binding in carried {
                let initial = binding.production.ok_or_else(|| {
                    CompilerDiagnostic::PlanInvalid {}.into_node(DiagnosticLocation::Graph)
                })?;
                let next = productions
                    .get(&binding.next_source)
                    .copied()
                    .ok_or_else(|| {
                        CompilerDiagnostic::PlanInvalid {}.into_node(DiagnosticLocation::Graph)
                    })?;
                if initial != next {
                    return Err(CompilerDiagnostic::PlanInvalid {}
                        .into_node(DiagnosticLocation::Graph)
                        .into());
                }
                productions.insert(binding.result, initial);
                sources.insert(PlanValueSource::ControlProduced(binding.result, initial));
            }
        }
        StructuredControlRegion::Call {
            target, results, ..
        } => {
            let path = GraphResourcePath(target.as_str().into());
            let abi = function_abis.get(&path).ok_or_else(|| {
                CompilerDiagnostic::PlanInvalid {}.into_node(DiagnosticLocation::Graph)
            })?;
            for binding in results {
                let production = abi
                    .results
                    .iter()
                    .find_map(|(parameter, value)| {
                        (*value == binding.callee_source)
                            .then(|| abi.result_productions.get(parameter).copied())
                            .flatten()
                    })
                    .ok_or_else(|| {
                        CompilerDiagnostic::PlanInvalid {}.into_node(DiagnosticLocation::Graph)
                    })?;
                if Some(production) != binding.production {
                    return Err(CompilerDiagnostic::PlanInvalid {}
                        .into_node(DiagnosticLocation::Graph)
                        .into());
                }
                productions.insert(binding.caller_destination, production);
                sources.insert(PlanValueSource::ControlProduced(
                    binding.caller_destination,
                    production,
                ));
            }
        }
    }
    Ok(())
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
        CompilerDiagnostic::NodeDisappeared {
            node_type: node.node_type_id.to_string().into(),
        }
        .into_node(DiagnosticLocation::Node(node.node_id))
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
    values.sort_by(compare_diagnostics);
    values.into_boxed_slice()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    struct DisappearingRegistry {
        fingerprint: RegistryFingerprint,
    }

    impl TypeEnvironment for DisappearingRegistry {
        fn concrete_implements(&self, _: &TypeId, _: &TypeClassId) -> Option<bool> {
            None
        }

        fn constructor_arity(&self, _: &TypeConstructorId) -> Option<usize> {
            None
        }
    }

    impl CompilerRegistry for DisappearingRegistry {
        fn fingerprint(&self) -> &RegistryFingerprint {
            &self.fingerprint
        }

        fn resolve(&self, _: &NodeTypeId) -> Option<RegistryNode<'_>> {
            None
        }
    }

    #[test]
    fn disappeared_node_emits_the_node_type_from_the_lowering_resolver() {
        let registry = DisappearingRegistry {
            fingerprint: RegistryFingerprint::from_bytes([9; 32]),
        };
        let node_id = NodeId::from_uuid(Uuid::from_u128(1));
        let node_type = NodeTypeId::new("yssbi.test.disappeared").unwrap();
        let node = ValidatedSemanticNode {
            node_id,
            node_type_id: node_type.clone(),
            protocol_fingerprint: ProtocolFingerprint::from_bytes([7; 32]),
            normalized_parameters: BTreeMap::new(),
            ports: Box::new([]),
        };

        let diagnostic = match resolve_for_lowering(&registry, &node) {
            Ok(_) => panic!("missing registry node must emit a diagnostic"),
            Err(diagnostic) => diagnostic,
        };

        assert_eq!(diagnostic.code.as_str(), "compiler.node.disappeared");
        assert_eq!(
            diagnostic.arguments,
            BTreeMap::from([(
                Box::from("node_type"),
                node_type.to_string().into_boxed_str(),
            )])
        );
        assert_eq!(diagnostic.primary, DiagnosticLocation::Node(node_id));
    }
}
