use polars::prelude::{AnyValue, DataFrame};

use std::collections::BTreeMap;

use yss_bayes_model::{
    BayesModelSpec, BinaryOp, Expression, LikelihoodSpec, MathFunction, UnaryOp,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BayesInputValidationError {
    pub code: &'static str,
    pub message: String,
    pub column: Option<String>,
    pub row: Option<usize>,
}

impl BayesInputValidationError {
    fn new(
        code: &'static str,
        message: impl Into<String>,
        column: impl Into<Option<String>>,
        row: impl Into<Option<usize>>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            column: column.into(),
            row: row.into(),
        }
    }
}

impl std::fmt::Display for BayesInputValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for BayesInputValidationError {}

pub fn validate_bayes_input_table(
    spec: &BayesModelSpec,
    table: &DataFrame,
) -> Result<(), BayesInputValidationError> {
    if table.height() == 0 {
        return Err(BayesInputValidationError::new(
            "bayes_input_empty",
            "Bayesian inference input table is empty.",
            None,
            None,
        ));
    }

    validate_response_column(spec, table)?;
    for column in spec.data_variables().values() {
        validate_numeric_predictor_column(table, column)?;
    }
    Ok(())
}

fn validate_response_column(
    spec: &BayesModelSpec,
    table: &DataFrame,
) -> Result<(), BayesInputValidationError> {
    for column_name in spec.response().data_variables.values() {
        validate_finite_numeric_column(
            table,
            column_name,
            "bayes_input_response_non_finite",
            "bayes_input_column_not_numeric",
            "response",
        )?;
    }
    let column_name = sole_response_column(spec)?;
    match spec.likelihood() {
        LikelihoodSpec::Normal { .. } => validate_response_expression(spec, table),
        LikelihoodSpec::BernoulliLogit { .. } => validate_bernoulli_response(table, column_name),
        LikelihoodSpec::PoissonLog { .. } => validate_poisson_response(table, column_name),
    }
}

fn sole_response_column(spec: &BayesModelSpec) -> Result<&str, BayesInputValidationError> {
    if spec.response().data_variables.len() != 1 {
        return Err(BayesInputValidationError::new(
            "bayes_input_response_binding_invalid",
            "Bayesian response must bind exactly one data variable.",
            None,
            None,
        ));
    }
    Ok(spec
        .response()
        .data_variables
        .values()
        .next()
        .expect("one binding"))
}

fn validate_response_expression(
    spec: &BayesModelSpec,
    table: &DataFrame,
) -> Result<(), BayesInputValidationError> {
    let column = sole_response_column(spec)?.to_string();
    for row in 0..table.height() {
        evaluate_response(
            &spec.response().expression,
            table,
            &spec.response().data_variables,
            row,
            &column,
        )?;
    }
    Ok(())
}

fn evaluate_response(
    expression: &Expression,
    table: &DataFrame,
    bindings: &BTreeMap<String, String>,
    row: usize,
    response_column: &str,
) -> Result<f64, BayesInputValidationError> {
    let value = match expression {
        Expression::Number { value } => *value,
        Expression::DataVariable { name } => {
            let column_name = bindings.get(name).ok_or_else(|| {
                BayesInputValidationError::new(
                    "bayes_input_response_binding_invalid",
                    format!("Response data variable `{name}` is not bound to a column."),
                    Some(response_column.to_string()),
                    Some(row),
                )
            })?;
            let value = table
                .column(column_name)
                .map_err(|_| missing_column(column_name))?
                .get(row)
                .map_err(|_| missing_column(column_name))?;
            numeric_value(value).ok_or_else(|| {
                BayesInputValidationError::new(
                    "bayes_input_column_not_numeric",
                    format!(
                        "Bayesian response column `{column_name}` must contain numeric values."
                    ),
                    Some(column_name.clone()),
                    Some(row),
                )
            })?
        }
        Expression::Column { name } => {
            let value = table
                .column(name)
                .map_err(|_| missing_column(name))?
                .get(row)
                .map_err(|_| missing_column(name))?;
            numeric_value(value).ok_or_else(|| {
                BayesInputValidationError::new(
                    "bayes_input_column_not_numeric",
                    format!("Bayesian response column `{name}` must contain numeric values."),
                    Some(name.clone()),
                    Some(row),
                )
            })?
        }
        Expression::Parameter { .. } => {
            return Err(response_eval_error(
                "bayes_input_response_parameter_forbidden",
                "Response expressions cannot contain parameters.",
                response_column,
                row,
            ));
        }
        Expression::Unary {
            op: UnaryOp::Neg,
            arg,
        } => -evaluate_response(arg, table, bindings, row, response_column)?,
        Expression::Binary { op, left, right } => {
            let left = evaluate_response(left, table, bindings, row, response_column)?;
            let right = evaluate_response(right, table, bindings, row, response_column)?;
            match op {
                BinaryOp::Add => left + right,
                BinaryOp::Sub => left - right,
                BinaryOp::Mul => left * right,
                BinaryOp::Div if right == 0.0 => {
                    return Err(response_eval_error(
                        "bayes_input_response_division_by_zero",
                        "Response expression divided by zero.",
                        response_column,
                        row,
                    ));
                }
                BinaryOp::Div => left / right,
                BinaryOp::Pow => left.powf(right),
            }
        }
        Expression::Call { function, args } => {
            let values = args
                .iter()
                .map(|arg| evaluate_response(arg, table, bindings, row, response_column))
                .collect::<Result<Vec<_>, _>>()?;
            match function {
                MathFunction::Exp => values[0].exp(),
                MathFunction::Ln if values[0] <= 0.0 => {
                    return Err(response_eval_error(
                        "bayes_input_response_ln_domain",
                        "Response ln argument must be greater than zero.",
                        response_column,
                        row,
                    ));
                }
                MathFunction::Ln => values[0].ln(),
                MathFunction::Sqrt if values[0] < 0.0 => {
                    return Err(response_eval_error(
                        "bayes_input_response_sqrt_domain",
                        "Response sqrt argument must be non-negative.",
                        response_column,
                        row,
                    ));
                }
                MathFunction::Sqrt => values[0].sqrt(),
                MathFunction::Abs => values[0].abs(),
                MathFunction::Sin => values[0].sin(),
                MathFunction::Cos => values[0].cos(),
                MathFunction::Min => values.into_iter().fold(f64::INFINITY, f64::min),
                MathFunction::Max => values.into_iter().fold(f64::NEG_INFINITY, f64::max),
            }
        }
    };
    if !value.is_finite() {
        return Err(response_eval_error(
            "bayes_input_response_result_non_finite",
            "Response expression returned a non-finite value.",
            response_column,
            row,
        ));
    }
    Ok(value)
}

fn response_eval_error(
    code: &'static str,
    message: &str,
    column: &str,
    row: usize,
) -> BayesInputValidationError {
    BayesInputValidationError::new(
        code,
        format!("{message} Row {}.", row + 1),
        Some(column.to_string()),
        Some(row),
    )
}

fn validate_numeric_predictor_column(
    table: &DataFrame,
    column_name: &str,
) -> Result<(), BayesInputValidationError> {
    validate_finite_numeric_column(
        table,
        column_name,
        "bayes_input_predictor_non_finite",
        "bayes_input_column_not_numeric",
        "predictor",
    )
}

fn validate_finite_numeric_column(
    table: &DataFrame,
    column_name: &str,
    non_finite_code: &'static str,
    non_numeric_code: &'static str,
    role: &str,
) -> Result<(), BayesInputValidationError> {
    let column = table
        .column(column_name)
        .map_err(|_| missing_column(column_name))?;
    for row in 0..table.height() {
        let value = column.get(row).map_err(|_| missing_column(column_name))?;
        if matches!(value, AnyValue::Null) {
            return Err(BayesInputValidationError::new(
                non_finite_code,
                format!(
                    "Bayesian {role} column `{column_name}` contains a missing or non-finite value at row {}.",
                    row + 1
                ),
                Some(column_name.to_string()),
                Some(row),
            ));
        }
        match numeric_value(value) {
            Some(value) if value.is_finite() => {}
            Some(_) => {
                return Err(BayesInputValidationError::new(
                    non_finite_code,
                    format!(
                        "Bayesian {role} column `{column_name}` contains a missing or non-finite value at row {}.",
                        row + 1
                    ),
                    Some(column_name.to_string()),
                    Some(row),
                ));
            }
            None => {
                return Err(BayesInputValidationError::new(
                    non_numeric_code,
                    format!("Bayesian {role} column `{column_name}` must contain numeric values."),
                    Some(column_name.to_string()),
                    Some(row),
                ));
            }
        }
    }
    Ok(())
}

fn validate_bernoulli_response(
    table: &DataFrame,
    column_name: &str,
) -> Result<(), BayesInputValidationError> {
    let column = table
        .column(column_name)
        .map_err(|_| missing_column(column_name))?;
    for row in 0..table.height() {
        let value = column.get(row).map_err(|_| missing_column(column_name))?;
        if matches!(value, AnyValue::Boolean(_)) {
            continue;
        }
        let Some(value) = numeric_value(value) else {
            return Err(BayesInputValidationError::new(
                "bayes_input_bernoulli_response_invalid",
                format!(
                    "BernoulliLogit response column `{column_name}` must contain boolean or 0/1 values."
                ),
                Some(column_name.to_string()),
                Some(row),
            ));
        };
        if !value.is_finite() || !(value == 0.0 || value == 1.0) {
            return Err(BayesInputValidationError::new(
                "bayes_input_bernoulli_response_invalid",
                format!(
                    "BernoulliLogit response column `{column_name}` contains a value other than 0/1 at row {}.",
                    row + 1
                ),
                Some(column_name.to_string()),
                Some(row),
            ));
        }
    }
    Ok(())
}

fn validate_poisson_response(
    table: &DataFrame,
    column_name: &str,
) -> Result<(), BayesInputValidationError> {
    let column = table
        .column(column_name)
        .map_err(|_| missing_column(column_name))?;
    for row in 0..table.height() {
        let value = column.get(row).map_err(|_| missing_column(column_name))?;
        let Some(value) = numeric_value(value) else {
            return Err(BayesInputValidationError::new(
                "bayes_input_column_not_numeric",
                format!("PoissonLog response column `{column_name}` must contain numeric counts."),
                Some(column_name.to_string()),
                Some(row),
            ));
        };
        if !value.is_finite() {
            return Err(BayesInputValidationError::new(
                "bayes_input_response_non_finite",
                format!(
                    "PoissonLog response column `{column_name}` contains a missing or non-finite value at row {}.",
                    row + 1
                ),
                Some(column_name.to_string()),
                Some(row),
            ));
        }
        if value < 0.0 {
            return Err(BayesInputValidationError::new(
                "bayes_input_poisson_response_negative",
                format!(
                    "PoissonLog response column `{column_name}` contains a negative count at row {}.",
                    row + 1
                ),
                Some(column_name.to_string()),
                Some(row),
            ));
        }
        if value.fract() != 0.0 {
            return Err(BayesInputValidationError::new(
                "bayes_input_poisson_response_not_integer",
                format!(
                    "PoissonLog response column `{column_name}` contains a non-integer count at row {}.",
                    row + 1
                ),
                Some(column_name.to_string()),
                Some(row),
            ));
        }
    }
    Ok(())
}

fn missing_column(column_name: &str) -> BayesInputValidationError {
    BayesInputValidationError::new(
        "bayes_input_column_missing",
        format!("Bayesian inference input column `{column_name}` is missing."),
        Some(column_name.to_string()),
        None,
    )
}

fn numeric_value(value: AnyValue<'_>) -> Option<f64> {
    match value {
        AnyValue::Float64(value) => Some(value),
        AnyValue::Float32(value) => Some(value as f64),
        AnyValue::Int64(value) => Some(value as f64),
        AnyValue::Int32(value) => Some(value as f64),
        AnyValue::Int16(value) => Some(value as f64),
        AnyValue::Int8(value) => Some(value as f64),
        AnyValue::UInt64(value) => Some(value as f64),
        AnyValue::UInt32(value) => Some(value as f64),
        AnyValue::UInt16(value) => Some(value as f64),
        AnyValue::UInt8(value) => Some(value as f64),
        _ => None,
    }
}
