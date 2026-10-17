//! Deferred work that outlives the view or window it reports back to.
use std::fmt::Display;
use std::panic::Location;

/// Async results, prompts and menu commands update a view the user may already have closed.
/// That release is expected, so it stays visible at debug level instead of being discarded.
pub trait LogReleased<T> {
    fn log_released(self) -> Option<T>;
}

impl<T, E: Display> LogReleased<T> for Result<T, E> {
    #[track_caller]
    fn log_released(self) -> Option<T> {
        match self {
            Ok(value) => Some(value),
            Err(error) => {
                let caller = Location::caller();
                tracing::debug!(%error, %caller, "deferred update target released");
                None
            }
        }
    }
}
