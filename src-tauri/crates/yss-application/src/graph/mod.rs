//! Graph application use cases, execution coordination and result lifecycle.

pub mod catalog;
pub mod compile;
pub mod edit;
pub(crate) mod execution_mapping;
mod finalization;
pub(crate) mod inputs;
pub mod open;
pub(crate) mod preview_generation;
pub mod resources;
pub mod results;
pub mod run;

pub use execution_mapping::{GraphPackageMappingError, execution_package_from_graph};
pub use inputs::{GraphContractMappingError, GraphInputError};
