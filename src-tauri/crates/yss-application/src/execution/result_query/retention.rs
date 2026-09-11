use super::ResultQueryApplicationError;
use crate::execution::ApplicationState;
use std::collections::BTreeSet;
use uuid::Uuid;
use yss_execution::result::{ResultReference, StoredResultSnapshot};

impl ApplicationState {
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
