use crate::project::{ProjectFilesystemError, ProjectState};
use std::collections::HashSet;
use yss_project_identity::OperationId;
use yss_project_identity::ProjectInstanceId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ResourceOperationOwner {
    project_instance_id: ProjectInstanceId,
    session_epoch: uuid::Uuid,
}

pub(crate) struct ResourceOperationLedger {
    owner: ResourceOperationOwner,
    in_flight: HashSet<OperationId>,
    completed: HashSet<OperationId>,
}

impl ResourceOperationLedger {
    pub(in crate::project) fn new(project_instance_id: ProjectInstanceId) -> Self {
        Self {
            owner: ResourceOperationOwner {
                project_instance_id,
                session_epoch: uuid::Uuid::new_v4(),
            },
            in_flight: HashSet::new(),
            completed: HashSet::new(),
        }
    }

    pub(in crate::project) fn reset_for_project(&mut self, project_instance_id: ProjectInstanceId) {
        self.owner = ResourceOperationOwner {
            project_instance_id,
            session_epoch: uuid::Uuid::new_v4(),
        };
        self.in_flight.clear();
        self.completed.clear();
    }

    fn reserve(
        ledger: std::sync::Arc<std::sync::Mutex<Self>>,
        project_instance_id: &ProjectInstanceId,
        operation_id: OperationId,
    ) -> Result<ResourceOperationReservation, ProjectFilesystemError> {
        let mut state = ledger
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.owner.project_instance_id != *project_instance_id {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "resource operation belongs to a replaced project session".into(),
            });
        }
        if state.in_flight.contains(&operation_id) || state.completed.contains(&operation_id) {
            return Err(ProjectFilesystemError::DuplicateOperation {
                message: format!(
                    "operation '{}' was already admitted for project '{}'",
                    operation_id, project_instance_id
                ),
            });
        }
        state.in_flight.insert(operation_id);
        let owner = state.owner.clone();
        drop(state);
        Ok(ResourceOperationReservation {
            ledger,
            owner,
            operation_id,
            completed: false,
        })
    }
}

pub(crate) struct ResourceOperationReservation {
    ledger: std::sync::Arc<std::sync::Mutex<ResourceOperationLedger>>,
    owner: ResourceOperationOwner,
    operation_id: OperationId,
    completed: bool,
}

impl ResourceOperationReservation {
    pub(crate) fn complete(mut self) {
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

impl Drop for ResourceOperationReservation {
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

impl ProjectState {
    pub(crate) fn reserve_resource_operation(
        &self,
        project_instance_id: &ProjectInstanceId,
        operation_id: OperationId,
    ) -> Result<ResourceOperationReservation, ProjectFilesystemError> {
        let publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != project_instance_id.as_str() {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project changed before resource operation admission".into(),
            });
        }
        let reservation = ResourceOperationLedger::reserve(
            std::sync::Arc::clone(&self.resource_operations),
            project_instance_id,
            operation_id,
        );
        drop(publication);
        reservation
    }
}
