//! Domain-neutral Harness engine: sessions, turns, Manager–Worker orchestration, the durable
//! tool ledger and knowledge retrieval. Domain tools, roles and authority are supplied by a
//! `HarnessExtension`.

#![forbid(unsafe_code)]

mod conversation;
mod error;
mod events;
mod host;
mod knowledge;
mod orchestration;
mod ports;
mod tools;

pub use error::HarnessError;
pub use host::{HarnessHost, HarnessSessionAccess};
pub use knowledge::{KnowledgeError, KnowledgeQuery, KnowledgeService};
pub use ports::HarnessPorts;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
