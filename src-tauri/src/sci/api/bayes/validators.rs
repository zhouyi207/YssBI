use std::collections::{BTreeMap, BTreeSet};

use super::draft::{BayesModelDraft, ColumnDType, ColumnMeta, SymbolRole};
use super::model::{
    Expression, InferenceConfig, LikelihoodSpec, MathFunction, ParameterConstraint, ParameterSpec,
    PriorSpec,
};
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
    fn error(&mut self, code: &str, message: impl Into<String>, path: impl Into<String>) {
        self.errors.push(error(code, message, path));
    }

    fn warning(&mut self, code: &str, message: impl Into<String>, path: impl Into<String>) {
        self.warnings.push(warning(code, message, path));
    }

    fn into_report(self) -> ValidationReport {
        ValidationReport::new(self.errors, self.warnings)
    }
}

fn validate_formula(draft: &BayesModelDraft, context: &mut ValidationContext) {
    if draft.formula_text.trim().is_empty() {
        context.error("FORMULA_REQUIRED", "请输入模型方程。", "formulaText");
    }
}

fn validate_dataset_and_bindings(draft: &BayesModelDraft, context: &mut ValidationContext) {
    let Some(dataset) = &draft.dataset else {
        context.error("DATASET_REQUIRED", "请选择数据源。", "dataset");
        return;
    };

    if dataset.source_id.trim().is_empty() {
        context.error("DATASET_REQUIRED", "请选择数据源。", "dataset.sourceId");
    }

    let columns = column_map(&dataset.columns);
    let dependent_symbols: Vec<_> = draft
        .symbols
        .iter()
        .filter(|symbol| symbol.role == SymbolRole::Dependent)
        .collect();
    if dependent_symbols.len() != 1 {
        context.error(
            "DEPENDENT_SYMBOL_REQUIRED",
            "必须且只能设置一个因变量符号。",
            "symbols",
        );
    }

    let Some(response_binding) = &draft.response_binding else {
        context.error("RESPONSE_REQUIRED", "请选择响应变量列。", "responseBinding");
        return;
    };

    if response_binding.symbol.trim().is_empty() {
        context.error(
            "RESPONSE_SYMBOL_REQUIRED",
            "响应表达式必须绑定一个基础数据符号。",
            "responseBinding.symbol",
        );
    }

    if response_binding.column.trim().is_empty() {
        context.error(
            "RESPONSE_REQUIRED",
            "请选择响应变量列。",
            "responseBinding.column",
        );
    } else if !columns.is_empty() && !columns.contains_key(response_binding.column.as_str()) {
        context.error(
            "RESPONSE_COLUMN_UNKNOWN",
            format!("响应变量列 {} 不存在。", response_binding.column),
            "responseBinding.column",
        );
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
                        "DATA_COLUMN_UNKNOWN",
                        format!("自变量 {} 绑定的数据列 {} 不存在。", symbol.name, column),
                        format!("dataBindings.{}", symbol.name),
                    );
                }
            }
            _ => context.error(
                "DATA_BINDING_REQUIRED",
                format!("自变量 {} 尚未绑定数据库列。", symbol.name),
                format!("dataBindings.{}", symbol.name),
            ),
        }
    }
}

fn validate_response_expression(draft: &BayesModelDraft, context: &mut ValidationContext) {
    let Some(response) = &draft.bound_response else {
        context.error(
            "RESPONSE_EXPRESSION_REQUIRED",
            "响应表达式尚未绑定。",
            "boundResponse",
        );
        return;
    };
    validate_expression_node(response, "boundResponse", context);

    let mut data = BTreeSet::new();
    let mut parameters = BTreeSet::new();
    collect_expression_symbols(response, &mut data, &mut parameters);
    if !parameters.is_empty() {
        context.error(
            "RESPONSE_PARAMETER_FORBIDDEN",
            "响应表达式不能引用模型参数。",
            "boundResponse",
        );
    }
    if data.len() != 1 {
        context.error(
            "RESPONSE_DATA_SYMBOL_COUNT_INVALID",
            "响应表达式必须且只能引用一个基础数据符号。",
            "boundResponse",
        );
    }
    if let Some(binding) = &draft.response_binding
        && data.len() == 1
        && !data.contains(&binding.symbol)
    {
        context.error(
            "RESPONSE_BINDING_MISMATCH",
            "响应表达式的数据符号与响应列绑定不一致。",
            "responseBinding.symbol",
        );
    }

    if !matches!(draft.likelihood, LikelihoodSpec::Normal { .. })
        && !matches!(response, Expression::DataVariable { name } if draft.response_binding.as_ref().is_some_and(|binding| binding.symbol == *name))
    {
        context.error(
            "LIKELIHOOD_RESPONSE_TRANSFORM_UNSUPPORTED",
            "BernoulliLogit 和 PoissonLog 仅支持未变换的响应符号。",
            "boundResponse",
        );
    }
}

fn validate_expression(draft: &BayesModelDraft, context: &mut ValidationContext) {
    let Some(predictor) = &draft.bound_predictor else {
        context.error(
            "PREDICTOR_REQUIRED",
            "预测表达式尚未解析或绑定。",
            "boundPredictor",
        );
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
            context.error(
                "PREDICTOR_DATA_SYMBOL_UNCONFIGURED",
                format!("预测表达式中的自变量 {} 尚未配置。", name),
                "symbols",
            );
        }
    }
    for name in predictor_parameters {
        if !configured_parameters.contains(name.as_str()) {
            context.error(
                "PREDICTOR_PARAMETER_UNCONFIGURED",
                format!("预测表达式中的参数 {} 尚未配置。", name),
                "parameters",
            );
        }
    }
}

fn validate_expression_node(expression: &Expression, path: &str, context: &mut ValidationContext) {
    match expression {
        Expression::Number { value } if !value.is_finite() => {
            context.error("EXPRESSION_NUMBER_INVALID", "表达式包含非法数值。", path);
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
        context.error(
            "EXPRESSION_FUNCTION_ARITY_INVALID",
            format!("函数 {:?} 的参数数量不合法。", function),
            path,
        );
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
                "LIKELIHOOD_RESPONSE_TYPE_INVALID",
                "Normal likelihood 需要数值型响应变量。",
                context,
            );
            validate_numeric_predictor_columns(draft, &columns, context);
            match draft
                .parameters
                .iter()
                .find(|parameter| parameter.name == sigma.parameter)
            {
                Some(parameter) if constraint_allows_positive(&parameter.constraint) => {}
                Some(_) => context.warning(
                    "LIKELIHOOD_SIGMA_CONSTRAINT_WARNING",
                    "Normal likelihood 的 sigma 参数建议使用 positive 约束。",
                    "parameters",
                ),
                None => context.error(
                    "LIKELIHOOD_SIGMA_PARAMETER_REQUIRED",
                    format!("sigma 参数 {} 不存在。", sigma.parameter),
                    "likelihood.sigma",
                ),
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
                "LIKELIHOOD_RESPONSE_TYPE_INVALID",
                "BernoulliLogit likelihood 需要 boolean 或 0/1 数值响应变量。",
                context,
            );
            validate_numeric_predictor_columns(draft, &columns, context);
        }
        LikelihoodSpec::PoissonLog { .. } => {
            validate_response_dtype(
                response_dtype,
                &[ColumnDType::Integer, ColumnDType::Number],
                "LIKELIHOOD_RESPONSE_TYPE_INVALID",
                "PoissonLog likelihood 需要计数型响应变量。",
                context,
            );
            validate_numeric_predictor_columns(draft, &columns, context);
            context.warning(
                "POISSON_RESPONSE_NON_NEGATIVE_UNCHECKED",
                "Poisson 响应变量需要非负；当前仅静态校验列类型。",
                "responseBinding.column",
            );
        }
    }
}

fn validate_response_dtype(
    dtype: Option<&ColumnDType>,
    allowed: &[ColumnDType],
    code: &str,
    message: &str,
    context: &mut ValidationContext,
) {
    let Some(dtype) = dtype else {
        return;
    };
    if !allowed.contains(dtype) {
        context.error(code, message, "responseBinding.column");
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
                "PREDICTOR_COLUMN_TYPE_INVALID",
                format!("自变量 {} 绑定的列 {} 不是数值列。", symbol, column),
                format!("dataBindings.{}", symbol),
            );
        }
    }
}

fn validate_parameters(draft: &BayesModelDraft, context: &mut ValidationContext) {
    if draft.parameters.is_empty() {
        context.warning(
            "NO_PARAMETERS",
            "当前模型尚未识别出未知参数。",
            "parameters",
        );
    }

    let mut seen = BTreeSet::new();
    for parameter in &draft.parameters {
        if parameter.name.trim().is_empty() {
            context.error("PARAMETER_NAME_REQUIRED", "参数名不能为空。", "parameters");
        }
        if !seen.insert(parameter.name.as_str()) {
            context.error(
                "PARAMETER_NAME_DUPLICATED",
                format!("参数 {} 重复。", parameter.name),
                "parameters",
            );
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
    if let ParameterConstraint::Bounded { lower, upper, .. } = constraint {
        if !lower.is_finite() || !upper.is_finite() || lower >= upper {
            context.error(
                "PARAMETER_BOUNDS_INVALID",
                format!("参数 {} 的下界必须小于上界。", name),
                format!("parameters.{}.constraint", name),
            );
        }
    }
}

fn validate_prior(name: &str, prior: &PriorSpec, context: &mut ValidationContext) {
    let invalid = match prior {
        PriorSpec::Normal([_, sd]) => *sd <= 0.0,
        PriorSpec::LogNormal([_, sd]) => *sd <= 0.0,
        PriorSpec::Uniform([lower, upper]) => lower >= upper,
        PriorSpec::Beta([alpha, beta]) => *alpha <= 0.0 || *beta <= 0.0,
        PriorSpec::Gamma([shape, scale]) => *shape <= 0.0 || *scale <= 0.0,
        PriorSpec::Exponential([rate]) => *rate <= 0.0,
        PriorSpec::StudentT([df, _, scale]) => *df <= 0.0 || *scale <= 0.0,
        PriorSpec::Cauchy([_, scale]) => *scale <= 0.0,
        PriorSpec::HalfNormal([scale]) => *scale <= 0.0,
    };
    if invalid {
        context.error(
            "PARAMETER_PRIOR_ARGS_INVALID",
            format!("参数 {} 的先验分布参数不合法。", name),
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
            "PARAMETER_PRIOR_CONSTRAINT_MISMATCH",
            format!("参数 {} 的先验分布与约束界限可能不匹配。", name),
            format!("parameters.{}", name),
        );
    }
}

fn validate_sampler(sampler: &InferenceConfig, context: &mut ValidationContext) {
    if sampler.chains == 0 {
        context.error(
            "SAMPLER_CHAINS_INVALID",
            "chains 必须大于 0。",
            "sampler.chains",
        );
    }
    if sampler.samples == 0 {
        context.error(
            "SAMPLER_SAMPLES_INVALID",
            "samples 必须大于 0。",
            "sampler.samples",
        );
    }
    if let Some(target_accept) = sampler.target_accept {
        if !(0.0..=1.0).contains(&target_accept) {
            context.error(
                "SAMPLER_TARGET_ACCEPT_INVALID",
                "target accept 必须在 0 到 1 之间。",
                "sampler.targetAccept",
            );
        }
    }
    if matches!(sampler.max_tree_depth, Some(0)) {
        context.error(
            "SAMPLER_MAX_TREE_DEPTH_INVALID",
            "max tree depth 必须大于 0。",
            "sampler.maxTreeDepth",
        );
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

pub fn collect_expression_symbols(
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
