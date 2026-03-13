//! Panel Summary node — runs FE, FD, RE and displays all results

use crate::execution::ExecutionEffect;
use crate::execution::context::NodeExecutionContextTrait;
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{DataSeriesValue, DataType, DataValue};
use ndarray::{Array1, Array2};
use polars::prelude::{Column, DataFrame};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use yss_sci::regression::panel::{fit_panel_fe, fit_panel_fe_time, fit_panel_fe_twoway, fit_panel_fd, fit_panel_lsdv, fit_panel_lsdv_time, fit_panel_re};

use super::info_nodes::{compute_aic_bic, Coefficient, DiagnosticInfo, ModelBasicInfo, OLSResult, PanelFEInfo};

// ======================== 结构体 ========================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelConfigure {
    pub constant: bool,
    pub cov_type: String,
}

/// VCE constant for Panel: cluster by entity (uses Entity ID from Panel Summary)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelVCECluster;

fn panel_vce_one_of_type() -> DataType {
    DataType::one_of(vec![
        DataType::Struct("VCENonRobust".to_string()),
        DataType::Struct("VCEHC0".to_string()),
        DataType::Struct("VCEHC1".to_string()),
        DataType::Struct("VCEHC2".to_string()),
        DataType::Struct("VCEHC3".to_string()),
        DataType::Struct("PanelVCECluster".to_string()),
    ])
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelSummaryResult {
    pub title: String,
    pub endog_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fe: Option<OLSResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fe_time: Option<OLSResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fe_twoway: Option<OLSResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lsdv: Option<OLSResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lsdv_time: Option<OLSResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fd: Option<OLSResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub re: Option<OLSResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<PanelErrors>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelErrors {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fe: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fe_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fe_twoway: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lsdv: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lsdv_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub re: Option<String>,
}

// ======================== 辅助函数 ========================

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
            let s = opt.ok_or("Panel: series contains null")?.to_string();
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
            let v = opt.ok_or("Panel: series contains null")?;
            let s = v.to_string();
            let idx = *value_to_idx.entry(s).or_insert_with(|| {
                let i = next_idx;
                next_idx += 1;
                i
            });
            indices.push(idx);
        }
    } else {
        return Err("Panel: Entity ID and Time ID must be Categorical or Int64 DataSeries".to_string());
    }
    Ok(indices)
}

fn panel_input_slots() -> Vec<PinSlot> {
    let exog_type = DataType::DataSeries(Box::new(DataType::one_of(vec![
        DataType::Float64,
        DataType::Categorical,
    ])));
    let id_type = DataType::DataSeries(Box::new(DataType::one_of(vec![
        DataType::Categorical,
        DataType::Int64,
    ])));

    vec![
        PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
        PinSlot::fixed(PinDefinition::data_input(
            "Y",
            DataRole::Custom("y".to_string()),
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::one_of(vec![
                DataType::Float64,
                DataType::Int64,
            ])))),
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
            "Entity ID",
            DataRole::Custom("entity_id".to_string()),
            PinDataTypeDefinition::concrete(id_type.clone()),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "Time ID",
            DataRole::Custom("time_id".to_string()),
            PinDataTypeDefinition::concrete(id_type),
        )),
        PinSlot::fixed(
            PinDefinition::data_input(
                "Config",
                DataRole::Custom("panel_config".to_string()),
                PinDataTypeDefinition::concrete(DataType::Struct("PanelConfigure".to_string())),
            )
            .with_optional(true),
        ),
    ]
}

fn build_panel_data(ctx: &mut dyn NodeExecutionContextTrait, constant: bool) -> Result<
    (
        Array1<f64>,
        Array2<f64>,
        Vec<usize>,
        Vec<usize>,
        Vec<i64>,
        Vec<(String, Option<String>)>,
        String,
        bool,
        Vec<String>,
        String,
        Vec<String>,
        String,
    ),
    String,
> {
    let endog_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("y".to_string())))?;
    let endog_id = match &endog_value {
        DataValue::DataSeries(v) => v.id.clone(),
        _ => return Err("Panel: Y must be a DataSeries".to_string()),
    };
    let endog_series = ctx.get_series(&endog_id)?;
    let endog_name = {
        let raw = endog_series.name().to_string();
        if raw.is_empty() { "y".to_string() } else { raw }
    };
    let endog_f64 = endog_series
        .cast(&polars::prelude::DataType::Float64)
        .map_err(|e| format!("Panel: Y cast: {}", e))?;

    let entity_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("entity_id".to_string())))?;
    let entity_id_str = match &entity_value {
        DataValue::DataSeries(v) => v.id.clone(),
        _ => return Err("Panel: Entity ID must be a DataSeries".to_string()),
    };
    let entity_id = series_to_group_indices(ctx, &entity_id_str)?;

    let time_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("time_id".to_string())))?;
    let time_id_str = match &time_value {
        DataValue::DataSeries(v) => v.id.clone(),
        _ => return Err("Panel: Time ID must be a DataSeries".to_string()),
    };
    let time_id = series_to_group_indices(ctx, &time_id_str)?;

    let n_raw = endog_f64.len();
    if entity_id.len() != n_raw || time_id.len() != n_raw {
        return Err(format!(
            "Panel: Entity ID ({}), Time ID ({}), Y ({}) must have same length",
            entity_id.len(),
            time_id.len(),
            n_raw
        ));
    }

    let exog_values = ctx.get_inputs_by_family(&PinRole::Data(DataRole::Inputs(0)))?;
    if exog_values.is_empty() {
        return Err("Panel: at least one X input is required".to_string());
    }

    let mut df_cols: Vec<Column> = vec![Column::from(endog_f64.with_name("__endog__".into()))];
    let mut exog_meta: Vec<(String, bool, DataSeriesValue)> = Vec::new();
    for (i, val) in exog_values.iter().enumerate() {
        let dsv = match val {
            DataValue::DataSeries(v) => v.clone(),
            _ => return Err(format!("Panel: X {} is not a DataSeries", i)),
        };
        let series = ctx.get_series(&dsv.id)?;
        let name = {
            let raw = series.name().to_string();
            if raw.is_empty() { format!("x{}", i + 1) } else { raw }
        };
        if series.len() != n_raw {
            return Err(format!("Panel: X '{}' len {} != Y len {}", name, series.len(), n_raw));
        }
        let is_cat = matches!(
            series.dtype(),
            polars::prelude::DataType::Categorical(_, _) | polars::prelude::DataType::Enum(_, _)
        );
        let col_series = if is_cat {
            series.cast(&polars::prelude::DataType::String).map_err(|e| e.to_string())?
        } else {
            series.cast(&polars::prelude::DataType::Float64).map_err(|e| e.to_string())?
        };
        df_cols.push(Column::from(col_series.with_name(name.as_str().into())));
        exog_meta.push((name, is_cat, dsv));
    }

    let entity_series = ctx.get_series(&entity_id_str)?;
    let entity_series_name = {
        let raw = entity_series.name().to_string();
        if raw.is_empty() { "Entity ID".to_string() } else { raw }
    };
    let time_series = ctx.get_series(&time_id_str)?;
    let time_series_name = {
        let raw = time_series.name().to_string();
        if raw.is_empty() { "Time ID".to_string() } else { raw }
    };
    df_cols.push(Column::from(entity_series.with_name("__entity__".into())));
    df_cols.push(Column::from(time_series.with_name("__time__".into())));

    let df = DataFrame::new(n_raw, df_cols)
        .map_err(|e| format!("Panel: {}", e))?
        .drop_nulls::<&str>(None)
        .map_err(|e| format!("Panel: drop_nulls: {}", e))?
        .sort(["__entity__", "__time__"], Default::default())
        .map_err(|e| format!("Panel: sort: {}", e))?;

    let n = df.height();
    if n == 0 {
        return Err("Panel: no valid observations after dropping nulls".to_string());
    }

    let endog_vec: Vec<f64> = df
        .column("__endog__")
        .map_err(|e| format!("Panel: {}", e))?
        .f64()
        .map_err(|e| format!("Panel: {}", e))?
        .into_no_null_iter()
        .collect();

    let (entity_after, entity_names): (Vec<usize>, Vec<String>) = {
        let s = df.column("__entity__").map_err(|e| format!("Panel: {}", e))?;
        let mut m: HashMap<String, usize> = HashMap::new();
        let mut idx_to_name: Vec<String> = Vec::new();
        let mut out = Vec::with_capacity(n);
        if matches!(s.dtype(), polars::prelude::DataType::Categorical(_, _) | polars::prelude::DataType::Enum(_, _)) {
            let str_s = s.cast(&polars::prelude::DataType::String).map_err(|e: polars::error::PolarsError| e.to_string())?;
            let ca = str_s.str().map_err(|e: polars::error::PolarsError| e.to_string())?;
            for opt in ca.into_iter() {
                let key: String = opt.ok_or("null")?.to_string();
                let idx = *m.entry(key.clone()).or_insert_with(|| {
                    let i = idx_to_name.len();
                    idx_to_name.push(key);
                    i
                });
                out.push(idx);
            }
        } else {
            let ca = s.i64().map_err(|e: polars::error::PolarsError| e.to_string())?;
            for opt in ca.into_iter() {
                let key: String = opt.ok_or("null")?.to_string();
                let idx = *m.entry(key.clone()).or_insert_with(|| {
                    let i = idx_to_name.len();
                    idx_to_name.push(key);
                    i
                });
                out.push(idx);
            }
        }
        (out, idx_to_name)
    };

    let (time_after, time_values, time_names): (Vec<usize>, Vec<i64>, Vec<String>) = {
        let s = df.column("__time__").map_err(|e| format!("Panel: {}", e))?;
        if matches!(s.dtype(), polars::prelude::DataType::Categorical(_, _) | polars::prelude::DataType::Enum(_, _)) {
            let str_s = s.cast(&polars::prelude::DataType::String).map_err(|e: polars::error::PolarsError| e.to_string())?;
            let ca = str_s.str().map_err(|e: polars::error::PolarsError| e.to_string())?;
            let values: Vec<String> = ca.into_iter()
                .map(|opt| opt.ok_or("null").map(|s: &str| s.to_string()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| "Panel: time contains null")?;
            let mut unique: Vec<String> = Vec::new();
            for v in &values {
                if !unique.contains(v) {
                    unique.push(v.clone());
                }
            }
            unique.sort();
            let m: HashMap<String, usize> = unique.iter().enumerate().map(|(i, k)| (k.clone(), i)).collect();
            let indices: Vec<usize> = values.iter().map(|k| *m.get(k).unwrap_or(&0)).collect();
            let vals: Vec<i64> = indices.iter().map(|&i| i as i64).collect();
            (indices, vals, unique)
        } else if s.dtype() == &polars::prelude::DataType::Int64 {
            let ca = s.i64().map_err(|e: polars::error::PolarsError| e.to_string())?;
            let values: Vec<i64> = ca.into_iter()
                .map(|opt| opt.ok_or("Panel: time contains null"))
                .collect::<Result<Vec<_>, _>>()?;
            let mut unique: Vec<i64> = Vec::new();
            for v in &values {
                if !unique.contains(v) {
                    unique.push(*v);
                }
            }
            unique.sort_unstable();
            let m: HashMap<i64, usize> = unique.iter().enumerate().map(|(i, &k)| (k, i)).collect();
            let indices: Vec<usize> = values.iter().map(|k| *m.get(k).unwrap_or(&0)).collect();
            let names: Vec<String> = unique.iter().map(|v| v.to_string()).collect();
            (indices, values, names)
        } else if s.dtype() == &polars::prelude::DataType::Date {
            let ca = s.date().map_err(|e: polars::error::PolarsError| e.to_string())?;
            let physical = ca.physical();
            let values: Vec<i32> = physical
                .into_iter()
                .map(|opt| opt.ok_or("Panel: time contains null"))
                .collect::<Result<Vec<_>, _>>()?;
            let mut unique: Vec<i32> = Vec::new();
            for v in &values {
                if !unique.contains(v) {
                    unique.push(*v);
                }
            }
            unique.sort_unstable();
            let m: HashMap<i32, usize> = unique.iter().enumerate().map(|(i, &k)| (k, i)).collect();
            let indices: Vec<usize> = values.iter().map(|k| *m.get(k).unwrap_or(&0)).collect();
            let vals: Vec<i64> = values.iter().map(|&v| v as i64).collect();
            let names: Vec<String> = unique.iter().map(|v| v.to_string()).collect();
            (indices, vals, names)
        } else {
            return Err("Panel: Time ID must be Categorical, Int64, or Date".to_string());
        }
    };

    let has_constant = constant;
    let mut exog_columns: Vec<Vec<f64>> = Vec::new();
    let mut all_labels: Vec<(String, Option<String>)> = vec![("const".to_string(), None)];

    for (series_name, is_categorical, dsv) in exog_meta {
        let col = df.column(&series_name).map_err(|e| format!("Panel: {}", e))?;
        if is_categorical {
            let str_ca = col.str().map_err(|e: polars::error::PolarsError| e.to_string())?;
            let values: Vec<String> = str_ca.into_no_null_iter().map(|s: &str| s.to_string()).collect();
            let mut unique: Vec<String> = Vec::new();
            for v in &values {
                if !unique.contains(v) {
                    unique.push(v.clone());
                }
            }
            if unique.len() < 2 {
                return Err(format!("Panel: categorical '{}' needs >= 2 unique values", series_name));
            }
            let drop_cat = unique[0].clone();
            for cat in unique.iter().filter(|c| **c != drop_cat) {
                let col_vec: Vec<f64> = values.iter().map(|v| if v == cat { 1.0 } else { 0.0 }).collect();
                exog_columns.push(col_vec);
                all_labels.push((series_name.clone(), Some(cat.clone())));
            }
        } else {
            let col_vec: Vec<f64> = col.f64().map_err(|e: polars::error::PolarsError| e.to_string())?.into_no_null_iter().collect();
            exog_columns.push(col_vec);
            all_labels.push((series_name, None));
        }
    }

    let k = exog_columns.len() + 1;
    let mut exog_raw = Vec::with_capacity(n * k);
    for i in 0..n {
        exog_raw.push(1.0);
        for col in &exog_columns {
            exog_raw.push(col[i]);
        }
    }
    let exog = Array2::from_shape_vec((n, k), exog_raw)
        .map_err(|e| format!("Panel: exog shape: {:?}", e))?;

    let endog = Array1::from(endog_vec);
    Ok((endog, exog, entity_after, time_after, time_values, all_labels, endog_name, has_constant, entity_names, entity_series_name, time_names, time_series_name))
}

fn panel_result_to_ols_result(
    pr: &yss_sci::regression::panel::PanelOLSResult,
    model_type: &str,
    method: &str,
    endog_name: &str,
    all_labels: &[(String, Option<String>)],
    label_offset: usize,
    num_groups_override: Option<usize>,
) -> OLSResult {
    let panel_fe_info = pr.fe_stats.as_ref().map(|s| PanelFEInfo {
        r2_within: s.r2_within,
        r2_between: s.r2_between,
        r2_overall: s.r2_overall,
        num_groups: num_groups_override.unwrap_or(pr.num_entities),
        obs_per_group_min: s.obs_per_group_min,
        obs_per_group_avg: s.obs_per_group_avg,
        obs_per_group_max: s.obs_per_group_max,
        sigma_u: s.sigma_u,
        sigma_e: s.sigma_e,
        rho: s.rho,
        corr_u_i_Xb: s.corr_u_i_xb,
    });
    let num_coeff = pr.betas.len();
    let mut coefficients = Vec::with_capacity(num_coeff);
    for i in 0..num_coeff {
        let (var, cat) = all_labels
            .get(label_offset + i)
            .cloned()
            .unwrap_or_else(|| (format!("x{}", i), None));
        coefficients.push(Coefficient {
            variable: var,
            category: cat,
            coef: pr.betas[i],
            std_err: pr.stds[i],
            t_value: pr.tvalues[i],
            p_value: pr.pvalues[i],
            ci_lower: pr.conf_int_left[i],
            ci_upper: pr.conf_int_right[i],
            is_significant: pr.pvalues[i] < 0.05,
        });
    }

    let (aic, bic) = compute_aic_bic(pr.num_observation, pr.betas.len(), pr.ss_residual);

    OLSResult {
        title: format!("{} Results", model_type),
        endog_name: endog_name.to_string(),
        model_basic_info: ModelBasicInfo {
            model_type: model_type.to_string(),
            method: method.to_string(),
            num_observation: pr.num_observation,
            r_squared: pr.r2,
            adj_r_squared: pr.r2_adjusted,
            f_statistic: pr.fvalue,
            prob_f_statistic: pr.f_p_value,
            wald_chi2: None,
            prob_wald_chi2: None,
            df_model: pr.df_model,
            df_residual: pr.df_residual,
            df_total: pr.df_total,
            ss_model: pr.ss_model,
            ss_residual: pr.ss_residual,
            ss_total: pr.ss_total,
            ms_model: pr.ms_model,
            ms_residual: pr.ms_residual,
            ms_total: pr.ms_total,
            covariance_type: pr.covariance_type.clone(),
            aic,
            bic,
        },
        coefficients,
        diagnostic_info: DiagnosticInfo {
            cond_no: pr.cond_no,
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
            panel_fe_info,
        },
        betas: pr.betas.to_vec(),
        cov_beta: (0..pr.cov_beta.nrows())
            .map(|i| pr.cov_beta.row(i).iter().cloned().collect())
            .collect(),
    }
}

// ======================== Panel Configure 节点 ========================

fn register_panel_configure(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "Panel Configure",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Panel regression configuration — Constant and VCE (cluster by entity default)")
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
                PinDataTypeDefinition::concrete(panel_vce_one_of_type()),
            )
            .with_optional(true),
        ),
        PinSlot::fixed(PinDefinition::data_output(
            "Config",
            DataRole::Result,
            PinDataTypeDefinition::concrete(DataType::Struct("PanelConfigure".to_string())),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let constant = ctx
            .get_input_by_role(&PinRole::Data(DataRole::Custom("constant".to_string())))
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let cov_type = ctx
            .get_input_by_role(&PinRole::Data(DataRole::Custom("vce".to_string())))
            .ok()
            .and_then(|v| v.as_handle_id().map(|s| s.to_string()))
            .and_then(|id| ctx.get_handle(&id).ok())
            .and_then(|h| {
                Some(if h.downcast_ref::<super::ols_nodes::VCENonRobust>().is_some() {
                    "nonrobust".to_string()
                } else if h.downcast_ref::<super::ols_nodes::VCEHC0>().is_some() {
                    "HC0".to_string()
                } else if h.downcast_ref::<super::ols_nodes::VCEHC1>().is_some() {
                    "HC1".to_string()
                } else if h.downcast_ref::<super::ols_nodes::VCEHC2>().is_some() {
                    "HC2".to_string()
                } else if h.downcast_ref::<super::ols_nodes::VCEHC3>().is_some() {
                    "HC3".to_string()
                } else if h.downcast_ref::<PanelVCECluster>().is_some() {
                    "cluster".to_string()
                } else {
                    return None;
                })
            })
            .unwrap_or_else(|| "cluster".to_string());

        let config = PanelConfigure {
            constant,
            cov_type,
        };
        let handle_id = ctx.put_handle(Box::new(config));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Result),
            DataValue::new_struct("PanelConfigure", handle_id),
        )?;
        Ok(())
    }));
    registry.register(definition);
}

fn register_panel_vce_cluster(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "VCE: Cluster (by Entity)",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Panel VCE: cluster-robust by Entity ID (default)")
    .with_pin_slots(vec![PinSlot::fixed(PinDefinition::data_output(
        "VCE",
        DataRole::Result,
        PinDataTypeDefinition::concrete(DataType::Struct("PanelVCECluster".to_string())),
    ))])
    .with_data_evaluator(Arc::new(|ctx| {
        let config = PanelVCECluster;
        let handle_id = ctx.put_handle(Box::new(config));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Result),
            DataValue::new_struct("PanelVCECluster", handle_id),
        )?;
        Ok(())
    }));
    registry.register(definition);
}

// ======================== 注册 ========================

pub fn register(registry: &NodeRegistry) {
    register_panel_configure(registry);
    register_panel_vce_cluster(registry);
    let mut slots = panel_input_slots();
    slots.push(PinSlot::fixed(PinDefinition::data_output(
        "Result",
        DataRole::Result,
        PinDataTypeDefinition::concrete(DataType::Struct("PanelSummaryResult".to_string())),
    )));
    slots.push(PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)));

    let definition = NodeDefinition::new(
        "Panel Summary",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Panel data regression — FE (Within), LSDV, First Difference, Random Effects. Entity ID and Time ID required (like Stata xtset). VCE: cluster by entity.")
    .with_pin_slots(slots)
    .with_flow_processor(Arc::new(|ctx| {
        let config = match ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("panel_config".to_string()))) {
            Ok(v) => v.as_handle_id().and_then(|id| ctx.get_handle(id).ok()).and_then(|h| {
                h.downcast_ref::<PanelConfigure>().cloned()
            }),
            Err(_) => None,
        };
        let constant = config.as_ref().map(|c| c.constant).unwrap_or(true);
        let cov_type = config
            .as_ref()
            .map(|c| c.cov_type.as_str())
            .unwrap_or("cluster");

        let (endog, exog, entity_id, time_id, time_values, all_labels, endog_name, _, entity_names, entity_series_name, time_names, time_series_name) =
            build_panel_data(ctx, constant)?;

        let cov_params: Option<yss_sci::regression::covariance::CovParams> = None;

        let mut fe_result = None;
        let mut fe_time_result = None;
        let mut fe_twoway_result = None;
        let mut lsdv_result = None;
        let mut lsdv_time_result = None;
        let mut fd_result = None;
        let mut re_result = None;
        let mut errors = PanelErrors {
            fe: None,
            fe_time: None,
            fe_twoway: None,
            lsdv: None,
            lsdv_time: None,
            fd: None,
            re: None,
        };

        // FE (Within): entity fixed effects
        match fit_panel_fe(&endog, &exog, &entity_id, constant, cov_type, cov_params.clone()) {
            Ok(pr) => {
                fe_result = Some(panel_result_to_ols_result(
                    &pr,
                    "Panel:FE",
                    "Fixed Effects (Within)",
                    &endog_name,
                    &all_labels,
                    0,
                    None,
                ));
            }
            Err(e) => errors.fe = Some(e),
        }

        // FE (Time): time fixed effects
        match fit_panel_fe_time(&endog, &exog, &entity_id, &time_id, constant, cov_type, cov_params.clone()) {
            Ok(pr) => {
                fe_time_result = Some(panel_result_to_ols_result(
                    &pr,
                    "Panel:FE(Time)",
                    "Time Fixed Effects",
                    &endog_name,
                    &all_labels,
                    0,
                    Some(pr.num_time_periods),
                ));
            }
            Err(e) => errors.fe_time = Some(e),
        }

        // FE (Two-Way): entity + time fixed effects
        match fit_panel_fe_twoway(&endog, &exog, &entity_id, &time_id, constant, cov_type, cov_params.clone()) {
            Ok(pr) => {
                fe_twoway_result = Some(panel_result_to_ols_result(
                    &pr,
                    "Panel:FE(Two-Way)",
                    "Two-Way Fixed Effects",
                    &endog_name,
                    &all_labels,
                    0,
                    None,
                ));
            }
            Err(e) => errors.fe_twoway = Some(e),
        }

        // LSDV: full labels = [const, x1, x2, ..., entity_1, entity_2, ...] (entity 0 is reference)
        match fit_panel_lsdv(&endog, &exog, &entity_id, constant, cov_type, cov_params.clone()) {
            Ok(pr) => {
                let mut lsdv_labels = all_labels.clone();
                for i in 1..entity_names.len() {
                    lsdv_labels.push((entity_series_name.clone(), Some(entity_names[i].clone())));
                }
                lsdv_result = Some(panel_result_to_ols_result(
                    &pr,
                    "Panel:LSDV",
                    "Least Squares Dummy Variables",
                    &endog_name,
                    &lsdv_labels,
                    0,
                    None,
                ));
            }
            Err(e) => errors.lsdv = Some(e),
        }

        // LSDV (Time): labels = [const, x1, x2, ..., time_1, time_2, ...] (time 0 is reference)
        match fit_panel_lsdv_time(&endog, &exog, &entity_id, &time_id, constant, cov_type, cov_params.clone()) {
            Ok(pr) => {
                let mut lsdv_time_labels = all_labels.clone();
                for i in 1..time_names.len() {
                    lsdv_time_labels.push((time_series_name.clone(), Some(time_names[i].clone())));
                }
                lsdv_time_result = Some(panel_result_to_ols_result(
                    &pr,
                    "Panel:LSDV(Time)",
                    "LSDV (Time Dummies)",
                    &endog_name,
                    &lsdv_time_labels,
                    0,
                    None,
                ));
            }
            Err(e) => errors.lsdv_time = Some(e),
        }

        match fit_panel_fd(&endog, &exog, &entity_id, &time_id, &time_values, constant, cov_type, cov_params.clone()) {
            Ok(pr) => {
                fd_result = Some(panel_result_to_ols_result(
                    &pr,
                    "Panel:FD",
                    "First Difference",
                    &endog_name,
                    &all_labels,
                    1,
                    None,
                ));
            }
            Err(e) => errors.fd = Some(e),
        }

        // RE may keep constant
        match fit_panel_re(&endog, &exog, &entity_id, constant, cov_type, cov_params.clone()) {
            Ok(pr) => {
                re_result = Some(panel_result_to_ols_result(
                    &pr,
                    "Panel:RE",
                    "Random Effects (GLS)",
                    &endog_name,
                    &all_labels,
                    0,
                    None,
                ));
            }
            Err(e) => errors.re = Some(e),
        }

        let has_any = fe_result.is_some() || fe_time_result.is_some() || fe_twoway_result.is_some()
            || lsdv_result.is_some() || lsdv_time_result.is_some() || fd_result.is_some() || re_result.is_some();
        if !has_any {
            return Err(format!(
                "Panel Summary: all models failed. FE: {:?}, FE(Time): {:?}, FE(Two-Way): {:?}, LSDV: {:?}, LSDV(Time): {:?}, FD: {:?}, RE: {:?}",
                errors.fe, errors.fe_time, errors.fe_twoway, errors.lsdv, errors.lsdv_time, errors.fd, errors.re
            ));
        }

        let has_errors = errors.fe.is_some() || errors.fe_time.is_some() || errors.fe_twoway.is_some()
            || errors.lsdv.is_some() || errors.lsdv_time.is_some() || errors.fd.is_some() || errors.re.is_some();

        let summary = PanelSummaryResult {
            title: "Panel Regression Results".to_string(),
            endog_name,
            fe: fe_result,
            fe_time: fe_time_result,
            fe_twoway: fe_twoway_result,
            lsdv: lsdv_result,
            lsdv_time: lsdv_time_result,
            fd: fd_result,
            re: re_result,
            errors: if has_errors { Some(errors) } else { None },
        };

        let json_data = serde_json::to_string(&summary)
            .map_err(|e| format!("Panel Summary: serialize: {}", e))?;

        ctx.open_window("panel_summary".to_string(), json_data);
        ctx.log("Panel Summary: completed".to_string());
        Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
    }));
    registry.register(definition);
}
