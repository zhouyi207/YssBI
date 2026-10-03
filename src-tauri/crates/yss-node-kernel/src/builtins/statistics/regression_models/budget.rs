use super::{integer, list, text};
use crate::builtins::series::Column;
use crate::builtins::statistics::common::{STRUCTURED_VALUE_BYTES, STRUCTURED_VALUE_COPIES};
use crate::{KernelError, KernelInvocation};
use yss_data_contract::TabularScalar;

pub(super) fn check(
    method: &str,
    inv: &KernelInvocation<'_>,
    data: &[Column],
    constant: bool,
    categories: usize,
    groups: usize,
) -> Result<(), KernelError> {
    let rows = data.first().map_or(0, |column| column.values.len());
    let predictors = inv.input_keys.iter().filter(|key| **key == "x").count();
    let inflation = inv
        .input_keys
        .iter()
        .filter(|key| **key == "inflation_predictors")
        .count();
    let mut design_columns = predictors + usize::from(constant);
    let mut parameters = design_columns;
    let mut models = 1;
    let mut basis_columns = 0;
    let mut components = 0;
    match method {
        "regression.logit.multinomial" => {
            parameters = design_columns * categories.saturating_sub(1);
        }
        "regression.logit.ordinal" => {
            parameters = design_columns + categories.saturating_sub(1);
        }
        "regression.zero_inflated_poisson" | "regression.zero_inflated_negative_binomial" => {
            design_columns += inflation + 1;
            parameters = design_columns
                + usize::from(method == "regression.zero_inflated_negative_binomial");
        }
        "regression.negative_binomial" | "regression.tobit" | "regression.beta" => {
            parameters += 1;
        }
        "regression.curve" => {
            design_columns = if text(inv, "curve_family")? == "polynomial" {
                let degree = if inv.parameter("degree").is_some() {
                    integer(inv, "degree")?
                } else {
                    1
                };
                if !(1..=8).contains(&degree) {
                    return Err(KernelError::InvalidParameter);
                }
                degree + 1
            } else {
                2
            };
            parameters = design_columns;
        }
        "regression.nonlinear" => {
            parameters = match text(inv, "nonlinear_family")? {
                "exponential" | "michaelis_menten" => 2,
                "logistic" | "gompertz" => 3,
                _ => return Err(KernelError::InvalidParameter),
            };
            design_columns = parameters;
        }
        "regression.nonlinear_formula" => {
            parameters = list(inv, "initial_values")?.len();
            design_columns = parameters;
        }
        "regression.deming" => {
            design_columns = 2;
            parameters = 2;
        }
        "regression.pls" => {
            components = integer(inv, "components")?;
            if components == 0 || components > predictors {
                return Err(KernelError::InvalidParameter);
            }
            // PLS centers the predictors and always returns an intercept.
            design_columns = predictors + 1;
            parameters = design_columns;
        }
        "regression.threshold" => {
            design_columns *= 2;
            parameters = design_columns;
        }
        "transform.rcs" => {
            let knots = match text(inv, "knot_mode")? {
                "auto" => integer(inv, "knot_count")?,
                "manual" => list(inv, "knots")?.len(),
                _ => return Err(KernelError::InvalidParameter),
            };
            if !(3..=8).contains(&knots) {
                return Err(KernelError::InvalidParameter);
            }
            basis_columns = knots - 1;
            design_columns = knots;
            parameters = knots;
        }
        "regression.hierarchical" => {
            let blocks = list(inv, "block_sizes")?;
            models = if blocks.is_empty() {
                predictors
            } else {
                blocks.len()
            };
        }
        "workflow.regression.univariate_multivariable" => models = predictors + 1,
        "workflow.regression.grouped" => models = groups,
        _ => {}
    }

    // Typed columns and the numeric/categorical preparation stay alive through fitting and
    // result conversion. Account for their actual capacities and string payloads.
    let mut input_bytes = 0usize;
    let mut response_label_bytes = 0usize;
    for (column_index, column) in data.iter().enumerate() {
        input_bytes = inv.control.check_bytes(
            column
                .values
                .capacity()
                .checked_mul(size_of::<TabularScalar>())
                .and_then(|n| input_bytes.checked_add(n)),
        )?;
        for (i, value) in column.values.iter().enumerate() {
            if i.is_multiple_of(1024) {
                inv.check_control()?;
            }
            if let TabularScalar::String(value) = value {
                input_bytes = inv
                    .control
                    .check_bytes(input_bytes.checked_add(value.len()))?;
                if column_index == 0 {
                    response_label_bytes = response_label_bytes.max(value.len());
                }
            }
        }
    }
    let bytes = (|| {
        let input_bytes = input_bytes.checked_add(
            rows.checked_mul(data.len() + usize::from(categories > 0) + usize::from(groups > 0))?
                .checked_mul(size_of::<f64>())?,
        )?;

        // Numerical workspaces contain f64 values, not RuntimeValue containers. Keep
        // headroom for design copies/decompositions, covariance matrices and IRLS vectors.
        let workspace = rows
            .checked_mul(design_columns)?
            .checked_mul(6)?
            .checked_add(parameters.checked_mul(parameters)?.checked_mul(8)?)?
            .checked_add(rows.checked_mul(12)?)?
            .checked_mul(size_of::<f64>())?;

        // Grouped models partition the rows; hierarchical/univariate models each retain
        // a full fitted/residual pair. Only categorical models return probability rows.
        let model_rows = if method == "workflow.regression.grouped" {
            rows
        } else {
            rows.checked_mul(models)?
        };
        let row_values = model_rows
            .checked_mul(2)?
            .checked_add(if categories > 0 {
                rows.checked_mul(categories + 2)?
            } else {
                0
            })?
            .checked_add(rows.checked_mul(basis_columns)?)?
            .checked_add(if method == "workflow.regression.grouped" {
                rows
            } else {
                0
            })?;
        // Include covariance, coefficient records, workflow metadata and PLS loadings.
        let metadata = parameters
            .checked_mul(parameters)?
            .checked_mul(2)?
            .checked_add(parameters.checked_mul(32)?)?
            .checked_add(256)?
            .checked_mul(models)?
            .checked_add(predictors.checked_mul(components)?.checked_mul(2)?)?;
        let result_values = row_values.checked_add(metadata)?;
        let mut encoded = result_values.checked_mul(STRUCTURED_VALUE_BYTES)?;
        if categories > 0 {
            encoded = encoded.checked_add(rows.checked_mul(response_label_bytes)?)?;
        }
        let encoding = encoded.checked_mul(STRUCTURED_VALUE_COPIES)?;
        let fitting = workspace.checked_add(result_values.checked_mul(size_of::<f64>())?)?;
        // Matrix fitting and JSON/runtime conversion are separate phases, not concurrent
        // allocations. The shared inputs and retained model results are charged in both.
        input_bytes.checked_add(fitting.max(encoding))
    })();
    inv.control.check_bytes(bytes).map(|_| ())
}
