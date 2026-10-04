//! Sequential, bounded partition storage and result combination; no graph callbacks.
mod input;
mod output;

use crate::DataFusionRuntime;
use input::{GroupInput, GroupReader};
use output::GroupOutput;
use std::sync::Arc;
use yss_relational_contract::{
    GroupMapMode, GroupedRelationHandle, RelationControl, RelationError, RelationGroup,
    RelationGroupMap, RelationHandle,
};

pub(crate) struct GroupMapping {
    engine: Arc<DataFusionRuntime>,
    source: GroupedRelationHandle,
    input: GroupReader,
    current: Option<GroupInput>,
    output: GroupOutput,
    ordinal: u64,
}

impl GroupMapping {
    pub(crate) fn begin(
        engine: Arc<DataFusionRuntime>,
        source: &GroupedRelationHandle,
        mode: GroupMapMode,
        control: &RelationControl,
    ) -> Result<Box<dyn RelationGroupMap>, RelationError> {
        control.check()?;
        let source = source.with_source(
            engine
                .runtime
                .as_ref()
                .ok_or(RelationError::QueryFailed)?
                .block_on(source.source().resolve(control.clone()))?,
        )?;
        let input = engine
            .runtime
            .as_ref()
            .ok_or(RelationError::QueryFailed)?
            .block_on(GroupReader::new(&source, control))?;
        let output = GroupOutput::new(&source, mode)?;
        Ok(Box::new(Self {
            engine,
            source,
            input,
            current: None,
            output,
            ordinal: 0,
        }))
    }
}

impl RelationGroupMap for GroupMapping {
    fn next(&mut self, control: &RelationControl) -> Result<Option<RelationGroup>, RelationError> {
        control.check()?;
        if self.current.is_some() {
            return Err(RelationError::InvalidInput);
        }
        self.current = self
            .engine
            .runtime
            .as_ref()
            .ok_or(RelationError::QueryFailed)?
            .block_on(self.input.next(&self.engine, control))?;
        let Some(group) = &self.current else {
            return Ok(None);
        };
        self.ordinal = self
            .ordinal
            .checked_add(1)
            .ok_or(RelationError::MemoryLimitExceeded)?;
        Ok(Some(RelationGroup {
            ordinal: self.ordinal,
            relation: group.value.clone(),
        }))
    }

    fn append(
        &mut self,
        result: &RelationHandle,
        control: &RelationControl,
    ) -> Result<(), RelationError> {
        let current = self.current.as_ref().ok_or(RelationError::InvalidInput)?;
        self.engine
            .runtime
            .as_ref()
            .ok_or(RelationError::QueryFailed)?
            .block_on(self.output.append(&self.engine, current, result, control))?;
        self.current = None;
        Ok(())
    }

    fn finish(
        mut self: Box<Self>,
        empty_result: Option<&RelationHandle>,
        control: &RelationControl,
    ) -> Result<RelationHandle, RelationError> {
        control.check()?;
        if self.current.is_some() || !self.input.exhausted() {
            return Err(RelationError::InvalidInput);
        }
        if self.ordinal == 0 {
            self.output.prepare_empty(
                &self.engine,
                self.source.source(),
                empty_result.ok_or(RelationError::InvalidInput)?,
                control,
            )?;
        } else if empty_result.is_some() {
            return Err(RelationError::InvalidInput);
        }
        self.output
            .finish(&self.engine, self.source.source(), control)
    }
}
