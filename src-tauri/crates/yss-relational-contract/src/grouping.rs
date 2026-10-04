//! Immutable grouping intent and a controlled, sequential group mapping boundary.
use crate::{RelationControl, RelationError, RelationHandle};
use std::{collections::BTreeSet, sync::Arc};

/// Proof of identical rows in identical order. It contains no query, data or resource lease.
#[derive(Clone, Debug, Default)]
pub struct RelationRowIdentity(Arc<()>);

impl PartialEq for RelationRowIdentity {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for RelationRowIdentity {}

#[derive(Clone, Debug, PartialEq)]
pub struct GroupedRelationHandle {
    source: RelationHandle,
    keys: Arc<[Box<str>]>,
}

impl GroupedRelationHandle {
    pub fn new(source: RelationHandle, keys: Arc<[Box<str>]>) -> Result<Self, RelationError> {
        if keys.is_empty()
            || keys.iter().collect::<BTreeSet<_>>().len() != keys.len()
            || keys
                .iter()
                .any(|key| source.schema().index_of(key).is_err())
        {
            return Err(RelationError::InvalidInput);
        }
        Ok(Self { source, keys })
    }
    pub fn source(&self) -> &RelationHandle {
        &self.source
    }
    pub fn keys(&self) -> &[Box<str>] {
        &self.keys
    }
    pub fn with_source(&self, source: RelationHandle) -> Result<Self, RelationError> {
        Self::new(source, Arc::clone(&self.keys))
    }
    pub fn begin_map(
        &self,
        mode: GroupMapMode,
        control: &RelationControl,
    ) -> Result<Box<dyn RelationGroupMap>, RelationError> {
        self.source
            .executor
            .clone()
            .begin_group_map(self, mode, control)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GroupMapMode {
    Apply { key_prefix: Box<str> },
    Transform,
}

#[derive(Clone, Debug)]
pub struct RelationGroup {
    /// One-based, deterministic group index for error locations.
    pub ordinal: u64,
    pub relation: RelationHandle,
}

/// Each `next` must be followed by `append` before requesting another group.
/// Dropping the session abandons unpublished output and releases temporary storage.
pub trait RelationGroupMap: Send {
    fn next(&mut self, control: &RelationControl) -> Result<Option<RelationGroup>, RelationError>;
    fn append(
        &mut self,
        result: &RelationHandle,
        control: &RelationControl,
    ) -> Result<(), RelationError>;
    /// Empty input requires a schema probe produced by invoking the function on the empty source.
    fn finish(
        self: Box<Self>,
        empty_result: Option<&RelationHandle>,
        control: &RelationControl,
    ) -> Result<RelationHandle, RelationError>;
}
