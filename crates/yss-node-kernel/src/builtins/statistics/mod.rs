//! Scientific node adapters, grouped by statistical domain.
mod anova;
mod association;
mod causal;
mod classical;
pub(super) mod common;
mod decision;
mod descriptive;
mod diagnostics;
mod doe;
mod inference;
mod linear;
mod longitudinal;
mod meta;
mod multivariate;
mod panel;
mod path;
mod power;
mod psychometrics;
mod quality;
mod regression;
mod regression_models;
mod spatial;
mod survey;
mod survival;
#[cfg(test)]
mod tests;
mod time_series;

pub(super) use crate::KernelInputSpec as Input;
use crate::{KernelContract, KernelId, KernelParameterKey, KernelRegistryBuilder};
pub(crate) use linear::{LinearKernel, execute};

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    longitudinal::register(builder);
    anova::register(builder);
    multivariate::register(builder);
    regression::register(builder);
    regression_models::register(builder);
    association::register(builder);
    causal::register(builder);
    spatial::register(builder);
    survival::register(builder);
    panel::register(builder);
    time_series::register(builder);
    diagnostics::register(builder);
    descriptive::register(builder);
    classical::register(builder);
    meta::register(builder);
    inference::register(builder);
    decision::register(builder);
    psychometrics::register(builder);
    path::register(builder);
    power::register(builder);
    quality::register(builder);
    doe::register(builder);
    survey::register(builder);
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
                1 + if (id.starts_with("yssbi.statistics.survival.")
                    && id != "yssbi.statistics.survival.competing_risks")
                    || id == "yssbi.statistics.workflow.subgroup"
                    || id == "yssbi.statistics.test.fisher_exact"
                    || id == "yssbi.statistics.test.binomial"
                    || id == "yssbi.statistics.test.poisson"
                {
                    7
                } else if matches!(
                    id,
                    "yssbi.statistics.test.t.paired"
                        | "yssbi.statistics.test.mcnemar"
                        | "yssbi.statistics.test.cmh"
                        | "yssbi.statistics.test.chisquare.crosstab"
                        | "yssbi.statistics.test.chisquare.goodness_of_fit"
                        | "yssbi.statistics.test.mann_whitney"
                        | "yssbi.statistics.test.kruskal_wallis"
                        | "yssbi.statistics.test.friedman"
                        | "yssbi.statistics.test.runs"
                        | "yssbi.statistics.test.mann_kendall"
                        | "yssbi.statistics.test.nonparametric.family"
                ) {
                    6
                } else if matches!(
                    id,
                    "yssbi.statistics.test.t.one_sample"
                        | "yssbi.statistics.test.t.independent"
                        | "yssbi.statistics.test.t.summary_input"
                        | "yssbi.statistics.test.z.mean"
                        | "yssbi.statistics.test.z.proportion"
                        | "yssbi.statistics.test.proportion.two"
                        | "yssbi.statistics.test.chisquare.general"
                        | "yssbi.statistics.test.proportion.multiple"
                        | "yssbi.statistics.test.equivalence"
                        | "yssbi.statistics.test.wilcoxon.one_sample"
                        | "yssbi.statistics.test.wilcoxon.paired"
                        | "yssbi.statistics.test.mood_median"
                        | "yssbi.statistics.test.cochran_q"
                        | "yssbi.statistics.test.levene"
                        | "yssbi.statistics.test.brown_forsythe"
                        | "yssbi.statistics.test.bartlett"
                        | "yssbi.statistics.psychometrics.item_analysis"
                ) {
                    5
                } else if matches!(
                    id,
                    "yssbi.statistics.survival.competing_risks"
                        | "yssbi.statistics.plot.calibration"
                        | "yssbi.statistics.plot.decision_curve"
                ) {
                    6
                } else if matches!(
                    id,
                    "yssbi.statistics.meta.inverse_variance"
                        | "yssbi.statistics.meta.fixed_effect"
                        | "yssbi.statistics.meta.random_effect"
                        | "yssbi.statistics.meta.cochran_q"
                        | "yssbi.statistics.meta.i_squared"
                        | "yssbi.statistics.meta.tau_squared"
                        | "yssbi.statistics.meta.regression"
                        | "yssbi.statistics.meta.egger"
                        | "yssbi.statistics.meta.begg"
                        | "yssbi.statistics.meta.leave_one_out"
                        | "yssbi.statistics.meta.sensitivity"
                        | "yssbi.statistics.meta.forest"
                        | "yssbi.statistics.meta.funnel"
                        | "yssbi.statistics.plot.forest"
                        | "yssbi.statistics.plot.funnel"
                        | "yssbi.statistics.workflow.mediation"
                        | "yssbi.statistics.workflow.moderated_mediation"
                        | "yssbi.statistics.diagnostic.ph"
                ) {
                    5
                } else if id == "yssbi.statistics.iv.2sls.summary" {
                    8
                } else if matches!(
                    id,
                    "yssbi.statistics.panel.fit"
                        | "yssbi.statistics.panel.compare"
                        | "yssbi.statistics.iv.liml.summary"
                        | "yssbi.statistics.logit.summary"
                        | "yssbi.statistics.probit.summary"
                        | "yssbi.statistics.var.summary"
                ) {
                    7
                } else if matches!(
                    id,
                    "yssbi.statistics.diagnostic.breusch_pagan"
                        | "yssbi.statistics.prais.summary"
                        | "yssbi.statistics.prais.fit"
                        | "yssbi.statistics.panel.did.randomization"
                        | "yssbi.statistics.panel.did.twfe"
                        | "yssbi.statistics.iv.2sls.fit"
                        | "yssbi.statistics.iv.liml.fit"
                        | "yssbi.statistics.adf.test"
                        | "yssbi.statistics.timeseries.irf"
                        | "yssbi.statistics.timeseries.fevd"
                        | "yssbi.statistics.econometrics.panel.fe"
                        | "yssbi.statistics.econometrics.panel.re"
                        | "yssbi.statistics.econometrics.panel.fd"
                        | "yssbi.statistics.econometrics.panel.between"
                ) {
                    6
                } else if id == "yssbi.statistics.diagnostic.wald" {
                    4
                } else if id.contains(".logit.")
                    || id.contains(".probit.")
                    || id.contains(".prais.")
                    || id.contains(".iv.")
                    || id.contains(".panel.")
                    || id.contains(".var.")
                    || id.contains(".vec.")
                    || id.contains(".adf.")
                    || id.ends_with(".irf")
                    || id.ends_with(".fevd")
                    || matches!(id, "yssbi.statistics.diagnostic.hausman")
                {
                    5
                } else if id.starts_with("yssbi.statistics.meta.")
                    || id.starts_with("yssbi.statistics.psychometrics.")
                    || id == "yssbi.statistics.inference.cluster_robust"
                    || id.starts_with("yssbi.statistics.mixed.")
                    || matches!(
                        id,
                        "yssbi.statistics.longitudinal.gee"
                            | "yssbi.statistics.diagnostic.collinearity"
                            | "yssbi.statistics.econometrics.gmm"
                            | "yssbi.statistics.econometrics.heckman_two_step"
                            | "yssbi.statistics.econometrics.sfa"
                            | "yssbi.statistics.econometrics.sur"
                            | "yssbi.statistics.causal.psm"
                            | "yssbi.statistics.causal.ipw"
                            | "yssbi.statistics.causal.regression_adjustment"
                            | "yssbi.statistics.causal.aipw"
                            | "yssbi.statistics.plot.nomogram"
                    )
                    || id.starts_with("yssbi.plot.")
                    || id.ends_with(".granger")
                    || matches!(
                        id,
                        "yssbi.statistics.workflow.moderation"
                            | "yssbi.statistics.diagnostic.white"
                            | "yssbi.statistics.diagnostic.information_matrix"
                            | "yssbi.statistics.workflow.moderation_advanced"
                            | "yssbi.statistics.sem.path"
                            | "yssbi.statistics.doe.response_surface"
                            | "yssbi.statistics.timeseries.ecm"
                    )
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
