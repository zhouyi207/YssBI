//! Scientific node adapters, grouped by statistical domain.
mod anova;
mod association;
mod causal;
mod classical;
pub(super) mod common;
mod descriptive;
mod diagnostics;
mod linear;
mod multivariate;
mod panel;
mod regression;
mod regression_models;
#[cfg(test)]
mod tests;
mod time_series;

pub(super) use crate::KernelInputSpec as Input;
use crate::{KernelContract, KernelId, KernelParameterKey, KernelRegistryBuilder};
pub(crate) use linear::{LinearKernel, execute};

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    anova::register(builder);
    multivariate::register(builder);
    regression::register(builder);
    regression_models::register(builder);
    association::register(builder);
    causal::register(builder);
    panel::register(builder);
    time_series::register(builder);
    diagnostics::register(builder);
    descriptive::register(builder);
    classical::register(builder);
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
                if id.contains(".logit.")
                    || id.contains(".probit.")
                    || id.contains(".prais.")
                    || id.contains(".iv.")
                    || id.contains(".panel.")
                    || id.contains(".var.")
                    || id.contains(".vec.")
                    || id.contains(".adf.")
                    || id.ends_with(".irf")
                    || id.ends_with(".fevd")
                {
                    5
                } else if id.starts_with("yssbi.plot.")
                    || id.ends_with(".granger")
                    || id.ends_with(".hausman")
                    || id.ends_with(".breusch_pagan")
                {
                    4
                } else {
                    3
                },
            )
            .unwrap(),
            contract,
            execute,
        )
        .expect("distinct scientific kernel");
}
