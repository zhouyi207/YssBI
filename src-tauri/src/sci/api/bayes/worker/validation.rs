use std::collections::{BTreeMap, BTreeSet};

use yss_bayes_model::{
    BayesModelSpec, BinaryOp, Expression, LikelihoodSpec, MathFunction, UnaryOp,
};
use yss_sci_contract::{StatisticalInput, StatisticalScalar};

pub(super) fn validate_inputs(
    model: &BayesModelSpec,
    inputs: &[StatisticalInput],
) -> Result<(), usize> {
    if inputs.is_empty() {
        return Err(0);
    }
    let mut by_name = BTreeMap::new();
    for (index, input) in inputs.iter().enumerate() {
        if input.name().is_empty() || by_name.insert(input.name(), (index, input)).is_some() {
            return Err(index);
        }
    }

    let required_columns = model
        .response()
        .data_variables
        .values()
        .chain(model.data_variables().values())
        .collect::<BTreeSet<_>>();
    let mut row_count = None;
    for (required_index, column) in required_columns.iter().enumerate() {
        let Some((input_index, input)) = by_name.get(column.as_str()).copied() else {
            return Err(required_index);
        };
        if input.values().is_empty()
            || row_count.is_some_and(|expected| expected != input.values().len())
            || input.values().iter().any(|value| {
                !matches!(value, Some(StatisticalScalar::Numeric(number)) if number.is_finite())
            })
        {
            return Err(input_index);
        }
        row_count.get_or_insert(input.values().len());
    }

    let Some(response_column) = model.response().data_variables.values().next() else {
        return Err(0);
    };
    let Some((response_index, response)) = by_name.get(response_column.as_str()).copied() else {
        return Err(0);
    };
    match model.likelihood() {
        LikelihoodSpec::Normal { .. } => {
            for row in 0..response.values().len() {
                if evaluate_response(
                    &model.response().expression,
                    &model.response().data_variables,
                    &by_name,
                    row,
                )
                .is_none()
                {
                    return Err(response_index);
                }
            }
        }
        LikelihoodSpec::BernoulliLogit { .. } => {
            if response.values().iter().any(|value| {
                !matches!(value, Some(StatisticalScalar::Numeric(number)) if *number == 0.0 || *number == 1.0)
            }) {
                return Err(response_index);
            }
        }
        LikelihoodSpec::PoissonLog { .. } => {
            if response.values().iter().any(|value| {
                !matches!(value, Some(StatisticalScalar::Numeric(number)) if *number >= 0.0 && number.fract() == 0.0)
            }) {
                return Err(response_index);
            }
        }
    }
    Ok(())
}

fn evaluate_response(
    expression: &Expression,
    bindings: &BTreeMap<String, String>,
    inputs: &BTreeMap<&str, (usize, &StatisticalInput)>,
    row: usize,
) -> Option<f64> {
    let value = match expression {
        Expression::Number { value } => *value,
        Expression::DataVariable { name } => {
            let column = bindings.get(name)?;
            numeric_input_value(inputs.get(column.as_str())?.1, row)?
        }
        Expression::Column { name } => numeric_input_value(inputs.get(name.as_str())?.1, row)?,
        Expression::Parameter { .. } => return None,
        Expression::Unary {
            op: UnaryOp::Neg,
            arg,
        } => -evaluate_response(arg, bindings, inputs, row)?,
        Expression::Binary { op, left, right } => {
            let left = evaluate_response(left, bindings, inputs, row)?;
            let right = evaluate_response(right, bindings, inputs, row)?;
            match op {
                BinaryOp::Add => left + right,
                BinaryOp::Sub => left - right,
                BinaryOp::Mul => left * right,
                BinaryOp::Div if right == 0.0 => return None,
                BinaryOp::Div => left / right,
                BinaryOp::Pow => left.powf(right),
            }
        }
        Expression::Call { function, args } => {
            let values = args
                .iter()
                .map(|arg| evaluate_response(arg, bindings, inputs, row))
                .collect::<Option<Vec<_>>>()?;
            let first = *values.first()?;
            match function {
                MathFunction::Exp => first.exp(),
                MathFunction::Ln if first <= 0.0 => return None,
                MathFunction::Ln => first.ln(),
                MathFunction::Sqrt if first < 0.0 => return None,
                MathFunction::Sqrt => first.sqrt(),
                MathFunction::Abs => first.abs(),
                MathFunction::Sin => first.sin(),
                MathFunction::Cos => first.cos(),
                MathFunction::Min => values.into_iter().fold(f64::INFINITY, f64::min),
                MathFunction::Max => values.into_iter().fold(f64::NEG_INFINITY, f64::max),
            }
        }
    };
    value.is_finite().then_some(value)
}

fn numeric_input_value(input: &StatisticalInput, row: usize) -> Option<f64> {
    match input.values().get(row)? {
        Some(StatisticalScalar::Numeric(value)) if value.is_finite() => Some(*value),
        Some(StatisticalScalar::Numeric(_)) | Some(StatisticalScalar::Category(_)) | None => None,
    }
}
