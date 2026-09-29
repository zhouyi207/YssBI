//! Rig adapter for the provider-neutral Harness Agent Driver port.

#![forbid(unsafe_code)]

mod arguments;
mod driver;
mod error;
mod messages;
mod provider;
mod stream;
mod tools;

pub use driver::RigAgentDriver;
pub use error::RigProviderConfigurationError;
pub use provider::{ConfigurableAgentDriver, openai_agent_driver};

#[cfg(test)]
mod tests;
