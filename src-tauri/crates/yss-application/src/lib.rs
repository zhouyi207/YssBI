//! Application use cases and desktop runtime initialization.

pub mod activity_panel;
pub mod automation;
pub mod chart;
pub mod database;
pub mod graph;
pub mod harness;
mod ipc;
pub mod plugins;
pub mod presentation;
pub mod project;
mod result_encoding;
pub mod runtime;
pub mod session;

pub use ipc::invoke_handler;
pub use runtime::initialize;
pub use session::ApplicationState;

pub mod events;
