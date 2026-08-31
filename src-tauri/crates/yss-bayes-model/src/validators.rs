//! Cross-field validation for Bayesian model drafts.

use std::collections::{BTreeMap, BTreeSet};

use super::draft::{BayesModelDraft, ColumnDType, ColumnMeta, SymbolRole};
use super::model::{
    Expression, InferenceConfig, LikelihoodSpec, MathFunction, ParameterConstraint, ParameterSpec,
    PriorSpec,
};
use super::spec_validation::{constraint_is_valid, prior_is_valid};
use super::validation::{ValidationIssue, ValidationReport, error, warning};

pub fn validate_draft(draft: &BayesModelDraft) -> ValidationReport {
    let mut context = ValidationContext::default();
    validate_formula(draft, &mut context);
    validate_dataset_and_bindings(draft, &mut context);
    validate_response_expression(draft, &mut context);
    validate_expression(draft, &mut context);
    validate_likelihood(draft, &mut context);
    validate_parameters(draft, &mut context);
    validate_sampler(&draft.sampler, &mut context);
    context.into_report()
}

#[derive(Default)]
struct ValidationContext {
    errors: Vec<ValidationIssue>,
    warnings: Vec<ValidationIssue>,
}

impl ValidationContext {
    fn error(&mut self, code: &str, path: impl Into<String>) {
        self.errors.push(error(code, path));
    }

    fn warning(&mut self, code: &str, path: impl Into<String>) {
        self.warnings.push(warning(code, path));
    }

    fn into_report(self) -> ValidationReport {
        ValidationReport::new(self.errors, self.warnings)
    }
}

fn validate_formula(draft: &BayesModelDraft, context: &mut ValidationContext) {
    if draft.formula_text.trim().is_empty() {
        context.error("formula_required", "formulaText");
    }
}

fn validate_dataset_and_bindings(draft: &BayesModelDraft, context: &mut ValidationContext) {
    let Some(dataset) = &draft.dataset else {
        context.error("dataset_required", "dataset");
        return;
    };

    if dataset.source_id.trim().is_empty() {
        context.error("dataset_required", "dataset.sourceId");
    }

    let columns = column_map(&dataset.columns);
    let dependent_symbols: Vec<_> = draft
        .symbols
        .iter()
        .filter(|symbol| symbol.role == SymbolRole::Dependent)
        .collect();
    if dependent_symbols.len() != 1 {
        context.error("dependent_symbol_required", "symbols");
    }

    let Some(response_binding) = &draft.response_binding else {
        context.error("response_required", "responseBinding");
        return;
    };

    if response_binding.symbol.trim().is_empty() {
        context.error("response_symbol_required", "responseBinding.symbol");
    }

    if response_binding.column.trim().is_empty() {
        context.error("response_required", "responseBinding.column");
    } else if !columns.is_empty() && !columns.contains_key(response_binding.column.as_str()) {
        context.error("response_column_unknown", "responseBinding.column");
    }

    for symbol in draft
        .symbols
        .iter()
        .filter(|symbol| symbol.role == SymbolRole::Independent)
    {
        match draft.data_bindings.get(&symbol.name) {
            Some(column) if !column.trim().is_empty() => {
                if !columns.is_empty() && !columns.contains_key(column.as_str()) {
                    context.error(
                        "data_column_unknown",
                        format!("dataBindings.{}", symbol.name),
                    );
                }
            }
            _ => context.error(
                "data_binding_required",
                format!("dataBindings.{}", symbol.name),
            ),
        }
    }
}

fn validate_response_expression(draft: &BayesModelDraft, context: &mut ValidationContext) {
    let Some(response) = &draft.bound_response else {
        context.error("response_expression_required", "boundResponse");
        return;
    };
    validate_expression_node(response, "boundResponse", context);

    let mut data = BTreeSet::new();
    let mut parameters = BTreeSet::new();
    collect_expression_symbols(response, &mut data, &mut parameters);
    if !parameters.is_empty() {
        context.error("response_parameter_forbidden", "boundResponse");
    }
    if data.len() != 1 {
        context.error("response_data_symbol_count_invalid", "boundResponse");
    }
    if let Some(binding) = &draft.response_binding
        && data.len() == 1
        && !data.contains(&binding.symbol)
    {
        context.error("response_binding_mismatch", "responseBinding.symbol");
    }

    if !matches!(draft.likelihood, LikelihoodSpec::Normal { .. })
        && !matches!(response, Expression::DataVariable { name } if draft.response_binding.as_ref().is_some_and(|binding| binding.symbol == *name))
    {
        context.error("likelihood_response_transform_unsupported", "boundResponse");
    }
}

fn validate_expression(draft: &BayesModelDraft, context: &mut ValidationContext) {
    let Some(predictor) = &draft.bound_predictor else {
        context.error("predictor_required", "boundPredictor");
        return;
    };

    validate_expression_node(predictor, "boundPredictor", context);

    let mut predictor_data = BTreeSet::new();
    let mut predictor_parameters = BTreeSet::new();
    collect_expression_symbols(predictor, &mut predictor_data, &mut predictor_parameters);
    let configured_data: BTreeSet<&str> = draft
        .symbols
        .iter()
        .filter(|symbol| symbol.role == SymbolRole::Independent)
        .map(|symbol| symbol.name.as_str())
        .collect();
    let configured_parameters: BTreeSet<&str> = draft
        .parameters
        .iter()
        .map(|parameter| parameter.name.as_str())
        .collect();

    for name in predictor_data {
        if !configured_data.contains(name.as_str()) {
            context.error("predictor_data_symbol_unconfigured", "symbols");
        }
    }
    for name in predictor_parameters {
        if !configured_parameters.contains(name.as_str()) {
            context.error("predictor_parameter_unconfigured", "parameters");
        }
    }
}

fn validate_expression_node(expression: &Expression, path: &str, context: &mut ValidationContext) {
    match expression {
        Expression::Number { value } if !value.is_finite() => {
            context.error("expression_number_invalid", path);
        }
        Expression::Number { .. }
        | Expression::DataVariable { .. }
        | Expression::Column { .. }
        | Expression::Parameter { .. } => {}
        Expression::Unary { arg, .. } => validate_expression_node(arg, path, context),
        Expression::Binary { left, right, .. } => {
            validate_expression_node(left, path, context);
            validate_expression_node(right, path, context);
        }
        Expression::Call { function, args } => {
            validate_function_arity(*function, args.len(), path, context);
            for arg in args {
                validate_expression_node(arg, path, context);
            }
        }
    }
}

fn validate_function_arity(
    function: MathFunction,
    count: usize,
    path: &str,
    context: &mut ValidationContext,
) {
    let valid = match function {
        MathFunction::Exp
        | MathFunction::Ln
        | MathFunction::Sqrt
        | MathFunction::Abs
        | MathFunction::Sin
        | MathFunction::Cos => count == 1,
        MathFunction::Min | MathFunction::Max => count >= 2,
    };
    if !valid {
        context.error("expression_function_arity_invalid", path);
    }
}

fn validate_likelihood(draft: &BayesModelDraft, context: &mut ValidationContext) {
    let Some(dataset) = &draft.dataset else {
        return;
    };
    let Some(response_binding) = &draft.response_binding else {
        return;
    };
    let columns = column_map(&dataset.columns);
    let response_dtype = columns
        .get(response_binding.column.as_str())
        .map(|column| &column.dtype);

    match &draft.likelihood {
        LikelihoodSpec::Normal { sigma, .. } => {
            validate_response_dtype(
                response_dtype,
                &[ColumnDType::Number, ColumnDType::Integer],
                "likelihood_response_type_invalid",
                context,
            );
            validate_numeric_predictor_columns(draft, &columns, context);
            match draft
                .parameters
                .iter()
                .find(|parameter| parameter.name == sigma.parameter)
            {
                Some(parameter) if constraint_allows_positive(&parameter.constraint) => {}
                Some(_) => context.warning("likelihood_sigma_constraint_warning", "parameters"),
                None => context.error("likelihood_sigma_parameter_required", "likelihood.sigma"),
            }
        }
        LikelihoodSpec::BernoulliLogit { .. } => {
            validate_response_dtype(
                response_dtype,
                &[
                    ColumnDType::Boolean,
                    ColumnDType::Integer,
                    ColumnDType::Number,
                ],
                "likelihood_response_type_invalid",
                context,
            );
            validate_numeric_predictor_columns(draft, &columns, context);
        }
        LikelihoodSpec::PoissonLog { .. } => {
            validate_response_dtype(
                response_dtype,
                &[ColumnDType::Integer, ColumnDType::Number],
                "likelihood_response_type_invalid",
                context,
            );
            validate_numeric_predictor_columns(draft, &columns, context);
            context.warning(
                "poisson_response_non_negative_unchecked",
                "responseBinding.column",
            );
        }
    }
}

fn validate_response_dtype(
    dtype: Option<&ColumnDType>,
    allowed: &[ColumnDType],
    code: &str,
    context: &mut ValidationContext,
) {
    let Some(dtype) = dtype else {
        return;
    };
    if !allowed.contains(dtype) {
        context.error(code, "responseBinding.column");
    }
}

fn validate_numeric_predictor_columns(
    draft: &BayesModelDraft,
    columns: &BTreeMap<&str, &ColumnMeta>,
    context: &mut ValidationContext,
) {
    for (symbol, column) in &draft.data_bindings {
        let Some(meta) = columns.get(column.as_str()) else {
            continue;
        };
        if !matches!(meta.dtype, ColumnDType::Number | ColumnDType::Integer) {
            context.error(
                "predictor_column_type_invalid",
                format!("dataBindings.{}", symbol),
            );
        }
    }
}

fn validate_parameters(draft: &BayesModelDraft, context: &mut ValidationContext) {
    if draft.parameters.is_empty() {
        context.warning("no_parameters", "parameters");
    }

    let mut seen = BTreeSet::new();
    for parameter in &draft.parameters {
        if parameter.name.trim().is_empty() {
            context.error("parameter_name_required", "parameters");
        }
        if !seen.insert(parameter.name.as_str()) {
            context.error("parameter_name_duplicated", "parameters");
        }
        validate_parameter(parameter, context);
    }
}

fn validate_parameter(parameter: &ParameterSpec, context: &mut ValidationContext) {
    validate_constraint(&parameter.name, &parameter.constraint, context);
    validate_prior(&parameter.name, &parameter.prior, context);
    validate_constraint_prior_compatibility(
        &parameter.name,
        &parameter.constraint,
        &parameter.prior,
        context,
    );
}

fn validate_constraint(
    name: &str,
    constraint: &ParameterConstraint,
    context: &mut ValidationContext,
) {
    if !constraint_is_valid(constraint) {
        context.error(
            "parameter_bounds_invalid",
            format!("parameters.{}.constraint", name),
        );
    }
}

fn validate_prior(name: &str, prior: &PriorSpec, context: &mut ValidationContext) {
    if !prior_is_valid(prior) {
        context.error(
            "parameter_prior_args_invalid",
            format!("parameters.{}.prior", name),
        );
    }
}

fn validate_constraint_prior_compatibility(
    name: &str,
    constraint: &ParameterConstraint,
    prior: &PriorSpec,
    context: &mut ValidationContext,
) {
    let compatible = match constraint {
        ParameterConstraint::Real => true,
        ParameterConstraint::Positive => matches!(
            prior,
            PriorSpec::LogNormal(_)
                | PriorSpec::Gamma(_)
                | PriorSpec::Exponential(_)
                | PriorSpec::HalfNormal(_)
        ),
        ParameterConstraint::Unit => match prior {
            PriorSpec::Beta(_) => true,
            PriorSpec::Uniform([lower, upper]) => *lower >= 0.0 && *upper <= 1.0,
            _ => false,
        },
        ParameterConstraint::Bounded { lower, upper, .. } => match prior {
            PriorSpec::Uniform([prior_lower, prior_upper]) => {
                prior_lower >= lower && prior_upper <= upper
            }
            PriorSpec::Beta(_) => *lower <= 0.0 && *upper >= 1.0,
            _ => true,
        },
    };
    if !compatible {
        context.warning(
            "parameter_prior_constraint_mismatch",
            format!("parameters.{}", name),
        );
    }
}

fn validate_sampler(sampler: &InferenceConfig, context: &mut ValidationContext) {
    if sampler.chains == 0 {
        context.error("sampler_chains_invalid", "sampler.chains");
    }
    if sampler.samples == 0 {
        context.error("sampler_samples_invalid", "sampler.samples");
    }
    if let Some(target_accept) = sampler.target_accept
        && !(0.0..=1.0).contains(&target_accept)
    {
        context.error("sampler_target_accept_invalid", "sampler.targetAccept");
    }
    if matches!(sampler.max_tree_depth, Some(0)) {
        context.error("sampler_max_tree_depth_invalid", "sampler.maxTreeDepth");
    }
}

fn constraint_allows_positive(constraint: &ParameterConstraint) -> bool {
    match constraint {
        ParameterConstraint::Positive => true,
        ParameterConstraint::Bounded { lower, .. } => *lower >= 0.0,
        _ => false,
    }
}

fn column_map(columns: &[ColumnMeta]) -> BTreeMap<&str, &ColumnMeta> {
    columns
        .iter()
        .map(|column| (column.name.as_str(), column))
        .collect()
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
