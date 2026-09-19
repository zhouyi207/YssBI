use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;
use yss_data_contract::ValueType;

use crate::{KernelError, KernelParameterKey, RuntimeValue};

/// The existing per-computation numeric input and materialized output budget.
pub const DEFAULT_MAX_INPUT_BYTES: usize = 128 * 1024 * 1024;

pub struct KernelControl {
    pub cancellation: Arc<AtomicBool>,
    pub deadline: Instant,
    pub max_input_bytes: usize,
}

impl KernelControl {
    pub fn new(cancellation: Arc<AtomicBool>, deadline: Instant) -> Self {
        Self {
            cancellation,
            deadline,
            max_input_bytes: DEFAULT_MAX_INPUT_BYTES,
        }
    }

    pub fn check(&self) -> Result<(), KernelError> {
        if self.cancellation.load(Ordering::Acquire) {
            return Err(KernelError::Cancelled);
        }
        if Instant::now() >= self.deadline {
            return Err(KernelError::DeadlineExceeded);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelField {
    pub name: Box<str>,
    pub data_type: ValueType,
}

/// Already resolved output metadata, in invocation-local output order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelOutputSpec {
    pub data_type: ValueType,
    pub fields: Option<Box<[KernelField]>>,
}

pub struct KernelInvocation<'a> {
    pub inputs: &'a [RuntimeValue],
    /// Template keys for repeatable inputs; declared inputs have no template.
    pub input_templates: &'a [Option<&'a str>],
    /// Literals and authorized resource values can be borrowed without copying large payloads.
    pub parameters: BTreeMap<KernelParameterKey, Cow<'a, RuntimeValue>>,
    pub outputs: &'a [KernelOutputSpec],
    pub control: &'a KernelControl,
}

impl KernelInvocation<'_> {
    pub fn parameter(&self, key: &str) -> Option<&RuntimeValue> {
        self.parameters.get(key).map(Cow::as_ref)
    }

    /// Long-running implementations also check this inside their own work loops.
    pub fn check_control(&self) -> Result<(), KernelError> {
        self.control.check()
    }
}
