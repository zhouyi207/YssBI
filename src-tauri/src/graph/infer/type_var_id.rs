use serde::{Deserialize, Serialize};

/// 类型变量 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TypeVarId(pub u32);

impl TypeVarId {
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        TypeVarId(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

impl Default for TypeVarId {
    fn default() -> Self {
        Self::new()
    }
}
