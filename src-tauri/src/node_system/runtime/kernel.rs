use super::{
    ActivationId, CancellationToken, CompiledParameterStore, FrameId, RunId, RunResourceSet,
    RuntimeValue,
};
use crate::node_system::plan::{CompiledParameterHandle, KernelHandle};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelErrorKind {
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelError {
    kind: KernelErrorKind,
    message: Box<str>,
}

impl KernelError {
    pub fn new(message: impl Into<Box<str>>) -> Self {
        Self {
            kind: KernelErrorKind::Failed,
            message: message.into(),
        }
    }

    pub fn cancelled(message: impl Into<Box<str>>) -> Self {
        Self {
            kind: KernelErrorKind::Cancelled,
            message: message.into(),
        }
    }

    pub fn kind(&self) -> KernelErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for KernelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for KernelError {}

pub struct KernelContext<'a> {
    pub run_id: RunId,
    pub frame_id: FrameId,
    pub activation_id: ActivationId,
    pub params: &'a CompiledParameterHandle,
    pub compiled_parameters: Option<&'a CompiledParameterStore>,
    pub resources: &'a RunResourceSet,
    pub cancellation: &'a CancellationToken,
}

impl KernelContext<'_> {
    pub fn parameters<T>(&self) -> Result<&T, KernelError>
    where
        T: std::any::Any + Send + Sync,
    {
        let store = self.compiled_parameters.ok_or_else(|| {
            KernelError::new(format!(
                "compiled parameter store is unavailable for '{}'",
                self.params.as_str()
            ))
        })?;
        store
            .get::<T>(self.params)
            .map_err(|error| KernelError::new(error.to_string()))?
            .ok_or_else(|| {
                KernelError::new(format!(
                    "compiled parameters '{}' were not found",
                    self.params.as_str()
                ))
            })
    }
}

pub trait Kernel: Send + Sync {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        inputs: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError>;
}

#[derive(Default)]
pub struct KernelRegistry {
    kernels: BTreeMap<KernelHandle, Arc<dyn Kernel>>,
}

impl KernelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        handle: KernelHandle,
        kernel: impl Kernel + 'static,
    ) -> Result<(), KernelRegistrationError> {
        if self
            .kernels
            .insert(handle.clone(), Arc::new(kernel))
            .is_some()
        {
            return Err(KernelRegistrationError { handle });
        }
        Ok(())
    }

    pub fn get(&self, handle: &KernelHandle) -> Option<&Arc<dyn Kernel>> {
        self.kernels.get(handle)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelRegistrationError {
    pub handle: KernelHandle,
}

impl fmt::Display for KernelRegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "kernel '{}' is already registered",
            self.handle.as_str()
        )
    }
}

impl std::error::Error for KernelRegistrationError {}
