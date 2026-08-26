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
use crate::graph_document::{
    ConnectionId, DynamicMemberLocator, DynamicPortBinding, FunctionParameterId, GraphDocument,
    GraphResourcePath, GraphRevision, NodeId, PortAddress, PortRef,
};
use crate::node_system::ProjectSessionId;
use crate::node_system::analysis::{
    AnalysisResourceReads, AnalysisResourceResolver, AnalysisSnapshot, AnalyzedNode,
    CompilationBasis, CompileId, CompileProvenance, ControlEdge, DiagnosticLocation,
    NodeDiagnostic, ResolvedDatabase, ResolvedDatabaseValue, ResolvedFunction,
    ResolvedFunctionValue, ResolvedInterface, ResolvedPort, ResolvedResource, ResolvedVariable,
    ResourceKey, ResourceObservationSet, ResourceObservedState, ResourceResolutionError,
    ResourceVersion, ResourceVersionSet, SemanticDependency, ValidatedSemanticGraph,
    ValidatedSemanticNode, ValidatedSemanticPort, ValueEdge,
};
use crate::node_system::document::FunctionDocument;
use crate::node_system::plan::{
    CompiledParameterHandle, CompiledResourceRequirement, ControlStep,
    EXECUTION_SEMANTICS_SCHEMA_VERSION, EffectDependency as PlannedEffectDependency, ExecutionPlan,
    ExecutionSemanticsVersion, FunctionPlanAbi, GraphOutputRef, KernelHandle, OperationIndex,
    OperationStableId, PlanResult, PlanValueSource, PlannedInput, PlannedOutput, PlannedRetry,
    PlannedValueContract, PlannedValueKind, RelationalBackendId, ResourceAccess, ResourceId,
    ResourceKind, StructuredControlRegion, ValueDependency, ValueRef, WorkloadClass,
};
use crate::node_system::protocol::{
    CachePolicy, ConnectionsPerPort, Determinism, EffectSemantics, EvaluationPolicy,
    InputConsumption, LiteralPolicy, NodeInstanceDisplaySpec, NodeProtocol, NodeTypeId,
    OutputProduction, ParameterEditorSpec, ParameterIssueKind, PortDirection, PortInstances,
    PortKind, PortSpec, Purity, ResolvedSchemaFact, ResourceDisplayKind, RetryPolicy, SchemaExpr,
    TypeClassId, TypeConstructorId, TypeExpr, TypeId, Value, validate_and_prepare_parameter_values,
};
use crate::node_system::registry::{
    NodeRegistry, PreparedNominalValue, ProtocolFingerprint, RegistryFingerprint,
    StructuralNodeRole, TransparentNodeRole, hash_canonical,
};
use std::collections::{BTreeMap, BTreeSet};
#[cfg(test)]
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "pipeline/analysis_state.rs"]
mod analysis_state;
#[path = "pipeline/call_closure.rs"]
mod call_closure;
#[path = "pipeline/function_abi.rs"]
mod function_abi;
#[path = "pipeline/lowering.rs"]
mod lowering;
#[path = "pipeline/registry_adapter.rs"]
mod registry_adapter;
#[path = "pipeline/resource_snapshot.rs"]
mod resource_snapshot;
use analysis_state::AnalysisState;
use function_abi::derive_function_abi;
pub(crate) use function_abi::finalize_function_abi_productions;
use lowering::{
    LowerGraphFailure, call_member_role, function_target, lower_graph, protocol_value_to_json,
    structural_role_name,
};
#[allow(unused_imports)]
pub(crate) use lowering::{PendingKernel, effective_cache_policy, effective_retry_policy};
use registry_adapter::CompilerNominalValidator;
pub use registry_adapter::{CompilerRegistry, RegistryNode, RegistryNodeBehavior};
pub use resource_snapshot::ResourceSnapshot;
use resource_snapshot::TrackedResourceResolver;

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

#[derive(Debug, Clone, PartialEq)]
pub struct CompilationSnapshot {
    pub provenance: CompileProvenance,
    pub document: GraphDocument,
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
    pub interface_projection: ValidatedInterfaceProjection,
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
}

#[cfg(test)]
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
        }
    }

    pub fn with_project_session_id(mut self, project_session_id: ProjectSessionId) -> Self {
        self.project_session_id = project_session_id;
        self
    }

    /// Captures every input identity needed to decide whether a result is still current.
    /// Callers should invoke this while holding their project read transaction, then release
    /// that lock before calling `compile_snapshot`.
    #[cfg(test)]
    pub fn snapshot(
        &self,
        graph_path: GraphResourcePath,
        document: &GraphDocument,
    ) -> CompilationSnapshot {
        let compile_id = CompileId::new(
            crate::node_system::allocate_nonzero_id(&NEXT_ADHOC_COMPILE_ID)
                .expect("process compile ID space exhausted")
                .get(),
        );
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
        CompilationSnapshot {
            provenance,
            document: document.clone(),
        }
    }

    pub fn compile_snapshot(
        &self,
        snapshot: &CompilationSnapshot,
        cancellation: &CompileCancellationToken,
    ) -> Result<CompileResult, CompileCancelled> {
        self.compile_snapshot_inner(snapshot, cancellation)
    }

    fn compile_snapshot_inner(
        &self,
        snapshot: &CompilationSnapshot,
        cancellation: &CompileCancellationToken,
    ) -> Result<CompileResult, CompileCancelled> {
        #[cfg(test)]
        COMPILE_SNAPSHOT_INVOCATIONS.fetch_add(1, Ordering::Relaxed);
        if let Err(error) = cancellation.checkpoint() {
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
        let closure = match self.function_abis_for_calls(
            &provisional_semantic,
            snapshot.provenance.compile_id,
            &mut resources,
            cancellation,
        ) {
            Ok(closure) => closure,
            Err(error) => return Err(error),
        };
        state.diagnostics.extend(closure.diagnostics);
        let mut function_abis = closure.abis;
        state.basis.resource_versions = resources.reads().clone();
        state.basis.resource_observations = resources.observations().clone();
        let decoded_literals = state.decoded_literals.clone();
        let mut analysis = state.snapshot();
        if let Err(error) = cancellation.checkpoint() {
            return Err(error);
        }
        if analysis.has_blocking_errors() {
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
                    .map(|abi| {
                        let plan = plan.as_ref().ok_or(())?;
                        finalize_function_abi_productions(plan, abi).map_err(|_| ())
                    })
                    .transpose();
                if abi_finalized.is_err() {
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
                    (
                        Some(semantic),
                        Some(basis),
                        plan,
                        CompilationOutcome::Succeeded,
                    )
                }
            }
            Err(LowerGraphFailure::Cancelled(error)) => return Err(error),
            Err(LowerGraphFailure::Internal(failure)) => (
                Some(semantic),
                None,
                None,
                CompilationOutcome::InternalFailure(failure),
            ),
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

    #[cfg(test)]
    pub fn compile(&self, document: &GraphDocument) -> CompileResult {
        let snapshot = self.snapshot(
            GraphResourcePath::new("events/test.yssbi-event")
                .expect("test graph resource path is valid"),
            document,
        );
        self.compile_snapshot(&snapshot, &CompileCancellationToken::new())
            .expect("a fresh cancellation token is not cancelled")
    }
}

fn semantic_for_lowering<R: CompilerRegistry>(
    registry: &R,
    semantic: CompilerSemanticGraph,
) -> CompilerSemanticGraph {
    collapse_transparent_nodes(registry, semantic)
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
