use super::{Input, common::*, install};
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};
mod forecast;

#[derive(Clone, Copy)]
enum Method {
    Adf,
    Var,
    LagOrder,
    Vec,
    Rank,
}
pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    forecast::register(builder);
    use Method::*;
    for (id, method, params, outputs) in [
        ("adf.test", Adf, &["lags", "regression"][..], 1),
        (
            "var.fit",
            Var,
            &["lags", "selected_lags", "constant", "dfk"][..],
            1,
        ),
        ("var.lag_order", LagOrder, &["max_lags"][..], 1),
        ("vec.fit", Vec, &["rank", "lags", "trend"][..], 1),
        ("vec.rank_test", Rank, &["max_lags", "trend"][..], 1),
    ] {
        install(
            builder,
            &format!("yssbi.statistics.{id}"),
            if matches!(method, Adf) {
                vec![Input::repeated("series", 1..=usize::MAX)]
            } else if matches!(method, Var) {
                vec![
                    Input::repeated("variables", 2..=usize::MAX),
                    Input::repeated("exogenous", 0..=usize::MAX),
                ]
            } else {
                vec![Input::repeated("variables", 2..=usize::MAX)]
            },
            params,
            outputs,
            move |inv| run(method, inv),
        );
    }
    install(
        builder,
        "yssbi.statistics.var.summary",
        vec![Input::fixed("model")],
        &[
            "model_summary",
            "coefficient_table",
            "lag_exclusion",
            "serial_tests",
            "stability",
            "serial_lags",
        ],
        1,
        var_summary,
    );
    install(
        builder,
        "yssbi.statistics.vec.summary",
        vec![Input::fixed("model")],
        &[
            "model_summary",
            "coefficient_table",
            "cointegration",
            "serial_tests",
            "stability",
            "serial_lags",
        ],
        1,
        vec_summary,
    );
}
fn run(method: Method, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let mut data = columns(&inv.inputs.iter().collect::<Vec<_>>(), inv, 0)?;
    let lags = integer(
        inv,
        if matches!(method, Method::LagOrder | Method::Rank) {
            "max_lags"
        } else {
            "lags"
        },
    )?;
    let variable_count = if matches!(method, Method::Var) {
        group(inv, "variables").len()
    } else {
        data.len()
    };
    let variable_names = inv
        .inputs
        .iter()
        .take(variable_count)
        .enumerate()
        .map(|(i, v)| input_label(v, format!("y{i}")))
        .collect::<Vec<_>>();
    let exogenous_names = inv
        .inputs
        .iter()
        .skip(variable_count)
        .enumerate()
        .map(|(i, v)| input_label(v, format!("exog{i}")))
        .collect::<Vec<_>>();
    let selected_lags = if matches!(method, Method::Var) {
        let selected = text(inv, "selected_lags")?;
        if selected.trim().is_empty() {
            (1..=lags).collect::<Vec<_>>()
        } else {
            selected
                .split(',')
                .map(|s| {
                    s.trim()
                        .parse::<usize>()
                        .map_err(|_| KernelError::InvalidParameter)
                })
                .collect::<Result<Vec<_>, _>>()?
        }
    } else {
        vec![]
    };
    let n = data[0].len();
    if lags >= n || lags > 1000 {
        return Err(KernelError::InvalidParameter);
    }
    check_fit_workspace(
        n,
        data.len()
            .checked_mul(if matches!(method, Method::Var) {
                selected_lags.len() + 2
            } else {
                lags + 2
            })
            .ok_or(KernelError::BudgetExceeded)?,
        true,
        "GLS",
        inv,
    )?;
    use yss_sci_runtime::time_series as ts;
    let result = match method {
        Method::Adf => {
            let regression = text(inv, "regression")?;
            if data.len() == 1 {
                ts::augmented_dickey_fuller(&data.remove(0), lags, regression).map(|mut result| {
                    result["series"] = serde_json::json!(input_label(&inv.inputs[0], "series1".into()));
                    result["seriesIndex"] = serde_json::json!(0);
                    result
                })
            } else {
                let mut tests = Vec::new();
                let mut rows = Vec::new();
                let mut auxiliary = Vec::new();
                for (i, series) in data.iter().enumerate() {
                    inv.check_control()?;
                    let name = input_label(&inv.inputs[i], format!("series{}", i + 1));
                    match ts::augmented_dickey_fuller(series, lags, regression) {
                        Ok(mut result) => {
                            result["series"] = serde_json::json!(name);
                            result["seriesIndex"] = serde_json::json!(i);
                            rows.push(serde_json::json!({
                                "series": name, "series_index": i,
                                "statistic": result["statistic"], "p_value": result["pValue"],
                                "observations": result["observations"], "lags": lags,
                                "regression": regression, "use_t_distribution": result["useTDistribution"],
                                "status": "success", "failure": null
                            }));
                            if let Some(table) = result["regressionTable"].as_array() {
                                for row in table {
                                    let mut row = row.clone();
                                    row["series"] = serde_json::json!(name);
                                    row["series_index"] = serde_json::json!(i);
                                    auxiliary.push(row);
                                }
                            }
                            tests.push(result);
                        }
                        Err(error) => rows.push(serde_json::json!({
                            "series": name, "series_index": i, "status": "failed",
                            "failure": format!("{error:?}"), "statistic": null,
                            "p_value": null, "observations": null, "lags": lags,
                            "regression": regression, "use_t_distribution": null
                        })),
                    }
                }
                let mut report = serde_json::json!({
                    "tests": tests, "test_rows": rows, "regression_rows": auxiliary
                });
                yss_sci_runtime::report_display::section(
                    &mut report, "tests", "DF / ADF results", "table", "/test_rows",
                    &[("series", "Series"), ("statistic", "Statistic"), ("p_value", "p-value"),
                      ("observations", "Observations"), ("lags", "Lags"), ("regression", "Deterministic terms"),
                      ("use_t_distribution", "Student t reference"), ("status", "Status"), ("failure", "Failure reason")],
                );
                yss_sci_runtime::report_display::section(
                    &mut report, "auxiliary", "ADF auxiliary regressions", "table", "/regression_rows",
                    &[("series", "Series"), ("variable", "Variable"), ("coefficient", "Coefficient"),
                      ("standardError", "Std. error"), ("t", "t"), ("pValue", "Auxiliary p-value")],
                );
                Ok(report)
            }
        }
        Method::Var => {
            if selected_lags.iter().any(|l| *l == 0 || *l >= n || *l > 1000) {
                return Err(KernelError::InvalidParameter);
            }
            let exogenous = data.split_off(variable_count);
            ts::var_fit_configured(data, exogenous, yss_sci_contract::time_series::var::VarOptions {
                lags: selected_lags, constant: boolean(inv, "constant")?,
                dfk: boolean(inv, "dfk")?, variable_names, exogenous_names,
            })
        }
        Method::LagOrder => ts::var_lag_order(data, lags),
        Method::Vec => ts::vec_fit_named(data, integer(inv, "rank")?, lags, text(inv, "trend")?, variable_names),
        Method::Rank => ts::vec_rank_test(data, lags, text(inv, "trend")?),
    }
    .map_err(sci)?;
    let result = value(result, inv)?;
    Ok(vec![result])
}

fn var_summary(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let fit: yss_sci_contract::time_series::var::VarFit = decode_model(inv)?;
    let options = yss_sci_contract::time_series::var::VarSummaryOptions {
        model_summary: boolean(inv, "model_summary")?,
        coefficient_table: boolean(inv, "coefficient_table")?,
        lag_exclusion: boolean(inv, "lag_exclusion")?,
        serial_tests: boolean(inv, "serial_tests")?,
        stability: boolean(inv, "stability")?,
        serial_lags: serial_lags(inv)?,
    };
    if options.stability {
        let dim = fit
            .var_names
            .len()
            .checked_mul(fit.lags.iter().copied().max().unwrap_or(0))
            .ok_or(KernelError::BudgetExceeded)?;
        inv.control
            .check_bytes(dim.checked_mul(dim).and_then(|v| v.checked_mul(8 * 6)))?;
        if dim
            .checked_mul(dim)
            .and_then(|v| v.checked_mul(dim))
            .is_none_or(|v| v > 100_000_000)
        {
            return Err(KernelError::BudgetExceeded);
        }
    }
    if options.lag_exclusion || options.serial_tests || options.stability {
        check_fit_workspace(
            fit.statistics.observations,
            fit.design.first().map_or(0, Vec::len) + fit.var_names.len(),
            true,
            "GLS",
            inv,
        )?;
    }
    let report = value(
        yss_sci_runtime::time_series::var_summary(&fit, options).map_err(sci)?,
        inv,
    )?;
    Ok(vec![report])
}
fn vec_summary(inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let fit: yss_sci_contract::time_series::vec::VecFit = decode_model(inv)?;
    let options = yss_sci_contract::time_series::vec::VecSummaryOptions {
        model_summary: boolean(inv, "model_summary")?,
        coefficient_table: boolean(inv, "coefficient_table")?,
        cointegration: boolean(inv, "cointegration")?,
        serial_tests: boolean(inv, "serial_tests")?,
        stability: boolean(inv, "stability")?,
        serial_lags: serial_lags(inv)?,
    };
    if options.serial_tests || options.stability {
        check_fit_workspace(
            fit.statistics.observations,
            fit.design.first().map_or(0, Vec::len) + fit.var_names.len(),
            true,
            "GLS",
            inv,
        )?;
    }
    let report = value(
        yss_sci_runtime::time_series::vec_summary(&fit, options).map_err(sci)?,
        inv,
    )?;
    Ok(vec![report])
}
fn serial_lags(inv: &KernelInvocation<'_>) -> Result<usize, KernelError> {
    let lags = integer(inv, "serial_lags")?;
    if !(1..=40).contains(&lags) {
        return Err(KernelError::InvalidParameter);
    }
    Ok(lags)
}
