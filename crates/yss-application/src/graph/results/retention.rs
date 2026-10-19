use super::ResultQueryApplicationError;
use crate::session::{ApplicationSession, ApplicationState};
use std::sync::{Arc, Weak};
use uuid::Uuid;
use yss_graph_execution::result::{ResultReference, StoredResultSnapshot};

/// An in-process reader releases its lease in the original execution session.
pub struct ResultLease {
    session: Weak<ApplicationSession>,
    reference: ResultReference,
    id: Uuid,
}

impl ResultLease {
    pub fn reference(&self) -> ResultReference {
        self.reference
    }
}

impl Drop for ResultLease {
    fn drop(&mut self) {
        if let Some(session) = self.session.upgrade() {
            session.execution().release_result_lease(self.id);
        }
    }
}

impl ApplicationState {
    pub fn retain_owned_result(
        &self,
        reference: ResultReference,
    ) -> Result<(ResultLease, StoredResultSnapshot), ResultQueryApplicationError> {
        let captured = self.capture_session()?;
        if captured.execution_session_id() != reference.execution_session_id {
            return Err(ResultQueryApplicationError::SessionChanged);
        }
        let (id, snapshot) = captured.execution().retain_result(reference.result_id)?;
        let lease = ResultLease {
            session: Arc::downgrade(&captured),
            reference,
            id,
        };
        self.revalidate_captured_session(&captured)
            .map_err(|_| ResultQueryApplicationError::SessionChanged)?;
        Ok((lease, snapshot))
    }
}
