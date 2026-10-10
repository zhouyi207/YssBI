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
            std::num::NonZeroU32::new(match id {
                "yssbi.statistics.econometrics.panel.fd"
                | "yssbi.statistics.econometrics.panel.fe"
                | "yssbi.statistics.panel.compare" => 12,
                "yssbi.statistics.meta.fixed_effect"
                | "yssbi.statistics.meta.inverse_variance"
                | "yssbi.statistics.meta.leave_one_out"
                | "yssbi.statistics.meta.random_effect"
                | "yssbi.statistics.meta.sensitivity" => 13,
                "yssbi.statistics.iv.2sls.fit"
                | "yssbi.statistics.meta.regression"
                | "yssbi.statistics.panel.fit" => 14,
                "yssbi.statistics.iv.liml.fit" => 16,
                "yssbi.statistics.iv.liml.summary" => 21,
                "yssbi.statistics.iv.2sls.summary" => 24,

                "yssbi.statistics.diagnostic.hausman" => 8,

                "yssbi.statistics.econometrics.panel.re"
                | "yssbi.statistics.adf.test"
                | "yssbi.statistics.econometrics.panel.between"
                | "yssbi.statistics.meta.egger"
                | "yssbi.statistics.panel.did.twfe"
                | "yssbi.statistics.prais.fit" => 11,
                "yssbi.statistics.test.nonparametric.family"
                | "yssbi.statistics.survival.aft"
                | "yssbi.statistics.survival.exponential"
                | "yssbi.statistics.survival.loglogistic"
                | "yssbi.statistics.survival.lognormal"
                | "yssbi.statistics.survival.weibull"
                | "yssbi.statistics.plot.forest"
                | "yssbi.statistics.plot.funnel"
                | "yssbi.statistics.prais.summary"
                | "yssbi.statistics.workflow.mediation"
                | "yssbi.statistics.workflow.moderated_mediation" => 10,
                "yssbi.statistics.logit.summary"
                | "yssbi.statistics.probit.summary"
                | "yssbi.statistics.survival.cox"
                | "yssbi.statistics.survival.time_dependent_cox"
                | "yssbi.statistics.test.brown_forsythe"
                | "yssbi.statistics.var.summary"
                | "yssbi.statistics.workflow.subgroup"
                | "yssbi.statistics.econometrics.heckman_two_step"
                | "yssbi.statistics.causal.aipw"
                | "yssbi.statistics.causal.ipw"
                | "yssbi.statistics.causal.psm"
                | "yssbi.statistics.posthoc.multiple_comparisons"
                | "yssbi.statistics.survey.linear_regression"
                | "yssbi.statistics.survey.logistic"
                | "yssbi.statistics.survey.poisson"
                | "yssbi.statistics.power.cluster_randomized"
                | "yssbi.statistics.power.mean_difference"
                | "yssbi.statistics.power.paired"
                | "yssbi.statistics.test.t.paired" => 9,
                "yssbi.statistics.diagnostic.breusch_pagan"
                | "yssbi.statistics.econometrics.gmm"
                | "yssbi.statistics.econometrics.sur"
                | "yssbi.statistics.test.binomial"
                | "yssbi.statistics.test.fisher_exact"
                | "yssbi.statistics.test.levene"
                | "yssbi.statistics.test.mann_kendall"
                | "yssbi.statistics.test.mann_whitney"
                | "yssbi.statistics.test.mood_median"
                | "yssbi.statistics.test.poisson"
                | "yssbi.statistics.test.runs"
                | "yssbi.statistics.causal.regression_adjustment"
                | "yssbi.statistics.meta.begg"
                | "yssbi.statistics.meta.cochran_q"
                | "yssbi.statistics.meta.i_squared"
                | "yssbi.statistics.meta.tau_squared"
                | "yssbi.statistics.diagnostic.wald"
                | "yssbi.statistics.doe.response_surface"
                | "yssbi.statistics.inference.cluster_robust"
                | "yssbi.statistics.spatial.ols"
                | "yssbi.statistics.spatial.panel"
                | "yssbi.statistics.spatial.slx"
                | "yssbi.statistics.test.equivalence"
                | "yssbi.statistics.test.t.independent"
                | "yssbi.statistics.test.t.one_sample"
                | "yssbi.statistics.test.t.summary_input"
                | "yssbi.statistics.timeseries.ecm"
                | "yssbi.statistics.workflow.moderation"
                | "yssbi.statistics.workflow.moderation_advanced" => 8,
                "yssbi.statistics.econometrics.panel.cointegration"
                | "yssbi.statistics.econometrics.panel.dynamic_gmm"
                | "yssbi.statistics.econometrics.panel.unit_root"
                | "yssbi.statistics.econometrics.sfa"
                | "yssbi.statistics.logit.fit"
                | "yssbi.statistics.panel.did.randomization"
                | "yssbi.statistics.plot.calibration"
                | "yssbi.statistics.plot.decision_curve"
                | "yssbi.statistics.probit.fit"
                | "yssbi.statistics.psychometrics.item_analysis"
                | "yssbi.statistics.survival.competing_risks"
                | "yssbi.statistics.test.bartlett"
                | "yssbi.statistics.test.chisquare.crosstab"
                | "yssbi.statistics.test.chisquare.general"
                | "yssbi.statistics.test.chisquare.goodness_of_fit"
                | "yssbi.statistics.test.cmh"
                | "yssbi.statistics.test.friedman"
                | "yssbi.statistics.test.kruskal_wallis"
                | "yssbi.statistics.test.mcnemar"
                | "yssbi.statistics.test.proportion.multiple"
                | "yssbi.statistics.test.proportion.two"
                | "yssbi.statistics.test.wilcoxon.one_sample"
                | "yssbi.statistics.test.wilcoxon.paired"
                | "yssbi.statistics.test.z.mean"
                | "yssbi.statistics.test.z.proportion"
                | "yssbi.statistics.timeseries.fevd"
                | "yssbi.statistics.timeseries.irf"
                | "yssbi.statistics.var.fit"
                | "yssbi.statistics.var.lag_order"
                | "yssbi.statistics.vec.fit"
                | "yssbi.statistics.vec.summary"
                | "yssbi.statistics.causal.rdd"
                | "yssbi.statistics.test.heterogeneity"
                | "yssbi.statistics.longitudinal.gee"
                | "yssbi.statistics.mixed.crossed_effects"
                | "yssbi.statistics.mixed.glmm"
                | "yssbi.statistics.mixed.hlm"
                | "yssbi.statistics.mixed.lmm"
                | "yssbi.statistics.mixed.logistic"
                | "yssbi.statistics.mixed.negative_binomial"
                | "yssbi.statistics.mixed.poisson"
                | "yssbi.statistics.mixed.random_intercept"
                | "yssbi.statistics.mixed.random_slope"
                | "yssbi.statistics.inference.confidence_interval"
                | "yssbi.statistics.postestimation.adjusted_predictions"
                | "yssbi.statistics.doe.dose_response"
                | "yssbi.statistics.sem.path" => 7,
                "yssbi.plot.boxplot"
                | "yssbi.plot.violin"
                | "yssbi.statistics.diagnostic.information_matrix"
                | "yssbi.statistics.diagnostic.ph"
                | "yssbi.statistics.diagnostic.white"
                | "yssbi.statistics.test.cochran_q"
                | "yssbi.statistics.timeseries.granger"
                | "yssbi.statistics.spatial.sac"
                | "yssbi.statistics.spatial.sdem"
                | "yssbi.statistics.spatial.sdm"
                | "yssbi.statistics.spatial.sem"
                | "yssbi.statistics.spatial.slm"
                | "yssbi.statistics.timeseries.phillips_perron"
                | "yssbi.statistics.timeseries.grey_prediction"
                | "yssbi.statistics.timeseries.arima"
                | "yssbi.statistics.timeseries.sarima"
                | "yssbi.statistics.meta.continuous"
                | "yssbi.statistics.meta.binary"
                | "yssbi.statistics.meta.single_proportion"
                | "yssbi.statistics.meta.mean"
                | "yssbi.statistics.meta.correlation"
                | "yssbi.statistics.meta.or_hr"
                | "yssbi.statistics.power.anova"
                | "yssbi.statistics.power.linear_regression"
                | "yssbi.plot.coefficient.view" => 6,
                "yssbi.statistics.diagnostic.breusch_godfrey"
                | "yssbi.statistics.diagnostic.collinearity"
                | "yssbi.statistics.diagnostic.ljung_box"
                | "yssbi.statistics.diagnostic.nested_comparison"
                | "yssbi.statistics.diagnostic.reset"
                | "yssbi.statistics.plot.nomogram"
                | "yssbi.statistics.quality.measurement_system"
                | "yssbi.statistics.spatial.moran"
                | "yssbi.statistics.test.normality"
                | "yssbi.statistics.workflow.delphi"
                | "yssbi.statistics.timeseries.kpss"
                | "yssbi.statistics.timeseries.exponential_smoothing"
                | "yssbi.statistics.timeseries.ets"
                | "yssbi.statistics.timeseries.holt_winters"
                | "yssbi.statistics.timeseries.arch"
                | "yssbi.statistics.timeseries.garch"
                | "yssbi.statistics.timeseries.egarch"
                | "yssbi.statistics.timeseries.gjr_garch"
                | "yssbi.statistics.survey.mean_proportion" => 5,
                _ if id.starts_with("yssbi.statistics.survival.")
                    && id != "yssbi.statistics.survival.competing_risks" =>
                {
                    8
                }
                _ if id.contains(".logit.")
                    || id.contains(".probit.")
                    || id.contains(".prais.")
                    || id.contains(".iv.")
                    || id.contains(".panel.")
                    || id.contains(".var.")
                    || id.contains(".vec.")
                    || id.contains(".adf.")
                    || id.ends_with(".irf")
                    || id.ends_with(".fevd") =>
                {
                    6
                }
                _ if id.starts_with("yssbi.statistics.meta.")
                    || id.starts_with("yssbi.statistics.psychometrics.")
                    || id.starts_with("yssbi.statistics.mixed.")
                    || id.starts_with("yssbi.plot.")
                    || id.starts_with("yssbi.statistics.anova.")
                    || id.ends_with(".granger") =>
                {
                    5
                }
                _ => 4,
            })
            .unwrap(),
            contract,
            execute,
        )
        .expect("distinct scientific kernel");
}
