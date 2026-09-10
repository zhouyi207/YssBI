//! Canonical validation for immutable Bayesian model specifications.

use std::collections::{BTreeMap, BTreeSet};

use super::model::{
    BayesModelSpec, Expression, LikelihoodSpec, MathFunction, ParameterConstraint, ParameterSpec,
    PriorSpec,
};

pub fn model_spec_is_valid(model: &BayesModelSpec) -> bool {
    if model.dataset().source_id.trim().is_empty()
        || model.display_formula().trim().is_empty()
        || model.response().data_variables.len() != 1
        || !valid_bindings(&model.response().data_variables)
        || !valid_bindings(model.data_variables())
        || !expression_is_valid(&model.response().expression)
        || !expression_is_valid(model.predictor())
        || !parameters_are_valid(model.parameters())
        || !sampler_is_valid(model)
    {
        return false;
    }

    let parameter_names = model.parameter_names();
    let mut response_data = BTreeSet::new();
    let mut response_parameters = BTreeSet::new();
    collect_expression_symbols(
        &model.response().expression,
        &mut response_data,
        &mut response_parameters,
    );
    let response_matches = response_data.len() == 1
        && response_parameters.is_empty()
        && response_data
            .iter()
            .all(|name| model.response().data_variables.contains_key(name));
    if !response_matches {
        return false;
    }

    if !matches!(model.likelihood(), LikelihoodSpec::Normal { .. })
        && !matches!(
            &model.response().expression,
            Expression::DataVariable { name }
                if model.response().data_variables.contains_key(name)
        )
    {
        return false;
    }

    let mut predictor_data = BTreeSet::new();
    let mut predictor_parameters = BTreeSet::new();
    collect_expression_symbols(
        model.predictor(),
        &mut predictor_data,
        &mut predictor_parameters,
    );
    if predictor_data
        .iter()
        .any(|name| !model.data_variables().contains_key(name))
        || predictor_parameters
            .iter()
            .any(|name| !parameter_names.contains(name.as_str()))
    {
        return false;
    }

    match model.likelihood() {
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

pub(super) fn constraint_is_valid(constraint: &ParameterConstraint) -> bool {
    match constraint {
        ParameterConstraint::Real | ParameterConstraint::Positive | ParameterConstraint::Unit => {
            true
        }
        ParameterConstraint::Bounded { lower, upper, .. } => {
            lower.is_finite() && upper.is_finite() && lower < upper
        }
    }
}

pub(super) fn prior_is_valid(prior: &PriorSpec) -> bool {
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
    model.sampler().chains > 0
        && model.sampler().samples > 0
        && model
            .sampler()
            .target_accept
            .is_none_or(|value| value.is_finite() && (0.0..=1.0).contains(&value))
        && !matches!(model.sampler().max_tree_depth, Some(0))
}
