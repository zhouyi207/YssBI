use super::*;

pub(super) struct PreparedOperation {
    pub(super) operation: OperationIndex,
    pub(super) owner_activation: ActivationId,
    pub(super) activation: ActivationId,
    pub(super) attempt: AttemptId,
    pub(super) input_result_ids: Box<[ResultId]>,
    pub(super) output_group: Option<ActivationResultGroup>,
    pub(super) memo_key: Option<OperationMemoKey>,
    pub(super) memo_policy: CachePolicy,
    pub(super) owns_memo_flight: bool,
    pub(super) awaits_memo_flight: bool,
    pub(super) reused_memo: bool,
    pub(super) class: WorkloadClass,
}

pub(super) struct DelayedRetry {
    pub(super) eligible_at: Instant,
    pub(super) tie_break: u64,
    pub(super) operation: OperationIndex,
    pub(super) owner_activation: ActivationId,
    pub(super) activation: ActivationId,
    pub(super) attempt: AttemptId,
    pub(super) input_result_ids: Box<[ResultId]>,
    pub(super) output_group: Option<ActivationResultGroup>,
    pub(super) memo_key: Option<OperationMemoKey>,
    pub(super) memo_policy: CachePolicy,
    pub(super) class: WorkloadClass,
}

impl PartialEq for DelayedRetry {
    fn eq(&self, other: &Self) -> bool {
        (self.eligible_at, self.tie_break) == (other.eligible_at, other.tie_break)
    }
}

impl Eq for DelayedRetry {}

impl PartialOrd for DelayedRetry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DelayedRetry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.eligible_at, self.tie_break).cmp(&(other.eligible_at, other.tie_break))
    }
}

pub(super) struct RunningOperation {
    pub(super) class: WorkloadClass,
    pub(super) owner_activation: ActivationId,
    pub(super) activation: ActivationId,
    pub(super) attempt: AttemptId,
    pub(super) input_result_ids: Box<[ResultId]>,
    pub(super) output_group: Option<ActivationResultGroup>,
    pub(super) memo_key: Option<OperationMemoKey>,
    pub(super) memo_policy: CachePolicy,
    pub(super) owns_memo_flight: bool,
    pub(super) reused_memo: bool,
}

pub(super) struct AdmissionBookkeeping {
    pub(super) operation: OperationIndex,
    pub(super) class: WorkloadClass,
    pub(super) activation_key: MemoKey,
    pub(super) previous_attempt: Option<AttemptId>,
    pub(super) memo_key: Option<OperationMemoKey>,
}

pub(super) struct MemoTables<'a> {
    pub(super) per_run: &'a SessionMemoization,
    pub(super) per_session: &'a SessionMemoization,
}

impl MemoTables<'_> {
    pub(super) fn for_policy(&self, policy: CachePolicy) -> Option<&SessionMemoization> {
        match policy {
            CachePolicy::Disabled => None,
            CachePolicy::PerRun => Some(self.per_run),
            CachePolicy::PerSession => Some(self.per_session),
        }
    }

    pub(super) fn abort_owned(&self, operation: &RunningOperation, error: RunError) {
        self.abort_flight(
            operation.owns_memo_flight,
            operation.memo_key.as_ref(),
            operation.memo_policy,
            error,
        );
    }

    pub(super) fn abort_prepared(&self, operation: &PreparedOperation, error: RunError) {
        self.abort_flight(
            operation.owns_memo_flight,
            operation.memo_key.as_ref(),
            operation.memo_policy,
            error,
        );
    }

    pub(super) fn abort_delayed(&self, operation: &DelayedRetry, error: RunError) {
        self.abort_flight(
            true,
            operation.memo_key.as_ref(),
            operation.memo_policy,
            error,
        );
    }

    pub(super) fn abort_flight(
        &self,
        owned: bool,
        key: Option<&OperationMemoKey>,
        policy: CachePolicy,
        error: RunError,
    ) {
        if !owned {
            return;
        }
        let Some(key) = key else {
            return;
        };
        let retryable = policy == CachePolicy::PerSession
            && matches!(
                error,
                RunError::Cancelled | RunError::DeadlineExceeded { .. }
            );
        self.for_policy(policy)
            .expect("memoized operation has a memo table")
            .abort(key, error, retryable);
    }
}

pub(super) struct WorkerCompletion {
    pub(super) completed_at: Instant,
    pub(super) completion: OperationCompletion,
    pub(super) panic: Option<Box<dyn std::any::Any + Send>>,
}
