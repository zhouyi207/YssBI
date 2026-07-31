//! Deterministic graph analysis and lowering pipeline.

mod control;
mod coordinator;
mod dependency;
pub mod dynamic_interface;
mod lowering;
mod pipeline;
mod project;
pub mod relational;
mod schema_analysis;
mod type_analysis;

pub use coordinator::{
    CompilationSlot, CompilationTask, CompileCancellationToken, CompileCancelled,
    CompileCoordinator, CompileProducts, PublishOutcome, PublishReport, ScheduleOutcome,
    compilation_basis,
};
pub use dynamic_interface::{
    InterfaceResolver, InterfaceResolverError, InterfaceResolverMember, InterfaceResolverRequest,
    InterfaceResolverSet, ProjectedDynamicPortBinding, SchemaFieldIdentityGuarantee,
    ValidatedInterfaceProjection, ValidatedNodeInterfaceProjection, ValidatedProjectedMember,
};
pub use lowering::{
    FragmentMetadata, FragmentResult, KernelFragment, LoweredKernel, LoweredNode, LoweringContext,
    LoweringError, NodeImplementation, NodeLowerer, RelationalInputBinding, RelationalNodeFragment,
    ScalarFragment,
};
#[cfg(test)]
pub(crate) use pipeline::compile_snapshot_invocations;
pub use pipeline::{
    CompilationSnapshot, CompileResult, CompilerAnalysis, CompilerRegistry, CompilerSemanticGraph,
    GraphCompiler, PublishedCompileAnalysis, RegistryNode, RegistryNodeBehavior, ResourceSnapshot,
};

pub type ProjectCompileCoordinator =
    CompileCoordinator<PublishedCompileAnalysis, crate::node_system::plan::ExecutionPlan>;
pub use project::{
    FUNCTION_CALL_ARGUMENTS_RESOLVER, FUNCTION_CALL_RESULTS_RESOLVER,
    FUNCTION_ENTRY_PARAMETERS_RESOLVER, FUNCTION_RETURN_RESULTS_RESOLVER,
    build_builtin_interface_resolvers, builtin_function_interface_resolver_ids,
};
pub use schema_analysis::{
    SchemaFact, SchemaResolutionContext, SchemaResolutionError, SchemaResolver, SchemaResolverSet,
};
pub use type_analysis::{TypeConstraintGraph, TypeEnvironment};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_dynamic;
#[cfg(test)]
mod tests_dynamic_pipeline;
