use polars::prelude::{AnyValue, DataFrame};

use super::model::{BayesModelSpec, LikelihoodSpec};

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
            "BAYES_INPUT_EMPTY",
            "Bayesian inference input table is empty.",
            None,
            None,
        ));
    }

    validate_response_column(spec, table)?;
    for column in spec.data_variables.values() {
        validate_numeric_predictor_column(table, column)?;
    }
    Ok(())
}

fn validate_response_column(
    spec: &BayesModelSpec,
    table: &DataFrame,
) -> Result<(), BayesInputValidationError> {
    let column_name = spec.response.column.as_str();
    match &spec.likelihood {
        LikelihoodSpec::Normal { .. } => validate_finite_numeric_column(
            table,
            column_name,
            "BAYES_INPUT_RESPONSE_NON_FINITE",
            "BAYES_INPUT_COLUMN_NOT_NUMERIC",
            "response",
        ),
        LikelihoodSpec::BernoulliLogit { .. } => validate_bernoulli_response(table, column_name),
        LikelihoodSpec::PoissonLog { .. } => validate_poisson_response(table, column_name),
    }
}

fn validate_numeric_predictor_column(
    table: &DataFrame,
    column_name: &str,
) -> Result<(), BayesInputValidationError> {
    validate_finite_numeric_column(
        table,
        column_name,
        "BAYES_INPUT_PREDICTOR_NON_FINITE",
        "BAYES_INPUT_COLUMN_NOT_NUMERIC",
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
                "BAYES_INPUT_BERNOULLI_RESPONSE_INVALID",
                format!(
                    "BernoulliLogit response column `{column_name}` must contain boolean or 0/1 values."
                ),
                Some(column_name.to_string()),
                Some(row),
            ));
        };
        if !value.is_finite() || !(value == 0.0 || value == 1.0) {
            return Err(BayesInputValidationError::new(
                "BAYES_INPUT_BERNOULLI_RESPONSE_INVALID",
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
                "BAYES_INPUT_COLUMN_NOT_NUMERIC",
                format!("PoissonLog response column `{column_name}` must contain numeric counts."),
                Some(column_name.to_string()),
                Some(row),
            ));
        };
        if !value.is_finite() {
            return Err(BayesInputValidationError::new(
                "BAYES_INPUT_RESPONSE_NON_FINITE",
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
                "BAYES_INPUT_POISSON_RESPONSE_NEGATIVE",
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
                "BAYES_INPUT_POISSON_RESPONSE_NOT_INTEGER",
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
        "BAYES_INPUT_COLUMN_MISSING",
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
