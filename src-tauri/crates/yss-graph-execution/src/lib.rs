//! Graph execution plans, kernels, runtime state, and result lifecycle.

#![deny(unused_must_use)]

pub mod error;
pub mod finalization;
pub mod graph_preparation;
pub mod identity;
mod kernel_invocation;
pub mod package_preparation;
pub mod plan;
pub mod ports;
pub mod resource_preparation;
pub mod result;
pub mod result_store;
pub mod run_registry;
pub mod state;

#[cfg(test)]
fn test_relations() -> std::sync::Arc<dyn yss_relational_contract::RelationFactory> {
    struct UnusedRelations;
    impl yss_relational_contract::RelationFactory for UnusedRelations {
        fn materialize(
            self: std::sync::Arc<Self>,
            _: arrow_array::RecordBatch,
            _: &yss_relational_contract::RelationControl,
        ) -> Result<yss_relational_contract::RelationHandle, yss_relational_contract::RelationError>
        {
            panic!("this unit test must not materialize a relation")
        }
    }
    std::sync::Arc::new(UnusedRelations)
}
