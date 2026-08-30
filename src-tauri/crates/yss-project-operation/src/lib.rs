//! Project-scoped operation admission and replay protection.
//!
//! Project publication remains owned by the caller. This crate owns only the
//! session-bound operation ledger and its RAII reservation lifecycle.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use thiserror::Error;
use yss_project_identity::{OperationId, ProjectInstanceId, ProjectSessionId};

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProjectOperationOwner {
    project_instance_id: ProjectInstanceId,
    project_session_id: ProjectSessionId,
}

#[derive(Debug)]
pub struct ProjectOperationLedger {
    owner: ProjectOperationOwner,
    in_flight: HashSet<OperationId>,
    completed: HashSet<OperationId>,
}

impl ProjectOperationLedger {
    pub fn new(
        project_instance_id: ProjectInstanceId,
        project_session_id: ProjectSessionId,
    ) -> Self {
        Self {
            owner: ProjectOperationOwner {
                project_instance_id,
                project_session_id,
            },
            in_flight: HashSet::new(),
            completed: HashSet::new(),
        }
    }

    pub fn reset_for_project(
        &mut self,
        project_instance_id: ProjectInstanceId,
        project_session_id: ProjectSessionId,
    ) {
        self.owner = ProjectOperationOwner {
            project_instance_id,
            project_session_id,
        };
        self.in_flight.clear();
        self.completed.clear();
    }

    pub fn reserve(
        ledger: Arc<Mutex<Self>>,
        project_instance_id: &ProjectInstanceId,
        operation_id: OperationId,
    ) -> Result<ProjectOperationReservation, ProjectOperationAdmissionError> {
        let mut state = ledger
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.owner.project_instance_id != *project_instance_id {
            return Err(ProjectOperationAdmissionError::StaleProject {
                requested_project_instance_id: project_instance_id.clone(),
                current_project_instance_id: state.owner.project_instance_id.clone(),
            });
        }
        if state.in_flight.contains(&operation_id) || state.completed.contains(&operation_id) {
            return Err(ProjectOperationAdmissionError::DuplicateOperation {
                operation_id,
                project_instance_id: project_instance_id.clone(),
            });
        }
        state.in_flight.insert(operation_id);
        let owner = state.owner.clone();
        drop(state);
        Ok(ProjectOperationReservation {
            ledger,
            owner,
            operation_id,
            completed: false,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ProjectOperationAdmissionError {
    #[error("resource operation belongs to a replaced project session")]
    StaleProject {
        requested_project_instance_id: ProjectInstanceId,
        current_project_instance_id: ProjectInstanceId,
    },
    #[error("operation '{operation_id}' was already admitted for project '{project_instance_id}'")]
    DuplicateOperation {
        operation_id: OperationId,
        project_instance_id: ProjectInstanceId,
    },
}

#[must_use = "dropping a reservation releases an uncompleted operation"]
#[derive(Debug)]
pub struct ProjectOperationReservation {
    ledger: Arc<Mutex<ProjectOperationLedger>>,
    owner: ProjectOperationOwner,
    operation_id: OperationId,
    completed: bool,
}

impl ProjectOperationReservation {
    pub fn complete(mut self) {
        let mut state = self
            .ledger
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.owner == self.owner {
            state.in_flight.remove(&self.operation_id);
            state.completed.insert(self.operation_id);
        }
        self.completed = true;
    }
}

impl Drop for ProjectOperationReservation {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        let mut state = self
            .ledger
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.owner == self.owner {
            state.in_flight.remove(&self.operation_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(label: &str) -> ProjectInstanceId {
        ProjectInstanceId::from_existing(format!("project-{label}"))
    }

    fn session(label: &str) -> ProjectSessionId {
        ProjectSessionId::new(format!("session-{label}"))
    }

    fn ledger(
        project_instance_id: ProjectInstanceId,
        project_session_id: ProjectSessionId,
    ) -> Arc<Mutex<ProjectOperationLedger>> {
        Arc::new(Mutex::new(ProjectOperationLedger::new(
            project_instance_id,
            project_session_id,
        )))
    }

    #[test]
    fn in_flight_operation_is_duplicate_and_drop_allows_retry() {
        let project = project("one");
        let ledger = ledger(project.clone(), session("one"));
        let operation = OperationId::new();
        let reservation =
            ProjectOperationLedger::reserve(Arc::clone(&ledger), &project, operation).unwrap();

        assert!(matches!(
            ProjectOperationLedger::reserve(Arc::clone(&ledger), &project, operation),
            Err(ProjectOperationAdmissionError::DuplicateOperation { .. })
        ));

        drop(reservation);
        assert!(ProjectOperationLedger::reserve(ledger, &project, operation).is_ok());
    }

    #[test]
    fn completed_operation_cannot_be_replayed() {
        let project = project("complete");
        let ledger = ledger(project.clone(), session("complete"));
        let operation = OperationId::new();
        ProjectOperationLedger::reserve(Arc::clone(&ledger), &project, operation)
            .unwrap()
            .complete();

        assert!(matches!(
            ProjectOperationLedger::reserve(ledger, &project, operation),
            Err(ProjectOperationAdmissionError::DuplicateOperation { .. })
        ));
    }

    #[test]
    fn replacement_rejects_the_previous_project_and_clears_replay_state() {
        let previous = project("previous");
        let next = project("next");
        let ledger = ledger(previous.clone(), session("previous"));
        let operation = OperationId::new();
        ProjectOperationLedger::reserve(Arc::clone(&ledger), &previous, operation)
            .unwrap()
            .complete();

        ledger
            .lock()
            .unwrap()
            .reset_for_project(next.clone(), session("next"));

        assert!(matches!(
            ProjectOperationLedger::reserve(Arc::clone(&ledger), &previous, operation),
            Err(ProjectOperationAdmissionError::StaleProject { .. })
        ));
        assert!(ProjectOperationLedger::reserve(ledger, &next, operation).is_ok());
    }

    #[test]
    fn canonical_session_id_isolates_reservations_when_instance_id_is_reused() {
        let project = project("reused");
        let ledger = ledger(project.clone(), session("old"));
        let operation = OperationId::new();
        let stale =
            ProjectOperationLedger::reserve(Arc::clone(&ledger), &project, operation).unwrap();

        ledger
            .lock()
            .unwrap()
            .reset_for_project(project.clone(), session("new"));
        let current =
            ProjectOperationLedger::reserve(Arc::clone(&ledger), &project, operation).unwrap();
        stale.complete();

        assert!(matches!(
            ProjectOperationLedger::reserve(Arc::clone(&ledger), &project, operation),
            Err(ProjectOperationAdmissionError::DuplicateOperation { .. })
        ));
        drop(current);
        assert!(ProjectOperationLedger::reserve(ledger, &project, operation).is_ok());
    }
}
