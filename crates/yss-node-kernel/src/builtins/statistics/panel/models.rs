use super::*;
use yss_sci_contract::{execution::*, panel::*};

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    for method in [
        "fe",
        "re",
        "fd",
        "between",
        "dynamic",
        "unit_root",
        "cointegration",
    ] {
        let mut inputs = vec![Input::fixed(if method == "unit_root" {
            "series"
        } else {
            "y"
        })];
        if method != "unit_root" {
            inputs.push(Input::repeated(
                "x",
                (if method == "dynamic" { 0 } else { 1 })..=if method == "cointegration" {
                    MAX_COINTEGRATION_PREDICTORS
                } else {
                    usize::MAX
                },
            ));
        }
        inputs.extend([Input::fixed("entity"), Input::fixed("time")]);
        let parameters: &[&str] = match method {
            "fe" | "re" => &["constant", "effects", "covariance"],
            "fd" => &["covariance"],
            "between" => &["constant", "effects"],
            "dynamic" => &["max_instrument_lag", "covariance"],
            _ => &["lags", "regression"],
        };
        install(
            builder,
            &format!("yssbi.statistics.econometrics.panel.{method}"),
            inputs,
            parameters,
            1,
            move |inv| execute(method, inv),
        );
    }
}

fn execute(method: &str, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let mut columns = columns(&inv.inputs.iter().collect::<Vec<_>>(), inv, 0)?;
    let time = columns.pop().ok_or(KernelError::ShapeMismatch)?;
    let entity = columns.pop().ok_or(KernelError::ShapeMismatch)?;
    let response = columns.remove(0);
    let control = ScientificExecutionControl::from_shared(
        inv.control.cancellation.clone(),
        inv.control.deadline,
    );
    let data = PanelData {
        response: &response,
        predictors: &columns,
        entity: &entity,
        time: &time,
    };
    let width = match method {
        "unit_root" | "cointegration" => {
            let lags = integer(inv, "lags")?;
            if lags >= response.len() {
                return Err(KernelError::InvalidParameter);
            }
            lags + columns.len() + 3
        }
        "dynamic" => {
            let lag = integer(inv, "max_instrument_lag")?;
            if lag < 2 || lag >= response.len() {
                return Err(KernelError::InvalidParameter);
            }
            lag + columns.len()
        }
        _ => columns.len() + 1,
    };
    // Admit aligned inputs, live designs/moment matrices and structured output together.
    let n = response.len();
    inv.control.check_bytes((|| {
        let input = n
            .checked_mul(columns.len() + 3)?
            .checked_mul(size_of::<RuntimeValue>() * 4)?;
        let workspace = n
            .checked_mul(width + 12)?
            .checked_mul(96)?
            .checked_add(width.checked_mul(width)?.checked_mul(128)?)?;
        let output = n
            .checked_mul(columns.len() + 16)?
            .checked_mul(STRUCTURED_VALUE_BYTES * STRUCTURED_VALUE_COPIES)?;
        input.checked_add(workspace)?.checked_add(output)
    })())?;
    let output = match method {
        "unit_root" | "cointegration" => {
            let options = PanelTestOptions {
                lags: integer(inv, "lags")?,
                deterministic: match text(inv, "regression")? {
                    "none" if method == "unit_root" => PanelDeterministic::None,
                    "constant" => PanelDeterministic::Constant,
                    "trend" => PanelDeterministic::Trend,
                    _ => return Err(KernelError::InvalidParameter),
                },
            };
            let result = if method == "unit_root" {
                yss_sci_runtime::panel::fisher_unit_root(data, options, &control)
            } else {
                yss_sci_runtime::panel::fisher_cointegration(data, options, &control)
            };
            value(result.map_err(computation_error)?, inv)?
        }
        "dynamic" => {
            let options = DynamicPanelOptions {
                max_instrument_lag: integer(inv, "max_instrument_lag")?,
                robust: match text(inv, "covariance")? {
                    "robust" => true,
                    "nonrobust" => false,
                    _ => return Err(KernelError::InvalidParameter),
                },
            };
            let mut result = yss_sci_runtime::panel::difference_gmm(data, options, &control)
                .map_err(computation_error)?;
            result.parameter_names[0] =
                format!("lag({},1)", input_label(&inv.inputs[0], "response".into()));
            for (j, input) in group(inv, "x").iter().enumerate() {
                result.parameter_names[j + 1] = input_label(input, format!("x{}", j + 1));
            }
            value(result, inv)?
        }
        _ => {
            let options = PanelOptions {
                estimator: match method {
                    "fe" => Estimator::FixedEffects,
                    "re" => Estimator::RandomEffects,
                    "fd" => Estimator::FirstDifference,
                    "between" => Estimator::Between,
                    _ => unreachable!(),
                },
                effects: if method == "fd" {
                    Effects::Entity
                } else {
                    match text(inv, "effects")? {
                        "entity" => Effects::Entity,
                        "time" => Effects::Time,
                        "two_way" if method != "between" => Effects::TwoWay,
                        _ => return Err(KernelError::InvalidParameter),
                    }
                },
                constant: method == "fd" || boolean(inv, "constant")?,
                covariance: if method == "between" {
                    "nonrobust"
                } else {
                    text(inv, "covariance")?
                }
                .into(),
            };
            inv.check_control()?;
            let mut fit =
                yss_sci_runtime::panel::fit_model(response, columns, entity, time, options)
                    .map_err(sci)?;
            inv.check_control()?;
            name_fit(&mut fit, inv);
            value(fit, inv)?
        }
    };
    Ok(vec![output])
}
