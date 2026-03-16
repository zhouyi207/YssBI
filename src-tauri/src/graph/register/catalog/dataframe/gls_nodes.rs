//! GLS (Generalized Least Squares) 回归节点

use crate::execution::ExecutionEffect;
use crate::execution::context::NodeExecutionContextTrait;
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{CategoricalRole, DataSeriesValue, DataType, DataValue};
use ndarray::{Array1, Array2};
use polars::prelude::{Column, DataFrame, NamedFrom, Series};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use yss_sci::regression::collinearity;
use yss_sci::regression::diagnostics;
use yss_sci::regression::linear_model::{GLS, GLSConfig};
use yss_sci::ts::align::infer_interval;
use yss_sci::ts::lag::ts_lag;

use super::info_nodes::{compute_aic_bic, BreuschPaganTest, BreuschPaganTests, Coefficient, DiagnosticInfo, DiagnosticTiming, ImTest, ImTestComponent, ModelBasicInfo, NormalityTests, OLSResult, OmitInfo, OmittedVariable, OvTest, OvTests, ResidualScatterData, VifEntry};
use std::time::Instant;
use super::ols_nodes::VariableSpec;

/// Re-export OLSModel for Predict compatibility (GLS outputs same structure)
pub use super::ols_nodes::OLSModel;

// ======================== 结构体 ========================

/// GLS 配置（仅 constant，后续可扩展）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GLSConfigure {
    pub constant: bool,
    /// Optional time series ID (from Time pin on configure node)
    pub time_series_id: Option<String>,
}

impl Default for GLSConfigure {
    fn default() -> Self {
        Self {
            constant: true,
            time_series_id: None,
        }
    }
}

/// GLS 回归拟合结果（内部 helper 返回值）
struct GLSFitResult {
    ols_result: OLSResult,
    ols_model: OLSModel,
}

/// 将 Polars DataFrame 转为 ndarray Array2 (n×n)
/// 每列转为 f64 vec，按列优先构建：arr[[i,j]] = 第 j 列的第 i 个元素
fn dataframe_to_array2(df: &DataFrame) -> Result<Array2<f64>, String> {
    let nrows = df.height();
    let ncols = df.width();
    if nrows != ncols {
        return Err(format!(
            "GLS: Sigma must be a square matrix (n×n), got {}×{}",
            nrows, ncols
        ));
    }
    let mut columns: Vec<Vec<f64>> = Vec::with_capacity(ncols);
    for col in df.columns() {
        let s = col
            .cast(&polars::prelude::DataType::Float64)
            .map_err(|e| format!("GLS: Sigma column '{}' must be numeric: {}", col.name(), e))?;
        let f64_ca = s
            .f64()
            .map_err(|e| format!("GLS: Sigma: {}", e))?;
        let vec: Vec<f64> = f64_ca.into_no_null_iter().collect();
        if vec.len() != nrows {
            return Err(format!(
                "GLS: Sigma column has {} rows, expected {}",
                vec.len(), nrows
            ));
        }
        columns.push(vec);
    }
    // Row-major: row i, col j = columns[j][i]
    let mut data = Vec::with_capacity(nrows * ncols);
    for i in 0..nrows {
        for j in 0..ncols {
            data.push(columns[j][i]);
        }
    }
    Array2::from_shape_vec((nrows, ncols), data)
        .map_err(|e| format!("GLS: failed to build Sigma matrix: {}", e))
}

// ======================== 共享辅助函数 ========================

fn gls_input_slots() -> Vec<PinSlot> {
    let exog_type = DataType::DataSeries(Box::new(DataType::one_of(vec![
        DataType::Float64,
        DataType::Categorical,
    ])));

    vec![
        PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
        PinSlot::fixed(PinDefinition::data_input(
            "Y",
            DataRole::Custom("y".to_string()),
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
        PinSlot::repeatable(
            PinDefinition::data_input(
                "",
                DataRole::Inputs(0),
                PinDataTypeDefinition::concrete(exog_type),
            ),
            "X",
            1,
            None,
        ),
        PinSlot::fixed(PinDefinition::data_input(
            "Sigma",
            DataRole::Custom("sigma".to_string()),
            PinDataTypeDefinition::concrete(DataType::DataFrame),
        )),
        PinSlot::fixed(
            PinDefinition::data_input(
                "Time",
                DataRole::Custom("time".to_string()),
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::one_of(vec![
                    DataType::Date,
                    DataType::Int64,
                ])))),
            )
            .with_optional(true),
        ),
        PinSlot::fixed(
            PinDefinition::data_input(
                "Config",
                DataRole::Custom("gls_config".to_string()),
                PinDataTypeDefinition::concrete(DataType::Struct("GLSConfigure".to_string())),
            )
            .with_optional(true),
        ),
    ]
}

fn run_gls_regression(ctx: &mut dyn NodeExecutionContextTrait) -> Result<GLSFitResult, String> {
    // ---- Extract endog ----
    let endog_value = ctx.get_input_by_role(
        &PinRole::Data(DataRole::Custom("y".to_string())),
    )?;
    let endog_id = match &endog_value {
        DataValue::DataSeries(v) => v.id.clone(),
        other => {
            let got = match other {
                DataValue::Null => "Null (unconnected or upstream not executed)",
                DataValue::Boolean(_) => "Boolean",
                DataValue::Int32(_) => "Int32",
                DataValue::Int64(_) => "Int64",
                DataValue::Float32(_) => "Float32",
                DataValue::Float64(_) => "Float64",
                DataValue::String(_) => "String",
                DataValue::Array(_) => "Array",
                DataValue::Object(_) => "Object",
                DataValue::DataFrame(_) => "DataFrame",
                DataValue::Struct { type_key, .. } => {
                    return Err(format!(
                        "GLS: Y input is not a DataSeries (got Struct<{}>). Check that Y is connected to Add/DataSeries output, not Config.",
                        type_key
                    ));
                }
                DataValue::DataSeries(_) => unreachable!(),
            };
            return Err(format!(
                "GLS: Y input is not a DataSeries (got {}). Ensure Y is connected to a DataSeries output (e.g. Add result).",
                got
            ));
        }
    };
    let endog_series = ctx.get_series(&endog_id)?;
    let endog_name = {
        let raw = endog_series.name().to_string();
        if raw.is_empty() { "y".to_string() } else { raw }
    };
    let endog_f64_series = endog_series
        .cast(&polars::prelude::DataType::Float64)
        .map_err(|e| format!("GLS: cannot cast Y to Float64: {}", e))?;

    // ---- Extract Sigma (DataFrame -> Array2) ----
    let sigma_value = ctx.get_input_by_role(
        &PinRole::Data(DataRole::Custom("sigma".to_string())),
    )?;
    let sigma_df_id = match &sigma_value {
        DataValue::DataFrame(id) => id.clone(),
        DataValue::Null => {
            return Err("GLS: Sigma input is not connected. Connect a DataFrame (n×n covariance matrix).".to_string());
        }
        _ => {
            return Err("GLS: Sigma must be a DataFrame (n×n covariance matrix)".to_string());
        }
    };
    let sigma_df = ctx.get_dataframe(&sigma_df_id)?;
    let sigma_full = dataframe_to_array2(sigma_df.as_ref())?;
    let n_raw = endog_f64_series.len();
    if sigma_full.nrows() != n_raw {
        return Err(format!(
            "GLS: Sigma is {}×{}, but Y has {} observations. Sigma must be n×n where n = number of observations.",
            sigma_full.nrows(), sigma_full.ncols(), n_raw
        ));
    }

    // ---- Get config (optional — falls back to GLSConfigure::default()) ----
    let config = match ctx.get_input_by_role(
        &PinRole::Data(DataRole::Custom("gls_config".to_string())),
    ) {
        Ok(config_value) => match config_value.as_handle_id() {
            Some(id) => {
                let handle = ctx.get_handle(&id.to_string())?;
                handle
                    .downcast_ref::<GLSConfigure>()
                    .ok_or("GLS: config handle is not a GLSConfigure")?
                    .clone()
            }
            None => GLSConfigure::default(),
        },
        Err(_) => GLSConfigure::default(),
    };
    let has_constant = config.constant;

    // ---- Extract exog (mixed numeric + categorical) ----
    let exog_data_values = ctx.get_inputs_by_family(
        &PinRole::Data(DataRole::Inputs(0)),
    )?;

    if exog_data_values.is_empty() {
        return Err("GLS: at least one X input is required".to_string());
    }

    // Optional time series for residual time info (from direct Time pin or from config)
    let time_series = match ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("time".to_string()))) {
        Ok(DataValue::DataSeries(v)) => {
            let ts = ctx.get_series(&v.id)?;
            if ts.len() != n_raw {
                return Err(format!(
                    "GLS: Time has {} observations, expected {} (must match Y length)",
                    ts.len(), n_raw
                ));
            }
            Some(ts)
        }
        _ => {
            if let Some(ref id) = config.time_series_id {
                let ts = ctx.get_series(id)?;
                if ts.len() != n_raw {
                    return Err(format!(
                        "GLS: Time from config has {} observations, expected {} (must match Y length)",
                        ts.len(), n_raw
                    ));
                }
                Some(ts)
            } else {
                None
            }
        }
    };

    // Build DataFrame with __idx__ + endog + optional time + all exog, then drop rows with null/NaN
    let mut df_cols: Vec<Column> = vec![
        Column::from(Series::new("__idx__".into(), (0..n_raw).map(|i| i as u32).collect::<Vec<u32>>())),
        Column::from(endog_f64_series.with_name("__endog__".into())),
    ];
    if let Some(ref ts) = time_series {
        df_cols.push(Column::from(ts.clone().with_name("__time__".into())));
    }
    let mut exog_meta: Vec<(String, bool, DataSeriesValue)> = Vec::new();
    for (i, val) in exog_data_values.iter().enumerate() {
        let dsv = match val {
            DataValue::DataSeries(v) => v.clone(),
            _ => return Err(format!("GLS: X input {} is not a DataSeries", i)),
        };
        let series = ctx.get_series(&dsv.id)?;
        let series_name = {
            let raw = series.name().to_string();
            if raw.is_empty() { format!("x{}", i + 1) } else { raw }
        };
        if series.len() != n_raw {
            return Err(format!(
                "GLS: X '{}' has {} observations, expected {} (must match Y length)",
                series_name, series.len(), n_raw
            ));
        }
        let is_categorical = matches!(
            series.dtype(),
            polars::prelude::DataType::Categorical(_, _) | polars::prelude::DataType::Enum(_, _)
        );
        let col_series = if is_categorical {
            series
                .cast(&polars::prelude::DataType::String)
                .map_err(|e| format!("GLS: cannot cast X {} to String: {}", i, e))?
        } else {
            series
                .cast(&polars::prelude::DataType::Float64)
                .map_err(|e| format!("GLS: cannot cast X {} to Float64: {}", i, e))?
        };
        df_cols.push(Column::from(col_series.with_name(series_name.as_str().into())));
        exog_meta.push((series_name, is_categorical, dsv));
    }
    let df = DataFrame::new(n_raw, df_cols)
        .map_err(|e| format!("GLS: failed to build DataFrame: {}", e))?
        .drop_nulls::<&str>(None)
        .map_err(|e| format!("GLS: drop_nulls failed: {}", e))?;
    let n = df.height();
    if n == 0 {
        return Err("GLS: no valid observations after dropping null/NaN values. Check that Y and X have at least some complete rows.".to_string());
    }

    // Subset Sigma to valid row/col indices
    let valid_indices: Vec<usize> = df
        .column("__idx__")
        .map_err(|e| format!("GLS: {}", e))?
        .u32()
        .map_err(|e| format!("GLS: {}", e))?
        .into_no_null_iter()
        .map(|i| i as usize)
        .collect();
    let sigma = ndarray::Array2::from_shape_fn((n, n), |(i, j)| sigma_full[[valid_indices[i], valid_indices[j]]]);

    let endog = Array1::from(
        df.column("__endog__")
            .map_err(|e| format!("GLS: {}", e))?
            .f64()
            .map_err(|e| format!("GLS: {}", e))?
            .into_no_null_iter()
            .collect::<Vec<f64>>(),
    );

    let mut exog_columns: Vec<Vec<f64>> = Vec::new();
    let mut col_labels: Vec<(String, Option<String>)> = Vec::new();
    let mut variable_specs: Vec<VariableSpec> = Vec::new();

    for (series_name, is_categorical, dsv) in exog_meta {
        let col = df.column(&series_name).map_err(|e| format!("GLS: {}", e))?;

        if is_categorical {
            let str_ca = col.str().map_err(|e| format!("GLS: X '{}': {}", series_name, e))?;
            let values: Vec<String> = str_ca.into_no_null_iter().map(|s: &str| s.to_string()).collect();
            let mut unique_ordered: Vec<String> = Vec::new();
            for v in &values {
                if !unique_ordered.contains(v) {
                    unique_ordered.push(v.clone());
                }
            }
            if unique_ordered.len() < 2 {
                return Err(format!(
                    "GLS: categorical variable '{}' must have at least 2 unique values (after dropping nulls)",
                    series_name
                ));
            }
            let dummy_info = dsv.dummy_info.as_ref();
            let role = dummy_info
                .map(|di| di.role.clone())
                .unwrap_or(CategoricalRole::General);
            let drop_cat = if let Some(di) = dummy_info {
                if let Some(ref specified) = di.drop_category {
                    if !unique_ordered.contains(specified) {
                        return Err(format!(
                            "GLS: drop_category '{}' not found in unique values of '{}'",
                            specified, series_name
                        ));
                    }
                    specified.clone()
                } else {
                    unique_ordered[0].clone()
                }
            } else if has_constant {
                unique_ordered[0].clone()
            } else {
                String::new()
            };
            let categories_to_include: Vec<&String> = if drop_cat.is_empty() {
                unique_ordered.iter().collect()
            } else {
                unique_ordered.iter().filter(|c| **c != drop_cat).collect()
            };
            for cat in &categories_to_include {
                let col: Vec<f64> = values.iter().map(|v| if v == *cat { 1.0 } else { 0.0 }).collect();
                exog_columns.push(col);
                col_labels.push((series_name.clone(), Some((*cat).clone())));
            }
            variable_specs.push(VariableSpec::Categorical {
                name: series_name,
                categories: unique_ordered,
                dropped: if drop_cat.is_empty() { String::new() } else { drop_cat },
                role,
            });
        } else {
            let values: Vec<f64> = col.f64().map_err(|e| format!("GLS: X '{}': {}", series_name, e))?.into_no_null_iter().collect();
            exog_columns.push(values);
            col_labels.push((series_name.clone(), None));
            variable_specs.push(VariableSpec::Numeric { name: series_name });
        }
    }

    // ---- Build exog matrix ----
    let k = if has_constant {
        exog_columns.len() + 1
    } else {
        exog_columns.len()
    };
    let mut exog_raw = Vec::with_capacity(n * k);

    let mut all_labels: Vec<(String, Option<String>)> = Vec::new();
    if has_constant {
        all_labels.push(("const".to_string(), None));
    }
    all_labels.extend(col_labels);

    for i in 0..n {
        if has_constant {
            exog_raw.push(1.0);
        }
        for col in &exog_columns {
            exog_raw.push(col[i]);
        }
    }

    let exog = Array2::from_shape_vec((n, k), exog_raw)
        .map_err(|e| format!("GLS: failed to build exog matrix: {}", e))?;

    // ---- Drop strictly collinear columns (continuous > dummy > intercept) ----
    let col_is_dummy: Vec<bool> = all_labels.iter().map(|(_, cat)| cat.is_some()).collect();
    let intercept_col = if has_constant { Some(0) } else { None };
    let (exog_use, omitted_indices) =
        collinearity::drop_collinear_columns(&exog, &col_is_dummy, intercept_col)?;
    let omit_info = if omitted_indices.is_empty() {
        None
    } else {
        let omitted: Vec<OmittedVariable> = omitted_indices
            .iter()
            .filter_map(|&i| all_labels.get(i))
            .map(|(var, cat)| OmittedVariable {
                variable: var.clone(),
                category: cat.clone(),
                reason: "collinearity".to_string(),
            })
            .collect();
        Some(OmitInfo { omitted })
    };
    let all_labels_use: Vec<(String, Option<String>)> = (0..k)
        .filter(|i| !omitted_indices.contains(i))
        .filter_map(|i| all_labels.get(i).cloned())
        .collect();

    // ---- Run GLS regression ----
    let sci_config = GLSConfig {
        constant: has_constant,
    };
    let gls = GLS {
        endog: endog.clone(),
        exog: exog_use.clone(),
        sigma: sigma.clone(),
        config: sci_config,
    };
    let result = gls.fit()?;

    // ---- Compute fitted values & residuals (in original space, not transformed) ----
    let t_fitted = Instant::now();
    let fitted_values: Vec<f64> = (0..n)
        .map(|i| {
            exog_use.row(i)
                .iter()
                .zip(result.betas.iter())
                .map(|(x, b)| x * b)
                .sum()
        })
        .collect();
    let residuals: Vec<f64> = endog
        .iter()
        .zip(fitted_values.iter())
        .map(|(y, yhat)| y - yhat)
        .collect();
    let fitted_residuals_ms = t_fitted.elapsed().as_millis() as u64;

    // ---- Build coefficient table ----
    let num_coeff = result.betas.len();
    let mut coefficients = Vec::with_capacity(num_coeff);
    for i in 0..num_coeff {
        let (var_name, category) = all_labels_use
            .get(i)
            .cloned()
            .unwrap_or_else(|| (format!("x{}", i), None));
        coefficients.push(Coefficient {
            variable: var_name,
            category,
            coef: result.betas[i],
            std_err: result.stds[i],
            t_value: result.tvalues[i],
            p_value: result.pvalues[i],
            ci_lower: result.conf_int_left[i],
            ci_upper: result.conf_int_right[i],
            is_significant: result.pvalues[i] < 0.05,
        });
    }

    let (bp_tests, bp_tests_ms) = if has_constant {
        let t_bp = Instant::now();
        let residuals_arr = Array1::from(residuals.clone());
        let fitted_arr = Array1::from(fitted_values.clone());
        let r_stata_rhs = diagnostics::breusch_pagan_stata_rhs(&exog_use, &residuals_arr).ok();
        let r_koenker_rhs = diagnostics::breusch_pagan_koenker_rhs(&exog_use, &residuals_arr).ok();
        let r_stata = diagnostics::breusch_pagan_stata(&residuals_arr, &fitted_arr).ok();
        let r_koenker = diagnostics::breusch_pagan_koenker(&residuals_arr, &fitted_arr).ok();
        let ms = t_bp.elapsed().as_millis() as u64;
        (
            Some(BreuschPaganTests {
                stata: r_stata.map(|r| BreuschPaganTest { lm_stat: r.lm_stat, df: r.df, p_value: r.p_value }),
                koenker: r_koenker.map(|r| BreuschPaganTest { lm_stat: r.lm_stat, df: r.df, p_value: r.p_value }),
                stata_rhs: r_stata_rhs.map(|r| BreuschPaganTest { lm_stat: r.lm_stat, df: r.df, p_value: r.p_value }),
                koenker_rhs: r_koenker_rhs.map(|r| BreuschPaganTest { lm_stat: r.lm_stat, df: r.df, p_value: r.p_value }),
            }),
            Some(ms),
        )
    } else {
        (None, None)
    };

    let (ov_tests, ov_tests_ms) = if has_constant && residuals.len() >= 8 {
        let t_ov = Instant::now();
        let y_arr = endog.clone();
        let fitted_arr = Array1::from(fitted_values.clone());
        let r_default = diagnostics::reset_test(&y_arr, &exog_use, &fitted_arr, None).ok();
        let r_rhs = diagnostics::reset_test_rhs(&y_arr, &exog_use, None).ok();
        let ms = t_ov.elapsed().as_millis() as u64;
        (
            Some(OvTests {
                default: r_default.map(|r| OvTest { f_stat: r.f_stat, df1: r.df1, df2: r.df2, p_value: r.p_value }),
                rhs: r_rhs.map(|r| OvTest { f_stat: r.f_stat, df1: r.df1, df2: r.df2, p_value: r.p_value }),
            }),
            Some(ms),
        )
    } else {
        (None, None)
    };

    let (im_test, im_test_ms) = if has_constant {
        let t_im = Instant::now();
        let residuals_arr = Array1::from(residuals.clone());
        let im = match diagnostics::im_test(&exog_use, &residuals_arr) {
            Ok(r) => Some(ImTest {
                heteroskedasticity: ImTestComponent {
                    chi2: r.heteroskedasticity.chi2,
                    df: r.heteroskedasticity.df,
                    p_value: r.heteroskedasticity.p_value,
                },
                skewness: ImTestComponent {
                    chi2: r.skewness.chi2,
                    df: r.skewness.df,
                    p_value: r.skewness.p_value,
                },
                kurtosis: ImTestComponent {
                    chi2: r.kurtosis.chi2,
                    df: r.kurtosis.df,
                    p_value: r.kurtosis.p_value,
                },
                total: ImTestComponent {
                    chi2: r.total.chi2,
                    df: r.total.df,
                    p_value: r.total.p_value,
                },
            }),
            Err(_) => None,
        };
        let ms = t_im.elapsed().as_millis() as u64;
        (im, Some(ms))
    } else {
        (None, None)
    };

    let vif = diagnostics::vif_centered(&exog_use, has_constant).ok().and_then(|entries| {
        let vif_entries: Vec<VifEntry> = entries
            .into_iter()
            .enumerate()
            .filter(|(j, e)| !(has_constant && *j == 0) && !e.vif.is_nan())
            .map(|(j, e)| {
                let (var_name, _) = all_labels_use
                    .get(j)
                    .cloned()
                    .unwrap_or_else(|| (format!("x{}", j), None));
                VifEntry {
                    variable: var_name,
                    vif: e.vif,
                    tolerance: e.tolerance,
                }
            })
            .collect();
        if vif_entries.is_empty() {
            None
        } else {
            Some(vif_entries)
        }
    });

    // Build residual_scatter (e vs e_lag1) using ts_lag when time is provided
    let residual_scatter = if time_series.is_some() && residuals.len() >= 2 {
        let time_col = df.column("__time__").map_err(|e| format!("GLS: {}", e))?;
        let time_s = time_col.clone().take_materialized_series();
        let residuals_s = Series::from_iter(residuals.iter().cloned()).with_name("residuals".into());

        let interval = infer_interval(&time_s).unwrap_or(1);
        match ts_lag(&time_s, &residuals_s, 1, interval) {
            Ok((full_times, e_aligned, e_lag1_series)) => {
                let e_vec: Vec<Option<f64>> = e_aligned
                    .f64()
                    .map_err(|e| format!("GLS: {}", e))?
                    .into_iter()
                    .collect();
                let e_lag1_vec: Vec<Option<f64>> = e_lag1_series
                    .f64()
                    .map_err(|e| format!("GLS: {}", e))?
                    .into_iter()
                    .collect();
                let time_str_s = full_times
                    .cast(&polars::prelude::DataType::String)
                    .map_err(|e| format!("GLS: time cast: {}", e))?;
                let time_str_vec: Vec<String> = time_str_s
                    .str()
                    .map_err(|e| format!("GLS: {}", e))?
                    .into_iter()
                    .map(|v| v.unwrap_or("").to_string())
                    .collect();

                let mut e = Vec::new();
                let mut e_lag1 = Vec::new();
                let mut time_out = Vec::new();
                for i in 0..e_vec.len().min(e_lag1_vec.len()) {
                    if let (Some(et), Some(et_lag1)) = (e_vec[i], e_lag1_vec[i]) {
                        e.push(et);
                        e_lag1.push(et_lag1);
                        time_out.push(time_str_vec.get(i).cloned().unwrap_or_default());
                    }
                }
                if e.is_empty() {
                    None
                } else {
                    Some(ResidualScatterData {
                        e,
                        e_lag1,
                        time: Some(time_out),
                    })
                }
            }
            Err(_) => None,
        }
    } else {
        None
    };

    let normality_tests = if has_constant && residuals.len() >= 8 {
        let residuals_arr = Array1::from(residuals.clone());
        diagnostics::normality_tests(&residuals_arr).ok().map(|r| NormalityTests {
            skewness: r.skewness,
            kurtosis: r.kurtosis,
            omnibus_stat: r.omnibus_stat,
            omnibus_p_value: r.omnibus_p_value,
            jarque_bera_stat: r.jarque_bera_stat,
            jarque_bera_p_value: r.jarque_bera_p_value,
        })
    } else {
        None
    };

    let ols_result = OLSResult {
        title: "GLS Regression Results".to_string(),
        endog_name,
        model_basic_info: {
            let (aic, bic) = compute_aic_bic(
                result.num_observation,
                result.betas.len(),
                result.ss_residual,
            );
            ModelBasicInfo {
                model_type: "GLS".to_string(),
                method: "Generalized Least Squares".to_string(),
                num_observation: result.num_observation,
                r_squared: result.r2,
                adj_r_squared: result.r2_adjusted,
                f_statistic: result.fvalue,
                prob_f_statistic: result.f_p_value,
                wald_chi2: None,
                prob_wald_chi2: None,
                log_likelihood: None,
                lr_chi2: None,
                prob_lr_chi2: None,
                chibar2: None,
                prob_chibar2: None,
                mle_iter_log_lik_const: None,
                mle_iter_log_lik: None,
                df_model: result.df_model,
                df_residual: result.df_residual,
                df_total: result.df_total,
                ss_model: result.ss_model,
                ss_residual: result.ss_residual,
                ss_total: result.ss_total,
                ms_model: result.ms_model,
                ms_residual: result.ms_residual,
                ms_total: result.ms_total,
                covariance_type: result.covariance_type.to_string(),
                aic,
                bic,
            }
        },
        coefficients,
        diagnostic_info: DiagnosticInfo {
            cond_no: result.cond_no,
            vif,
            bp_tests,
            ov_tests,
            im_test,
            normality_tests,
            fitted_values,
            residuals,
            leverage: diagnostics::leverage(&exog_use).unwrap_or_default(),
            residual_scatter,
            exog: Some((0..n).map(|i| exog_use.row(i).iter().cloned().collect()).collect()),
            timing: Some(DiagnosticTiming {
                fitted_residuals_ms: Some(fitted_residuals_ms),
                bp_tests_ms,
                ov_tests_ms,
                im_test_ms,
            }),
            prais_info: None,
            iv2sls_first_stage: None,
            iv2sls_first_stage_summary: None,
            iv2sls_overid: None,
            iv2sls_overid_dims: None,
            iv2sls_hausman: None,
            iv2sls_endogenous: None,
            ivliml_kappa: None,
            ivliml_overid: None,
            classification_table: None,
            exog_means: None,
            panel_fe_info: None,
            omit_info,
        },
        betas: result.betas.to_vec(),
        cov_beta: (0..result.cov_beta.nrows())
            .map(|i| result.cov_beta.row(i).iter().cloned().collect())
            .collect(),
        cov_beta_nonrobust: None,
    };

    let kept_indices = if omitted_indices.is_empty() {
        None
    } else {
        Some((0..k).filter(|i| !omitted_indices.contains(i)).collect())
    };
    let ols_model = OLSModel {
        betas: result.betas.to_vec(),
        has_constant,
        variable_specs,
        kept_indices,
    };

    Ok(GLSFitResult { ols_result, ols_model })
}

// ======================== 注册入口 ========================

pub fn register(registry: &NodeRegistry) {
    register_gls_configure(registry);
    register_gls(registry);
    register_gls_summary(registry);
}

// ======================== GLS Configure 节点 ========================

fn register_gls_configure(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "GLS Configure",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("GLS regression configuration — constant term (optional)")
    .with_pin_slots(vec![
        PinSlot::fixed(
            PinDefinition::data_input(
                "Constant",
                DataRole::Custom("constant".to_string()),
                PinDataTypeDefinition::concrete(DataType::Boolean),
            )
            .with_optional(true),
        ),
        PinSlot::fixed(
            PinDefinition::data_input(
                "Time",
                DataRole::Custom("time".to_string()),
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::one_of(vec![
                    DataType::Int64,
                    DataType::Date,
                ])))),
            )
            .with_optional(true),
        ),
        PinSlot::fixed(PinDefinition::data_output(
            "Config",
            DataRole::Result,
            PinDataTypeDefinition::concrete(DataType::Struct("GLSConfigure".to_string())),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let constant = ctx
            .get_input_by_role(&PinRole::Data(DataRole::Custom("constant".to_string())))
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let time_series_id = ctx
            .get_input_by_role(&PinRole::Data(DataRole::Custom("time".to_string())))
            .ok()
            .and_then(|v| match &v {
                DataValue::DataSeries(dsv) => Some(dsv.id.clone()),
                _ => None,
            });

        let config = GLSConfigure {
            constant,
            time_series_id,
        };
        let handle_id = ctx.put_handle(Box::new(config));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Result),
            DataValue::new_struct("GLSConfigure", handle_id),
        )?;
        Ok(())
    }));
    registry.register(definition);
}

// ======================== GLS 节点 ========================

fn register_gls(registry: &NodeRegistry) {
    let mut slots = gls_input_slots();
    slots.push(PinSlot::fixed(PinDefinition::data_output(
        "Model",
        DataRole::Custom("ols_model".to_string()),
        PinDataTypeDefinition::concrete(DataType::Struct("OLSModel".to_string())),
    )));
    slots.push(PinSlot::fixed(PinDefinition::data_output(
        "Fitted",
        DataRole::Custom("ols_fitted".to_string()),
        PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
    )));
    slots.push(PinSlot::fixed(PinDefinition::data_output(
        "Residuals",
        DataRole::Custom("ols_residuals".to_string()),
        PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
    )));
    slots.push(PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)));

    let definition = NodeDefinition::new(
        "GLS",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Generalized Least Squares regression — outputs the fitted model for prediction")
    .with_pin_slots(slots)
    .with_flow_processor(Arc::new(|ctx| {
        let fit = run_gls_regression(ctx)?;

        let model_handle_id = ctx.put_handle(Box::new(fit.ols_model));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Custom("ols_model".to_string())),
            DataValue::new_struct("OLSModel", model_handle_id),
        )?;

        let fitted_series = Series::from_iter(fit.ols_result.diagnostic_info.fitted_values.into_iter())
            .with_name("fitted".into());
        let fitted_id = ctx.put_series(fitted_series)?;
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Custom("ols_fitted".to_string())),
            DataValue::DataSeries(DataSeriesValue::with_element_type(fitted_id, DataType::Float64)),
        )?;

        let residuals_series = Series::from_iter(fit.ols_result.diagnostic_info.residuals.into_iter())
            .with_name("residuals".into());
        let residuals_id = ctx.put_series(residuals_series)?;
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Custom("ols_residuals".to_string())),
            DataValue::DataSeries(DataSeriesValue::with_element_type(residuals_id, DataType::Float64)),
        )?;

        ctx.log("GLS: regression completed".to_string());
        Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
    }));
    registry.register(definition);
}

// ======================== GLS Summary 节点 ========================

fn register_gls_summary(registry: &NodeRegistry) {
    let mut slots = gls_input_slots();
    slots.push(PinSlot::fixed(PinDefinition::data_output(
        "Result",
        DataRole::Result,
        PinDataTypeDefinition::concrete(DataType::Struct("OLSResult".to_string())),
    )));
    slots.push(PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)));

    let definition = NodeDefinition::new(
        "GLS Summary",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Generalized Least Squares regression — outputs results and opens the summary window")
    .with_pin_slots(slots)
    .with_flow_processor(Arc::new(|ctx| {
        let fit = run_gls_regression(ctx)?;

        let json_data = serde_json::to_string(&fit.ols_result)
            .map_err(|e| format!("GLS Summary: failed to serialize: {}", e))?;

        let result_handle_id = ctx.put_handle(Box::new(fit.ols_result));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Result),
            DataValue::new_struct("OLSResult", result_handle_id),
        )?;

        ctx.open_window("ols_summary".to_string(), json_data);

        ctx.log("GLS Summary: regression completed".to_string());
        Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
    }));
    registry.register(definition);
}
