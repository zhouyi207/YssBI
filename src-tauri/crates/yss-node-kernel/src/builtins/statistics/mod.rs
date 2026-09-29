//! Scientific node adapters, grouped by statistical domain.
mod causal;
pub(super) mod common;
mod density;
mod descriptive;
mod diagnostics;
mod linear;
mod panel;
mod regression;
#[cfg(test)]
mod tests;
mod time_series;

pub(super) use crate::KernelInputSpec as Input;
use crate::{KernelContract, KernelId, KernelParameterKey, KernelRegistryBuilder};
pub(crate) use linear::{LinearKernel, execute};

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    regression::register(builder);
    causal::register(builder);
    panel::register(builder);
    time_series::register(builder);
    diagnostics::register(builder);
    density::register(builder);
    descriptive::register(builder);
}

pub(super) fn install(
    builder: &mut KernelRegistryBuilder,
    id: &str,
    inputs: Vec<Input>,
    parameters: &[&str],
    outputs: usize,
    execute: impl Fn(
        &crate::KernelInvocation<'_>,
    ) -> Result<Vec<crate::RuntimeValue>, crate::KernelError>
    + Send
    + Sync
    + 'static,
) {
    let contract = KernelContract::new(
        inputs,
        parameters
            .iter()
            .map(|key| KernelParameterKey::new((*key).into()).expect("parameter key")),
        outputs..=outputs,
    )
    .expect("scientific node contract");
    builder
        .register(
            KernelId::new(id.into()).expect("kernel id"),
            std::num::NonZeroU32::new(
                if id.contains(".iv.")
                    || id.contains(".panel.")
                    || id.contains(".var.")
                    || id.contains(".vec.")
                    || id.contains(".logit.")
                    || id.contains(".probit.")
                    || id.contains(".prais.")
                    || id.ends_with(".granger")
                    || id.ends_with(".irf")
                    || id.ends_with(".fevd")
                    || id.ends_with(".hausman")
                    || id.ends_with(".breusch_pagan")
                {
                    3
                } else {
                    2
                },
            )
            .unwrap(),
            contract,
            execute,
        )
        .expect("distinct scientific kernel");
}
