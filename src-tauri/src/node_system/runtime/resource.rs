use super::RunError;
use crate::node_system::analysis::CompileProvenance;
use crate::node_system::plan::{CompiledResourceRequirement, ResourceId};
use std::any::Any;
use std::collections::BTreeMap;
use std::fmt;

pub trait ResourceLease: Send + Sync {
    fn resource_id(&self) -> &ResourceId;
    fn as_any(&self) -> &dyn Any;
}

pub trait ResourceProvider: Send + Sync {
    fn validate_plan(
        &self,
        _provenance: &CompileProvenance,
        _requirements: &[CompiledResourceRequirement],
    ) -> Result<(), ResourceError> {
        Ok(())
    }

    fn acquire(
        &self,
        requirement: &CompiledResourceRequirement,
    ) -> Result<Box<dyn ResourceLease>, ResourceError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceErrorKind {
    Acquire,
    SnapshotMismatch,
    UnsupportedAccess,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceError {
    kind: ResourceErrorKind,
    message: Box<str>,
}

impl ResourceError {
    pub fn new(message: impl Into<Box<str>>) -> Self {
        Self {
            kind: ResourceErrorKind::Acquire,
            message: message.into(),
        }
    }

    pub fn snapshot_mismatch(message: impl Into<Box<str>>) -> Self {
        Self {
            kind: ResourceErrorKind::SnapshotMismatch,
            message: message.into(),
        }
    }

    pub fn unsupported_access(message: impl Into<Box<str>>) -> Self {
        Self {
            kind: ResourceErrorKind::UnsupportedAccess,
            message: message.into(),
        }
    }

    pub fn kind(&self) -> ResourceErrorKind {
        self.kind
    }

    pub fn into_message(self) -> Box<str> {
        self.message
    }
}

impl fmt::Display for ResourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ResourceError {}

#[derive(Default)]
pub struct RunResourceSet {
    leases: BTreeMap<ResourceId, Box<dyn ResourceLease>>,
}

impl RunResourceSet {
    pub fn acquire(
        requirements: &[CompiledResourceRequirement],
        provider: &dyn ResourceProvider,
    ) -> Result<Self, RunError> {
        let mut leases = BTreeMap::new();
        for requirement in requirements {
            match provider.acquire(requirement) {
                Ok(lease) => {
                    leases.insert(requirement.resource.clone(), lease);
                }
                Err(_) if requirement.optional => {}
                Err(error) => {
                    return Err(RunError::ResourceAcquire {
                        resource: requirement.resource.clone(),
                        message: error.into_message(),
                    });
                }
            }
        }
        Ok(Self { leases })
    }

    pub fn get(&self, resource: &ResourceId) -> Option<&dyn ResourceLease> {
        self.leases.get(resource).map(Box::as_ref)
    }

    pub fn len(&self) -> usize {
        self.leases.len()
    }

    pub fn is_empty(&self) -> bool {
        self.leases.is_empty()
    }
}
