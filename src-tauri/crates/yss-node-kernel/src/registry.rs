//! Frozen execution capabilities shared by composition, readiness checks and dispatch.
use crate::{
    KernelError, KernelFingerprint, KernelId, KernelInvocation, KernelParameterKey, RuntimeValue,
};
use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;
use std::ops::RangeInclusive;
use std::sync::Arc;

/// The lowered parameter fields and output arity accepted by an implementation.
/// Graph remains the authority for port types and parameter value validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelContract {
    parameters: BTreeSet<KernelParameterKey>,
    outputs: RangeInclusive<usize>,
}

impl KernelContract {
    pub fn new(
        parameters: impl IntoIterator<Item = KernelParameterKey>,
        outputs: RangeInclusive<usize>,
    ) -> Result<Self, KernelRegistrationError> {
        if outputs.is_empty() {
            return Err(KernelRegistrationError::InvalidOutputArity);
        }
        let mut fields = BTreeSet::new();
        for field in parameters {
            if !fields.insert(field.clone()) {
                return Err(KernelRegistrationError::DuplicateParameter(field));
            }
        }
        Ok(Self {
            parameters: fields,
            outputs,
        })
    }

    pub fn validate_binding<'a>(
        &self,
        parameters: impl IntoIterator<Item = &'a str>,
        outputs: RangeInclusive<usize>,
    ) -> Result<(), KernelBindingError> {
        if self
            .parameters
            .iter()
            .map(|field| field.as_str())
            .collect::<BTreeSet<_>>()
            != parameters.into_iter().collect::<BTreeSet<_>>()
        {
            return Err(KernelBindingError::Parameters);
        }
        if outputs.is_empty()
            || !self.outputs.contains(outputs.start())
            || !self.outputs.contains(outputs.end())
        {
            return Err(KernelBindingError::Outputs);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum KernelRegistrationError {
    #[error("execution kernel is already registered: {0}")]
    DuplicateKernel(KernelId),
    #[error("kernel parameter field is duplicated: {0:?}")]
    DuplicateParameter(KernelParameterKey),
    #[error("kernel output arity is empty")]
    InvalidOutputArity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum KernelBindingError {
    #[error("node parameters differ from the execution kernel contract")]
    Parameters,
    #[error("node outputs exceed the execution kernel contract")]
    Outputs,
}

type KernelFn =
    dyn Fn(&KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> + Send + Sync;

struct RegisteredKernel {
    revision: NonZeroU32,
    contract: KernelContract,
    execute: Arc<KernelFn>,
}

#[derive(Default)]
pub struct KernelRegistryBuilder {
    kernels: BTreeMap<KernelId, RegisteredKernel>,
}

impl KernelRegistryBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_builtins() -> Self {
        let mut builder = Self::new();
        crate::builtins::register_builtin_kernels(&mut builder);
        builder
    }

    /// Bump revision whenever implementation behavior changes, even if its ABI does not.
    pub fn register(
        &mut self,
        id: KernelId,
        revision: NonZeroU32,
        contract: KernelContract,
        execute: impl Fn(&KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError>
        + Send
        + Sync
        + 'static,
    ) -> Result<(), KernelRegistrationError> {
        if self.kernels.contains_key(&id) {
            return Err(KernelRegistrationError::DuplicateKernel(id));
        }
        self.kernels.insert(
            id,
            RegisteredKernel {
                revision,
                contract,
                execute: Arc::new(execute),
            },
        );
        Ok(())
    }

    pub fn contract(&self, id: &str) -> Option<&KernelContract> {
        self.kernels.get(id).map(|kernel| &kernel.contract)
    }

    pub fn freeze(self) -> KernelRegistry {
        let manifest = self
            .kernels
            .iter()
            .map(|(id, kernel)| {
                (
                    id.as_str(),
                    kernel.revision.get(),
                    kernel
                        .contract
                        .parameters
                        .iter()
                        .map(|field| field.as_str())
                        .collect::<Vec<_>>(),
                    *kernel.contract.outputs.start() as u64,
                    (*kernel.contract.outputs.end() != usize::MAX)
                        .then_some(*kernel.contract.outputs.end() as u64),
                )
            })
            .collect::<Vec<_>>();
        let fingerprint = yss_canonical_hash::hash_canonical("yssbi.kernel-registry.v1", &manifest)
            .expect("kernel manifest contains only strings and integers");
        KernelRegistry {
            kernels: self.kernels,
            fingerprint: KernelFingerprint::from_bytes(fingerprint),
        }
    }
}

pub struct KernelRegistry {
    kernels: BTreeMap<KernelId, RegisteredKernel>,
    fingerprint: KernelFingerprint,
}

impl Default for KernelRegistry {
    fn default() -> Self {
        KernelRegistryBuilder::with_builtins().freeze()
    }
}

impl KernelRegistry {
    pub const fn fingerprint(&self) -> KernelFingerprint {
        self.fingerprint
    }

    pub fn supports(&self, id: &str) -> bool {
        self.kernels.contains_key(id)
    }

    pub fn execute(
        &self,
        id: &KernelId,
        invocation: &KernelInvocation<'_>,
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        let kernel = self.kernels.get(id).ok_or(KernelError::KernelNotFound)?;
        invocation.check_control()?;
        if invocation.inputs.len() != invocation.input_groups.len() {
            return Err(KernelError::Failed);
        }
        if !kernel
            .contract
            .parameters
            .iter()
            .eq(invocation.parameters.keys())
            || !kernel.contract.outputs.contains(&invocation.outputs.len())
        {
            return Err(KernelError::Failed);
        }
        let result = (kernel.execute)(invocation)?;
        invocation.check_control()?;
        if result.len() != invocation.outputs.len() {
            return Err(KernelError::Failed);
        }
        Ok(result)
    }
}
