use super::ResultQueryApplicationError;
use crate::session::{ApplicationSession, ApplicationState};
use std::collections::BTreeSet;
use std::sync::{Arc, Weak};
use uuid::Uuid;
use yss_graph_execution::result::{ResultReference, StoredResultSnapshot};

/// An in-process reader releases its lease in the original execution session.
pub struct ResultLease {
    session: Weak<ApplicationSession>,
    reference: ResultReference,
    id: Uuid,
    owner: String,
}

impl ResultLease {
    pub fn reference(&self) -> ResultReference {
        self.reference
    }
}

impl Drop for ResultLease {
    fn drop(&mut self) {
        if let Some(session) = self.session.upgrade() {
            let _ = session
                .execution()
                .release_result_lease(self.id, &self.owner);
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
        let id = Uuid::new_v4();
        let owner = format!("native-result:{id}");
        let snapshot = captured
            .execution()
            .retain_result(reference.result_id, id, &owner, None)?;
        let lease = ResultLease {
            session: Arc::downgrade(&captured),
            reference,
            id,
            owner,
        };
        self.revalidate_captured_session(&captured)
            .map_err(|_| ResultQueryApplicationError::SessionChanged)?;
        Ok((lease, snapshot))
    }

    pub fn retain_result(
        &self,
        reference: ResultReference,
        lease_id: Uuid,
        owner: &str,
        handoff: Option<&str>,
    ) -> Result<StoredResultSnapshot, ResultQueryApplicationError> {
        let captured = self.capture_session()?;
        if captured.execution_session_id() != reference.execution_session_id {
            return Err(ResultQueryApplicationError::SessionChanged);
        }
        let snapshot =
            captured
                .execution()
                .retain_result(reference.result_id, lease_id, owner, handoff)?;
        if self.revalidate_captured_session(&captured).is_err() {
            let _ = captured.execution().release_result_lease(lease_id, owner);
            return Err(ResultQueryApplicationError::SessionChanged);
        }
        Ok(snapshot)
    }

    pub fn claim_result_lease(
        &self,
        lease_id: Uuid,
        owner: &str,
    ) -> Result<StoredResultSnapshot, ResultQueryApplicationError> {
        let captured = self.capture_session()?;
        let snapshot = captured.execution().claim_result_lease(lease_id, owner)?;
        if self.revalidate_captured_session(&captured).is_err() {
            let _ = captured.execution().release_result_lease(lease_id, owner);
            return Err(ResultQueryApplicationError::SessionChanged);
        }
        Ok(snapshot)
    }

    pub fn release_result_lease(
        &self,
        lease_id: Uuid,
        owner: &str,
    ) -> Result<(), ResultQueryApplicationError> {
        // Session teardown already releases its entire store, including outstanding leases.
        if let Ok(captured) = self.capture_session() {
            captured.execution().release_result_lease(lease_id, owner)?;
        }
        Ok(())
    }

    pub fn reconcile_result_leases(&self, owner: &str, active: &BTreeSet<Uuid>) {
        if let Ok(captured) = self.capture_session() {
            captured.execution().reconcile_result_leases(owner, active);
        }
    }

    pub fn close_result_owner(&self, owner: &str) {
        if let Ok(captured) = self.capture_session() {
            captured.execution().close_result_owner(owner);
        }
    }
}
