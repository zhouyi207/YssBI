//! IV:2SLS (Instrumental Variables Two-Stage Least Squares) 回归节点
//!
//! Stata ivregress 2sls: depvar [varlist1] (varlist2 = varlistiv)
//! - varlist1: exogenous (X:exogs, repeatable DataSeries)
//! - varlist2: endogenous (X:endog, DataFrame)
//! - varlistiv: instruments (x_instruments)
//!
//! Configure 与 OLS 一致：Constant, VCE, Time

use crate::execution::{ExecutionEffect, ReportKind};
use crate::execution::context::NodeExecutionContextTrait;
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::register::catalog::docs;
use crate::graph::value::{DataType, DataValue};
use ndarray::{Array1, Array2};
use polars::prelude::{Column, DataFrame, NamedFrom, Series};
use std::sync::Arc;
use yss_sci::regression::linear_model::{CovParams, IV2SLS, IV2SLSConfig};

use super::info_nodes::{
    Coefficient, Iv2slsEndogenousTest, Iv2slsFirstStageResult, Iv2slsFirstStageSummary,
    Iv2slsHausmanTest, Iv2slsOveridDims, Iv2slsOveridTest, Iv2slsStockYogoBiasRow,
    Iv2slsStockYogoCv, Iv2slsStockYogoSizeRow, ModelBasicInfo, OLSResult, compute_aic_bic,
};
use super::ols_nodes::{
    OLSClusterConfig, OLSConfigure, OLSCovarianceConfig, OLSFixedScaleConfig, OLSHACConfig,
    OLSNeweyConfig, VCEHC0, VCEHC1, VCEHC2, VCEHC3, VCENonRobust, format_covariance_type_display,
};

// ======================== 共享辅助函数 ========================

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

pub(crate) fn iv_2sls_input_slots() -> Vec<PinSlot> {
    let x_exog_type = DataType::DataSeries(Box::new(DataType::one_of(vec![
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
                PinDataTypeDefinition::concrete(x_exog_type),
            ),
            "X:exogs",
            1,
            None,
        ),
        PinSlot::fixed(PinDefinition::data_input(
            "X:endog",
            DataRole::Custom("x_endog".to_string()),
            PinDataTypeDefinition::concrete(DataType::DataFrame),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "x_instruments",
            DataRole::Custom("x_instruments".to_string()),
            PinDataTypeDefinition::concrete(DataType::DataFrame),
        )),
        PinSlot::fixed(
            PinDefinition::data_input(
                "Time",
                DataRole::Custom("time".to_string()),
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::one_of(
                    vec![DataType::Date, DataType::Int64],
                )))),
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

fn run_iv_2sls_regression(ctx: &mut dyn NodeExecutionContextTrait) -> Result<OLSResult, String> {
    // ---- Extract Y ----
    let endog_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("y".to_string())))?;
    let endog_id = match &endog_value {
        DataValue::DataSeries(v) => v.id.clone(),
        other => {
            let got = match other {
                DataValue::Null => "Null (unconnected or upstream not executed)",
                DataValue::DataFrame(_) => "DataFrame",
                DataValue::Struct { type_key, .. } => {
                    return Err(format!(
                        "IV:2SLS: Y input is not a DataSeries (got Struct<{}>).",
                        type_key
                    ));
                }
                _ => "other type",
            };
            return Err(format!("IV:2SLS: Y must be a DataSeries (got {}).", got));
        }
    };
    let endog_series = ctx.get_data_series(&endog_id)?;
    let endog_name = {
        let raw = endog_series.name().to_string();
        if raw.is_empty() { "y".to_string() } else { raw }
    };
    let endog_f64 = endog_series
        .cast(&polars::prelude::DataType::Float64)
        .map_err(|e| format!("IV:2SLS: cannot cast Y to Float64: {}", e))?;
    let n_raw = endog_f64.len();

    // ---- Get config (optional) ----
    // Stata ivregress default: small=false (no df adjustment, Wald/z). Override default when no config.
    let config =
        match ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("ols_config".to_string()))) {
            Ok(config_value) => match config_value.as_handle_id() {
                Some(id) => {
                    let handle = ctx.get_handle(&id.to_string())?;
                    handle
                        .downcast_ref::<OLSConfigure>()
                        .ok_or("IV:2SLS: config handle is not an OLSConfigure")?
                        .clone()
                }
                None => {
                    let mut c = OLSConfigure::default();
                    c.small = false;
                    c
                }
            },
            Err(_) => {
                let mut c = OLSConfigure::default();
                c.small = false;
                c
            }
        };
    let has_constant = config.constant;

    // ---- Extract X:endog (DataFrame, endogenous variables) ----
    let x_endog_value =
        ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("x_endog".to_string())))?;
    let x_endog_id = match &x_endog_value {
        DataValue::DataFrame(id) => id.clone(),
        DataValue::Null => {
            return Err("IV:2SLS: X:endog is not connected. Connect a DataFrame.".to_string());
        }
        _ => return Err("IV:2SLS: X:endog must be a DataFrame.".to_string()),
    };
    let x_endog_df = ctx.get_dataframe(&x_endog_id)?;
    let endog_df = x_endog_df.as_ref();
    if endog_df.height() != n_raw {
        return Err(format!(
            "IV:2SLS: X:endog has {} rows, expected {} (must match Y length)",
            endog_df.height(),
            n_raw
        ));
    }

    // ---- Extract x_instruments (DataFrame) ----
    let x_instruments_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Custom(
        "x_instruments".to_string(),
    )))?;
    let x_instruments_id = match &x_instruments_value {
        DataValue::DataFrame(id) => id.clone(),
        DataValue::Null => {
            return Err(
                "IV:2SLS: x_instruments is not connected. Connect a DataFrame.".to_string(),
            );
        }
        _ => return Err("IV:2SLS: x_instruments must be a DataFrame.".to_string()),
    };
    let x_instruments_df = ctx.get_dataframe(&x_instruments_id)?;
    let inst_df = x_instruments_df.as_ref();
    if inst_df.height() != n_raw {
        return Err(format!(
            "IV:2SLS: x_instruments has {} rows, expected {} (must match Y length)",
            inst_df.height(),
            n_raw
        ));
    }

    // ---- Extract X (repeatable DataSeries, exogenous variables) ----
    let x_exog_values = ctx.get_inputs_by_family(&PinRole::Data(DataRole::Inputs(0)))?;
    if x_exog_values.is_empty() {
        return Err("IV:2SLS: at least one X:exogs input is required".to_string());
    }

    // ---- Build combined DataFrame ----
    let mut df_cols: Vec<Column> = vec![
        Column::from(Series::new(
            "__idx__".into(),
            (0..n_raw).map(|i| i as u32).collect::<Vec<u32>>(),
        )),
        Column::from(endog_f64.with_name("__endog__".into())),
    ];

    // Add X (exogenous) series from repeatable inputs
    for (i, val) in x_exog_values.iter().enumerate() {
        let dsv = match val {
            DataValue::DataSeries(v) => v.clone(),
            _ => return Err(format!("IV:2SLS: X:exogs {} is not a DataSeries", i)),
        };
        let series = ctx.get_data_series(&dsv.id)?;
        let series_name = {
            let raw = series.name().to_string();
            if raw.is_empty() {
                format!("exog_{}", i + 1)
            } else {
                raw
            }
        };
        if series.len() != n_raw {
            return Err(format!(
                "IV:2SLS: X:exogs '{}' has {} obs, expected {}",
                series_name,
                series.len(),
                n_raw
            ));
        }
        let col_f64 = series
            .cast(&polars::prelude::DataType::Float64)
            .map_err(|e| {
                format!(
                    "IV:2SLS: cannot cast X:exogs '{}' to Float64: {}",
                    series_name, e
                )
            })?;
        df_cols.push(Column::from(
            col_f64.with_name(series_name.to_string().into()),
        ));
    }

    // Add x_instruments columns
    for col in inst_df.columns() {
        let name = col.name();
        if name == "__endog__" {
            continue;
        }
        let col_f64 = col.cast(&polars::prelude::DataType::Float64).map_err(|e| {
            format!(
                "IV:2SLS: x_instruments column '{}' must be numeric: {}",
                name, e
            )
        })?;
        df_cols.push(Column::from(
            col_f64.with_name(format!("__inst_{}", name).into()),
        ));
    }

    // Add X:endog columns from DataFrame
    for col in endog_df.columns() {
        let name = col.name();
        if name == "__endog__" {
            continue;
        }
        let col_f64 = col
            .cast(&polars::prelude::DataType::Float64)
            .map_err(|e| format!("IV:2SLS: X:endog column '{}' must be numeric: {}", name, e))?;
        df_cols.push(Column::from(
            col_f64.with_name(format!("__endog_{}", name).into()),
        ));
    }

    // Optional time
    let time_series =
        match ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("time".to_string()))) {
            Ok(DataValue::DataSeries(v)) => {
                let ts = ctx.get_data_series(&v.id)?;
                if ts.len() != n_raw {
                    return Err(format!(
                        "IV:2SLS: Time has {} obs, expected {}",
                        ts.len(),
                        n_raw
                    ));
                }
                Some(ts)
            }
            _ => {
                if let Some(ref id) = config.time_series_id {
                    let ts = ctx.get_data_series(id)?;
                    if ts.len() != n_raw {
                        return Err(format!(
                            "IV:2SLS: Time from config has {} obs, expected {}",
                            ts.len(),
                            n_raw
                        ));
                    }
                    Some(ts)
                } else {
                    None
                }
            }
        };
    if let Some(ref ts) = time_series {
        df_cols.push(Column::from(ts.clone().with_name("__time__".into())));
    }

    let df = DataFrame::new(n_raw, df_cols)
        .map_err(|e| format!("IV:2SLS: failed to build DataFrame: {}", e))?
        .drop_nulls::<&str>(None)
        .map_err(|e| format!("IV:2SLS: drop_nulls failed: {}", e))?;
    let n = df.height();
    if n == 0 {
        return Err("IV:2SLS: no valid observations after dropping null/NaN.".to_string());
    }

    // ---- Extract arrays ----
    let endog = Array1::from(
        df.column("__endog__")
            .map_err(|e| format!("IV:2SLS: {}", e))?
            .f64()
            .map_err(|e| format!("IV:2SLS: {}", e))?
            .into_no_null_iter()
            .collect::<Vec<f64>>(),
    );

    let mut exog_col_names: Vec<String> = Vec::new();
    let mut exog_cols: Vec<Vec<f64>> = Vec::new();
    for (i, val) in x_exog_values.iter().enumerate() {
        let dsv = match val {
            DataValue::DataSeries(v) => v.clone(),
            _ => continue,
        };
        let series = ctx.get_data_series(&dsv.id)?;
        let series_name = {
            let raw = series.name().to_string();
            if raw.is_empty() {
                format!("exog_{}", i + 1)
            } else {
                raw
            }
        };
        let col = df
            .column(&series_name)
            .map_err(|e| format!("IV:2SLS: {}", e))?;
        let vec: Vec<f64> = col
            .f64()
            .map_err(|e| format!("IV:2SLS: X:exogs '{}': {}", series_name, e))?
            .into_no_null_iter()
            .collect();
        exog_cols.push(vec);
        exog_col_names.push(series_name);
    }

    let inst_col_names: Vec<String> = inst_df
        .columns()
        .iter()
        .filter(|c| c.name() != "__endog__")
        .map(|c| c.name().to_string())
        .collect();
    let mut inst_cols: Vec<Vec<f64>> = Vec::new();
    for name in &inst_col_names {
        let col = df
            .column(&format!("__inst_{}", name))
            .map_err(|e| format!("IV:2SLS: {}", e))?;
        let vec: Vec<f64> = col
            .f64()
            .map_err(|e| format!("IV:2SLS: x_instruments '{}': {}", name, e))?
            .into_no_null_iter()
            .collect();
        inst_cols.push(vec);
    }

    let endog_col_names: Vec<String> = endog_df
        .columns()
        .iter()
        .filter(|c| c.name() != "__endog__")
        .map(|c| c.name().to_string())
        .collect();
    let mut endog_cols: Vec<Vec<f64>> = Vec::new();
    for name in &endog_col_names {
        let col = df
            .column(&format!("__endog_{}", name))
            .map_err(|e| format!("IV:2SLS: {}", e))?;
        let vec: Vec<f64> = col
            .f64()
            .map_err(|e| format!("IV:2SLS: X:endog '{}': {}", name, e))?
            .into_no_null_iter()
            .collect();
        endog_cols.push(vec);
    }

    let k_exog = exog_cols.len();
    let k_endog = endog_cols.len();
    let k_iv = inst_cols.len();

    if k_iv < k_endog {
        return Err(format!(
            "IV:2SLS: underidentified — {} instruments < {} endogenous. Need at least {} instruments.",
            k_iv, k_endog, k_endog
        ));
    }

    let exog = if k_exog > 0 {
        let mut raw = Vec::with_capacity(n * k_exog);
        for i in 0..n {
            for col in &exog_cols {
                raw.push(col[i]);
            }
        }
        Array2::from_shape_vec((n, k_exog), raw)
            .map_err(|e| format!("IV:2SLS: exog matrix: {}", e))?
    } else {
        Array2::zeros((n, 0))
    };

    let endog_reg = {
        let mut raw = Vec::with_capacity(n * k_endog);
        for i in 0..n {
            for col in &endog_cols {
                raw.push(col[i]);
            }
        }
        Array2::from_shape_vec((n, k_endog), raw)
            .map_err(|e| format!("IV:2SLS: endog_matrix: {}", e))?
    };

    let instruments = {
        let mut raw = Vec::with_capacity(n * k_iv);
        for i in 0..n {
            for col in &inst_cols {
                raw.push(col[i]);
            }
        }
        Array2::from_shape_vec((n, k_iv), raw)
            .map_err(|e| format!("IV:2SLS: instruments matrix: {}", e))?
    };

    let valid_indices: Vec<usize> = df
        .column("__idx__")
        .map_err(|e| format!("IV:2SLS: {}", e))?
        .u32()
        .map_err(|e| format!("IV:2SLS: {}", e))?
        .into_no_null_iter()
        .map(|i| i as usize)
        .collect();

    let cov_params = config.cov_config.as_ref().map(|c| match c {
        OLSCovarianceConfig::FixedScale { scale } => CovParams::FixedScale { scale: *scale },
        OLSCovarianceConfig::Cluster { cluster_id } => CovParams::Cluster {
            cluster_id: valid_indices.iter().map(|&i| cluster_id[i]).collect(),
            xtreg_fe_style: false,
        },
        OLSCovarianceConfig::HAC { kernel, bandwidth } => CovParams::HAC {
            kernel: kernel.clone(),
            bandwidth: *bandwidth,
        },
        OLSCovarianceConfig::Newey { lag } => CovParams::Newey { lag: *lag },
    });

    let sci_config = IV2SLSConfig {
        constant: has_constant,
        cov_type: config.cov_type.clone(),
        cov_params,
        small: config.small,
    };
    let z_var_names: Vec<String> = {
        let mut names = Vec::new();
        if has_constant {
            names.push("const".to_string());
        }
        names.extend(exog_col_names.clone());
        names.extend(inst_col_names.clone());
        names
    };

    let iv2sls = IV2SLS {
        endog: endog.clone(),
        exog,
        endog_reg,
        instruments,
        config: sci_config,
        endog_names: Some(endog_col_names.clone()),
        z_var_names: Some(z_var_names),
    };
    let result = iv2sls.fit()?;

    // ---- Build coefficient labels ----
    let mut all_labels: Vec<(String, Option<String>)> = Vec::new();
    if has_constant {
        all_labels.push(("const".to_string(), None));
    }
    for name in &exog_col_names {
        all_labels.push((name.clone(), None));
    }
    for name in &endog_col_names {
        all_labels.push((name.clone(), None));
    }

    let mut coefficients = Vec::new();
    for i in 0..result.betas.len() {
        let (var_name, category) = all_labels
            .get(i)
            .cloned()
            .unwrap_or_else(|| (format!("x{}", i), None));
        coefficients.push(Coefficient {
            variable: var_name,
            category,
            coef: result.betas[i],
            std_err: result.stds[i],
            t_value: result.zvalues[i],
            p_value: result.pvalues[i],
            ci_lower: result.conf_int_left[i],
            ci_upper: result.conf_int_right[i],
            is_significant: result.pvalues[i] < 0.05,
        });
    }

    let (aic, bic) = compute_aic_bic(
        result.num_observation,
        result.betas.len(),
        result.ss_residual,
    );

    let cov_beta_vec: Vec<Vec<f64>> = (0..result.cov_beta.nrows())
        .map(|i| {
            (0..result.cov_beta.ncols())
                .map(|j| result.cov_beta[[i, j]])
                .collect()
        })
        .collect();

    let iv2sls_first_stage: Vec<Iv2slsFirstStageResult> = result
        .first_stage
        .iter()
        .map(|fs| Iv2slsFirstStageResult {
            endog_name: fs.endog_name.clone(),
            var_names: fs.var_names.clone(),
            coefficients: (0..fs.betas.len())
                .map(|i| {
                    let var_name = fs
                        .var_names
                        .get(i)
                        .cloned()
                        .unwrap_or_else(|| format!("z{}", i + 1));
                    Coefficient {
                        variable: var_name,
                        category: None,
                        coef: fs.betas[i],
                        std_err: fs.stds[i],
                        t_value: fs.tvalues[i],
                        p_value: fs.pvalues[i],
                        ci_lower: fs.conf_int_left[i],
                        ci_upper: fs.conf_int_right[i],
                        is_significant: fs.pvalues[i] < 0.05,
                    }
                })
                .collect(),
            r_squared: fs.r2,
            adj_r_squared: fs.r2_adjusted,
        })
        .collect();

    Ok(OLSResult {
        title: "IV:2SLS Regression Results".to_string(),
        endog_name,
        model_basic_info: ModelBasicInfo {
            model_type: "IV:2SLS".to_string(),
            method: "Two-Stage Least Squares".to_string(),
            num_observation: result.num_observation,
            r_squared: result.r2,
            adj_r_squared: result.r2_adjusted,
            f_statistic: result.wald_chi2,
            prob_f_statistic: result.wald_chi2_p_value,
            wald_chi2: Some(result.wald_chi2),
            prob_wald_chi2: Some(result.wald_chi2_p_value),
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
            covariance_type: format_covariance_type_display(
                &result.covariance_type,
                config.cov_config.as_ref(),
            ),
            aic,
            bic,
        },
        coefficients,
        diagnostic_info: super::info_nodes::DiagnosticInfo {
            cond_no: result.cond_no,
            vif: None,
            bp_tests: None,
            ov_tests: None,
            im_test: None,
            normality_tests: None,
            fitted_values: vec![],
            residuals: vec![],
            leverage: vec![],
            residual_scatter: None,
            exog: None,
            timing: None,
            prais_info: None,
            iv2sls_first_stage: Some(iv2sls_first_stage),
            iv2sls_first_stage_summary: Some(Iv2slsFirstStageSummary {
                k_included_instruments: result.first_stage_summary.k_included_instruments,
                k_excluded_instruments: result.first_stage_summary.k_excluded_instruments,
                k_endogenous_regressors: result.first_stage_summary.k_endogenous_regressors,
                r2: result.first_stage_summary.r2,
                r2_adjusted: result.first_stage_summary.r2_adjusted,
                partial_r2: result.first_stage_summary.partial_r2,
                f_stat: result.first_stage_summary.f_stat,
                f_p_value: result.first_stage_summary.f_p_value,
                f_df1: result.first_stage_summary.f_df1,
                f_df2: result.first_stage_summary.f_df2,
                shea_partial_r2: result.first_stage_summary.shea_partial_r2.clone(),
                shea_adj_partial_r2: result.first_stage_summary.shea_adj_partial_r2.clone(),
                min_eigenvalue: result.first_stage_summary.min_eigenvalue,
                min_eigenvalue_cv_note: result.first_stage_summary.min_eigenvalue_cv_note.clone(),
                min_eigenvalue_cv: result.first_stage_summary.min_eigenvalue_cv.as_ref().map(
                    |cv| Iv2slsStockYogoCv {
                        bias: cv.bias.as_ref().map(|b| Iv2slsStockYogoBiasRow {
                            pct_5: b.pct_5,
                            pct_10: b.pct_10,
                            pct_20: b.pct_20,
                            pct_30: b.pct_30,
                        }),
                        size: Iv2slsStockYogoSizeRow {
                            pct_10: cv.size.pct_10,
                            pct_15: cv.size.pct_15,
                            pct_20: cv.size.pct_20,
                            pct_25: cv.size.pct_25,
                        },
                    },
                ),
            }),
            iv2sls_overid: result.overid.as_ref().map(|o| Iv2slsOveridTest {
                test_type: o.test_type.clone(),
                sargan_stat: o.sargan_stat,
                sargan_p_value: o.sargan_p_value,
                basmann_stat: o.basmann_stat,
                basmann_p_value: o.basmann_p_value,
                wooldridge_stat: o.wooldridge_stat,
                wooldridge_p_value: o.wooldridge_p_value,
                df: o.df,
            }),
            iv2sls_overid_dims: Some(Iv2slsOveridDims {
                k_iv: result.overid_k_iv,
                k_endog: result.overid_k_endog,
            }),
            iv2sls_hausman: result.hausman.as_ref().map(|h| Iv2slsHausmanTest {
                stat: h.stat,
                p_value: h.p_value,
                df: h.df,
            }),
            iv2sls_endogenous: result.endogenous.as_ref().map(|e| Iv2slsEndogenousTest {
                durbin_stat: e.durbin_stat,
                durbin_p_value: e.durbin_p_value,
                wu_stat: e.wu_stat,
                wu_p_value: e.wu_p_value,
                df: e.df,
                wu_df_denom: e.wu_df_denom,
            }),
            ivliml_kappa: None,
            ivliml_overid: None,
            classification_table: None,
            exog_means: None,
            panel_fe_info: None,
            omit_info: None,
        },
        betas: result.betas.to_vec(),
        cov_beta: cov_beta_vec,
        cov_beta_nonrobust: None,
    })
}

// ======================== 注册入口 ========================

pub fn register(registry: &NodeRegistry) {
    register_iv_2sls_configure(registry);
    register_iv_2sls_summary(registry);
    super::iv_liml_nodes::register(registry);
}

// ======================== IV:2SLS Configure 节点 ========================

fn register_iv_2sls_configure(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "IV:2SLS Configure",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
        .with_documentation(docs::iv::IV_2SLS_CONFIGURE_ZH, docs::iv::IV_2SLS_CONFIGURE_EN)
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
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::one_of(
                    vec![DataType::Int64, DataType::Date],
                )))),
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
            .get_input_by_role(&PinRole::Data(DataRole::Custom("constant".to_string())))
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

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

        // Stata ivregress default: no small-sample adjustment (small=false), report Wald/z.
        // small=true would use ESS/(n-k) and F/t; we match Stata default.
        let config = OLSConfigure {
            constant,
            cov_type,
            cov_config,
            time_series_id,
            small: false,
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

// ======================== IV:2SLS Summary 节点 ========================

fn register_iv_2sls_summary(registry: &NodeRegistry) {
    let mut slots = iv_2sls_input_slots();
    slots.push(PinSlot::fixed(PinDefinition::data_output(
        "Result",
        DataRole::Result,
        PinDataTypeDefinition::concrete(DataType::Struct("OLSResult".to_string())),
    )));
    slots.push(PinSlot::fixed(PinDefinition::exec_output(
        "Out",
        ExecRole::ExecOut,
    )));

    let definition = NodeDefinition::new(
        "IV:2SLS Summary",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
        .with_documentation(docs::iv::IV_2SLS_SUMMARY_ZH, docs::iv::IV_2SLS_SUMMARY_EN)
    .with_pin_slots(slots)
    .with_flow_processor(Arc::new(|ctx| {
        let fit = run_iv_2sls_regression(ctx)?;

        let json_data = serde_json::to_string(&fit)
            .map_err(|e| format!("IV:2SLS Summary: failed to serialize: {}", e))?;

        let result_handle_id = ctx.put_handle(Box::new(fit));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Result),
            DataValue::new_struct("OLSResult", result_handle_id),
        )?;

        ctx.publish_report(ReportKind::Iv2slsSummary, json_data);

        ctx.log("IV:2SLS Summary: regression completed".to_string());
        Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
    }));
    registry.register(definition);
}
