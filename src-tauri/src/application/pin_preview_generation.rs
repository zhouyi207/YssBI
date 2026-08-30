use std::sync::atomic::{AtomicU64, Ordering};
use yss_execution::identity::MAX_SAFE_PREVIEW_GENERATION;

#[derive(Debug)]
pub struct PinPreviewGenerationAllocator {
    current: AtomicU64,
}

impl PinPreviewGenerationAllocator {
    pub const fn new() -> Self {
        Self {
            current: AtomicU64::new(0),
        }
    }

    #[cfg(test)]
    const fn with_current(current: u64) -> Self {
        Self {
            current: AtomicU64::new(current),
        }
    }

    pub fn allocate(&self) -> Result<u64, PinPreviewGenerationExhausted> {
        self.current
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                (current < MAX_SAFE_PREVIEW_GENERATION).then_some(current + 1)
            })
            .map(|previous| previous + 1)
            .map_err(|_| PinPreviewGenerationExhausted)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PinPreviewGenerationExhausted;

static PIN_PREVIEW_GENERATIONS: PinPreviewGenerationAllocator =
    PinPreviewGenerationAllocator::new();

pub fn allocate_pin_preview_generation() -> Result<u64, PinPreviewGenerationExhausted> {
    PIN_PREVIEW_GENERATIONS.allocate()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exhaustion_never_wraps_or_reuses_the_maximum() {
        let allocator =
            PinPreviewGenerationAllocator::with_current(MAX_SAFE_PREVIEW_GENERATION - 1);

        assert_eq!(allocator.allocate(), Ok(MAX_SAFE_PREVIEW_GENERATION));
        assert_eq!(allocator.allocate(), Err(PinPreviewGenerationExhausted));
        assert_eq!(allocator.allocate(), Err(PinPreviewGenerationExhausted));
    }
}
