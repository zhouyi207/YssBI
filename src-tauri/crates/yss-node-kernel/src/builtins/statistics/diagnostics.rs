use super::{Input, common::*, install};
mod models;
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};
use yss_sci_contract::diagnostics::residual::ResidualDiagnostic as Test;
use yss_sci_contract::regression::summary::LinearSummaryOptions;

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    models::register(builder);
    for (name, parameters) in [
        ("breusch_pagan", &["rhs", "koenker"][..]),
        ("white", &[][..]),
        ("information_matrix", &[][..]),
        ("reset", &["rhs"][..]),
        ("vif", &[][..]),
        ("leverage", &[][..]),
        ("breusch_godfrey", &["lags", "bg_nomiss0"][..]),
        ("wald", &["hypothesis"][..]),
    ] {
        install(
            builder,
            &format!("yssbi.statistics.diagnostic.{name}"),
            vec![Input::fixed("model")],
            parameters,
            1,
            move |inv| model_test(name, inv),
        );
    }
    for (id, name, parameters) in [
        ("yssbi.statistics.test.normality", "normality", &[][..]),
        (
            "yssbi.statistics.diagnostic.durbin_watson",
            "durbin_watson",
            &[][..],
        ),
        (
            "yssbi.statistics.diagnostic.ljung_box",
            "ljung_box",
            &["lags"][..],
        ),
        ("yssbi.statistics.timeseries.acf", "acf", &["lags"][..]),
        ("yssbi.statistics.timeseries.pacf", "pacf", &["lags"][..]),
    ] {
        install(
            builder,
            id,
            vec![Input::fixed("series")],
            parameters,
            1,
            move |inv| series_test(name, inv),
        );
    }
    for (id, method) in [
        ("yssbi.statistics.timeseries.granger", "granger"),
        ("yssbi.statistics.timeseries.irf", "irf"),
        ("yssbi.statistics.timeseries.fevd", "fevd"),
        ("yssbi.statistics.diagnostic.hausman", "hausman"),
    ] {
        install(
            builder,
            id,
            vec![Input::fixed("model")],
            if matches!(method, "irf" | "fevd") {
                &["steps"]
            } else {
                &[]
            },
            1,
            move |inv| postestimation(method, inv),
        );
    }
}

fn postestimation(
    method: &str,
    inv: &KernelInvocation<'_>,
) -> Result<Vec<RuntimeValue>, KernelError> {
    let report = if method == "hausman" {
        let fit: yss_sci_contract::causal::iv::InstrumentalVariableFit = decode_model(inv)?;
        check_fit_workspace(
            fit.residuals.len(),
            fit.design.exogenous.len() + fit.design.endogenous.len() + fit.design.instruments.len(),
            fit.options.constant,
            "GLS",
            inv,
        )?;
        #[derive(serde::Serialize)]
        struct HausmanReport {
            hausman: yss_sci_contract::causal::iv::HausmanTest,
        }
        value(
            HausmanReport {
                hausman: yss_sci_runtime::causal::iv::hausman(&fit).map_err(sci)?,
            },
            inv,
        )?
    } else {
        let fit: yss_sci_contract::time_series::var::VarFit = decode_model(inv)?;
        let k = fit.var_names.len();
        check_fit_workspace(
            fit.statistics.observations,
            fit.design.first().map_or(0, Vec::len),
            true,
            "OLS",
            inv,
        )?;
        let report = if method == "granger" {
            yss_sci_runtime::time_series::var_granger(&fit).map_err(sci)?
        } else {
            let steps = integer(inv, "steps")?;
            if !(1..=1000).contains(&steps) {
                return Err(KernelError::InvalidParameter);
            }
            inv.control.check_bytes(
                (steps + 1)
                    .checked_mul(k)
                    .and_then(|n| n.checked_mul(k))
                    .and_then(|n| n.checked_mul(1024)),
            )?;
            if method == "irf" {
                yss_sci_runtime::time_series::var_impulse_responses(&fit, steps).map_err(sci)?
            } else {
                yss_sci_runtime::time_series::var_variance_decomposition(&fit, steps)
                    .map_err(sci)?
            }
        };
        value(report, inv)?
    };
    Ok(vec![report])
}

fn model_test(name: &str, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let Some(RuntimeValue::LinearRegression(model)) = inv.inputs.first() else {
        return Err(KernelError::InvalidNumericInput);
    };
    let k = model.design.len();
    // White and IM include cross-products; RESET adds powers through degree four.
    check_fit_workspace(
        model.residuals.len(),
        k.checked_mul(k + 4).ok_or(KernelError::BudgetExceeded)?,
        model.constant,
        "OLS",
        inv,
    )?;
    let result = if matches!(name, "wald" | "breusch_godfrey") {
        let mut options = LinearSummaryOptions::default();
        if name == "wald" {
            options.hypothesis_test = true;
            options.hypothesis = text(inv, "hypothesis")?.into();
        } else {
            options.serial_tests = true;
            options.serial_lags = integer(inv, "lags")?;
            options.bg_nomiss0 = boolean(inv, "bg_nomiss0")?;
        }
        let summary = model.summarize(options, inv.control)?;
        let analysis = summary
            .summary
            .as_ref()
            .ok_or(KernelError::ScientificFailure)?;
        if name == "wald" {
            value(
                analysis
                    .hypothesis
                    .as_deref()
                    .ok_or(KernelError::ScientificFailure)?,
                inv,
            )?
        } else {
            value(
                analysis
                    .serial
                    .as_ref()
                    .and_then(|v| v.bg.as_ref())
                    .ok_or(KernelError::ScientificFailure)?,
                inv,
            )?
        }
    } else {
        let test = match name {
            "breusch_pagan" => Test::BreuschPagan {
                rhs: boolean(inv, "rhs")?,
                koenker: boolean(inv, "koenker")?,
            },
            "white" => Test::White,
            "information_matrix" => Test::InformationMatrix,
            "reset" => Test::Reset {
                rhs: boolean(inv, "rhs")?,
            },
            "vif" => Test::Vif,
            "leverage" => Test::Leverage,
            _ => return Err(KernelError::InvalidParameter),
        };
        value(
            yss_sci_runtime::diagnostics::residual::diagnose(model, test)
                .map_err(|_| KernelError::ScientificFailure)?,
            inv,
        )?
    };
    Ok(vec![result])
}

fn series_test(name: &str, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let data = columns(
        &[inv.inputs.first().ok_or(KernelError::InvalidNumericInput)?],
        inv,
        0,
    )?
    .remove(0);
    let result = match name {
        "normality" => value(
            yss_sci_runtime::diagnostics::residual::normality(&data)
                .map_err(|_| KernelError::ScientificFailure)?,
            inv,
        )?,
        "acf" | "pacf" => {
            let lag = integer(inv, "lags")?;
            if !(1..=40).contains(&lag) {
                return Err(KernelError::InvalidParameter);
            }
            let result = yss_sci_runtime::time_series::acf_pacf(
                yss_sci_contract::time_series::acf_pacf::AcfPacfRequest {
                    values: data,
                    max_lag: lag,
                },
                &yss_sci_contract::execution::ScientificExecutionControl::from_shared(
                    inv.control.cancellation.clone(),
                    inv.control.deadline,
                ),
            )
            .map_err(|e| match e {
                yss_sci_contract::execution::ScientificComputationError::Cancelled => {
                    KernelError::Cancelled
                }
                yss_sci_contract::execution::ScientificComputationError::DeadlineExceeded => {
                    KernelError::DeadlineExceeded
                }
                _ => KernelError::ScientificFailure,
            })?;
            #[derive(serde::Serialize)]
            struct CorrelationReport<'a> {
                function: &'a str,
                observations: usize,
                values: &'a [f64],
            }
            value(
                CorrelationReport {
                    function: name,
                    observations: result.n,
                    values: if name == "acf" {
                        &result.acf
                    } else {
                        &result.pacf
                    },
                },
                inv,
            )?
        }
        "durbin_watson" | "ljung_box" => {
            let lags = if name == "ljung_box" {
                integer(inv, "lags")?
            } else {
                1
            };
            if !(1..=40).contains(&lags) {
                return Err(KernelError::InvalidParameter);
            }
            let result = yss_sci_runtime::diagnostics::serial_correlation::compute_serial_tests(
                yss_sci_contract::diagnostics::serial_correlation::SerialTestsInput {
                    residuals: data,
                    lags,
                    exog: None,
                    bg_nomiss0: true,
                },
            )
            .map_err(sci)?;
            if name == "durbin_watson" {
                value(result.dw, inv)?
            } else {
                value(result.q.ok_or(KernelError::ScientificFailure)?, inv)?
            }
        }
        _ => return Err(KernelError::InvalidParameter),
    };
    Ok(vec![result])
}
