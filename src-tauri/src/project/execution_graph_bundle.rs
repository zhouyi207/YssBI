//! Frozen graph bodies for a single execution run.
//!
//! Live `ProjectState` graphs share `Arc<RwLock<GraphDataState>>` with the editor.
//! `prepare_execution_bundle` deep-snapshots every event/function graph reachable
//! via Call Function so mid-run edits cannot change in-flight execution.

use super::{ProjectData, ProjectStore};
use std::sync::{Arc, RwLock};

/// Immutable execution view: snapshotted graphs + shared project store.
#[derive(Clone)]
pub struct ExecutionGraphBundle {
    pub project_data: Arc<RwLock<ProjectData>>,
    pub project_store: Arc<RwLock<ProjectStore>>,
}
