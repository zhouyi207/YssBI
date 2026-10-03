use super::{columns::*, *};
pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    for (method, parameters, min) in [
        ("vikor", &["cost_criteria", "majority_weight"][..], 1),
        ("coupling_coordination", &[][..], 2),
        ("obstacle_degree", &["cost_criteria", "rescale"][..], 1),
    ] {
        install(
            builder,
            &format!("yssbi.statistics.decision.{method}"),
            vec![
                Input::repeated("criteria", min..=usize::MAX),
                Input::repeated("criterion_weights", 0..=1),
            ],
            parameters,
            3,
            move |inv| execute(method, inv),
        );
    }
}
fn execute(method: &str, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let data = read_criteria(inv)?;
    let p = data.columns.len();
    let rows = if method == "obstacle_degree" {
        data.rows()
            .checked_mul(p)
            .ok_or(KernelError::BudgetExceeded)?
    } else {
        data.rows()
    };
    admit(&data, rows, 5, false, inv)?;
    let weights = explicit_weights(&data, !group(inv, "criterion_weights").is_empty(), inv)?;
    let control = Control::from_shared(inv.control.cancellation.clone(), inv.control.deadline);
    let (report, table, weights) = match method {
        "vikor" => {
            let result = yss_sci_runtime::decision::compromise::vikor(
                &data.columns,
                &costs(inv, p)?,
                &weights,
                number(inv, "majority_weight")?,
                &control,
            )
            .map_err(computation_error)?;
            (
                named_report(&result.summary, &data, inv)?,
                numeric_table(
                    &result.rows,
                    1,
                    [
                        "observation",
                        "compromise_score",
                        "group_utility",
                        "individual_regret",
                        "rank",
                    ],
                    |r| {
                        [
                            r.observation as f64,
                            r.compromise_score,
                            r.group_utility,
                            r.individual_regret,
                            r.rank,
                        ]
                    },
                    inv,
                )?,
                numeric_list(&result.summary.weights, inv)?,
            )
        }
        "coupling_coordination" => {
            let result =
                yss_sci_runtime::decision::systems::coupling(&data.columns, &weights, &control)
                    .map_err(computation_error)?;
            (
                named_report(&result.summary, &data, inv)?,
                numeric_table(
                    &result.rows,
                    1,
                    [
                        "observation",
                        "coupling",
                        "coordination_index",
                        "coordination_degree",
                    ],
                    |r| {
                        [
                            Some(r.observation as f64),
                            r.coupling,
                            Some(r.coordination_index),
                            Some(r.coordination_degree),
                        ]
                    },
                    inv,
                )?,
                numeric_list(&result.summary.weights, inv)?,
            )
        }
        "obstacle_degree" => {
            let result = yss_sci_runtime::decision::systems::obstacles(
                &data.columns,
                &costs(inv, p)?,
                &weights,
                boolean(inv, "rescale")?,
                &control,
            )
            .map_err(computation_error)?;
            (
                named_report(&result.summary, &data, inv)?,
                numeric_table(
                    &result.rows,
                    1,
                    [
                        "observation",
                        "criterion",
                        "deviation",
                        "weighted_deviation",
                        "obstacle_percent",
                    ],
                    |r| {
                        [
                            Some(r.observation as f64),
                            Some(r.criterion as f64),
                            Some(r.deviation),
                            Some(r.weighted_deviation),
                            r.obstacle_percent,
                        ]
                    },
                    inv,
                )?,
                numeric_list(&result.summary.weights, inv)?,
            )
        }
        _ => return Err(KernelError::InvalidParameter),
    };
    Ok(vec![report, table, weights])
}
