use super::{Input, common::*, install};
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};
use yss_data_contract::TabularScalar;
use yss_sci_contract::{execution::*, regression::models::IterationOptions, spatial::*};
use yss_sci_runtime::spatial::{moran, regression, weights};

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    for method in [
        "weights", "moran", "ols", "slm", "sem", "sac", "sdm", "sdem", "slx", "panel",
    ] {
        let mut inputs = vec![];
        if method != "weights" {
            inputs.push(Input::fixed("weights"));
        }
        inputs.push(Input::fixed("units"));
        if method == "weights" {
            inputs.extend([Input::fixed("x"), Input::fixed("y")]);
        } else {
            if method == "panel" {
                inputs.push(Input::fixed("periods"));
            }
            inputs.push(Input::fixed("y"));
            if method != "moran" {
                inputs.push(Input::repeated("x", 1..=usize::MAX));
            }
        }
        let parameters = match method {
            "weights" => vec![
                "spatial_weight_rule",
                "spatial_neighbors",
                "spatial_radius",
                "spatial_power",
                "spatial_symmetrize",
                "spatial_row_standardize",
            ],
            "moran" => vec!["spatial_permutations", "seed"],
            "ols" | "slx" => vec!["constant"],
            "panel" => vec!["spatial_panel_model", "max_iterations", "tolerance"],
            _ => vec!["constant", "max_iterations", "tolerance"],
        };
        install(
            builder,
            &format!("yssbi.statistics.spatial.{method}"),
            inputs,
            &parameters,
            1,
            move |inv| execute(method, inv),
        );
    }
}
fn budget(
    inv: &KernelInvocation<'_>,
    units: usize,
    rows: usize,
    predictors: usize,
    retained: usize,
) -> Result<(), KernelError> {
    // Dense eigensystems/solves, full likelihood Hessian, materialized columns,
    // and the weights/report's simultaneous JSON/runtime containers.
    inv.control.check_bytes((|| {
        let p = predictors.checked_mul(2)?.checked_add(4)?;
        retained
            .checked_add(units.checked_mul(units)?.checked_mul(640)?)?
            .checked_add(rows.checked_mul(p.checked_add(12)?)?.checked_mul(384)?)?
            .checked_add(p.checked_mul(p)?.checked_mul(512)?)
    })())?;
    Ok(())
}
fn execute(method: &str, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    inv.check_control()?;
    let control = ScientificExecutionControl::from_shared(
        inv.control.cancellation.clone(),
        inv.control.deadline,
    );
    if method == "weights" {
        let (columns, retained) = materialize(inv)?;
        let (codes, units) = categories(&columns[0], false, inv)?;
        if codes.len() != units.len() {
            return Err(KernelError::InvalidParameter);
        }
        budget(inv, units.len(), units.len(), 0, retained)?;
        let x = numeric(&columns[1], false, inv)?;
        let y = numeric(&columns[2], false, inv)?;
        let options = WeightsOptions {
            rule: match text(inv, "spatial_weight_rule")? {
                "knn" => WeightRule::Knn,
                "distance_band" => WeightRule::DistanceBand,
                "inverse_distance" => WeightRule::InverseDistance,
                _ => return Err(KernelError::InvalidParameter),
            },
            neighbors: integer(inv, "spatial_neighbors")?,
            radius: number(inv, "spatial_radius")?,
            power: number(inv, "spatial_power")?,
            symmetrize: boolean(inv, "spatial_symmetrize")?,
            row_standardize: boolean(inv, "spatial_row_standardize")?,
        };
        let w = weights::construct(&x, &y, options, &control).map_err(computation_error)?;
        return Ok(vec![value(
            SpatialWeights {
                units,
                matrix: w.matrix,
                options: w.options,
                islands: w.islands,
            },
            inv,
        )?]);
    }
    let w: SpatialWeights<TabularScalar> = decode_model(inv)?;
    if w.units.len() != w.matrix.len() {
        return Err(KernelError::ShapeMismatch);
    }
    let materialization = KernelInvocation {
        relations: inv.relations,
        inputs: &inv.inputs[1..],
        input_keys: &inv.input_keys[1..],
        parameters: Default::default(),
        outputs: inv.outputs,
        control: inv.control,
    };
    let (columns, retained) = materialize(&materialization)?;
    let n = columns[0].values.len();
    let predictor_indices = inv
        .input_keys
        .iter()
        .enumerate()
        .filter_map(|(i, k)| (*k == "x").then_some(i))
        .collect::<Vec<_>>();
    budget(inv, w.units.len(), n, predictor_indices.len(), retained)?;
    weights::validate(&w.matrix, &control).map_err(computation_error)?;
    let combined = super::super::series::Column {
        values: w.units.iter().chain(&columns[0].values).cloned().collect(),
        metadata: None,
        metadata_bytes: 0,
    };
    let (codes, labels) = categories(&combined, false, inv)?;
    if labels.len() != w.units.len()
        || codes[..w.units.len()]
            .iter()
            .enumerate()
            .any(|(i, c)| i != *c)
    {
        return Err(KernelError::InvalidParameter);
    }
    let unit_codes = codes[w.units.len()..].to_vec();
    let (period_codes, period_labels) = if method == "panel" {
        categories(&columns[1], false, inv)?
    } else {
        (vec![0; n], vec![])
    };
    let periods = if method == "panel" {
        period_labels.len()
    } else {
        1
    };
    if w.units.len().checked_mul(periods) != Some(n) || (method == "panel" && periods < 2) {
        return Err(KernelError::ShapeMismatch);
    }
    let mut order = vec![usize::MAX; n];
    for i in 0..n {
        inv.check_control()?;
        let slot = period_codes[i] * w.units.len() + unit_codes[i];
        if order[slot] != usize::MAX {
            return Err(KernelError::InvalidParameter);
        }
        order[slot] = i;
    }
    if order.contains(&usize::MAX) {
        return Err(KernelError::ShapeMismatch);
    }
    let numeric_column = |key_index: usize| -> Result<Vec<f64>, KernelError> {
        let v = numeric(&columns[key_index - 1], false, inv)?;
        Ok(order.iter().map(|&i| v[i]).collect())
    };
    let response_index = inv
        .input_keys
        .iter()
        .position(|k| *k == "y")
        .ok_or(KernelError::InputLayoutMismatch)?;
    let y = numeric_column(response_index)?;
    if method == "moran" {
        return Ok(vec![value(
            moran::analyze(
                &y,
                &w.matrix,
                MoranOptions {
                    permutations: integer(inv, "spatial_permutations")?,
                    seed: integer(inv, "seed")? as u64,
                },
                &control,
            )
            .map_err(computation_error)?,
            inv,
        )?]);
    }
    let x = predictor_indices
        .iter()
        .map(|&i| numeric_column(i))
        .collect::<Result<Vec<_>, _>>()?;
    let labels = predictor_indices
        .iter()
        .enumerate()
        .map(|(j, &i)| input_label(&inv.inputs[i], format!("x{}", j + 1)))
        .collect::<Vec<_>>();
    let kind = match if method == "panel" {
        text(inv, "spatial_panel_model")?
    } else {
        method
    } {
        "ols" => SpatialMethod::Ols,
        "slm" => SpatialMethod::Slm,
        "sem" => SpatialMethod::Sem,
        "sac" => SpatialMethod::Sac,
        "sdm" => SpatialMethod::Sdm,
        "sdem" => SpatialMethod::Sdem,
        "slx" => SpatialMethod::Slx,
        _ => return Err(KernelError::InvalidParameter),
    };
    let iteration = if matches!(method, "ols" | "slx") {
        IterationOptions::default()
    } else {
        IterationOptions {
            max_iterations: integer(inv, "max_iterations")?,
            tolerance: number(inv, "tolerance")?,
        }
    };
    let mut model = if method == "panel" {
        regression::panel(kind, &y, &x, &w.matrix, iteration, &control)
    } else {
        regression::fit(
            kind,
            &y,
            &x,
            &w.matrix,
            SpatialOptions {
                constant: boolean(inv, "constant")?,
                iteration,
            },
            &control,
        )
    }
    .map_err(computation_error)?;
    let offset = usize::from(model.constant);
    for (j, label) in labels.iter().enumerate() {
        model.coefficients[offset + j].term.clone_from(label);
        if kind.lag_x() {
            model.coefficients[offset + labels.len() + j].term = format!("W:{label}");
        }
        model.impacts[j].term.clone_from(label);
    }
    for values in [
        &mut model.fitted,
        &mut model.residuals,
        &mut model.innovations,
        &mut model.reduced_fitted,
    ] {
        let sorted = std::mem::take(values);
        *values = vec![0.0; n];
        for (slot, &original) in order.iter().enumerate() {
            values[original] = sorted[slot];
        }
    }
    Ok(vec![value(
        SpatialModelReport {
            model,
            unit_labels: w.units,
            period_labels,
            observation_units: unit_codes,
            observation_periods: if method == "panel" {
                period_codes
            } else {
                vec![]
            },
        },
        inv,
    )?])
}
