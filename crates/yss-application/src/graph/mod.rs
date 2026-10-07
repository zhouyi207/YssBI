//! Graph application use cases, execution coordination and result lifecycle.

pub mod catalog;
pub mod edit;
pub mod editing;
mod finalization;
pub(crate) mod inputs;
pub mod open;
pub mod resources;
pub mod results;
pub mod run;

pub use inputs::{GraphContractMappingError, GraphInputError};
