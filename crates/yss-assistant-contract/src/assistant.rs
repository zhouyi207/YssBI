//! Platform-neutral Assistant read projections over Harness events and tool records.
mod events;
mod inspection;

pub use events::{
    AssistantEvent, AssistantEventKind, AssistantProjectionError, AssistantToolIdentity,
    GRAPH_EXECUTION_NOTICE, GraphExecutionNotice, PLAN_PROPOSED_NOTICE,
};
pub use inspection::{AssistantResultReference, AssistantToolInspection};
