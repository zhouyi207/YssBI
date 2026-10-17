//! Application use cases and desktop runtime initialization.

pub mod activity_panel;
pub mod automation;
pub mod chart;
pub mod database;
mod display_naming;
pub mod file_resources;
pub mod graph;
pub mod harness;
pub mod plugins;
pub mod presentation;
pub mod project;
mod result_encoding;
pub mod runtime;
pub mod session;

pub use session::ApplicationState;

pub mod events;
