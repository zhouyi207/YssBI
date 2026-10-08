//! Platform-neutral Assistant read projections over Harness events and tool records.
mod events;
mod inspection;

pub use events::{AssistantEvent, AssistantEventKind, AssistantToolIdentity};
pub use inspection::{AssistantResultReference, AssistantToolInspection};
