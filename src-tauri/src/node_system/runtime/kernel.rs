use super::{
    ActivationId, CancellationToken, CompiledParameterStore, FrameId, RunDeadline, RunId,
    RunOutputSink, RunOutputStream, RunPhase, RunResourceOwner, RunResourceSet, RuntimeValue,
};
use crate::node_system::document::{GraphResourcePath, NodeId, PortAddress};
use crate::node_system::plan::{CompiledParameterHandle, KernelHandle};
use crate::project::{NumericTolerance, ProjectComputationSettings, StatisticalMissingValuePolicy};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelErrorKind {
    Permanent,
    Transient,
    Cancelled,
    DeadlineExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelError {
    kind: KernelErrorKind,
    message: Box<str>,
}

impl KernelError {
    pub fn new(message: impl Into<Box<str>>) -> Self {
        Self {
            kind: KernelErrorKind::Permanent,
            message: message.into(),
        }
    }

    pub fn transient(message: impl Into<Box<str>>) -> Self {
        Self {
            kind: KernelErrorKind::Transient,
            message: message.into(),
        }
    }

    pub fn cancelled(message: impl Into<Box<str>>) -> Self {
        Self {
            kind: KernelErrorKind::Cancelled,
            message: message.into(),
        }
    }

    pub fn deadline_exceeded() -> Self {
        Self {
            kind: KernelErrorKind::DeadlineExceeded,
            message: "run deadline exceeded during kernel execution".into(),
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

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveComputationSettings {
    pub numeric_tolerance: NumericTolerance,
    pub statistical_missing_value_policy: StatisticalMissingValuePolicy,
}

impl From<&ProjectComputationSettings> for EffectiveComputationSettings {
    fn from(settings: &ProjectComputationSettings) -> Self {
        Self {
            numeric_tolerance: settings.numeric.tolerance,
            statistical_missing_value_policy: settings.missing_values.statistics,
        }
    }
}

impl Default for EffectiveComputationSettings {
    fn default() -> Self {
        Self::from(&ProjectComputationSettings::default())
    }
}

pub struct KernelContext<'a> {
    pub run_id: RunId,
    pub frame_id: FrameId,
    pub activation_id: ActivationId,
    pub(crate) source_graph_path: &'a GraphResourcePath,
    pub(crate) source_node_id: NodeId,
    pub(crate) run_output: &'a dyn RunOutputSink,
    pub(crate) computation_settings: EffectiveComputationSettings,
    pub params: &'a CompiledParameterHandle,
    pub compiled_parameters: Option<&'a CompiledParameterStore>,
    pub resources: &'a RunResourceSet,
    pub resource_owner: &'a RunResourceOwner,
    pub cancellation: &'a CancellationToken,
    pub deadline: Option<RunDeadline>,
}

impl KernelContext<'_> {
    pub fn emit_stdout(&self, text: &str, source_port: PortAddress) {
        self.run_output.emit(
            RunOutputStream::Stdout,
            text,
            self.source_graph_path,
            self.source_node_id,
            &source_port,
        );
    }

    pub(crate) fn source_node_id(&self) -> NodeId {
        self.source_node_id
    }

    pub fn computation_settings(&self) -> EffectiveComputationSettings {
        self.computation_settings
    }

    pub fn check_terminal(&self) -> Result<(), KernelError> {
        self.cancellation
            .check()
            .map_err(|_| KernelError::cancelled("kernel execution was cancelled"))?;
        if let Some(deadline) = self.deadline {
            deadline
                .check(self.cancellation, RunPhase::Kernel)
                .map_err(|error| match error {
                    super::RunError::Cancelled => {
                        KernelError::cancelled("kernel execution was cancelled")
                    }
                    super::RunError::DeadlineExceeded { .. } => KernelError::deadline_exceeded(),
                    _ => unreachable!("terminal check has only cancellation or deadline outcomes"),
                })?;
        }
        Ok(())
    }

    pub fn wait_for(&self, duration: Duration) -> Result<(), KernelError> {
        let operation_end = Instant::now() + duration;
        loop {
            self.check_terminal()?;
            let now = Instant::now();
            if now >= operation_end {
                return Ok(());
            }
            let mut wait = operation_end - now;
            if let Some(deadline) = self.deadline {
                wait = wait.min(
                    deadline
                        .remaining(self.cancellation, RunPhase::Kernel)
                        .map_err(|error| match error {
                            super::RunError::Cancelled => {
                                KernelError::cancelled("kernel execution was cancelled")
                            }
                            super::RunError::DeadlineExceeded { .. } => {
                                KernelError::deadline_exceeded()
                            }
                            _ => unreachable!("terminal check has only terminal outcomes"),
                        })?,
                );
            }
            if self.cancellation.wait_timeout(wait) {
                self.check_terminal()?;
            }
        }
    }

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
