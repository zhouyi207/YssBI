//! VAR 向量自回归节点
//!
//! 实现与 Stata varbasic 一致：VAR(p) 估计、正交化 IRF、FEVD。

use super::var_types::{
    VARCoefDisplay, VAREquationDisplay, VARGrangerDisplay, VARLmarDisplay, VARStableDisplay,
    VARSummaryResult, VARWleDisplay,
};
use crate::execution::{ExecutionEffect, ReportKind};
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::register::catalog::docs;
use crate::graph::value::DataValue;
use ndarray::Array2;
use polars::prelude::DataType;
use std::sync::Arc;
use yss_sci::api::time_series::{
    VAR, VARConfig, VARSocResult, var_regression_times_stata, var_varsoc,
};

// ======================== 辅助 ========================

/// 是否为可参与数值估计的列类型：所有整数 / 浮点宽度 + Decimal（统一 cast 到 Float64）。
fn is_var_numeric(dtype: &DataType) -> bool {
    dtype.is_primitive_numeric() || dtype.is_decimal()
}

#[inline]
fn finite_f64(opt: Option<f64>) -> bool {
    matches!(opt, Some(x) if x.is_finite())
}

/// 全样本外生矩阵：与 Y 同行数，null 或非有限值为 NaN（Stata 式：仅当期 exog[t] 参与该行是否可估）
fn exog_dataframe_to_array2_full_nan(
    df: &polars::prelude::DataFrame,
    expected_rows: usize,
) -> Result<(Array2<f64>, Vec<String>), String> {
    if df.height() != expected_rows {
        return Err(format!(
            "VAR: exog DataFrame has {} rows, expected {}",
            df.height(),
            expected_rows
        ));
    }
    let mut columns: Vec<Vec<f64>> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let nrows = expected_rows;
    for col in df.columns() {
        if !is_var_numeric(col.dtype()) {
            continue;
        }
        let s = col
            .cast(&polars::prelude::DataType::Float64)
            .map_err(|e| format!("VAR: exog column '{}' cast failed: {}", col.name(), e))?;
        let f64_ca = s.f64().map_err(|e| format!("VAR: exog: {}", e))?;
        let vec: Vec<f64> = (0..nrows)
            .map(|i| f64_ca.get(i).filter(|v| v.is_finite()).unwrap_or(f64::NAN))
            .collect();
        columns.push(vec);
        names.push(col.name().to_string());
    }
    if columns.is_empty() {
        return Err("VAR: exog DataFrame has no numeric columns".to_string());
    }
    let ncols = columns.len();
    let mut data = Vec::with_capacity(nrows * ncols);
    for i in 0..nrows {
        for j in 0..ncols {
            data.push(columns[j][i]);
        }
    }
    let arr = Array2::from_shape_vec((nrows, ncols), data)
        .map_err(|e| format!("VAR: failed to build exog matrix: {}", e))?;
    Ok((arr, names))
}

// ======================== 注册 ========================

fn var_input_slots() -> Vec<PinSlot> {
    vec![
        PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
        PinSlot::repeatable(
            PinDefinition::data_input(
                "",
                DataRole::Inputs(0),
                PinDataTypeDefinition::concrete(crate::graph::value::DataType::DataSeries(
                    Box::new(crate::graph::value::DataType::Float64),
                )),
            ),
            "Variables",
            1,
            None,
        ),
        PinSlot::fixed(
            PinDefinition::data_input(
                "Exog DataFrame",
                DataRole::Custom("exog_df".to_string()),
                PinDataTypeDefinition::concrete(crate::graph::value::DataType::DataFrame),
            )
            .with_optional(true),
        ),
        PinSlot::fixed(
            PinDefinition::data_input(
                "Lags",
                DataRole::Custom("lags".to_string()),
                PinDataTypeDefinition::concrete(crate::graph::value::DataType::Int64),
            )
            .with_optional(true)
            .with_default_value(DataValue::Int64(2)),
        ),
    ]
}

use crate::execution::context::NodeExecutionContextTrait;

fn run_var(ctx: &mut dyn NodeExecutionContextTrait) -> Result<VARSummaryResult, String> {
    let lags_val = ctx
        .get_input_by_role(&PinRole::Data(DataRole::Custom("lags".to_string())))
        .ok()
        .and_then(|v| v.as_i64())
        .unwrap_or(2);
    let lags_p = lags_val as usize;
    if lags_p < 1 {
        return Err("VAR: lags must be >= 1".to_string());
    }
    let lags: Vec<usize> = (1..=lags_p).collect();

    let var_inputs = ctx.get_inputs_by_family(&PinRole::Data(DataRole::Inputs(0)))?;
    if var_inputs.is_empty() {
        return Err("VAR: at least 1 variable required".to_string());
    }

    let mut y_cols: Vec<Vec<Option<f64>>> = Vec::new();
    let mut var_names = Vec::new();
    let mut n: Option<usize> = None;

    for v in &var_inputs {
        let dsv = match v {
            crate::graph::value::DataValue::DataSeries(s) => s.clone(),
            _ => return Err("VAR: each variable must be a DataSeries".to_string()),
        };
        let s = ctx.get_data_series(&dsv.id)?;
        let name = s.name().to_string();
        if name.is_empty() {
            var_names.push(format!("y{}", y_cols.len()));
        } else {
            var_names.push(name);
        }
        let s_f64 = s
            .cast(&polars::prelude::DataType::Float64)
            .map_err(|e| format!("VAR: cannot cast to Float64: {}", e))?;
        let ca = s_f64.f64().map_err(|e| format!("VAR: {}", e))?;
        let len = ca.len();
        match n {
            None => n = Some(len),
            Some(nn) if nn != len => {
                return Err(format!(
                    "VAR: variable '{}' has {} rows, expected {}",
                    var_names.last().cloned().unwrap_or_default(),
                    len,
                    nn
                ));
            }
            _ => {}
        }
        let col: Vec<Option<f64>> = (0..len).map(|i| ca.get(i)).collect();
        y_cols.push(col);
    }

    let n = n.ok_or_else(|| "VAR: no variables".to_string())?;
    let k = y_cols.len();

    let exog_df_id: Option<String> =
        match ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("exog_df".to_string()))) {
            Ok(DataValue::DataFrame(id)) => Some(id.clone()),
            Ok(DataValue::Null) | Err(_) => None,
            _ => None,
        };

    let (y, exog, exog_names, regression_times, complete_sample_rows) = if let Some(ref id) =
        exog_df_id
    {
        let df = ctx.get_dataframe(id)?;
        let df = df.as_ref();
        if df.height() != n {
            return Err(format!(
                "VAR: exog DataFrame has {} rows, expected {} (must align with Variables length)",
                df.height(),
                n
            ));
        }
        let mut y_data = Vec::with_capacity(n * k);
        for i in 0..n {
            for col in &y_cols {
                y_data.push(col[i].filter(|x| x.is_finite()).unwrap_or(f64::NAN));
            }
        }
        let y = Array2::from_shape_vec((n, k), y_data)
            .map_err(|e| format!("VAR: failed to build Y matrix: {}", e))?;
        let (exog_mat, exog_names) = exog_dataframe_to_array2_full_nan(df, n)?;
        let times = var_regression_times_stata(&y, &lags, Some(&exog_mat))?;
        let n_reg = times.len();
        if n_reg <= lags_p {
            return Err(format!(
                "VAR: Stata-style sample has {} regression period(s); need more than {} (max lag)",
                n_reg, lags_p
            ));
        }
        ctx.log(format!(
                "VAR: aligned timeline T={} rows (exog); VAR({}) effective regression n={} (t≥{} with finite y, lags, exog[t])",
                n, lags_p, n_reg, lags_p
            ));
        (y, Some(exog_mat), Some(exog_names), Some(times), n)
    } else {
        let mut keep = vec![true; n];
        for col in &y_cols {
            for i in 0..n {
                if !finite_f64(col[i]) {
                    keep[i] = false;
                }
            }
        }
        let row_pick: Vec<usize> = (0..n).filter(|&i| keep[i]).collect();
        let n_keep = row_pick.len();
        if n_keep == 0 {
            return Err(
                "VAR: no complete observations (missing or non-finite endogenous values)"
                    .to_string(),
            );
        }
        if n_keep <= lags_p {
            return Err(format!(
                "VAR: after listwise deletion, {} observations remain; need more than {} (lags)",
                n_keep, lags_p
            ));
        }
        let dropped = n - n_keep;
        if dropped > 0 {
            ctx.log(format!(
                    "VAR: listwise deletion removed {} observation(s) with missing/non-finite endogenous (no exog)",
                    dropped
                ));
        }
        ctx.log(format!(
            "VAR: complete sample T={} rows; VAR({}) estimation n={} (= T − {})",
            n_keep,
            lags_p,
            n_keep.saturating_sub(lags_p),
            lags_p
        ));
        let mut y_data = Vec::with_capacity(n_keep * k);
        for &i in &row_pick {
            for col in &y_cols {
                let v = col[i].ok_or_else(|| {
                    "VAR: internal error: complete-case row has null endogenous".to_string()
                })?;
                y_data.push(v);
            }
        }
        let y = Array2::from_shape_vec((n_keep, k), y_data)
            .map_err(|e| format!("VAR: failed to build Y matrix: {}", e))?;
        (y, None, None, None, n_keep)
    };

    let var_config = VARConfig {
        constant: true,
        lags,
        step: 8,
        dfk: false,
        mlag: 2,
        sample_start_offset: None,
        skip_extras: false,
    };

    let var = VAR {
        y,
        exog,
        config: var_config,
        var_names: Some(var_names.clone()),
        exog_names,
        regression_times,
    };
    let result = var.fit()?;

    let mut coefficients = Vec::new();
    for eq in 0..result.var_names.len() {
        for (j, label) in result
            .coef_labels
            .get(eq)
            .unwrap_or(&vec![])
            .iter()
            .enumerate()
        {
            if j < result.coefficients[eq].len() {
                coefficients.push(VARCoefDisplay {
                    eq_name: result
                        .equations
                        .get(eq)
                        .map(|e| e.eq_name.clone())
                        .unwrap_or_else(|| format!("eq{}", eq)),
                    variable: label.clone(),
                    coef: result.coefficients[eq][j],
                    std_err: result
                        .std_errs
                        .get(eq)
                        .and_then(|se| se.get(j))
                        .copied()
                        .unwrap_or(0.0),
                    z_value: result
                        .z_values
                        .get(eq)
                        .and_then(|zv| zv.get(j))
                        .copied()
                        .unwrap_or(0.0),
                    p_value: result
                        .p_values
                        .get(eq)
                        .and_then(|pv| pv.get(j))
                        .copied()
                        .unwrap_or(1.0),
                    ci_lower: result
                        .ci_lower
                        .get(eq)
                        .and_then(|c| c.get(j))
                        .copied()
                        .unwrap_or(0.0),
                    ci_upper: result
                        .ci_upper
                        .get(eq)
                        .and_then(|c| c.get(j))
                        .copied()
                        .unwrap_or(0.0),
                });
            }
        }
    }

    let equations = result
        .equations
        .iter()
        .enumerate()
        .map(|(i, e)| VAREquationDisplay {
            eq_name: var_names
                .get(i)
                .cloned()
                .unwrap_or_else(|| e.eq_name.clone()),
            parms: e.parms,
            rmse: e.rmse,
            r_sq: e.r_sq,
            chi2: e.chi2,
            p_chi2: e.p_chi2,
        })
        .collect();

    let varwle = result
        .varwle
        .iter()
        .map(|r| VARWleDisplay {
            eq_name: r.eq_name.clone(),
            lag: r.lag,
            chi2: r.chi2,
            df: r.df,
            p_value: r.p_value,
        })
        .collect();

    let varlmar = result
        .varlmar
        .iter()
        .map(|r| VARLmarDisplay {
            lag: r.lag,
            chi2: r.chi2,
            df: r.df,
            p_value: r.p_value,
        })
        .collect();

    let varstable = result
        .varstable
        .iter()
        .map(|r| VARStableDisplay {
            re: r.re,
            im: r.im,
            modulus: r.modulus,
        })
        .collect();

    let vargranger = result
        .vargranger
        .iter()
        .map(|r| VARGrangerDisplay {
            eq_name: r.eq_name.clone(),
            excluded: r.excluded.clone(),
            chi2: r.chi2,
            df: r.df,
            p_value: r.p_value,
        })
        .collect();

    Ok(VARSummaryResult {
        title: "Vector Autoregression".to_string(),
        var_names: var_names.clone(),
        complete_sample_rows,
        var_max_lag: lags_p,
        num_observation: result.num_observation,
        log_likelihood: result.log_likelihood,
        aic: result.aic,
        fpe: result.fpe,
        hqic: result.hqic,
        sbic: result.sbic,
        det_sigma_ml: result.det_sigma_ml,
        equations,
        coefficients,
        sigma: result.sigma,
        oirf: result.oirf,
        fevd: result.fevd,
        varwle,
        varlmar,
        varstable,
        vargranger,
    })
}

fn varsoc_input_slots() -> Vec<PinSlot> {
    vec![
        PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
        PinSlot::repeatable(
            PinDefinition::data_input(
                "",
                DataRole::Inputs(0),
                PinDataTypeDefinition::concrete(crate::graph::value::DataType::DataSeries(
                    Box::new(crate::graph::value::DataType::Float64),
                )),
            ),
            "Variables",
            1,
            None,
        ),
        PinSlot::fixed(
            PinDefinition::data_input(
                "Max lag",
                DataRole::Custom("maxlag".to_string()),
                PinDataTypeDefinition::concrete(crate::graph::value::DataType::Int64),
            )
            .with_optional(true)
            .with_default_value(DataValue::Int64(4)),
        ),
    ]
}

fn run_varsoc(ctx: &mut dyn NodeExecutionContextTrait) -> Result<VARSocResult, String> {
    let maxlag_val = ctx
        .get_input_by_role(&PinRole::Data(DataRole::Custom("maxlag".to_string())))
        .ok()
        .and_then(|v| v.as_i64())
        .unwrap_or(4);
    let maxlag = maxlag_val as usize;
    if maxlag < 1 {
        return Err("VAR varsoc: maxlag must be >= 1".to_string());
    }

    let var_inputs = ctx.get_inputs_by_family(&PinRole::Data(DataRole::Inputs(0)))?;
    if var_inputs.is_empty() {
        return Err("VAR varsoc: at least 1 variable required".to_string());
    }

    let mut series_list = Vec::new();
    let mut var_names = Vec::new();
    for v in &var_inputs {
        let dsv = match v {
            crate::graph::value::DataValue::DataSeries(s) => s.clone(),
            _ => return Err("VAR varsoc: each variable must be a DataSeries".to_string()),
        };
        let s = ctx.get_data_series(&dsv.id)?;
        let name = s.name().to_string();
        if name.is_empty() {
            var_names.push(format!("y{}", series_list.len()));
        } else {
            var_names.push(name);
        }
        let vals: Vec<f64> = s
            .cast(&polars::prelude::DataType::Float64)
            .map_err(|e| format!("VAR varsoc: cannot cast to Float64: {}", e))?
            .f64()
            .map_err(|e| format!("VAR varsoc: {}", e))?
            .into_no_null_iter()
            .collect();
        series_list.push(vals);
    }

    let n = series_list[0].len();
    for (i, s) in series_list.iter().enumerate() {
        if s.len() != n {
            return Err(format!(
                "VAR varsoc: variable '{}' has {} rows, expected {}",
                var_names
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| format!("y{}", i)),
                s.len(),
                n
            ));
        }
    }

    if n <= maxlag {
        return Err(format!(
            "VAR varsoc: need T > maxlag ({}), got T={}",
            maxlag, n
        ));
    }

    let k = series_list.len();
    let mut y_data = Vec::with_capacity(n * k);
    for i in 0..n {
        for j in 0..k {
            y_data.push(series_list[j][i]);
        }
    }
    let y = Array2::from_shape_vec((n, k), y_data)
        .map_err(|e| format!("VAR varsoc: failed to build Y matrix: {}", e))?;

    var_varsoc(y, maxlag, Some(var_names))
}

fn register_varsoc(registry: &NodeRegistry) {
    let mut slots = varsoc_input_slots();
    slots.push(PinSlot::fixed(PinDefinition::data_output(
        "Result",
        DataRole::Result,
        PinDataTypeDefinition::concrete(crate::graph::value::DataType::Struct(
            "VARSocResult".to_string(),
        )),
    )));
    slots.push(PinSlot::fixed(PinDefinition::exec_output(
        "Out",
        ExecRole::ExecOut,
    )));

    let definition = NodeDefinition::new(
        "VAR Lag Order (varsoc)",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_documentation(docs::var::VAR_VARSOC_ZH, docs::var::VAR_VARSOC_EN)
    .with_pin_slots(slots)
    .with_flow_processor(Arc::new(|ctx| {
        let result = run_varsoc(ctx)?;

        let json_data = serde_json::to_string(&result)
            .map_err(|e| format!("VAR varsoc: failed to serialize: {}", e))?;

        let result_handle_id = ctx.put_handle(Box::new(result));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Result),
            DataValue::new_struct("VARSocResult", result_handle_id),
        )?;

        ctx.publish_report(ReportKind::VarSoc, json_data);

        ctx.log("VAR varsoc: completed".to_string());
        Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
    }));
    registry.register(definition);
}

pub fn register(registry: &NodeRegistry) {
    register_var_summary(registry);
    register_varsoc(registry);
}

fn register_var_summary(registry: &NodeRegistry) {
    let mut slots = var_input_slots();
    slots.push(PinSlot::fixed(PinDefinition::data_output(
        "Result",
        DataRole::Result,
        PinDataTypeDefinition::concrete(crate::graph::value::DataType::Struct(
            "VARSummaryResult".to_string(),
        )),
    )));
    slots.push(PinSlot::fixed(PinDefinition::exec_output(
        "Out",
        ExecRole::ExecOut,
    )));

    let definition = NodeDefinition::new(
        "VAR Summary",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_documentation(docs::var::VAR_SUMMARY_ZH, docs::var::VAR_SUMMARY_EN)
    .with_pin_slots(slots)
    .with_flow_processor(Arc::new(|ctx| {
        let result = run_var(ctx)?;

        let json_data = serde_json::to_string(&result)
            .map_err(|e| format!("VAR Summary: failed to serialize: {}", e))?;

        let result_handle_id = ctx.put_handle(Box::new(result));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Result),
            DataValue::new_struct("VARSummaryResult", result_handle_id),
        )?;

        ctx.publish_report(ReportKind::VarSummary, json_data);

        ctx.log("VAR Summary: completed".to_string());
        Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
    }));
    registry.register(definition);
}
