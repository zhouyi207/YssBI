use std::fmt;
use uuid::Uuid;

/// Opaque native identity of a directory, independent of its current path spelling.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct RootIdentity(String);

impl RootIdentity {
    pub(crate) fn from_native(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Identity of one filesystem transaction and its private staging directory.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct TransactionId(Uuid);

impl TransactionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub const fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }
}

impl Default for TransactionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TransactionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}
