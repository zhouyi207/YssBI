//! Deterministic graph analysis and lowering pipeline.

mod control;
mod coordinator;
mod dependency;
mod diagnostics;
pub mod dynamic_interface;
mod lowering;
mod pipeline;
mod project;
pub mod relational;
mod schema_analysis;
mod specialization;
mod type_analysis;

pub use coordinator::{
    CompilationSlot, CompilationTask, CompileCancellationToken, CompileCancelled,
    CompileCoordinator, CompileProducts, PublishOutcome, PublishReport, PublishedExecutionPlan,
    ScheduleOutcome, SelectedExecutionPlan, compilation_basis,
};
pub use diagnostics::CompilerDiagnosticDefinitionError;
pub(crate) use diagnostics::{
    COMPILER_DIAGNOSTIC_DEFINITIONS, CompilerDiagnostic, CompilerDiagnosticLocation,
    CompilerNodeDiagnostic, compare_diagnostics, managed_node_role_name, node_scope_name,
    port_kind_name, validate_compiler_diagnostic_definitions,
};
pub use dynamic_interface::{
    InterfaceResolver, InterfaceResolverError, InterfaceResolverMember, InterfaceResolverRequest,
    InterfaceResolverSet, ProjectedDynamicPortBinding, SchemaFieldIdentityGuarantee,
    ValidatedInterfaceProjection, ValidatedNodeInterfaceProjection, ValidatedProjectedMember,
};
pub use lowering::{
    FragmentMetadata, FragmentResult, KernelFragment, LoweredKernel, LoweredNode, LoweringContext,
    LoweringError, LoweringInvariant, NodeImplementation, NodeLowerer, PreparedParameterValue,
    RelationalInputBinding, RelationalNodeFragment, ScalarFragment, ValidatedNodeConfig,
};
#[cfg(test)]
pub(crate) use pipeline::compile_snapshot_invocations;
pub use pipeline::{
    CompilationSnapshot, CompileResult, CompilerAnalysis, CompilerRegistry, CompilerSemanticGraph,
    GraphCompiler, PublishedCompileAnalysis, RegistryNode, RegistryNodeBehavior, ResourceSnapshot,
};

pub type ProjectCompileCoordinator =
    CompileCoordinator<PublishedCompileAnalysis, std::sync::Arc<PublishedExecutionPlan>>;
pub use project::{
    FUNCTION_CALL_ARGUMENTS_RESOLVER, FUNCTION_CALL_RESULTS_RESOLVER,
    FUNCTION_ENTRY_PARAMETERS_RESOLVER, FUNCTION_RETURN_RESULTS_RESOLVER,
    build_builtin_interface_resolvers, builtin_function_interface_resolver_ids,
};
pub use schema_analysis::{
    SchemaFact, SchemaResolutionContext, SchemaResolutionError, SchemaResolver, SchemaResolverSet,
};
pub use specialization::{DemandPlanError, ExecutionPlanBasis, NormalizedExecutionDemand};
pub use type_analysis::{TypeConstraintGraph, TypeEnvironment};

#[cfg(test)]
mod task1_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_dynamic;
#[cfg(test)]
mod tests_dynamic_pipeline;
