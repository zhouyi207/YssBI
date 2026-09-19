//! Frozen execution capabilities shared by composition, readiness checks and dispatch.
use crate::{
    KernelError, KernelFingerprint, KernelId, KernelInvocation, KernelParameterKey, RuntimeValue,
};
use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;
use std::ops::RangeInclusive;
use std::sync::Arc;

/// One ordered input key, either a declared port or a repeatable input group.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelInputSpec {
    key: Box<str>,
    count: RangeInclusive<usize>,
}

impl KernelInputSpec {
    pub fn fixed(key: &str) -> Self {
        Self::repeated(key, 1..=1)
    }

    pub fn repeated(key: &str, count: RangeInclusive<usize>) -> Self {
        Self {
            key: key.into(),
            count,
        }
    }
}

/// The input layout, lowered parameter fields and output arity accepted by an implementation.
/// Graph remains the authority for port types and parameter value validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelContract {
    inputs: Box<[KernelInputSpec]>,
    parameters: BTreeSet<KernelParameterKey>,
    outputs: RangeInclusive<usize>,
}

impl KernelContract {
    pub fn new(
        inputs: impl IntoIterator<Item = KernelInputSpec>,
        parameters: impl IntoIterator<Item = KernelParameterKey>,
        outputs: RangeInclusive<usize>,
    ) -> Result<Self, KernelRegistrationError> {
        if outputs.is_empty() {
            return Err(KernelRegistrationError::InvalidOutputArity);
        }
        let inputs: Box<[_]> = inputs.into_iter().collect();
        let mut keys = BTreeSet::new();
        for input in &inputs {
            if input.key.trim().is_empty() || input.count.is_empty() || !keys.insert(&input.key) {
                return Err(KernelRegistrationError::InvalidInputLayout);
            }
        }
        let mut fields = BTreeSet::new();
        for field in parameters {
            if !fields.insert(field.clone()) {
                return Err(KernelRegistrationError::DuplicateParameter(field));
            }
        }
        Ok(Self {
            inputs,
            parameters: fields,
            outputs,
        })
    }

    pub fn validate_binding<'a>(
        &self,
        inputs: impl IntoIterator<Item = (&'a str, RangeInclusive<usize>)>,
        parameters: impl IntoIterator<Item = &'a str>,
        outputs: RangeInclusive<usize>,
    ) -> Result<(), KernelBindingError> {
        let mut inputs = inputs.into_iter();
        for expected in &self.inputs {
            let Some((key, count)) = inputs.next() else {
                return Err(KernelBindingError::Inputs);
            };
            if key != expected.key.as_ref()
                || count.is_empty()
                || !expected.count.contains(count.start())
                || !expected.count.contains(count.end())
            {
                return Err(KernelBindingError::Inputs);
            }
        }
        if inputs.next().is_some() {
            return Err(KernelBindingError::Inputs);
        }
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

    fn accepts_inputs(&self, keys: &[&str]) -> bool {
        let mut remaining = keys;
        for input in &self.inputs {
            let count = remaining
                .iter()
                .take_while(|key| **key == input.key.as_ref())
                .count();
            if !input.count.contains(&count) {
                return false;
            }
            remaining = &remaining[count..];
        }
        remaining.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum KernelRegistrationError {
    #[error("kernel input layout contains empty or duplicate keys, or an empty arity")]
    InvalidInputLayout,
    #[error("execution kernel is already registered: {0}")]
    DuplicateKernel(KernelId),
    #[error("kernel parameter field is duplicated: {0:?}")]
    DuplicateParameter(KernelParameterKey),
    #[error("kernel output arity is empty")]
    InvalidOutputArity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum KernelBindingError {
    #[error("node inputs differ from the execution kernel contract")]
    Inputs,
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
                        .inputs
                        .iter()
                        .map(|input| {
                            (
                                input.key.as_ref(),
                                *input.count.start() as u64,
                                (*input.count.end() != usize::MAX)
                                    .then_some(*input.count.end() as u64),
                            )
                        })
                        .collect::<Vec<_>>(),
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
        if invocation.inputs.len() != invocation.input_keys.len()
            || !kernel.contract.accepts_inputs(invocation.input_keys)
        {
            return Err(KernelError::InputLayoutMismatch);
        }
        if !kernel
            .contract
            .parameters
            .iter()
            .eq(invocation.parameters.keys())
        {
            return Err(KernelError::InvalidParameter);
        }
        if !kernel.contract.outputs.contains(&invocation.outputs.len()) {
            return Err(KernelError::OutputContractMismatch);
        }
        let result = (kernel.execute)(invocation)?;
        invocation.check_control()?;
        if result.len() != invocation.outputs.len()
            || result
                .iter()
                .zip(invocation.outputs)
                .any(|(value, output)| !value.matches_carrier(&output.data_type))
        {
            return Err(KernelError::OutputContractMismatch);
        }
        Ok(result)
    }
}
