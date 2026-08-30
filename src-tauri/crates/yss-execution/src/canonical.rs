use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ExecutionCommitId(u64);

impl ExecutionCommitId {
    pub const fn from_existing(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CommittedEffectReceipt {
    commit: ExecutionCommitId,
}

impl CommittedEffectReceipt {
    #[allow(
        dead_code,
        reason = "receipt construction is activated by the runtime cutover"
    )]
    pub(crate) const fn new(commit: ExecutionCommitId) -> Self {
        Self { commit }
    }

    pub const fn commit(self) -> ExecutionCommitId {
        self.commit
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum CanonicalExecutionError {
    #[error("execution commit identity is invalid")]
    InvalidCommitIdentity,
}
