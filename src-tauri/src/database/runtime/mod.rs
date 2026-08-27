use crate::database::error::{DatabaseError, DatabaseOperation};
use crate::database_contract::{
    DatabaseDecl, DatabaseDeclarationObservationSet, DatabaseId, DatabaseSessionIdentity,
    DatabaseSessionOpenRequest,
};
use std::collections::HashMap;
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

#[derive(Clone, Default)]
pub struct DatabaseRuntimeRegistry;

struct DatabaseAdmissionState {
    closed: bool,
}

pub struct DatabaseRuntimeSession {
    basis: DatabaseSessionBasis,
    admission: Mutex<DatabaseAdmissionState>,
    drain: Arc<DatabaseSessionDrainState>,
    revisions: Mutex<HashMap<DatabaseId, DatabaseRuntimeRevisions>>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct DatabaseRuntimeRevisions {
    pub(crate) runtime: u64,
    pub(crate) schema: u64,
}

struct DatabaseSessionBasis {
    identity: DatabaseSessionIdentity,
    generation: NonZeroU64,
    _root: Option<PathBuf>,
    _declarations: Arc<[DatabaseDecl]>,
    _observations: DatabaseDeclarationObservationSet,
}

#[derive(Clone)]
pub struct DatabaseSessionDrainControl {
    deadline: DatabaseDrainDeadline,
}

#[derive(Debug)]
struct DatabaseSessionDrainState {
    state: Mutex<DatabaseDrainState>,
    changed: Condvar,
}

#[derive(Debug)]
struct DatabaseDrainState {
    draining: bool,
    outstanding: DatabaseOutstandingWork,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DatabaseDrainDeadline(Instant);

impl DatabaseDrainDeadline {
    pub fn at(deadline: Instant) -> Self {
        Self(deadline)
    }

    fn remaining(&self, now: Instant) -> Option<Duration> {
        self.0.checked_duration_since(now)
    }
}

impl DatabaseSessionDrainControl {
    pub fn new(deadline: DatabaseDrainDeadline) -> Self {
        Self { deadline }
    }

    pub fn deadline(&self) -> DatabaseDrainDeadline {
        self.deadline
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatabaseAdmissionCloseOutcome {
    Closed,
    AlreadyClosed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatabaseDrainOutcome {
    Drained {
        outstanding: DatabaseOutstandingWork,
    },
    TimedOut {
        outstanding: DatabaseOutstandingWork,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DatabaseOutstandingWork {
    operation_leases: usize,
    pending_prepares: usize,
    recoveries: usize,
}

impl DatabaseOutstandingWork {
    #[cfg(test)]
    pub(crate) const fn operation_leases(self) -> usize {
        self.operation_leases
    }

    fn is_empty(self) -> bool {
        self.operation_leases == 0 && self.pending_prepares == 0 && self.recoveries == 0
    }
}

#[must_use = "database operation leases are released when this guard is dropped"]
#[derive(Debug)]
#[allow(
    dead_code,
    reason = "the operation lease is staged until a later Database operation seam"
)]
pub(crate) struct DatabaseOperationLease {
    shared: Arc<DatabaseSessionDrainState>,
}

impl DatabaseRuntimeRegistry {
    pub fn new() -> Self {
        Self
    }

    pub fn open_session(
        &self,
        request: DatabaseSessionOpenRequest,
    ) -> Result<DatabaseRuntimeSession, DatabaseError> {
        request
            .validate()
            .map_err(|_| DatabaseError::invalid_request(DatabaseOperation::OpenSession, None))?;
        let (identity, generation, root, declarations, observations) = request.into_parts();
        let revisions = declarations
            .iter()
            .map(|declaration| (declaration.id.clone(), DatabaseRuntimeRevisions::default()))
            .collect();
        Ok(DatabaseRuntimeSession {
            basis: DatabaseSessionBasis {
                identity,
                generation,
                _root: root,
                _declarations: declarations,
                _observations: observations,
            },
            admission: Mutex::new(DatabaseAdmissionState { closed: false }),
            drain: Arc::new(DatabaseSessionDrainState {
                state: Mutex::new(DatabaseDrainState {
                    draining: false,
                    outstanding: DatabaseOutstandingWork::default(),
                }),
                changed: Condvar::new(),
            }),
            revisions: Mutex::new(revisions),
        })
    }
}

impl DatabaseRuntimeSession {
    pub fn identity(&self) -> &DatabaseSessionIdentity {
        &self.basis.identity
    }

    pub fn generation(&self) -> NonZeroU64 {
        self.basis.generation
    }

    pub(crate) fn declarations(&self) -> &[DatabaseDecl] {
        &self.basis._declarations
    }

    pub(crate) fn observations(&self) -> &DatabaseDeclarationObservationSet {
        &self.basis._observations
    }

    pub(crate) fn revisions(&self, database: &DatabaseId) -> Option<DatabaseRuntimeRevisions> {
        lock_or_recover(&self.revisions).get(database).copied()
    }

    pub(crate) fn advance_revisions(
        &self,
        database: &DatabaseId,
        schema_changed: bool,
    ) -> Result<DatabaseRuntimeRevisions, DatabaseError> {
        let mut revisions = lock_or_recover(&self.revisions);
        let current = revisions.get_mut(database).ok_or_else(|| {
            DatabaseError::not_found(DatabaseOperation::CommitMutation, Some(database.clone()))
        })?;
        current.runtime = current.runtime.checked_add(1).ok_or_else(|| {
            DatabaseError::conflict(DatabaseOperation::CommitMutation, Some(database.clone()))
        })?;
        if schema_changed {
            current.schema = current.schema.checked_add(1).ok_or_else(|| {
                DatabaseError::conflict(DatabaseOperation::CommitMutation, Some(database.clone()))
            })?;
        }
        Ok(*current)
    }

    pub fn close_admission(&self) -> DatabaseAdmissionCloseOutcome {
        let mut state = lock_or_recover(&self.admission);
        if state.closed {
            DatabaseAdmissionCloseOutcome::AlreadyClosed
        } else {
            state.closed = true;
            DatabaseAdmissionCloseOutcome::Closed
        }
    }

    #[allow(
        dead_code,
        reason = "admission is staged until a later Database operation seam"
    )]
    pub(crate) fn admit_operation(
        &self,
        operation: DatabaseOperation,
    ) -> Result<DatabaseOperationLease, DatabaseError> {
        let admission = lock_or_recover(&self.admission);
        if admission.closed {
            return Err(DatabaseError::admission_closed(operation, None));
        }

        let mut drain = lock_or_recover(&self.drain.state);
        if drain.draining {
            return Err(DatabaseError::admission_closed(operation, None));
        }
        drain.outstanding.operation_leases += 1;
        drop(drain);
        drop(admission);
        Ok(DatabaseOperationLease {
            shared: Arc::clone(&self.drain),
        })
    }

    pub fn drain(&self, control: &DatabaseSessionDrainControl) -> DatabaseDrainOutcome {
        let admission = lock_or_recover(&self.admission);
        let mut state = lock_or_recover(&self.drain.state);
        state.draining = true;
        drop(admission);
        loop {
            if state.outstanding.is_empty() {
                return DatabaseDrainOutcome::Drained {
                    outstanding: state.outstanding,
                };
            }

            let now = Instant::now();
            let Some(remaining) = control.deadline.remaining(now) else {
                return DatabaseDrainOutcome::TimedOut {
                    outstanding: state.outstanding,
                };
            };
            let (next_state, wait_result) = self
                .drain
                .changed
                .wait_timeout(state, remaining)
                .unwrap_or_else(PoisonError::into_inner);
            state = next_state;
            if wait_result.timed_out() && !state.outstanding.is_empty() {
                return DatabaseDrainOutcome::TimedOut {
                    outstanding: state.outstanding,
                };
            }
        }
    }
}

impl Drop for DatabaseOperationLease {
    fn drop(&mut self) {
        let mut state = lock_or_recover(&self.shared.state);
        debug_assert!(state.outstanding.operation_leases > 0);
        state.outstanding.operation_leases = state.outstanding.operation_leases.saturating_sub(1);
        drop(state);
        self.shared.changed.notify_all();
    }
}

fn lock_or_recover<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
