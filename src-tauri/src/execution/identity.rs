use uuid::Uuid;

/// Identity of one execution session. It is intentionally unrelated to the
/// Project and Graph session identifiers.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ExecutionSessionId(Uuid);

impl ExecutionSessionId {
    pub fn new(value: Uuid) -> Self {
        Self(value)
    }

    pub fn as_uuid(self) -> Uuid {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeGeneration(u64);

impl RuntimeGeneration {
    pub const INITIAL: Self = Self(0);

    pub const fn from_existing(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}
