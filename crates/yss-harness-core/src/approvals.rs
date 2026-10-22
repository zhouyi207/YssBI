//! User approval of tools that run code on the user's machine.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use tokio::sync::oneshot;
use yss_harness_contract::{ApprovalDecision, ProjectSessionBinding};

#[derive(Default)]
pub(crate) struct Approvals {
    pending: Mutex<BTreeMap<String, oneshot::Sender<ApprovalDecision>>>,
    // Kept in memory only: a restart asks again.
    allowed: Mutex<BTreeSet<(String, String)>>,
}

impl Approvals {
    pub(crate) fn allowed(&self, project: &ProjectSessionBinding, tool: &str) -> bool {
        self.allowed
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(&(project.project_instance_id().to_string(), tool.to_owned()))
    }

    pub(crate) fn register(&self, approval_id: String) -> oneshot::Receiver<ApprovalDecision> {
        let (sender, receiver) = oneshot::channel();
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(approval_id, sender);
        receiver
    }

    pub(crate) fn forget(&self, approval_id: &str) {
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(approval_id);
    }

    pub(crate) fn remember(&self, project: &ProjectSessionBinding, tool: &str) {
        self.allowed
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert((project.project_instance_id().to_string(), tool.to_owned()));
    }

    /// Whether a request with this id was still waiting.
    pub(crate) fn resolve(&self, approval_id: &str, decision: ApprovalDecision) -> bool {
        let sender = self
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(approval_id);
        sender.is_some_and(|sender| sender.send(decision).is_ok())
    }
}
