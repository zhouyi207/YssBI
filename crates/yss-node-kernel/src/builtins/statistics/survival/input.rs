use super::super::common::{
    STRUCTURED_VALUE_BYTES, STRUCTURED_VALUE_COPIES, categories, group, input_label, integer,
    materialize, numeric,
};
use crate::{KernelError, KernelInvocation};
use yss_data_contract::TabularScalar;

pub(super) struct PreparedInputs {
    pub time: Vec<f64>,
    pub event: Vec<f64>,
    pub predictors: Vec<Vec<f64>>,
    pub predictor_labels: Vec<String>,
    pub group_codes: Vec<usize>,
    pub group_labels: Vec<TabularScalar>,
    pub status: Option<Vec<usize>>,
    pub start: Vec<f64>,
    pub treatment: Vec<f64>,
    pub predicted_risk: Vec<f64>,
}

pub(super) fn prepare(
    method: &str,
    inv: &KernelInvocation<'_>,
) -> Result<PreparedInputs, KernelError> {
    let (materialized, retained) = materialize(inv)?;
    let n = materialized.first().map_or(0, |c| c.values.len());
    let index = |key: &str| {
        inv.input_keys
            .iter()
            .position(|k| *k == key)
            .ok_or(KernelError::InvalidNumericInput)
    };
    let read = |key: &str, binary| numeric(&materialized[index(key)?], binary, inv);
    let (group_codes, group_labels) = match method {
        "survival.kaplan_meier" | "survival.nelson_aalen" => {
            if let Ok(i) = index("groups") {
                categories(&materialized[i], false, inv)?
            } else {
                (vec![0; n], vec![TabularScalar::String("All".into())])
            }
        }
        "survival.logrank" | "workflow.subgroup" => {
            categories(&materialized[index("groups")?], false, inv)?
        }
        "survival.time_dependent_cox" => categories(&materialized[index("subjects")?], false, inv)?,
        _ => (Vec::new(), Vec::new()),
    };
    let status = if method == "survival.competing_risks" {
        Some(
            materialized[index("status")?]
                .values
                .iter()
                .enumerate()
                .map(|(i, value)| {
                    if i.is_multiple_of(1024) {
                        inv.check_control()?;
                    }
                    status_code(value)
                })
                .collect::<Result<Vec<_>, _>>()?,
        )
    } else {
        None
    };
    let causes = {
        let mut causes = std::collections::BTreeSet::new();
        if let Some(status) = &status {
            for (i, &value) in status.iter().enumerate() {
                if i.is_multiple_of(1024) {
                    inv.check_control()?;
                }
                if value > 0 {
                    causes.insert(value);
                }
            }
        }
        causes.len()
    };
    let predictors = group(inv, "x");
    let width = predictors
        .len()
        .checked_add(if method == "workflow.subgroup" {
            group_labels.len()
        } else {
            0
        })
        .and_then(|v| v.checked_add(4))
        .ok_or(KernelError::BudgetExceeded)?;
    // Only subgroup labels become design columns; Log-rank labels size its covariance.
    let matrix_width = if method == "survival.logrank" {
        group_labels.len()
    } else {
        width
    };
    let plot_points = if method == "plot.decision_curve" {
        integer(inv, "decision_points")?
    } else {
        n
    };
    inv.control.check_bytes((|| {
        let matrix_items = matrix_width.checked_mul(matrix_width)?;
        let workspace = n
            .checked_mul(width.checked_add(8)?)?
            .checked_mul(128)?
            .checked_add(matrix_items.checked_mul(192)?)?
            .checked_add(causes.checked_mul(128)?)?;
        let output_items = n
            .checked_mul(causes.checked_add(24)?)?
            .checked_add(matrix_items)?
            .checked_add(plot_points.checked_mul(16)?)?
            .checked_add(group_labels.len().checked_mul(8)?)?;
        retained.checked_add(workspace)?.checked_add(
            output_items.checked_mul(STRUCTURED_VALUE_BYTES * STRUCTURED_VALUE_COPIES)?,
        )
    })())?;
    let predictor_labels = predictors
        .iter()
        .enumerate()
        .map(|(j, v)| input_label(v, format!("x{}", j + 1)))
        .collect();
    let mut predictors = vec![];
    for (i, key) in inv.input_keys.iter().enumerate() {
        if *key == "x" {
            predictors.push(numeric(&materialized[i], false, inv)?);
        }
    }
    let time = read(
        if method == "survival.time_dependent_cox" {
            "stop"
        } else {
            "time"
        },
        false,
    )?;
    let event = if status.is_some() {
        Vec::new()
    } else {
        read("event", true)?
    };
    let start = if method == "survival.time_dependent_cox" {
        read("start", false)?
    } else {
        Vec::new()
    };
    let treatment = if method == "workflow.subgroup" {
        read("treatment", true)?
    } else {
        Vec::new()
    };
    let predicted_risk = if matches!(method, "plot.calibration" | "plot.decision_curve") {
        read("predicted_risk", false)?
    } else {
        Vec::new()
    };
    Ok(PreparedInputs {
        time,
        event,
        predictors,
        predictor_labels,
        group_codes,
        group_labels,
        status,
        start,
        treatment,
        predicted_risk,
    })
}

fn status_code(value: &TabularScalar) -> Result<usize, KernelError> {
    match value {
        TabularScalar::Integer(v) => {
            usize::try_from(*v).map_err(|_| KernelError::InvalidNumericInput)
        }
        TabularScalar::Unsigned(v) => {
            usize::try_from(*v).map_err(|_| KernelError::InvalidNumericInput)
        }
        TabularScalar::Float64(v)
            if v.as_f64() >= 0.0
                && v.as_f64() < (usize::MAX as f64)
                && v.as_f64().fract() == 0.0 =>
        {
            Ok(v.as_f64() as usize)
        }
        _ => Err(KernelError::InvalidNumericInput),
    }
}
