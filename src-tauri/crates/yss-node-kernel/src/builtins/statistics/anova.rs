use super::super::{numeric_input, series};
use super::{
    Input,
    common::{text, value},
};
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};
use std::cmp::Ordering;
use yss_data_contract::TabularScalar;
use yss_sci_contract::{anova::*, execution::*};
use yss_sci_runtime::anova as sci;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Method {
    OneWay,
    TwoWay,
    ThreeWay,
    Factorial,
    Ancova,
    Manova,
    Repeated,
}

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    use Method::*;
    for (id, method, min, max) in [
        ("one_way", OneWay, 1, 1),
        ("two_way", TwoWay, 2, 2),
        ("three_way", ThreeWay, 3, 3),
        ("factorial", Factorial, 1, MAX_ANOVA_FACTORS),
        ("ancova", Ancova, 1, MAX_ANOVA_FACTORS),
        ("manova", Manova, 1, MAX_ANOVA_FACTORS),
        ("repeated_measures", Repeated, 1, MAX_REPEATED_FACTORS),
    ] {
        let mut inputs = if method == Manova {
            vec![Input::repeated("responses", 2..=MAX_MANOVA_RESPONSES)]
        } else {
            vec![Input::fixed("response")]
        };
        if method == Repeated {
            inputs.push(Input::fixed("subjects"));
        }
        inputs.push(Input::repeated("factors", min..=max));
        if method == Ancova {
            inputs.push(Input::repeated("covariates", 1..=MAX_ANOVA_COVARIATES));
        }
        let parameters = match method {
            OneWay => &[][..],
            Repeated => &["sphericity_correction"][..],
            _ => &["model_terms", "sums_of_squares"][..],
        };
        super::install(
            builder,
            &format!("yssbi.statistics.anova.{id}"),
            inputs,
            parameters,
            1,
            move |inv| execute(method, inv),
        );
    }
}

fn scientific_error(error: ScientificComputationError) -> KernelError {
    match error {
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
fn options(method: Method, inv: &KernelInvocation<'_>) -> Result<AnovaOptions, KernelError> {
    if method == Method::OneWay || method == Method::Repeated {
        return Ok(AnovaOptions::default());
    }
    Ok(AnovaOptions {
        model: match text(inv, "model_terms")? {
            "main_effects" => FactorialModel::MainEffects,
            "full_factorial" => FactorialModel::FullFactorial,
            _ => return Err(KernelError::InvalidParameter),
        },
        sums_of_squares: match text(inv, "sums_of_squares")? {
            "type_i" => SumsOfSquares::TypeI,
            "type_ii" => SumsOfSquares::TypeII,
            "type_iii" => SumsOfSquares::TypeIII,
            _ => return Err(KernelError::InvalidParameter),
        },
    })
}

fn materialize(inv: &KernelInvocation<'_>) -> Result<(Vec<series::Column>, usize), KernelError> {
    inv.check_control()?;
    let columns = if inv
        .inputs
        .iter()
        .all(|v| matches!(v.unannotated(), RuntimeValue::Series(_)))
    {
        let handles = inv
            .inputs
            .iter()
            .map(|v| match v.unannotated() {
                RuntimeValue::Series(handle) => handle.clone(),
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();
        series::load(&handles, inv)?
    } else {
        let Some(RuntimeValue::List(first)) = inv.inputs.first().map(RuntimeValue::unannotated)
        else {
            return Err(KernelError::UnalignedSeries);
        };
        for input in inv.inputs {
            let RuntimeValue::List(column) = input.unannotated() else {
                return Err(KernelError::UnalignedSeries);
            };
            if column.len() != first.len() {
                return Err(KernelError::ShapeMismatch);
            }
        }
        inv.control.check_bytes(
            first
                .len()
                .checked_mul(inv.inputs.len())
                .and_then(|n| n.checked_mul(size_of::<RuntimeValue>() * 8)),
        )?;
        inv.inputs
            .iter()
            .map(|v| series::column(v, inv))
            .collect::<Result<Vec<_>, _>>()?
    };
    let n = columns.first().map_or(0, |column| column.values.len());
    let mut retained = inv.control.check_bytes(
        n.checked_mul(columns.len())
            .and_then(|n| n.checked_mul(size_of::<RuntimeValue>() * 8)),
    )?;
    for column in &columns {
        for (i, scalar) in column.values.iter().enumerate() {
            if i.is_multiple_of(1024) {
                inv.check_control()?;
            }
            if let TabularScalar::String(label) = scalar {
                retained = inv.control.check_bytes(
                    label
                        .len()
                        .checked_mul(8)
                        .and_then(|n| retained.checked_add(n)),
                )?;
            }
        }
    }
    Ok((columns, retained))
}

fn numerical(column: &series::Column, inv: &KernelInvocation<'_>) -> Result<Vec<f64>, KernelError> {
    let mut values = inv.control.reserve(column.values.len())?;
    for (i, scalar) in column.values.iter().enumerate() {
        if i.is_multiple_of(1024) {
            inv.check_control()?;
        }
        values.push(numeric_input(Some(&RuntimeValue::Scalar(scalar.clone())))?);
    }
    Ok(values)
}

fn factor(
    column: &series::Column,
    inv: &KernelInvocation<'_>,
) -> Result<(Factor, Vec<TabularScalar>), KernelError> {
    let mut labels = Vec::<TabularScalar>::new();
    let mut values = inv.control.reserve(column.values.len())?;
    for (i, scalar) in column.values.iter().enumerate() {
        if i.is_multiple_of(1024) {
            inv.check_control()?;
        }
        if matches!(scalar, TabularScalar::Null) {
            return Err(KernelError::InvalidNumericInput);
        }
        let index = match labels
            .iter()
            .position(|label| label.compare(scalar) == Some(Ordering::Equal))
        {
            Some(index) => index,
            None => {
                if labels.len() == MAX_ANOVA_LEVELS {
                    return Err(KernelError::InvalidParameter);
                }
                labels.push(scalar.clone());
                labels.len() - 1
            }
        };
        values.push(index);
    }
    Ok((
        Factor {
            values,
            levels: labels.len(),
        },
        labels,
    ))
}

// Sorting exact scalar values avoids both lossy numeric IDs and quadratic subject lookup.
fn subjects(
    column: &series::Column,
    inv: &KernelInvocation<'_>,
) -> Result<Vec<usize>, KernelError> {
    let first = column
        .values
        .first()
        .ok_or(KernelError::InvalidNumericInput)?;
    for (i, value) in column.values.iter().enumerate() {
        if i.is_multiple_of(1024) {
            inv.check_control()?;
        }
        if matches!(value, TabularScalar::Null) || value.compare(first).is_none() {
            return Err(KernelError::InvalidNumericInput);
        }
    }
    let mut order = inv.control.reserve(column.values.len())?;
    order.extend(0..column.values.len());
    order.sort_unstable_by(|&a, &b| {
        column.values[a]
            .compare(&column.values[b])
            .expect("comparable subject labels")
    });
    inv.check_control()?;
    let mut ids = inv.control.reserve(column.values.len())?;
    ids.resize(column.values.len(), 0);
    let mut previous = None;
    let mut id = 0;
    for (i, row) in order.into_iter().enumerate() {
        if i.is_multiple_of(1024) {
            inv.check_control()?;
        }
        if previous.is_some_and(|previous| {
            column.values[row].compare(&column.values[previous]) != Some(Ordering::Equal)
        }) {
            id += 1;
        }
        ids[row] = id;
        previous = Some(row);
    }
    Ok(ids)
}

fn execute(method: Method, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let options = options(method, inv)?;
    let (columns, retained) = materialize(inv)?;
    let mut factors = Vec::new();
    let mut labels = Vec::new();
    let mut responses = Vec::new();
    let mut covariates = Vec::new();
    let mut subject_ids = None;
    for (key, column) in inv.input_keys.iter().zip(&columns) {
        match *key {
            "response" | "responses" => responses.push(numerical(column, inv)?),
            "factors" => {
                let (coded, levels) = factor(column, inv)?;
                factors.push(coded);
                labels.push(levels);
            }
            "covariates" => covariates.push(numerical(column, inv)?),
            "subjects" => subject_ids = Some(subjects(column, inv)?),
            _ => return Err(KernelError::InputLayoutMismatch),
        }
    }
    let levels = factors
        .iter()
        .map(|factor| factor.levels)
        .collect::<Vec<_>>();
    let width = if method == Method::Repeated {
        levels
            .iter()
            .try_fold(1usize, |n, &level| n.checked_mul(level))
            .filter(|&n| n <= MAX_REPEATED_CELLS)
            .ok_or(KernelError::InvalidParameter)?
    } else {
        design_columns(&levels, covariates.len(), options.model)
            .ok_or(KernelError::InvalidParameter)?
    };
    let n = responses
        .first()
        .ok_or(KernelError::InvalidNumericInput)?
        .len();
    // Admit retained columns, concurrent nested-fit matrices and encoded report together.
    let bytes = (|| {
        let design = n.checked_mul(width)?.checked_mul(8)?;
        let matrices = width.checked_mul(width)?.checked_mul(12)?;
        let response = n.checked_mul(responses.len())?.checked_mul(8)?;
        let report = width
            .checked_mul(
                responses
                    .len()
                    .checked_mul(responses.len())?
                    .checked_add(32)?,
            )?
            .checked_mul(512)?;
        design
            .checked_add(matrices)?
            .checked_add(response)?
            .checked_mul(size_of::<f64>())?
            .checked_add(report)?
            .checked_add(retained)
    })();
    inv.control.check_bytes(bytes)?;
    let control = ScientificExecutionControl::from_shared(
        inv.control.cancellation.clone(),
        inv.control.deadline,
    );
    let result = if method == Method::Manova {
        value(
            sci::manova(&responses, &factors, options, &control)
                .map_err(scientific_error)?
                .map_levels(|i, level| labels[i][level].clone()),
            inv,
        )?
    } else if method == Method::Repeated {
        let correction = match text(inv, "sphericity_correction")? {
            "none" => SphericityCorrection::None,
            "greenhouse_geisser" => SphericityCorrection::GreenhouseGeisser,
            _ => return Err(KernelError::InvalidParameter),
        };
        value(
            sci::repeated_measures(
                &responses[0],
                &subject_ids.ok_or(KernelError::InvalidParameter)?,
                &factors,
                correction,
                &control,
            )
            .map_err(scientific_error)?
            .map_levels(|i, level| labels[i][level].clone()),
            inv,
        )?
    } else {
        value(
            sci::anova(&responses[0], &factors, &covariates, options, &control)
                .map_err(scientific_error)?
                .map_levels(|i, level| labels[i][level].clone()),
            inv,
        )?
    };
    Ok(vec![result])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{KernelControl, KernelOutputSpec};
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering as AtomicOrdering},
        },
        time::{Duration, Instant},
    };
    use yss_data_contract::ValueType;

    #[test]
    fn anova_admission_counts_workspace_and_honors_interruption() {
        let relations = crate::tests::relations();
        let numbers = |values: &[f64]| {
            RuntimeValue::List(
                values
                    .iter()
                    .map(|&v| RuntimeValue::float64(v).unwrap())
                    .collect(),
            )
        };
        let inputs = [
            numbers(&[1., 2., 3., 2., 4., 6.]),
            numbers(&[0., 0., 0., 1., 1., 1.]),
        ];
        let outputs = [KernelOutputSpec {
            data_type: ValueType::Struct("statistics.report".into()),
            fields: None,
        }];
        let mut control = KernelControl::new(
            Arc::new(AtomicBool::new(false)),
            Instant::now() + Duration::from_secs(30),
        );
        control.max_input_bytes = 32 * 1024;
        let mut inv = KernelInvocation {
            relations: &relations,
            inputs: &inputs,
            input_keys: &["response", "factors"],
            parameters: Default::default(),
            outputs: &outputs,
            control: &control,
        };
        assert!(matches!(
            execute(Method::OneWay, &inv),
            Err(KernelError::BudgetExceeded)
        ));
        let admitted = KernelControl {
            cancellation: control.cancellation.clone(),
            deadline: control.deadline,
            max_input_bytes: 1024 * 1024,
        };
        inv.control = &admitted;
        assert!(execute(Method::OneWay, &inv).is_ok());
        admitted.cancellation.store(true, AtomicOrdering::Release);
        assert!(matches!(
            execute(Method::OneWay, &inv),
            Err(KernelError::Cancelled)
        ));
        admitted.cancellation.store(false, AtomicOrdering::Release);
        let expired = KernelControl {
            cancellation: admitted.cancellation.clone(),
            deadline: Instant::now(),
            max_input_bytes: admitted.max_input_bytes,
        };
        inv.control = &expired;
        assert!(matches!(
            execute(Method::OneWay, &inv),
            Err(KernelError::DeadlineExceeded)
        ));
    }
}
