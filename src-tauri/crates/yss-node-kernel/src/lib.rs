//! Graph-independent node invocation contracts and frozen execution capabilities.

#![deny(unused_must_use)]

mod builtins;
mod error;
mod identity;
mod invocation;
mod registry;
mod value;

pub use error::KernelError;
pub use identity::{InvalidKernelIdentity, KernelFingerprint, KernelId, KernelParameterKey};
pub use invocation::{KernelControl, KernelField, KernelInvocation, KernelOutputSpec};
pub use registry::{
    KernelBindingError, KernelContract, KernelRegistrationError, KernelRegistry,
    KernelRegistryBuilder,
};
pub use value::{AnnotatedRuntimeValue, RuntimeValue, RuntimeValueError};

#[cfg(test)]
mod tests;
