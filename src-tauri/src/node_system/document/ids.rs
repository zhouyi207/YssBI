use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

macro_rules! uuid_id {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            pub const fn from_uuid(value: Uuid) -> Self {
                Self(value)
            }

            pub const fn as_uuid(self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

uuid_id!(NodeId);
uuid_id!(ConnectionId);
uuid_id!(PortInstanceId);
uuid_id!(OperationId);
uuid_id!(HistoryEntryId);

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct GraphRevision(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RevisionExhausted {
    pub retained: u64,
}

impl std::fmt::Display for RevisionExhausted {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "revision is exhausted at {}", self.retained)
    }
}

impl std::error::Error for RevisionExhausted {}

impl GraphRevision {
    pub const INITIAL: Self = Self(0);

    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    pub const fn checked_next(self) -> Result<Self, RevisionExhausted> {
        match self.0.checked_add(1) {
            Some(next) => Ok(Self(next)),
            None => Err(RevisionExhausted { retained: self.0 }),
        }
    }

    #[cfg(test)]
    pub fn next(self) -> Self {
        self.checked_next().expect("test revision is available")
    }
}

pub type ResourceRevision = GraphRevision;

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct ProjectRevision(u64);

impl ProjectRevision {
    pub const INITIAL: Self = Self(0);

    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    pub const fn checked_next(self) -> Result<Self, RevisionExhausted> {
        match self.0.checked_add(1) {
            Some(next) => Ok(Self(next)),
            None => Err(RevisionExhausted { retained: self.0 }),
        }
    }

    #[cfg(test)]
    pub fn next(self) -> Self {
        self.checked_next()
            .expect("test project revision is available")
    }
}

pub type ProjectTransactionRevision = ProjectRevision;
