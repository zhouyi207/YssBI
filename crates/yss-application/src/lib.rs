//! Application use cases and desktop runtime initialization.

pub mod activity_panel;
pub mod automation;
pub mod chart;
pub mod database;
mod docs;
pub mod file_resources;
pub mod graph;
pub mod harness;
#[cfg(feature = "tauri-host")]
mod ipc;
mod minds;
pub mod plugins;
pub mod presentation;
pub mod project;
mod result_encoding;
pub mod runtime;
pub mod session;

#[cfg(feature = "tauri-host")]
pub use ipc::invoke_handler;
#[cfg(feature = "tauri-host")]
pub use ipc::runtime::initialize;
pub use session::ApplicationState;

pub mod events;
