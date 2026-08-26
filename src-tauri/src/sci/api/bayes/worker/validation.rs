use std::collections::{BTreeMap, BTreeSet};

use crate::sci::api::bayes::model::{
    BayesModelSpec, BinaryOp, Expression, LikelihoodSpec, MathFunction, ParameterConstraint,
    ParameterSpec, PriorSpec, UnaryOp,
};
use crate::sci::api::computation::{StatisticalInput, StatisticalScalar};

pub(super) fn model_is_valid(model: &BayesModelSpec) -> bool {
    if model.dataset.source_id.trim().is_empty()
        || model.display_formula.trim().is_empty()
        || model.response.data_variables.len() != 1
        || !valid_bindings(&model.response.data_variables)
        || !valid_bindings(&model.data_variables)
        || !expression_is_valid(&model.response.expression)
        || !expression_is_valid(&model.predictor)
        || !parameters_are_valid(&model.parameters)
        || !sampler_is_valid(model)
    {
        return false;
    }

    let parameter_names = model.parameter_names();
    let mut response_data = BTreeSet::new();
    let mut response_parameters = BTreeSet::new();
    collect_expression_symbols(
        &model.response.expression,
        &mut response_data,
        &mut response_parameters,
    );
    let response_matches = response_data.len() == 1
        && response_parameters.is_empty()
        && response_data
            .iter()
            .all(|name| model.response.data_variables.contains_key(name));
    if !response_matches {
        return false;
    }

    if !matches!(model.likelihood, LikelihoodSpec::Normal { .. })
        && !matches!(
            &model.response.expression,
            Expression::DataVariable { name }
                if model.response.data_variables.contains_key(name)
        )
    {
        return false;
    }

    let mut predictor_data = BTreeSet::new();
    let mut predictor_parameters = BTreeSet::new();
    collect_expression_symbols(
        &model.predictor,
        &mut predictor_data,
        &mut predictor_parameters,
    );
    if predictor_data
        .iter()
        .any(|name| !model.data_variables.contains_key(name))
        || predictor_parameters
            .iter()
            .any(|name| !parameter_names.contains(name.as_str()))
    {
        return false;
    }

    match &model.likelihood {
        LikelihoodSpec::Normal { sigma, .. } => parameter_names.contains(sigma.parameter.as_str()),
        LikelihoodSpec::BernoulliLogit { .. } | LikelihoodSpec::PoissonLog { .. } => true,
    }
}

fn valid_bindings(bindings: &BTreeMap<String, String>) -> bool {
    bindings
        .iter()
        .all(|(name, column)| !name.trim().is_empty() && !column.trim().is_empty())
}

fn expression_is_valid(expression: &Expression) -> bool {
    match expression {
        Expression::Number { value } => value.is_finite(),
        Expression::DataVariable { name }
        | Expression::Column { name }
        | Expression::Parameter { name } => !name.trim().is_empty(),
        Expression::Unary { arg, .. } => expression_is_valid(arg),
        Expression::Binary { left, right, .. } => {
            expression_is_valid(left) && expression_is_valid(right)
        }
        Expression::Call { function, args } => {
            let arity_is_valid = match function {
                MathFunction::Exp
                | MathFunction::Ln
                | MathFunction::Sqrt
                | MathFunction::Abs
                | MathFunction::Sin
                | MathFunction::Cos => args.len() == 1,
                MathFunction::Min | MathFunction::Max => args.len() >= 2,
            };
            arity_is_valid && args.iter().all(expression_is_valid)
        }
    }
}

fn collect_expression_symbols(
    expression: &Expression,
    data: &mut BTreeSet<String>,
    parameters: &mut BTreeSet<String>,
) {
    match expression {
        Expression::Number { .. } => {}
        Expression::DataVariable { name } | Expression::Column { name } => {
            data.insert(name.clone());
        }
        Expression::Parameter { name } => {
            parameters.insert(name.clone());
        }
        Expression::Unary { arg, .. } => collect_expression_symbols(arg, data, parameters),
        Expression::Binary { left, right, .. } => {
            collect_expression_symbols(left, data, parameters);
            collect_expression_symbols(right, data, parameters);
        }
        Expression::Call { args, .. } => {
            for arg in args {
                collect_expression_symbols(arg, data, parameters);
            }
        }
    }
}

fn parameters_are_valid(parameters: &[ParameterSpec]) -> bool {
    let mut names = BTreeSet::new();
    parameters.iter().all(|parameter| {
        !parameter.name.trim().is_empty()
            && names.insert(parameter.name.as_str())
            && constraint_is_valid(&parameter.constraint)
            && prior_is_valid(&parameter.prior)
    })
}

fn constraint_is_valid(constraint: &ParameterConstraint) -> bool {
    match constraint {
        ParameterConstraint::Real | ParameterConstraint::Positive | ParameterConstraint::Unit => {
            true
        }
        ParameterConstraint::Bounded { lower, upper, .. } => {
            lower.is_finite() && upper.is_finite() && lower < upper
        }
    }
}

fn prior_is_valid(prior: &PriorSpec) -> bool {
    match prior {
        PriorSpec::Normal([mean, sd]) | PriorSpec::LogNormal([mean, sd]) => {
            mean.is_finite() && sd.is_finite() && *sd > 0.0
        }
        PriorSpec::Uniform([lower, upper]) => {
            lower.is_finite() && upper.is_finite() && lower < upper
        }
        PriorSpec::Beta([alpha, beta]) | PriorSpec::Gamma([alpha, beta]) => {
            alpha.is_finite() && beta.is_finite() && *alpha > 0.0 && *beta > 0.0
        }
        PriorSpec::Exponential([rate]) | PriorSpec::HalfNormal([rate]) => {
            rate.is_finite() && *rate > 0.0
        }
        PriorSpec::StudentT([degrees, location, scale]) => {
            degrees.is_finite()
                && location.is_finite()
                && scale.is_finite()
                && *degrees > 0.0
                && *scale > 0.0
        }
        PriorSpec::Cauchy([location, scale]) => {
            location.is_finite() && scale.is_finite() && *scale > 0.0
        }
    }
}

fn sampler_is_valid(model: &BayesModelSpec) -> bool {
    model.sampler.chains > 0
        && model.sampler.samples > 0
        && model
            .sampler
            .target_accept
            .is_none_or(|value| value.is_finite() && (0.0..=1.0).contains(&value))
        && !matches!(model.sampler.max_tree_depth, Some(0))
}

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
        .response
        .data_variables
        .values()
        .chain(model.data_variables.values())
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

    let Some(response_column) = model.response.data_variables.values().next() else {
        return Err(0);
    };
    let Some((response_index, response)) = by_name.get(response_column.as_str()).copied() else {
        return Err(0);
    };
    match &model.likelihood {
        LikelihoodSpec::Normal { .. } => {
            for row in 0..response.values().len() {
                if evaluate_response(
                    &model.response.expression,
                    &model.response.data_variables,
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
