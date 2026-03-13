//! IV:LIML (Limited Information Maximum Likelihood) 回归节点
//!
//! Stata ivregress liml: depvar [varlist1] (varlist2 = varlistiv)
//! 与 IV:2SLS 相同的输入结构，共享数据提取逻辑

use crate::execution::ExecutionEffect;
use crate::execution::context::NodeExecutionContextTrait;
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{DataValue, DataType};
use ndarray::{Array1, Array2};
use polars::prelude::{Column, DataFrame, NamedFrom, Series};
use std::sync::Arc;
use yss_sci::regression::linear_model::{CovParams, IVLIML, IVLIMLConfig};

use super::info_nodes::{
    Coefficient, Iv2slsFirstStageResult, Iv2slsFirstStageSummary, Iv2slsOveridDims, Iv2slsStockYogoBiasRow,
    Iv2slsStockYogoCv, Iv2slsStockYogoSizeRow, IvLimlOveridTest, ModelBasicInfo, OLSResult,
};
use super::iv_2sls_nodes::iv_2sls_input_slots;
use super::ols_nodes::{format_covariance_type_display, OLSConfigure, OLSCovarianceConfig};

/// 提取的 IV 数据（2SLS 与 LIML 共用）
pub fn extract_iv_data(
    ctx: &mut dyn NodeExecutionContextTrait,
) -> Result<
    (
        Array1<f64>,
        Array2<f64>,
        Array2<f64>,
        Array2<f64>,
        OLSConfigure,
        String,
        Vec<String>,
        Vec<String>,
        Vec<String>,
        Vec<String>,
        usize,
        bool,
        Option<CovParams>,
    ),
    String,
> {
    let endog_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("y".to_string())))?;
    let endog_id = match &endog_value {
        DataValue::DataSeries(v) => v.id.clone(),
        other => {
            let got = match other {
                DataValue::Null => "Null",
                DataValue::DataFrame(_) => "DataFrame",
                DataValue::Struct { type_key, .. } => {
                    return Err(format!("IV: Y input is not a DataSeries (got Struct<{}>).", type_key));
                }
                _ => "other type",
            };
            return Err(format!("IV: Y must be a DataSeries (got {}).", got));
        }
    };
    let endog_series = ctx.get_series(&endog_id)?;
    let endog_name = {
        let raw = endog_series.name().to_string();
        if raw.is_empty() { "y".to_string() } else { raw }
    };
    let endog_f64 = endog_series
        .cast(&polars::prelude::DataType::Float64)
        .map_err(|e| format!("IV: cannot cast Y to Float64: {}", e))?;
    let n_raw = endog_f64.len();

    let config = match ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("ols_config".to_string()))) {
        Ok(config_value) => match config_value.as_handle_id() {
            Some(id) => {
                let handle = ctx.get_handle(&id.to_string())?;
                handle
                    .downcast_ref::<OLSConfigure>()
                    .ok_or("IV: config handle is not an OLSConfigure")?
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

    let x_endog_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("x_endog".to_string())))?;
    let x_endog_id = match &x_endog_value {
        DataValue::DataFrame(id) => id.clone(),
        DataValue::Null => return Err("IV: X:endog is not connected.".to_string()),
        _ => return Err("IV: X:endog must be a DataFrame.".to_string()),
    };
    let x_endog_df = ctx.get_dataframe(&x_endog_id)?;
    let endog_df = x_endog_df.as_ref();
    if endog_df.height() != n_raw {
        return Err(format!("IV: X:endog has {} rows, expected {}", endog_df.height(), n_raw));
    }

    let x_instruments_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("x_instruments".to_string())))?;
    let x_instruments_id = match &x_instruments_value {
        DataValue::DataFrame(id) => id.clone(),
        DataValue::Null => return Err("IV: x_instruments is not connected.".to_string()),
        _ => return Err("IV: x_instruments must be a DataFrame.".to_string()),
    };
    let x_instruments_df = ctx.get_dataframe(&x_instruments_id)?;
    let inst_df = x_instruments_df.as_ref();
    if inst_df.height() != n_raw {
        return Err(format!("IV: x_instruments has {} rows, expected {}", inst_df.height(), n_raw));
    }

    let x_exog_values = ctx.get_inputs_by_family(&PinRole::Data(DataRole::Inputs(0)))?;
    if x_exog_values.is_empty() {
        return Err("IV: at least one X:exogs input is required".to_string());
    }

    let mut df_cols: Vec<Column> = vec![
        Column::from(Series::new("__idx__".into(), (0..n_raw).map(|i| i as u32).collect::<Vec<u32>>())),
        Column::from(endog_f64.with_name("__endog__".into())),
    ];

    for (i, val) in x_exog_values.iter().enumerate() {
        let dsv = match val {
            DataValue::DataSeries(v) => v.clone(),
            _ => return Err(format!("IV: X:exogs {} is not a DataSeries", i)),
        };
        let series = ctx.get_series(&dsv.id)?;
        let series_name = if series.name().is_empty() { format!("exog_{}", i + 1) } else { series.name().to_string() };
        if series.len() != n_raw {
            return Err(format!("IV: X:exogs '{}' has {} obs, expected {}", series_name, series.len(), n_raw));
        }
        let col_f64 = series
            .cast(&polars::prelude::DataType::Float64)
            .map_err(|e| format!("IV: cannot cast X:exogs '{}': {}", series_name, e))?;
        df_cols.push(Column::from(col_f64.with_name(series_name.to_string().into())));
    }

    for col in inst_df.columns() {
        let name = col.name();
        if name == "__endog__" { continue; }
        let col_f64 = col
            .cast(&polars::prelude::DataType::Float64)
            .map_err(|e| format!("IV: x_instruments '{}': {}", name, e))?;
        df_cols.push(Column::from(col_f64.with_name(format!("__inst_{}", name).into())));
    }

    for col in endog_df.columns() {
        let name = col.name();
        if name == "__endog__" { continue; }
        let col_f64 = col
            .cast(&polars::prelude::DataType::Float64)
            .map_err(|e| format!("IV: X:endog '{}': {}", name, e))?;
        df_cols.push(Column::from(col_f64.with_name(format!("__endog_{}", name).into())));
    }

    let time_series = match ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("time".to_string()))) {
        Ok(DataValue::DataSeries(v)) => {
            let ts = ctx.get_series(&v.id)?;
            if ts.len() != n_raw {
                return Err(format!("IV: Time has {} obs, expected {}", ts.len(), n_raw));
            }
            Some(ts)
        }
        _ => config.time_series_id.as_ref().and_then(|id| {
            let ts = ctx.get_series(id).ok()?;
            if ts.len() != n_raw { return None; }
            Some(ts)
        }),
    };
    if let Some(ref ts) = time_series {
        df_cols.push(Column::from(ts.clone().with_name("__time__".into())));
    }

    let df = DataFrame::new(n_raw, df_cols)
        .map_err(|e| format!("IV: failed to build DataFrame: {}", e))?
        .drop_nulls::<&str>(None)
        .map_err(|e| format!("IV: drop_nulls failed: {}", e))?;
    let n = df.height();
    if n == 0 {
        return Err("IV: no valid observations after dropping null/NaN.".to_string());
    }

    let endog = Array1::from(
        df.column("__endog__").map_err(|e| format!("IV: {}", e))?
            .f64().map_err(|e| format!("IV: {}", e))?
            .into_no_null_iter().collect::<Vec<f64>>(),
    );

    let mut exog_col_names = Vec::new();
    let mut exog_cols = Vec::new();
    for (i, val) in x_exog_values.iter().enumerate() {
        let dsv = match val { DataValue::DataSeries(v) => v.clone(), _ => continue };
        let series = ctx.get_series(&dsv.id)?;
        let series_name = if series.name().is_empty() { format!("exog_{}", i + 1) } else { series.name().to_string() };
        let col = df.column(&series_name).map_err(|e| format!("IV: {}", e))?;
        let vec: Vec<f64> = col.f64().map_err(|e| format!("IV: {}", e))?.into_no_null_iter().collect();
        exog_cols.push(vec);
        exog_col_names.push(series_name);
    }

    let inst_col_names: Vec<String> = inst_df.columns().iter()
        .filter(|c| c.name() != "__endog__")
        .map(|c| c.name().to_string())
        .collect();
    let mut inst_cols = Vec::new();
    for name in &inst_col_names {
        let col = df.column(&format!("__inst_{}", name)).map_err(|e| format!("IV: {}", e))?;
        let vec: Vec<f64> = col.f64().map_err(|e| format!("IV: {}", e))?.into_no_null_iter().collect();
        inst_cols.push(vec);
    }

    let endog_col_names: Vec<String> = endog_df.columns().iter()
        .filter(|c| c.name() != "__endog__")
        .map(|c| c.name().to_string())
        .collect();
    let mut endog_cols = Vec::new();
    for name in &endog_col_names {
        let col = df.column(&format!("__endog_{}", name)).map_err(|e| format!("IV: {}", e))?;
        let vec: Vec<f64> = col.f64().map_err(|e| format!("IV: {}", e))?.into_no_null_iter().collect();
        endog_cols.push(vec);
    }

    let k_exog = exog_cols.len();
    let k_endog = endog_cols.len();
    let k_iv = inst_cols.len();
    if k_iv < k_endog {
        return Err(format!("IV: underidentified — {} instruments < {} endogenous.", k_iv, k_endog));
    }

    let exog = if k_exog > 0 {
        let mut raw = Vec::with_capacity(n * k_exog);
        for i in 0..n {
            for col in &exog_cols { raw.push(col[i]); }
        }
        Array2::from_shape_vec((n, k_exog), raw).map_err(|e| format!("IV: exog: {}", e))?
    } else {
        Array2::zeros((n, 0))
    };

    let endog_reg = {
        let mut raw = Vec::with_capacity(n * k_endog);
        for i in 0..n {
            for col in &endog_cols { raw.push(col[i]); }
        }
        Array2::from_shape_vec((n, k_endog), raw).map_err(|e| format!("IV: endog: {}", e))?
    };

    let instruments = {
        let mut raw = Vec::with_capacity(n * k_iv);
        for i in 0..n {
            for col in &inst_cols { raw.push(col[i]); }
        }
        Array2::from_shape_vec((n, k_iv), raw).map_err(|e| format!("IV: instruments: {}", e))?
    };

    let valid_indices: Vec<usize> = df.column("__idx__").map_err(|e| format!("IV: {}", e))?
        .u32().map_err(|e| format!("IV: {}", e))?
        .into_no_null_iter().map(|i| i as usize).collect();

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

    let z_var_names: Vec<String> = {
        let mut names = Vec::new();
        if has_constant { names.push("const".to_string()); }
        names.extend(exog_col_names.clone());
        names.extend(inst_col_names.clone());
        names
    };

    Ok((
        endog,
        exog,
        endog_reg,
        instruments,
        config,
        endog_name,
        endog_col_names,
        exog_col_names,
        inst_col_names,
        z_var_names,
        n,
        has_constant,
        cov_params,
    ))
}

fn run_iv_liml_regression(ctx: &mut dyn NodeExecutionContextTrait) -> Result<OLSResult, String> {
    let (endog, exog, endog_reg, instruments, config, endog_name, endog_col_names, exog_col_names, _inst_col_names, z_var_names, _n, has_constant, cov_params) =
        extract_iv_data(ctx)?;

    let sci_config = IVLIMLConfig {
        constant: has_constant,
        cov_type: config.cov_type.clone(),
        cov_params,
        small: config.small,
    };

    let ivliml = IVLIML {
        endog,
        exog,
        endog_reg,
        instruments,
        config: sci_config,
        endog_names: Some(endog_col_names.clone()),
        z_var_names: Some(z_var_names),
    };
    let result = ivliml.fit()?;

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

    let (aic, bic) = super::info_nodes::compute_aic_bic(
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
                    let var_name = fs.var_names.get(i).cloned().unwrap_or_else(|| format!("z{}", i + 1));
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
        title: "IV:LIML Regression Results".to_string(),
        endog_name,
        model_basic_info: ModelBasicInfo {
            model_type: "IV:LIML".to_string(),
            method: "Limited Information Maximum Likelihood".to_string(),
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
                min_eigenvalue_cv: result.first_stage_summary.min_eigenvalue_cv.as_ref().map(|cv| Iv2slsStockYogoCv {
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
                }),
            }),
            iv2sls_overid: None,
            iv2sls_overid_dims: Some(Iv2slsOveridDims {
                k_iv: result.overid_k_iv,
                k_endog: result.overid_k_endog,
            }),
            iv2sls_hausman: None,
            iv2sls_endogenous: None,
            ivliml_kappa: Some(result.kappa),
            ivliml_overid: result.overid.as_ref().map(|o| IvLimlOveridTest {
                anderson_rubin_stat: o.anderson_rubin_stat,
                anderson_rubin_p_value: o.anderson_rubin_p_value,
                basmann_stat: o.basmann_stat,
                basmann_p_value: o.basmann_p_value,
                df: o.df,
                df_denom: o.df_denom,
            }),
            classification_table: None,
            exog_means: None,
            panel_fe_info: None,
            omit_info: None,
        },
        betas: result.betas.to_vec(),
        cov_beta: cov_beta_vec,
    })
}

pub fn register(registry: &NodeRegistry) {
    let mut slots = iv_2sls_input_slots();
    slots.push(PinSlot::fixed(PinDefinition::data_output(
        "Result",
        DataRole::Result,
        PinDataTypeDefinition::concrete(DataType::Struct("OLSResult".to_string())),
    )));
    slots.push(PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)));

    let definition = NodeDefinition::new(
        "IV:LIML Summary",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Instrumental Variables LIML regression — outputs results and opens the summary window")
    .with_pin_slots(slots)
    .with_flow_processor(Arc::new(|ctx| {
        let fit = run_iv_liml_regression(ctx)?;
        let json_data = serde_json::to_string(&fit)
            .map_err(|e| format!("IV:LIML Summary: failed to serialize: {}", e))?;
        let result_handle_id = ctx.put_handle(Box::new(fit));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Result),
            crate::graph::value::DataValue::new_struct("OLSResult", result_handle_id),
        )?;
        ctx.open_window("ols_summary".to_string(), json_data);
        ctx.log("IV:LIML Summary: regression completed".to_string());
        Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
    }));
    registry.register(definition);
}
