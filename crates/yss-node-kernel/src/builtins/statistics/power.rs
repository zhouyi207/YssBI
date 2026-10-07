//! Scalar planning adapters; no observation rows or fitted-sample state enter these nodes.
use super::{common::*, install};
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};
use yss_sci_contract::{execution::ScientificExecutionControl as Control, power::*};
pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    for (method, parameters, fixed_alternative) in [
        ("principles", &["effect_size"][..], None),
        ("mean_difference", &["effect_size", "design"][..], None),
        ("paired", &["effect_size"][..], None),
        ("variance", &["variance_ratio"][..], None),
        ("proportion", &["null_proportion", "proportion"][..], None),
        (
            "proportion_difference",
            &["proportion1", "proportion2"][..],
            None,
        ),
        (
            "correlation",
            &["null_correlation", "correlation"][..],
            None,
        ),
        (
            "anova",
            &["groups", "effect_f"][..],
            Some(PowerAlternative::Greater),
        ),
        (
            "linear_regression",
            &["predictors", "effect_f_squared"][..],
            Some(PowerAlternative::Greater),
        ),
        (
            "generalized_model",
            &["baseline_rate", "rate_ratio", "exposure"][..],
            None,
        ),
        (
            "logistic",
            &["baseline_probability", "odds_ratio"][..],
            None,
        ),
        (
            "cox",
            &["hazard_ratio", "event_fraction", "predictor_variance"][..],
            None,
        ),
        (
            "logrank",
            &["hazard_ratio", "event_fraction", "allocation"][..],
            None,
        ),
        (
            "cluster_randomized",
            &["effect_size", "cluster_size", "icc"][..],
            None,
        ),
        (
            "noninferiority",
            &["difference", "margin", "higher_is_better"][..],
            Some(PowerAlternative::Greater),
        ),
        (
            "equivalence",
            &["difference", "margin"][..],
            Some(PowerAlternative::TwoSided),
        ),
    ] {
        let mut params = vec!["solve_for", "sample_size", "target_power", "alpha"];
        if fixed_alternative.is_none() {
            params.push("alternative");
        }
        params.extend(parameters);
        install(
            builder,
            &format!("yssbi.statistics.power.{method}"),
            vec![],
            &params,
            1,
            move |inv| compute(inv, method, fixed_alternative),
        );
    }
}
fn compute(
    inv: &KernelInvocation<'_>,
    method: &str,
    fixed_alternative: Option<PowerAlternative>,
) -> Result<Vec<RuntimeValue>, KernelError> {
    inv.control.check_bytes(Some(65536))?;
    let alternative = if let Some(a) = fixed_alternative {
        a
    } else {
        match text(inv, "alternative")? {
            "two_sided" => PowerAlternative::TwoSided,
            "greater" => PowerAlternative::Greater,
            "less" => PowerAlternative::Less,
            _ => return Err(KernelError::InvalidParameter),
        }
    };
    let request = match text(inv, "solve_for")? {
        "power" => PowerRequest::Power {
            sample_size: integer(inv, "sample_size")?,
        },
        "sample_size" => PowerRequest::SampleSize {
            target_power: number(inv, "target_power")?,
        },
        _ => return Err(KernelError::InvalidParameter),
    };
    let model = model(inv, method)?;
    let control = Control::from_shared(inv.control.cancellation.clone(), inv.control.deadline);
    let result = yss_sci_runtime::power::compute(
        model,
        PowerOptions {
            alpha: number(inv, "alpha")?,
            alternative,
            request,
        },
        &control,
    )
    .map_err(computation_error)?;
    Ok(vec![value(result, inv)?])
}
fn model(inv: &KernelInvocation<'_>, method: &str) -> Result<PowerModel, KernelError> {
    let num = |key| number(inv, key);
    Ok(match method {
        "principles" => PowerModel::NormalMean {
            standardized_effect: num("effect_size")?,
        },
        "mean_difference" | "paired" => PowerModel::TMean {
            standardized_effect: num("effect_size")?,
            design: if method == "paired" {
                MeanDesign::Paired
            } else {
                match text(inv, "design")? {
                    "one_sample" => MeanDesign::OneSample,
                    "independent" => MeanDesign::Independent,
                    _ => return Err(KernelError::InvalidParameter),
                }
            },
        },
        "variance" => PowerModel::Variance {
            variance_ratio: num("variance_ratio")?,
        },
        "proportion" => PowerModel::Proportion {
            null_proportion: num("null_proportion")?,
            proportion: num("proportion")?,
        },
        "proportion_difference" => PowerModel::ProportionDifference {
            proportion1: num("proportion1")?,
            proportion2: num("proportion2")?,
        },
        "correlation" => PowerModel::Correlation {
            null_correlation: num("null_correlation")?,
            correlation: num("correlation")?,
        },
        "anova" => PowerModel::Anova {
            groups: integer(inv, "groups")?,
            effect_f: num("effect_f")?,
        },
        "linear_regression" => PowerModel::LinearRegression {
            predictors: integer(inv, "predictors")?,
            effect_f_squared: num("effect_f_squared")?,
        },
        "generalized_model" => PowerModel::PoissonRate {
            baseline_rate: num("baseline_rate")?,
            rate_ratio: num("rate_ratio")?,
            exposure: num("exposure")?,
        },
        "logistic" => PowerModel::Logistic {
            baseline_probability: num("baseline_probability")?,
            odds_ratio: num("odds_ratio")?,
        },
        "cox" | "logrank" => PowerModel::Survival {
            hazard_ratio: num("hazard_ratio")?,
            event_fraction: num("event_fraction")?,
            predictor_variance: if method == "cox" {
                num("predictor_variance")?
            } else {
                let q = num("allocation")?;
                if q <= 0. || q >= 1. {
                    return Err(KernelError::InvalidParameter);
                }
                q * (1. - q)
            },
        },
        "cluster_randomized" => PowerModel::ClusterRandomized {
            standardized_effect: num("effect_size")?,
            cluster_size: integer(inv, "cluster_size")?,
            intraclass_correlation: num("icc")?,
        },
        "noninferiority" => PowerModel::Noninferiority {
            standardized_difference: num("difference")?,
            margin: num("margin")?,
            higher_is_better: boolean(inv, "higher_is_better")?,
        },
        "equivalence" => PowerModel::Equivalence {
            standardized_difference: num("difference")?,
            margin: num("margin")?,
        },
        _ => return Err(KernelError::InvalidParameter),
    })
}
