//! Graph-independent node invocation contracts and frozen execution capabilities.

#![deny(unused_must_use)]

mod builtins;
mod error;
mod identity;
mod invocation;
mod linear_summary;
mod registry;
mod value;

pub use error::KernelError;
pub use identity::{InvalidKernelIdentity, KernelFingerprint, KernelId, KernelParameterKey};
pub use invocation::{KernelControl, KernelField, KernelInvocation, KernelOutputSpec};
pub use linear_summary::{LinearRegressionValue, LinearSummary};
pub use registry::{
    KernelBindingError, KernelContract, KernelInputSpec, KernelRegistrationError, KernelRegistry,
    KernelRegistryBuilder,
};
pub use value::{AnnotatedRuntimeValue, RuntimeValue, RuntimeValueError};

#[cfg(test)]
mod tests;
