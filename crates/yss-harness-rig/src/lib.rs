//! Rig adapter for the provider-neutral Harness Agent Driver port.

#![forbid(unsafe_code)]

mod arguments;
mod context;
mod driver;
mod error;
mod messages;
mod provider;
mod recovery;
mod run_options;
mod stream;
mod tools;

pub use driver::RigAgentDriver;
pub use error::RigProviderConfigurationError;
pub use provider::{RigProviderClient, provider_presets};
pub use run_options::model_default_reasoning_effort;

#[cfg(test)]
mod tests;
