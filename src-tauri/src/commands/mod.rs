//! Tauri Command API Layer
//!
//! This module contains all Tauri commands organized by domain.
//! Commands are thin wrappers that handle parameter validation and delegate to services.

pub mod dataframe;
pub mod events;
pub mod execution;
pub mod functions;
pub mod macros;
pub mod nodes;
pub mod project;
pub mod schema;
pub mod settings;
pub mod variables;

// Re-export all commands for easy registration
pub use dataframe::*;
pub use events::*;
pub use execution::*;
pub use functions::*;
pub use macros::*;
pub use nodes::*;
pub use project::*;
pub use schema::*;
pub use settings::*;
pub use variables::*;
