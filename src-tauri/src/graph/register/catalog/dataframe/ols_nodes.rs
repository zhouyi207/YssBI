//! OLS 回归节点

use crate::execution::ExecutionEffect;
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{CategoricalRole, DataSeriesValue, DataType, DataValue};
use ndarray::{Array1, Array2};
use polars::prelude::{Column, DataFrame, Series};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use yss_sci::regression::diagnostics;
use yss_sci::regression::linear_model::{OLSConfig, OLS};
use yss_sci::ts::align::infer_interval;
use yss_sci::ts::lag::ts_lag;

use super::info_nodes::{compute_aic_bic, BreuschPaganTest, BreuschPaganTests, Coefficient, DiagnosticInfo, DiagnosticTiming, ImTest, ImTestComponent, ModelBasicInfo, NormalityTests, OLSResult, OvTest, OvTests, ResidualScatterData, VifEntry};
use std::time::Instant;

// ======================== 结构体 ========================

/// VCE (Variance-Covariance Estimator) 内部表示，用于 OLSConfigure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum OLSCovarianceConfig {
    FixedScale { scale: f64 },
    Cluster { cluster_id: Vec<usize> },
    HAC { kernel: String, bandwidth: Option<i64> },
    /// Stata newey: Bartlett + n/(n-k)，与 HAC (ivreg2) 不同
    Newey { lag: Option<i64> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OLSConfigure {
    pub constant: bool,
    pub cov_type: String,
    pub cov_config: Option<OLSCovarianceConfig>,
    /// Optional time series ID (from Time pin on configure node)
    pub time_series_id: Option<String>,
    /// Stata small: degrees-of-freedom adjustment. true = ESS/(n-k), false = ESS/n (Stata default).
    pub small: bool,
}

impl Default for OLSConfigure {
    fn default() -> Self {
        Self {
            constant: true,
            cov_type: "nonrobust".to_string(),
            cov_config: None,
            time_series_id: None,
            small: true,
        }
    }
}

/// 格式化 Covariance Type 用于窗口展示
pub(super) fn format_covariance_type_display(cov_type: &str, cov_config: Option<&OLSCovarianceConfig>) -> String {
    if cov_type == "HAC" {
        if let Some(OLSCovarianceConfig::HAC { kernel, bandwidth }) = cov_config {
            let bw_str = match bandwidth {
                Some(b) => format!("bandwidth={}", b),
                None => "bandwidth=auto".to_string(),
            };
            return format!("HAC ({}, {})", kernel, bw_str);
        }
    }
    if cov_type == "newey" {
        if let Some(OLSCovarianceConfig::Newey { lag }) = cov_config {
            let lag_str = match lag {
                Some(l) => format!("lag={}", l),
                None => "lag=auto".to_string(),
            };
            return format!("Newey ({})", lag_str);
        }
    }
    cov_type.to_string()
}

/// VCE 简单类型（无参）的 unit struct，用于 OneOf 和常量节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VCENonRobust;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VCEHC0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VCEHC1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VCEHC2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VCEHC3;

/// 各 Config 节点输出的结构体（用于 downcast）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OLSFixedScaleConfig {
    pub scale: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OLSClusterConfig {
    pub cluster_id: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OLSHACConfig {
    pub kernel: String,
    pub bandwidth: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OLSNeweyConfig {
    pub lag: Option<i64>,
}

/// 变量规格：记录每个输入变量在拟合时的处理方式（供预测复用）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum VariableSpec {
    Numeric {
        name: String,
    },
    Categorical {
        name: String,
        /// 拟合时的所有 unique 值（有序）
        categories: Vec<String>,
        /// 被剔除的参考类别
        dropped: String,
        /// 语义角色
        role: CategoricalRole,
    },
}

/// OLS 模型（拟合产物，携带完整的变量编码规格）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OLSModel {
    pub betas: Vec<f64>,
    pub has_constant: bool,
    pub variable_specs: Vec<VariableSpec>,
}

/// OLS 回归拟合结果（内部 helper 返回值）
struct OLSFitResult {
    ols_result: OLSResult,
    ols_model: OLSModel,
}

// ======================== 共享辅助函数 ========================

fn ols_input_slots() -> Vec<PinSlot> {
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
                DataRole::Custom("ols_config".to_string()),
                PinDataTypeDefinition::concrete(DataType::Struct("OLSConfigure".to_string())),
            )
            .with_optional(true),
        ),
    ]
}

use crate::execution::context::NodeExecutionContextTrait;
use std::collections::HashMap;

fn run_ols_regression(ctx: &mut dyn NodeExecutionContextTrait) -> Result<OLSFitResult, String> {
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
                        "OLS: Y input is not a DataSeries (got Struct<{}>). Check that Y is connected to Add/DataSeries output, not Config.",
                        type_key
                    ));
                }
                DataValue::DataSeries(_) => unreachable!(),
            };
            return Err(format!(
                "OLS: Y input is not a DataSeries (got {}). Ensure Y is connected to a DataSeries output (e.g. Add result).",
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
        .map_err(|e| format!("OLS: cannot cast Y to Float64: {}", e))?;

    // ---- Get config (optional — falls back to OLSConfigure::default()) ----
    let config = match ctx.get_input_by_role(
        &PinRole::Data(DataRole::Custom("ols_config".to_string())),
    ) {
        Ok(config_value) => match config_value.as_handle_id() {
            Some(id) => {
                let handle = ctx.get_handle(&id.to_string())?;
                handle
                    .downcast_ref::<OLSConfigure>()
                    .ok_or("OLS: config handle is not an OLSConfigure")?
                    .clone()
            }
            None => OLSConfigure::default(),
        },
        Err(_) => OLSConfigure::default(),
    };
    let has_constant = config.constant;

    // ---- Extract exog (mixed numeric + categorical) ----
    let exog_data_values = ctx.get_inputs_by_family(
        &PinRole::Data(DataRole::Inputs(0)),
    )?;

    if exog_data_values.is_empty() {
        return Err("OLS: at least one X input is required".to_string());
    }

    // Optional time series for residual time info (from direct Time pin or from config)
    let time_series = match ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("time".to_string()))) {
        Ok(DataValue::DataSeries(v)) => {
            let ts = ctx.get_series(&v.id)?;
            if ts.len() != endog_f64_series.len() {
                return Err(format!(
                    "OLS: Time has {} observations, expected {} (must match Y length)",
                    ts.len(), endog_f64_series.len()
                ));
            }
            Some(ts)
        }
        _ => {
            if let Some(ref id) = config.time_series_id {
                let ts = ctx.get_series(id)?;
                if ts.len() != endog_f64_series.len() {
                    return Err(format!(
                        "OLS: Time from config has {} observations, expected {} (must match Y length)",
                        ts.len(), endog_f64_series.len()
                    ));
                }
                Some(ts)
            } else {
                None
            }
        }
    };

    // Build DataFrame with endog + optional time + all exog, then drop rows with null/NaN
    let n_raw = endog_f64_series.len();
    let mut df_cols: Vec<Column> = vec![
        Column::from(endog_f64_series.with_name("__endog__".into())),
    ];
    if let Some(ref ts) = time_series {
        df_cols.push(Column::from(ts.clone().with_name("__time__".into())));
    }
    let mut exog_meta: Vec<(String, bool, DataSeriesValue)> = Vec::new();
    for (i, val) in exog_data_values.iter().enumerate() {
        let dsv = match val {
            DataValue::DataSeries(v) => v.clone(),
            _ => return Err(format!("OLS: X input {} is not a DataSeries", i)),
        };
        let series = ctx.get_series(&dsv.id)?;
        let series_name = {
            let raw = series.name().to_string();
            if raw.is_empty() { format!("x{}", i + 1) } else { raw }
        };
        if series.len() != n_raw {
            return Err(format!(
                "OLS: X '{}' has {} observations, expected {} (must match Y length)",
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
                .map_err(|e| format!("OLS: cannot cast X {} to String: {}", i, e))?
        } else {
            series
                .cast(&polars::prelude::DataType::Float64)
                .map_err(|e| format!("OLS: cannot cast X {} to Float64: {}", i, e))?
        };
        df_cols.push(Column::from(col_series.with_name(series_name.as_str().into())));
        exog_meta.push((series_name, is_categorical, dsv));
    }
    let df = DataFrame::new(n_raw, df_cols)
        .map_err(|e| format!("OLS: failed to build DataFrame: {}", e))?
        .drop_nulls::<&str>(None)
        .map_err(|e| format!("OLS: drop_nulls failed: {}", e))?;
    let n = df.height();
    if n == 0 {
        return Err("OLS: no valid observations after dropping null/NaN values. Check that Y and X have at least some complete rows.".to_string());
    }

    let endog = Array1::from(
        df.column("__endog__")
            .map_err(|e| format!("OLS: {}", e))?
            .f64()
            .map_err(|e| format!("OLS: {}", e))?
            .into_no_null_iter()
            .collect::<Vec<f64>>(),
    );

    let mut exog_columns: Vec<Vec<f64>> = Vec::new();
    let mut col_labels: Vec<(String, Option<String>)> = Vec::new();
    let mut variable_specs: Vec<VariableSpec> = Vec::new();

    for (series_name, is_categorical, dsv) in exog_meta {
        let col = df.column(&series_name).map_err(|e| format!("OLS: {}", e))?;

        if is_categorical {
            let str_ca = col.str().map_err(|e| format!("OLS: X '{}': {}", series_name, e))?;
            let values: Vec<String> = str_ca.into_no_null_iter().map(|s: &str| s.to_string()).collect();
            let mut unique_ordered: Vec<String> = Vec::new();
            for v in &values {
                if !unique_ordered.contains(v) {
                    unique_ordered.push(v.clone());
                }
            }
            if unique_ordered.len() < 2 {
                return Err(format!(
                    "OLS: categorical variable '{}' must have at least 2 unique values (after dropping nulls)",
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
                            "OLS: drop_category '{}' not found in unique values of '{}'",
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
            let values: Vec<f64> = col.f64().map_err(|e| format!("OLS: X '{}': {}", series_name, e))?.into_no_null_iter().collect();
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
        .map_err(|e| format!("OLS: failed to build exog matrix: {}", e))?;

    // ---- Run OLS regression ----
    let cov_params = config.cov_config.as_ref().map(|c| match c {
        OLSCovarianceConfig::FixedScale { scale } => yss_sci::regression::linear_model::CovParams::FixedScale {
            scale: *scale,
        },
        OLSCovarianceConfig::Cluster { cluster_id } => yss_sci::regression::linear_model::CovParams::Cluster {
            cluster_id: cluster_id.clone(),
            xtreg_fe_style: false,
        },
        OLSCovarianceConfig::HAC { kernel, bandwidth } => yss_sci::regression::linear_model::CovParams::HAC {
            kernel: kernel.clone(),
            bandwidth: *bandwidth,
        },
        OLSCovarianceConfig::Newey { lag } => yss_sci::regression::linear_model::CovParams::Newey {
            lag: *lag,
        },
    });

    let sci_config = OLSConfig {
        constant: has_constant,
        cov_type: config.cov_type.clone(),
        cov_params,
    };
    let ols = OLS {
        endog: endog.clone(),
        exog: exog.clone(),
        config: sci_config,
    };
    let result = ols.fit()?;

    // ---- Compute fitted values & residuals ----
    let t_fitted = Instant::now();
    let fitted_values: Vec<f64> = (0..n)
        .map(|i| {
            exog.row(i)
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
        let (var_name, category) = all_labels
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
        let r_stata_rhs = diagnostics::breusch_pagan_stata_rhs(&exog, &residuals_arr).ok();
        let r_koenker_rhs = diagnostics::breusch_pagan_koenker_rhs(&exog, &residuals_arr).ok();
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

    let (im_test, im_test_ms) = if has_constant {
        let t_im = Instant::now();
        let residuals_arr = Array1::from(residuals.clone());
        let im = match diagnostics::im_test(&exog, &residuals_arr) {
            Ok(r) => {
                Some(ImTest {
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
                })
            }
            Err(_) => None,
        };
        let ms = t_im.elapsed().as_millis() as u64;
        (im, Some(ms))
    } else {
        (None, None)
    };

    let (ov_tests, ov_tests_ms) = if has_constant && residuals.len() >= 8 {
        let t_ov = Instant::now();
        let y_arr = endog.clone();
        let fitted_arr = Array1::from(fitted_values.clone());
        let r_default = diagnostics::reset_test(&y_arr, &exog, &fitted_arr, None).ok();
        let r_rhs = diagnostics::reset_test_rhs(&y_arr, &exog, None).ok();
        let ms = t_ov.elapsed().as_millis() as u64;
        (
            Some(OvTests {
                default: r_default.map(|r| OvTest {
                    f_stat: r.f_stat,
                    df1: r.df1,
                    df2: r.df2,
                    p_value: r.p_value,
                }),
                rhs: r_rhs.map(|r| OvTest {
                    f_stat: r.f_stat,
                    df1: r.df1,
                    df2: r.df2,
                    p_value: r.p_value,
                }),
            }),
            Some(ms),
        )
    } else {
        (None, None)
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

    let vif = diagnostics::vif_centered(&exog, has_constant).ok().and_then(|entries| {
        let vif_entries: Vec<VifEntry> = entries
            .into_iter()
            .enumerate()
            .filter(|(j, e)| !(has_constant && *j == 0) && !e.vif.is_nan())
            .map(|(j, e)| {
                let (var_name, _) = all_labels
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
    // ts_lag 内部已做 align，返回 (full_times, aligned, lagged)
    let residual_scatter = if time_series.is_some() && residuals.len() >= 2 {
        let time_col = df.column("__time__").map_err(|e| format!("OLS: {}", e))?;
        let time_s = time_col.clone().take_materialized_series();
        let residuals_s = Series::from_iter(residuals.iter().cloned()).with_name("residuals".into());

        let interval = infer_interval(&time_s).unwrap_or(1);
        match ts_lag(&time_s, &residuals_s, 1, interval) {
            Ok((full_times, e_aligned, e_lag1_series)) => {
                let e_vec: Vec<Option<f64>> = e_aligned
                    .f64()
                    .map_err(|e| format!("OLS: {}", e))?
                    .into_iter()
                    .collect();
                let e_lag1_vec: Vec<Option<f64>> = e_lag1_series
                    .f64()
                    .map_err(|e| format!("OLS: {}", e))?
                    .into_iter()
                    .collect();
                let time_str_s = full_times
                    .cast(&polars::prelude::DataType::String)
                    .map_err(|e| format!("OLS: time cast: {}", e))?;
                let time_str_vec: Vec<String> = time_str_s
                    .str()
                    .map_err(|e| format!("OLS: {}", e))?
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

    let ols_result = OLSResult {
        title: "OLS Regression Results".to_string(),
        endog_name: endog_name,
        model_basic_info: {
            let (aic, bic) = compute_aic_bic(
                result.num_observation,
                result.betas.len(),
                result.ss_residual,
            );
            ModelBasicInfo {
                model_type: "OLS".to_string(),
                method: "Least Squares".to_string(),
                num_observation: result.num_observation,
                r_squared: result.r2,
                adj_r_squared: result.r2_adjusted,
                f_statistic: result.fvalue,
                prob_f_statistic: result.f_p_value,
                wald_chi2: None,
                prob_wald_chi2: None,
                df_model: result.df_model,
                df_residual: result.df_residual,
                df_total: result.df_total,
                ss_model: result.ss_model,
                ss_residual: result.ss_residual,
                ss_total: result.ss_total,
                ms_model: result.ms_model,
                ms_residual: result.ms_residual,
                ms_total: result.ms_total,
                covariance_type: format_covariance_type_display(&result.covariance_type, config.cov_config.as_ref()),
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
            leverage: diagnostics::leverage(&exog).unwrap_or_default(),
            residual_scatter,
            exog: Some((0..n).map(|i| exog.row(i).iter().cloned().collect()).collect()),
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
        },
        betas: result.betas.to_vec(),
        cov_beta: (0..result.cov_beta.nrows())
            .map(|i| result.cov_beta.row(i).iter().cloned().collect())
            .collect(),
    };

    let ols_model = OLSModel {
        betas: result.betas.to_vec(),
        has_constant,
        variable_specs,
    };

    Ok(OLSFitResult { ols_result, ols_model })
}

// ======================== 注册入口 ========================

/// VCE pin 可接受的 OneOf 类型（拖动时自动筛选兼容节点）
fn vce_one_of_type() -> DataType {
    DataType::one_of(vec![
        DataType::Struct("VCENonRobust".to_string()),
        DataType::Struct("VCEHC0".to_string()),
        DataType::Struct("VCEHC1".to_string()),
        DataType::Struct("VCEHC2".to_string()),
        DataType::Struct("VCEHC3".to_string()),
        DataType::Struct("OLSFixedScaleConfig".to_string()),
        DataType::Struct("OLSClusterConfig".to_string()),
        DataType::Struct("OLSHACConfig".to_string()),
        DataType::Struct("OLSNeweyConfig".to_string()),
    ])
}

pub fn register(registry: &NodeRegistry) {
    register_ols_vce_constants(registry);
    register_ols_configure(registry);
    register_ols_fixed_scale_config(registry);
    register_ols_cluster_config(registry);
    register_ols_hac_config(registry);
    register_ols_newey_config(registry);
    register_ols(registry);
    register_ols_summary(registry);
}

// ======================== VCE 常量节点（简单类型） ========================

fn register_ols_vce_constants(registry: &NodeRegistry) {
    let vce_constants = [
        ("NonRobust", "VCENonRobust"),
        ("HC0", "VCEHC0"),
        ("HC1 (robust)", "VCEHC1"),
        ("HC2", "VCEHC2"),
        ("HC3", "VCEHC3"),
    ];
    for (name, struct_key) in vce_constants {
        let struct_key = struct_key.to_string();
        let definition = NodeDefinition::new(
            format!("VCE: {}", name),
            vec!["Data".to_string(), "Statistics".to_string()],
        )
        .with_ui_style("dataframe")
        .with_description(format!("VCE constant — {}", name))
        .with_pin_slots(vec![PinSlot::fixed(PinDefinition::data_output(
            "VCE",
            DataRole::Result,
            PinDataTypeDefinition::concrete(DataType::Struct(struct_key.clone())),
        ))])
        .with_data_evaluator(Arc::new(move |ctx| {
            let config: Box<dyn std::any::Any + Send + Sync> = match struct_key.as_str() {
                "VCENonRobust" => Box::new(VCENonRobust),
                "VCEHC0" => Box::new(VCEHC0),
                "VCEHC1" => Box::new(VCEHC1),
                "VCEHC2" => Box::new(VCEHC2),
                "VCEHC3" => Box::new(VCEHC3),
                _ => return Err("Unknown VCE type".to_string()),
            };
            let handle_id = ctx.put_handle(config);
            ctx.emit_output_by_role(
                &PinRole::Data(DataRole::Result),
                DataValue::new_struct(struct_key.clone(), handle_id),
            )?;
            Ok(())
        }));
        registry.register(definition);
    }
}

// ======================== OLS Fixed Scale Config 节点 ========================

fn register_ols_fixed_scale_config(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "OLS Fixed Scale Config",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Fixed scale covariance config — user-specified scale for cov_type 'fixed scale'")
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Scale",
            DataRole::Custom("scale".to_string()),
            PinDataTypeDefinition::concrete(DataType::Float64),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Config",
            DataRole::Result,
            PinDataTypeDefinition::concrete(DataType::Struct("OLSFixedScaleConfig".to_string())),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let scale_val = ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("scale".to_string())))?;
        let scale = scale_val
            .as_f64()
            .ok_or("OLS Fixed Scale Config: Scale must be Float64".to_string())?;
        if scale <= 0.0 {
            return Err("OLS Fixed Scale Config: Scale must be positive".to_string());
        }
        let config = OLSFixedScaleConfig { scale };
        let handle_id = ctx.put_handle(Box::new(config));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Result),
            DataValue::new_struct("OLSFixedScaleConfig", handle_id),
        )?;
        Ok(())
    }));
    registry.register(definition);
}

/// 将 DataSeries 转为 group 索引 Vec<usize>（0-based，按首次出现顺序编号）
fn series_to_group_indices(ctx: &mut dyn NodeExecutionContextTrait, series_id: &str) -> Result<Vec<usize>, String> {
    let series = ctx.get_series(series_id)?;
    let n = series.len();

    let mut indices = Vec::with_capacity(n);
    let mut value_to_idx: HashMap<String, usize> = HashMap::new();
    let mut next_idx = 0usize;

    if matches!(
        series.dtype(),
        polars::prelude::DataType::Categorical(_, _) | polars::prelude::DataType::Enum(_, _)
    ) {
        let str_series = series
            .cast(&polars::prelude::DataType::String)
            .map_err(|e| e.to_string())?;
        let ca = str_series.str().map_err(|e| e.to_string())?;
        for opt in ca.into_iter() {
            let s = opt.ok_or("Config: series contains null")?.to_string();
            let idx = *value_to_idx.entry(s).or_insert_with(|| {
                let i = next_idx;
                next_idx += 1;
                i
            });
            indices.push(idx);
        }
    } else if series.dtype() == &polars::prelude::DataType::Int64 {
        let ca = series.i64().map_err(|e| e.to_string())?;
        for opt in ca.into_iter() {
            let v = opt.ok_or("Config: series contains null")?;
            let s = v.to_string();
            let idx = *value_to_idx.entry(s).or_insert_with(|| {
                let i = next_idx;
                next_idx += 1;
                i
            });
            indices.push(idx);
        }
    } else {
        return Err("Config: Cluster/Entity/Time/Group ID must be Categorical or Int64 DataSeries".to_string());
    }

    Ok(indices)
}

// ======================== OLS Cluster Config 节点 ========================

fn register_ols_cluster_config(registry: &NodeRegistry) {
    let ds_type = DataType::DataSeries(Box::new(DataType::one_of(vec![
        DataType::Categorical,
        DataType::Int64,
    ])));
    let definition = NodeDefinition::new(
        "OLS Cluster Config",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Cluster-robust covariance config — connect Cluster ID (group labels) for cov_type 'cluster'")
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Cluster ID",
            DataRole::Input,
            PinDataTypeDefinition::concrete(ds_type),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Config",
            DataRole::Result,
            PinDataTypeDefinition::concrete(DataType::Struct("OLSClusterConfig".to_string())),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let ds_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Input))?;
        let series_id = match &ds_value {
            DataValue::DataSeries(v) => v.id.clone(),
            _ => return Err("OLS Cluster Config: Cluster ID must be a DataSeries".to_string()),
        };
        let cluster_id = series_to_group_indices(ctx, &series_id)?;
        let config = OLSClusterConfig { cluster_id };
        let handle_id = ctx.put_handle(Box::new(config));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Result),
            DataValue::new_struct("OLSClusterConfig", handle_id),
        )?;
        Ok(())
    }));
    registry.register(definition);
}

// ======================== OLS HAC Config 节点 ========================

fn register_ols_hac_config(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "OLS HAC Config",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("HAC (Heteroscedasticity and Autocorrelation Consistent) covariance config for cov_type 'HAC'")
    .with_pin_slots(vec![
        PinSlot::fixed(
            PinDefinition::data_input(
                "Kernel",
                DataRole::Custom("kernel".to_string()),
                PinDataTypeDefinition::concrete(DataType::String),
            )
            .with_optional(true)
            .with_metadata(true, "dropdown")
            .with_widget_options(vec![
                "Bartlett".to_string(),
                "Parzen".to_string(),
                "Quadratic Spectral".to_string(),
            ]),
        ),
        PinSlot::fixed(
            PinDefinition::data_input(
                "Bandwidth",
                DataRole::Custom("bandwidth".to_string()),
                PinDataTypeDefinition::concrete(DataType::Int64),
            )
            .with_optional(true),
        ),
        PinSlot::fixed(PinDefinition::data_output(
            "Config",
            DataRole::Result,
            PinDataTypeDefinition::concrete(DataType::Struct("OLSHACConfig".to_string())),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let kernel = ctx
            .get_input_by_role(&PinRole::Data(DataRole::Custom("kernel".to_string())))
            .ok()
            .and_then(|v| v.as_string().map(|s| s.to_string()))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Bartlett".to_string());
        let bandwidth = ctx
            .get_input_by_role(&PinRole::Data(DataRole::Custom("bandwidth".to_string())))
            .ok()
            .and_then(|v| v.as_i64());
        let config = OLSHACConfig { kernel, bandwidth };
        let handle_id = ctx.put_handle(Box::new(config));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Result),
            DataValue::new_struct("OLSHACConfig", handle_id),
        )?;
        Ok(())
    }));
    registry.register(definition);
}

// ======================== OLS Newey Config 节点 ========================

fn register_ols_newey_config(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "VCE: Newey",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Stata newey 风格 — Bartlett kernel + n/(n-k)，与 HAC (ivreg2) 不同")
    .with_pin_slots(vec![
        PinSlot::fixed(
            PinDefinition::data_input(
                "Lag",
                DataRole::Custom("lag".to_string()),
                PinDataTypeDefinition::concrete(DataType::Int64),
            )
            .with_optional(true),
        ),
        PinSlot::fixed(PinDefinition::data_output(
            "Config",
            DataRole::Result,
            PinDataTypeDefinition::concrete(DataType::Struct("OLSNeweyConfig".to_string())),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let lag = ctx
            .get_input_by_role(&PinRole::Data(DataRole::Custom("lag".to_string())))
            .ok()
            .and_then(|v| v.as_i64());
        let config = OLSNeweyConfig { lag };
        let handle_id = ctx.put_handle(Box::new(config));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Result),
            DataValue::new_struct("OLSNeweyConfig", handle_id),
        )?;
        Ok(())
    }));
    registry.register(definition);
}

// ======================== OLS & WLS Configure 节点 ========================

fn register_ols_configure(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "OLS & WLS Configure",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("OLS & WLS regression configuration — input pins compose the output Config")
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
                "VCE",
                DataRole::Custom("vce".to_string()),
                PinDataTypeDefinition::concrete(vce_one_of_type()),
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
            PinDataTypeDefinition::concrete(DataType::Struct("OLSConfigure".to_string())),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let constant = ctx
            .get_input_by_role(&PinRole::Data(DataRole::Custom("constant".to_string())))?
            .as_bool()
            .ok_or("OLS & WLS Configure: Constant must be a boolean")?;

        let (cov_type, cov_config) = ctx
            .get_input_by_role(&PinRole::Data(DataRole::Custom("vce".to_string())))
            .ok()
            .and_then(|v| v.as_handle_id().map(|s| s.to_string()))
            .and_then(|id| ctx.get_handle(&id).ok())
            .and_then(|h| {
                Some(if h.downcast_ref::<VCENonRobust>().is_some() {
                    ("nonrobust".to_string(), None)
                } else if h.downcast_ref::<VCEHC0>().is_some() {
                    ("HC0".to_string(), None)
                } else if h.downcast_ref::<VCEHC1>().is_some() {
                    ("HC1".to_string(), None)
                } else if h.downcast_ref::<VCEHC2>().is_some() {
                    ("HC2".to_string(), None)
                } else if h.downcast_ref::<VCEHC3>().is_some() {
                    ("HC3".to_string(), None)
                } else if let Some(c) = h.downcast_ref::<OLSFixedScaleConfig>() {
                    (
                        "fixed scale".to_string(),
                        Some(OLSCovarianceConfig::FixedScale { scale: c.scale }),
                    )
                } else if let Some(c) = h.downcast_ref::<OLSClusterConfig>() {
                    (
                        "cluster".to_string(),
                        Some(OLSCovarianceConfig::Cluster {
                            cluster_id: c.cluster_id.clone(),
                        }),
                    )
                } else if let Some(c) = h.downcast_ref::<OLSHACConfig>() {
                    (
                        "HAC".to_string(),
                        Some(OLSCovarianceConfig::HAC {
                            kernel: c.kernel.clone(),
                            bandwidth: c.bandwidth,
                        }),
                    )
                } else if let Some(c) = h.downcast_ref::<OLSNeweyConfig>() {
                    (
                        "newey".to_string(),
                        Some(OLSCovarianceConfig::Newey { lag: c.lag }),
                    )
                } else {
                    return None;
                })
            })
            .unwrap_or_else(|| ("nonrobust".to_string(), None));

        let time_series_id = ctx
            .get_input_by_role(&PinRole::Data(DataRole::Custom("time".to_string())))
            .ok()
            .and_then(|v| match &v {
                DataValue::DataSeries(dsv) => Some(dsv.id.clone()),
                _ => None,
            });

        let config = OLSConfigure {
            constant,
            cov_type,
            cov_config,
            time_series_id,
            small: true,
        };
        let handle_id = ctx.put_handle(Box::new(config));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Result),
            DataValue::new_struct("OLSConfigure", handle_id),
        )?;
        Ok(())
    }));
    registry.register(definition);
}

// ======================== OLS 节点 ========================

fn register_ols(registry: &NodeRegistry) {
    let mut slots = ols_input_slots();
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
        "OLS",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Ordinary Least Squares regression — outputs the fitted model for prediction")
    .with_pin_slots(slots)
    .with_flow_processor(Arc::new(|ctx| {
        let fit = run_ols_regression(ctx)?;

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

        ctx.log("OLS: regression completed".to_string());
        Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
    }));
    registry.register(definition);
}

// ======================== OLS Summary 节点 ========================

fn register_ols_summary(registry: &NodeRegistry) {
    let mut slots = ols_input_slots();
    slots.push(PinSlot::fixed(PinDefinition::data_output(
        "Result",
        DataRole::Result,
        PinDataTypeDefinition::concrete(DataType::Struct("OLSResult".to_string())),
    )));
    slots.push(PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)));

    let definition = NodeDefinition::new(
        "OLS Summary",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Ordinary Least Squares regression — outputs results and opens the summary window")
    .with_pin_slots(slots)
    .with_flow_processor(Arc::new(|ctx| {
        let fit = run_ols_regression(ctx)?;

        let json_data = serde_json::to_string(&fit.ols_result)
            .map_err(|e| format!("OLS Summary: failed to serialize: {}", e))?;

        let result_handle_id = ctx.put_handle(Box::new(fit.ols_result));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Result),
            DataValue::new_struct("OLSResult", result_handle_id),
        )?;

        ctx.open_window("ols_summary".to_string(), json_data);

        ctx.log("OLS Summary: regression completed".to_string());
        Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
    }));
    registry.register(definition);
}
