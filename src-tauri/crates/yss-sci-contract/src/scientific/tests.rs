use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};

use super::{ScientificCancellationToken, ScientificExecutionControl};

#[test]
fn scientific_control_preserves_shared_cancellation_and_deadline() {
    let cancellation = Arc::new(AtomicBool::new(false));
    let deadline = Instant::now() + Duration::from_secs(5);
    let control = ScientificExecutionControl::from_shared(Arc::clone(&cancellation), deadline);
    assert!(!control.cancellation.is_cancelled());

    cancellation.store(true, std::sync::atomic::Ordering::Release);
    assert!(control.cancellation.is_cancelled());
    assert_eq!(control.deadline, deadline);

    let token = ScientificCancellationToken::new();
    let cloned = token.clone();
    token.cancel();
    assert!(cloned.is_cancelled());
}
