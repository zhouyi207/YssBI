use crate::node_system::plan::CompiledParameterHandle;
use std::any::{Any, type_name};
use std::collections::BTreeMap;
use std::fmt;

struct CompiledParameterEntry {
    type_name: &'static str,
    value: Box<dyn Any + Send + Sync>,
}

#[derive(Default)]
pub struct CompiledParameterStore {
    parameters: BTreeMap<CompiledParameterHandle, CompiledParameterEntry>,
}

impl CompiledParameterStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert<T>(
        &mut self,
        handle: CompiledParameterHandle,
        parameters: T,
    ) -> Result<(), CompiledParameterRegistrationError>
    where
        T: Any + Send + Sync,
    {
        if self.parameters.contains_key(&handle) {
            return Err(CompiledParameterRegistrationError { handle });
        }
        self.parameters.insert(
            handle,
            CompiledParameterEntry {
                type_name: type_name::<T>(),
                value: Box::new(parameters),
            },
        );
        Ok(())
    }

    pub fn get<T>(
        &self,
        handle: &CompiledParameterHandle,
    ) -> Result<Option<&T>, CompiledParameterTypeError>
    where
        T: Any + Send + Sync,
    {
        let Some(entry) = self.parameters.get(handle) else {
            return Ok(None);
        };
        entry
            .value
            .downcast_ref::<T>()
            .map(Some)
            .ok_or_else(|| CompiledParameterTypeError {
                handle: handle.clone(),
                expected: type_name::<T>(),
                actual: entry.type_name,
            })
    }

    pub fn contains(&self, handle: &CompiledParameterHandle) -> bool {
        self.parameters.contains_key(handle)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledParameterRegistrationError {
    pub handle: CompiledParameterHandle,
}

impl fmt::Display for CompiledParameterRegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "compiled parameters '{}' are already registered",
            self.handle.as_str()
        )
    }
}

impl std::error::Error for CompiledParameterRegistrationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledParameterTypeError {
    pub handle: CompiledParameterHandle,
    pub expected: &'static str,
    pub actual: &'static str,
}

impl fmt::Display for CompiledParameterTypeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "compiled parameters '{}' have type '{}'; expected '{}'",
            self.handle.as_str(),
            self.actual,
            self.expected
        )
    }
}

impl std::error::Error for CompiledParameterTypeError {}
