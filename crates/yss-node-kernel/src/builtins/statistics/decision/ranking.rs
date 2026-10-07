//! Numeric criterion matrices and separate criterion-weight vectors.
use super::{columns::*, *};

const METHODS: &[&str] = &[
    "weights",
    "entropy_weight",
    "critic",
    "information_weight",
    "independence_weight",
    "composite_index",
    "topsis",
    "grey_relational",
    "wrsr",
    "efficacy_coefficient",
    "entropy_topsis",
];
pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    for &method in METHODS {
        let mut inputs = vec![Input::repeated("criteria", 1..=usize::MAX)];
        let mut parameters = vec!["cost_criteria"];
        if configurable_weights(method) {
            inputs.push(Input::repeated("criterion_weights", 0..=1));
            parameters.push("weight_method");
        }
        if configurable_normalization(method) {
            parameters.push("normalization");
        }
        if method == "grey_relational" {
            parameters.push("resolution");
        }
        let id = if method == "entropy_topsis" {
            "yssbi.statistics.workflow.entropy_topsis".to_string()
        } else {
            format!("yssbi.statistics.decision.{method}")
        };
        install(builder, &id, inputs, &parameters, 3, move |inv| {
            execute(method, inv)
        });
    }
}
fn configurable_weights(method: &str) -> bool {
    matches!(
        method,
        "weights"
            | "composite_index"
            | "topsis"
            | "grey_relational"
            | "wrsr"
            | "efficacy_coefficient"
    )
}
fn configurable_normalization(method: &str) -> bool {
    !matches!(method, "grey_relational" | "wrsr" | "efficacy_coefficient")
}
fn weighting(method: &str, inv: &KernelInvocation<'_>) -> Result<WeightMethod, KernelError> {
    let method = if configurable_weights(method) {
        text(inv, "weight_method")?
    } else {
        method
    };
    Ok(match method {
        "equal" => WeightMethod::Equal,
        "explicit" => WeightMethod::Explicit,
        "entropy" | "entropy_weight" | "entropy_topsis" => WeightMethod::Entropy,
        "critic" => WeightMethod::Critic,
        "information" | "information_weight" => WeightMethod::Information,
        "independence" | "independence_weight" => WeightMethod::Independence,
        _ => return Err(KernelError::InvalidParameter),
    })
}
fn execute(method: &str, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let data = read_criteria(inv)?;
    let weighting = weighting(method, inv)?;
    admit(
        &data,
        data.rows(),
        5,
        weighting == WeightMethod::Independence,
        inv,
    )?;
    let costs = costs(inv, data.columns.len())?;
    let weights = explicit_weights(&data, weighting == WeightMethod::Explicit, inv)?;
    let normalization = if configurable_normalization(method) {
        match text(inv, "normalization")? {
            "none" => Normalization::None,
            "minmax" => Normalization::MinMax,
            "vector" => Normalization::Vector,
            _ => return Err(KernelError::InvalidParameter),
        }
    } else if method == "wrsr" {
        Normalization::None
    } else {
        Normalization::MinMax
    };
    let ranking = match method {
        "topsis" | "entropy_topsis" => RankingMethod::Topsis,
        "grey_relational" => RankingMethod::GreyRelational,
        "wrsr" => RankingMethod::Wrsr,
        "efficacy_coefficient" => RankingMethod::Efficacy,
        _ => RankingMethod::Composite,
    };
    let options = RankingOptions {
        method: ranking,
        weighting,
        normalization,
        costs,
        weights,
        grey_resolution: if method == "grey_relational" {
            number(inv, "resolution")?
        } else {
            0.5
        },
    };
    let control = Control::from_shared(inv.control.cancellation.clone(), inv.control.deadline);
    let result = yss_sci_runtime::decision::ranking::calculate(&data.columns, options, &control)
        .map_err(computation_error)?;
    let report = named_report(&result.summary, &data, inv)?;
    let table = if ranking == RankingMethod::Topsis {
        numeric_table(
            &result.rows,
            1,
            [
                "observation",
                "score",
                "rank",
                "distance_best",
                "distance_worst",
            ],
            |r| {
                [
                    r.observation as f64,
                    r.score,
                    r.rank,
                    r.distance_best,
                    r.distance_worst,
                ]
            },
            inv,
        )?
    } else {
        numeric_table(
            &result.rows,
            1,
            ["observation", "score", "rank"],
            |r| [r.observation as f64, r.score, r.rank],
            inv,
        )?
    };
    Ok(vec![
        report,
        table,
        numeric_list(&result.summary.weighting.weights, inv)?,
    ])
}
