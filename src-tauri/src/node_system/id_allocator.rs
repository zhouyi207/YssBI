use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IdExhausted;

pub(crate) fn allocate_nonzero_id(allocator: &AtomicU64) -> Result<NonZeroU64, IdExhausted> {
    let id = allocator
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            NonZeroU64::new(current)?.get().checked_add(1)
        })
        .map_err(|_| IdExhausted)?;
    NonZeroU64::new(id).ok_or(IdExhausted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_nonzero_allocator_stays_exhausted_after_overflow() {
        let allocator = AtomicU64::new(u64::MAX - 1);

        assert_eq!(
            allocate_nonzero_id(&allocator).map(NonZeroU64::get),
            Ok(u64::MAX - 1)
        );
        for _ in 0..2 {
            assert_eq!(allocate_nonzero_id(&allocator), Err(IdExhausted));
        }
        assert_eq!(allocator.load(Ordering::Relaxed), u64::MAX);
    }
}
