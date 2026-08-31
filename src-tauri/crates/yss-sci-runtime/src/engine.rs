//! Scientific-computing execution context.

#[derive(Debug, Clone, Copy, Default)]
pub struct SciContext;

impl SciContext {
    pub const fn rust() -> Self {
        Self
    }
}
