mod catalog;
mod common;
mod editor;
mod execution;
mod leases;
mod reports;
mod resources;
mod results;

// The nested registry also needs the wrapper macros emitted next to each command.
pub use catalog::*;
pub use editor::*;
pub use execution::*;
pub use leases::*;
pub use reports::*;
pub use resources::*;
pub use results::*;
