use super::{
    Input,
    common::{boolean, integer, number, text, value},
};
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};
use yss_data_contract::TabularScalar;
use yss_sci_contract::{execution::*, longitudinal::*, regression::models::IterationOptions};
use yss_sci_runtime::longitudinal as sci;

const METHODS: &[&str] = &[
    "longitudinal.gee",
    "mixed.hlm",
    "mixed.lmm",
    "mixed.glmm",
    "mixed.random_intercept",
    "mixed.random_slope",
    "mixed.crossed_effects",
    "mixed.logistic",
    "mixed.poisson",
    "mixed.negative_binomial",
];

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    for &method in METHODS {
        let mut inputs = vec![
            Input::fixed("response"),
            Input::repeated("predictors", 0..=usize::MAX),
            Input::repeated(
                "groups",
                if method == "mixed.crossed_effects" {
                    2
                } else {
                    1
                }..=if matches!(method, "mixed.hlm" | "mixed.crossed_effects") {
                    usize::MAX
                } else {
                    1
                },
            ),
        ];
        if matches!(method, "mixed.lmm" | "mixed.random_slope") {
            inputs.push(Input::repeated(
                "random_predictors",
                if method == "mixed.random_slope" { 1 } else { 0 }..=usize::MAX,
            ));
        }
        let mut parameters = vec!["constant", "max_iterations", "tolerance"];
        if method == "longitudinal.gee" || method == "mixed.glmm" {
            parameters.push("longitudinal_family");
        }
        if method == "longitudinal.gee" {
            parameters.push("working_correlation");
        } else if !matches!(
            method,
            "mixed.glmm" | "mixed.logistic" | "mixed.poisson" | "mixed.negative_binomial"
        ) {
            parameters.push("mixed_estimation");
        }
        super::install(
            builder,
            &format!("yssbi.statistics.{method}"),
            inputs,
            &parameters,
            1,
            move |inv| execute(method, inv),
        );
    }
}

fn error(e: ScientificComputationError) -> KernelError {
    match e {
        ScientificComputationError::Cancelled => KernelError::Cancelled,
        ScientificComputationError::DeadlineExceeded => KernelError::DeadlineExceeded,
        ScientificComputationError::InvalidInput {
            violation: ScientificInputViolation::ShapeMismatch,
        } => KernelError::ShapeMismatch,
        ScientificComputationError::InvalidInput {
            violation: ScientificInputViolation::ParameterOutOfRange,
        } => KernelError::InvalidParameter,
        ScientificComputationError::InvalidInput { .. } => KernelError::InvalidNumericInput,
        ScientificComputationError::ComputationFailed => KernelError::ScientificFailure,
    }
}

fn execute(method: &str, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let (columns, retained) = super::common::materialize(inv)?;
    let n = columns
        .first()
        .ok_or(KernelError::InvalidNumericInput)?
        .values
        .len();
    let mut y = Vec::new();
    let mut predictors = Vec::new();
    let mut random_predictors = Vec::new();
    let mut groups = Vec::new();
    let mut group_labels = Vec::new();
    for (key, column) in inv.input_keys.iter().zip(&columns) {
        match *key {
            "response" => y = super::common::numeric(column, true, inv)?,
            "predictors" => predictors.push(super::common::numeric(column, false, inv)?),
            "random_predictors" => {
                random_predictors.push(super::common::numeric(column, false, inv)?)
            }
            "groups" => {
                let (codes, labels) = super::common::categories(column, false, inv)?;
                groups.push(Grouping {
                    codes,
                    levels: labels.len(),
                });
                group_labels.push(labels);
            }
            _ => return Err(KernelError::InputLayoutMismatch),
        }
    }
    let constant = boolean(inv, "constant")?;
    let gaussian_mixed = !matches!(
        method,
        "longitudinal.gee"
            | "mixed.glmm"
            | "mixed.logistic"
            | "mixed.poisson"
            | "mixed.negative_binomial"
    );
    // Admit the peak of fitting and report encoding. Only Gaussian mixed models
    // allocate observation-by-observation covariance matrices.
    let bytes = (|| {
        let p = predictors.len().checked_add(usize::from(constant))?;
        let components = groups.len().checked_add(random_predictors.len())?;
        let levels = groups
            .iter()
            .try_fold(0usize, |sum, g| sum.checked_add(g.levels))?;
        let numeric = n
            .checked_mul(p.checked_add(12 + groups.len() + random_predictors.len())?)?
            .checked_add(levels.checked_mul(4)?)?
            .checked_mul(size_of::<f64>())?;
        let workspace = if gaussian_mixed {
            let square = n.checked_mul(n)?;
            let kernels = square.checked_mul(components)?;
            // Rank decomposition of the lower-triangle covariance design.
            let rank = n
                .checked_mul(n.checked_add(1)?)?
                .checked_div(2)?
                .checked_mul(components.checked_add(1)?)?
                .checked_mul(4)?;
            let profile = square
                .checked_mul(4)?
                .checked_add(n.checked_mul(p)?.checked_mul(3)?)?
                .checked_add(p.checked_mul(p)?.checked_mul(6)?)?;
            kernels
                .checked_add(rank.max(profile))?
                .checked_mul(size_of::<f64>())?
        } else {
            let parameters = p.checked_add(2)?;
            parameters
                .checked_mul(parameters)?
                .checked_mul(8 * size_of::<f64>())?
        };
        let report_values = n
            .checked_mul(3)?
            .checked_add(p.checked_mul(p)?)?
            .checked_add(p.checked_mul(24)?)?
            .checked_add(components.checked_mul(12)?)?
            .checked_add(
                levels
                    .checked_mul(1 + random_predictors.len())?
                    .checked_mul(12)?,
            )?
            .checked_add(256)?;
        let encoding = report_values.checked_mul(
            super::common::STRUCTURED_VALUE_BYTES * super::common::STRUCTURED_VALUE_COPIES,
        )?;
        retained
            .checked_add(numeric)?
            .checked_add(workspace.max(encoding))
    })();
    inv.control.check_bytes(bytes)?;
    let control = ScientificExecutionControl::from_shared(
        inv.control.cancellation.clone(),
        inv.control.deadline,
    );
    let iteration = IterationOptions {
        max_iterations: integer(inv, "max_iterations")?,
        tolerance: number(inv, "tolerance")?,
    };
    let family = match method {
        "mixed.logistic" => ResponseFamily::Binomial,
        "mixed.poisson" => ResponseFamily::Poisson,
        "mixed.negative_binomial" => ResponseFamily::NegativeBinomial,
        "mixed.glmm" | "longitudinal.gee" => match text(inv, "longitudinal_family")? {
            "gaussian" if method == "longitudinal.gee" => ResponseFamily::Gaussian,
            "binomial" => ResponseFamily::Binomial,
            "poisson" => ResponseFamily::Poisson,
            "negative_binomial" if method == "mixed.glmm" => ResponseFamily::NegativeBinomial,
            _ => return Err(KernelError::InvalidParameter),
        },
        _ => ResponseFamily::Gaussian,
    };
    let fit = if method == "longitudinal.gee" {
        sci::gee(
            &y,
            &predictors,
            &groups[0],
            GeeOptions {
                family,
                constant,
                iteration,
                correlation: match text(inv, "working_correlation")? {
                    "independence" => WorkingCorrelation::Independence,
                    "exchangeable" => WorkingCorrelation::Exchangeable,
                    _ => return Err(KernelError::InvalidParameter),
                },
            },
            &control,
        )
    } else if family != ResponseFamily::Gaussian {
        sci::generalized_mixed(
            &y,
            &predictors,
            &groups[0],
            family,
            constant,
            iteration,
            &control,
        )
    } else {
        sci::linear_mixed(
            &y,
            &predictors,
            &groups,
            &random_predictors,
            MixedOptions {
                constant,
                iteration,
                nested: method == "mixed.hlm",
                estimation: match text(inv, "mixed_estimation")? {
                    "ml" => MixedEstimation::Ml,
                    "reml" => MixedEstimation::Reml,
                    _ => return Err(KernelError::InvalidParameter),
                },
            },
            &control,
        )
    }
    .map_err(error)?;
    #[derive(serde::Serialize)]
    struct Report {
        #[serde(flatten)]
        fit: LongitudinalResult,
        group_labels: Vec<Vec<TabularScalar>>,
    }
    Ok(vec![value(Report { fit, group_labels }, inv)?])
}
