use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct SciCancellationToken {
    cancelled: Arc<AtomicBool>,
}

pub struct SciCancellationSource {
    token: SciCancellationToken,
}

impl SciCancellationSource {
    pub fn new() -> (Self, SciCancellationToken) {
        let token = SciCancellationToken {
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        (
            Self {
                token: token.clone(),
            },
            token,
        )
    }

    pub fn cancel(&self) {
        self.token.cancelled.store(true, Ordering::Release);
    }

    pub fn token(&self) -> SciCancellationToken {
        self.token.clone()
    }
}

impl SciCancellationToken {
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

#[derive(Clone, Copy)]
pub struct AbsoluteDeadline(Instant);

impl AbsoluteDeadline {
    pub fn at(instant: Instant) -> Self {
        Self(instant)
    }

    pub fn is_expired(&self, now: Instant) -> bool {
        now >= self.0
    }

    pub fn remaining(&self, now: Instant) -> Option<Duration> {
        (!self.is_expired(now))
            .then(|| self.0.checked_duration_since(now))
            .flatten()
    }
}

pub struct ExecutionControl {
    cancellation: SciCancellationToken,
    deadline: AbsoluteDeadline,
}

impl ExecutionControl {
    pub fn new(cancellation: SciCancellationToken, deadline: AbsoluteDeadline) -> Self {
        Self {
            cancellation,
            deadline,
        }
    }

    pub fn cancellation(&self) -> &SciCancellationToken {
        &self.cancellation
    }

    pub fn deadline(&self) -> AbsoluteDeadline {
        self.deadline
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    pub fn is_expired(&self, now: Instant) -> bool {
        self.deadline.is_expired(now)
    }

    pub fn remaining(&self, now: Instant) -> Option<Duration> {
        self.deadline.remaining(now)
    }
}

pub struct CancelDeliveryControl {
    deadline: AbsoluteDeadline,
}

impl CancelDeliveryControl {
    pub fn new(deadline: AbsoluteDeadline) -> Self {
        Self { deadline }
    }

    pub fn deadline(&self) -> AbsoluteDeadline {
        self.deadline
    }

    pub fn is_expired(&self, now: Instant) -> bool {
        self.deadline.is_expired(now)
    }

    pub fn remaining(&self, now: Instant) -> Option<Duration> {
        self.deadline.remaining(now)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{AbsoluteDeadline, CancelDeliveryControl, ExecutionControl, SciCancellationSource};

    #[test]
    fn cancellation_source_sets_every_clone_once_and_never_resets() {
        let (source, token) = SciCancellationSource::new();
        let cloned = token.clone();

        assert!(!token.is_cancelled());
        source.cancel();
        source.cancel();

        assert!(token.is_cancelled());
        assert!(cloned.is_cancelled());
    }

    #[test]
    fn controls_preserve_explicit_monotonic_and_independent_deadlines() {
        let now = Instant::now();
        let future = now
            .checked_add(Duration::from_secs(5))
            .expect("short test deadline must be representable");
        let (source, token) = SciCancellationSource::new();
        source.cancel();

        let execution = ExecutionControl::new(token, AbsoluteDeadline::at(future));
        let cancel_delivery = CancelDeliveryControl::new(AbsoluteDeadline::at(now));

        assert!(execution.is_cancelled());
        assert!(!execution.is_expired(now));
        assert_eq!(execution.remaining(now), Some(Duration::from_secs(5)));
        assert!(!execution.deadline().is_expired(now));
        assert!(cancel_delivery.is_expired(now));
        assert_eq!(cancel_delivery.remaining(now), None);
        assert!(cancel_delivery.deadline().is_expired(now));
    }
}
