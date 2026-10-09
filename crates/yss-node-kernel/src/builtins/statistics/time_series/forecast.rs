use super::super::{Input, common::*, install};
use crate::{KernelError, KernelInvocation, KernelRegistryBuilder, RuntimeValue};
use yss_sci_contract::{
    execution::*, regression::models::IterationOptions, time_series::forecast::*,
};
use yss_sci_runtime::time_series as sci;

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    for method in [
        "arima",
        "sarima",
        "ecm",
        "arch",
        "garch",
        "egarch",
        "gjr_garch",
        "grey_prediction",
        "exponential_smoothing",
        "ets",
        "holt_winters",
        "markov_prediction",
        "phillips_perron",
        "kpss",
        "correlogram",
        "time_series",
    ] {
        let plot = matches!(method, "correlogram" | "time_series");
        let mut inputs = vec![Input::fixed("series")];
        if method == "ecm" {
            inputs.push(Input::repeated("x", 1..=usize::MAX));
        }
        if method == "time_series" {
            inputs.push(Input::repeated("time", 0..=1));
        }
        let mut params = match method {
            "arima" => vec!["ts_p", "ts_d", "ts_q", "constant", "ts_confidence"],
            "sarima" => vec![
                "ts_p",
                "ts_d",
                "ts_q",
                "ts_seasonal_p",
                "ts_seasonal_d",
                "ts_seasonal_q",
                "ts_period",
                "constant",
                "ts_confidence",
            ],
            "ecm" => vec!["ts_lags", "constant"],
            "arch" => vec!["ts_p", "constant"],
            "garch" | "gjr_garch" => vec!["ts_p", "ts_q", "constant"],
            "egarch" => vec!["ts_p", "ts_q", "constant", "ts_simulations", "ts_seed"],
            "exponential_smoothing" => vec!["ts_alpha", "ts_optimize"],
            "ets" | "holt_winters" => vec![
                "ts_alpha",
                "ts_beta",
                "ts_gamma",
                "ts_phi",
                "ts_optimize",
                "ts_trend",
                "ts_damped",
                "ts_seasonality",
                "ts_period",
            ],
            "markov_prediction" => vec!["ts_pseudocount"],
            "phillips_perron" | "kpss" => vec!["ts_bandwidth", "ts_deterministic"],
            "correlogram" => vec!["ts_maximum_lag"],
            _ => vec![],
        };
        if !matches!(
            method,
            "ecm" | "phillips_perron" | "kpss" | "correlogram" | "time_series"
        ) {
            params.push("ts_horizon");
        }
        if matches!(
            method,
            "arima"
                | "sarima"
                | "arch"
                | "garch"
                | "egarch"
                | "gjr_garch"
                | "exponential_smoothing"
                | "ets"
                | "holt_winters"
        ) {
            params.extend(["max_iterations", "tolerance"]);
        }
        install(
            builder,
            &format!(
                "yssbi.statistics.{}.{method}",
                if plot { "plot" } else { "timeseries" }
            ),
            inputs,
            &params,
            1,
            move |inv| execute(method, inv),
        );
    }
}
fn execute(method: &str, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let (data, retained) = materialize(inv)?;
    let first = data.first().ok_or(KernelError::InvalidNumericInput)?;
    let n = first.values.len();
    let control = ScientificExecutionControl::from_shared(
        inv.control.cancellation.clone(),
        inv.control.deadline,
    );
    let optional = |key| {
        if inv.parameter(key).is_some() {
            integer(inv, key)
        } else {
            Ok(0)
        }
    };
    let h = optional("ts_horizon")?;
    let p = optional("ts_p")?;
    let q = optional("ts_q")?;
    let sp = optional("ts_seasonal_p")?;
    let sq = optional("ts_seasonal_q")?;
    let period = optional("ts_period")?;
    let d = optional("ts_d")?;
    let sd = optional("ts_seasonal_d")?;
    let lags = optional("ts_lags")?;
    let width = (|| {
        p.checked_mul(2)?
            .checked_add(q)?
            .checked_add(sp)?
            .checked_add(sq)?
            .checked_add(8)?
            .checked_add(lags.checked_add(1)?.checked_mul(data.len())?)
    })()
    .ok_or(KernelError::BudgetExceeded)?;
    let history = (|| {
        sp.max(sq)
            .checked_add(sd)?
            .checked_mul(period)?
            .checked_add(p.max(q))?
            .checked_add(d)
    })();
    inv.control.check_bytes((|| {
        let observations = n.checked_add(h)?.checked_add(history?)?;
        let workspace = n
            .checked_mul(width)?
            .checked_add(width.checked_mul(width)?.checked_mul(12)?)?
            .checked_mul(16)?;
        let report = observations
            .checked_mul(8)?
            .checked_add(width.checked_mul(width)?)?
            .checked_add(512)?
            .checked_mul(STRUCTURED_VALUE_BYTES * STRUCTURED_VALUE_COPIES)?;
        retained.checked_add(workspace)?.checked_add(report)
    })())?;
    if method == "markov_prediction" {
        let (states, labels) = categories(first, false, inv)?;
        let k = labels.len();
        let max_label_bytes = labels
            .iter()
            .map(|v| match v {
                yss_data_contract::TabularScalar::String(s) => s.len(),
                _ => 0,
            })
            .max()
            .unwrap_or(0);
        inv.control.check_bytes((|| {
            retained
                .checked_add(h.checked_add(k)?.checked_mul(max_label_bytes)?)?
                .checked_add(
                    k.checked_mul(k)?
                        .checked_mul(2)?
                        .checked_add(h.checked_mul(k)?)?
                        .checked_add(n)?
                        .checked_add(256)?
                        .checked_mul(STRUCTURED_VALUE_BYTES * STRUCTURED_VALUE_COPIES)?,
                )
        })())?;
        let result =
            sci::markov_prediction(&states, k, h, number(inv, "ts_pseudocount")?, &control)
                .map_err(computation_error)?;
        let mut record = match value(&result, inv)? {
            RuntimeValue::Record(v) => v,
            _ => unreachable!(),
        };
        let out = std::sync::Arc::make_mut(&mut record);
        out.insert(
            "state_labels".into(),
            RuntimeValue::List(labels.iter().cloned().map(RuntimeValue::Scalar).collect()),
        );
        out.insert(
            "forecast_labels".into(),
            RuntimeValue::List(
                result
                    .forecast_states
                    .iter()
                    .map(|&j| RuntimeValue::Scalar(labels[j].clone()))
                    .collect(),
            ),
        );
        return Ok(vec![RuntimeValue::Record(record)]);
    }
    let y = numeric(first, false, inv)?;
    let iteration = if inv.parameter("max_iterations").is_some() {
        IterationOptions {
            max_iterations: integer(inv, "max_iterations")?,
            tolerance: number(inv, "tolerance")?,
        }
    } else {
        IterationOptions::default()
    };
    let output = match method {
        "arima" | "sarima" => value(
            sci::arima(
                &y,
                ArimaOptions {
                    p,
                    d,
                    q,
                    seasonal_p: sp,
                    seasonal_d: sd,
                    seasonal_q: sq,
                    period: if method == "arima" { 1 } else { period },
                    constant: boolean(inv, "constant")?,
                    horizon: h,
                    confidence: number(inv, "ts_confidence")?,
                    iteration,
                },
                &control,
            )
            .map_err(computation_error)?,
            inv,
        )?,
        "grey_prediction" => value(
            sci::grey_prediction(&y, h, &control).map_err(computation_error)?,
            inv,
        )?,
        "ecm" => {
            let x = data[1..]
                .iter()
                .map(|c| numeric(c, false, inv))
                .collect::<Result<Vec<_>, _>>()?;
            let result = sci::ecm(
                &y,
                &x,
                EcmOptions {
                    lags,
                    constant: boolean(inv, "constant")?,
                },
                &control,
            )
            .map_err(computation_error)?;
            value(result, inv)?
        }
        "phillips_perron" | "kpss" => {
            let deterministic = match text(inv, "ts_deterministic")? {
                "none" => Deterministic::None,
                "constant" => Deterministic::Constant,
                "trend" => Deterministic::Trend,
                _ => return Err(KernelError::InvalidParameter),
            };
            let bandwidth = integer(inv, "ts_bandwidth")?;
            value(
                if method == "kpss" {
                    sci::kpss(&y, bandwidth, deterministic, &control)
                } else {
                    sci::phillips_perron(&y, bandwidth, deterministic, &control)
                }
                .map_err(computation_error)?,
                inv,
            )?
        }
        "exponential_smoothing" | "ets" | "holt_winters" => {
            let simple = method == "exponential_smoothing";
            let seasonality = if simple {
                Seasonality::None
            } else {
                match text(inv, "ts_seasonality")? {
                    "none" if method == "ets" => Seasonality::None,
                    "additive" => Seasonality::Additive,
                    "multiplicative" if method == "holt_winters" => Seasonality::Multiplicative,
                    _ => return Err(KernelError::InvalidParameter),
                }
            };
            let trend = !simple && boolean(inv, "ts_trend")?;
            let o = SmoothingOptions {
                trend,
                damped: !simple && boolean(inv, "ts_damped")?,
                seasonality,
                period,
                alpha: number(inv, "ts_alpha")?,
                beta: if simple { 0.1 } else { number(inv, "ts_beta")? },
                gamma: if simple {
                    0.1
                } else {
                    number(inv, "ts_gamma")?
                },
                phi: if simple { 0.98 } else { number(inv, "ts_phi")? },
                optimize: boolean(inv, "ts_optimize")?,
                horizon: h,
                iteration,
            };
            value(
                sci::exponential_smoothing(&y, o, &control).map_err(computation_error)?,
                inv,
            )?
        }
        "arch" | "garch" | "egarch" | "gjr_garch" => {
            let model = match method {
                "arch" => VolatilityMethod::Arch,
                "garch" => VolatilityMethod::Garch,
                "egarch" => VolatilityMethod::Egarch,
                _ => VolatilityMethod::GjrGarch,
            };
            value(
                sci::volatility(
                    &y,
                    VolatilityOptions {
                        method: model,
                        p,
                        q,
                        constant: boolean(inv, "constant")?,
                        horizon: h,
                        simulations: optional("ts_simulations")?,
                        seed: optional("ts_seed")? as u64,
                        iteration,
                    },
                    &control,
                )
                .map_err(computation_error)?,
                inv,
            )?
        }
        "correlogram" => value(
            yss_sci_runtime::visualization::correlogram(
                &y,
                integer(inv, "ts_maximum_lag")?,
                &control,
            )
            .map_err(computation_error)?,
            inv,
        )?,
        "time_series" => {
            let time = if let Some(col) = data.get(1) {
                numeric(col, false, inv)?
            } else {
                (1..=n).map(|i| i as f64).collect()
            };
            if time.windows(2).any(|w| w[0] >= w[1]) {
                return Err(KernelError::InvalidParameter);
            }
            let mut plot = yss_sci_runtime::visualization::xy(&time, &y, true, &control)
                .map_err(computation_error)?;
            plot.x_label = if data.len() > 1 {
                input_label(&inv.inputs[1], "Time".into())
            } else {
                "Observation".into()
            };
            plot.y_label = input_label(&inv.inputs[0], "Series".into());
            value(plot, inv)?
        }
        _ => return Err(KernelError::InvalidParameter),
    };
    inv.check_control()?;
    Ok(vec![output])
}
