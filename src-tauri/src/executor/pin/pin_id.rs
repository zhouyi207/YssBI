//! Pin 标识符

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Pin 唯一标识符
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PinId(Uuid);

impl PinId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn nil() -> Self {
        Self(Uuid::nil())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for PinId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for PinId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for PinId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<PinId> for Uuid {
    fn from(id: PinId) -> Self {
        id.0
    }
}
