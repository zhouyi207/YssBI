//! Panel Summary node — runs FE, FD, RE and displays all results

use crate::execution::{ExecutionEffect, ReportKind};
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
use statrs::distribution::{ChiSquared, ContinuousCDF, FisherSnedecor};
use std::collections::HashMap;
use std::sync::Arc;
use yss_sci::regression::collinearity;
use yss_sci::regression::linear_model::{OLS, OLSConfig};
use yss_sci::regression::panel::{
    fit_panel_fd, fit_panel_fe, fit_panel_fe_time, fit_panel_fe_twoway, fit_panel_lsdv,
    fit_panel_lsdv_time, fit_panel_lsdv_twoway, fit_panel_re_be, fit_panel_re_be_time,
    fit_panel_re_fgls, fit_panel_re_fgls_time, fit_panel_re_fgls_twoway, fit_panel_re_mle,
    fit_panel_re_mle_time, fit_panel_re_mle_twoway,
};
use yss_sci::tools::{IntoFaer, IntoFaerCol, IntoNdarray};

use super::info_nodes::{
    Coefficient, DiagnosticInfo, ModelBasicInfo, OLSResult, ObsPerGroupInfo, OmitInfo,
    OmittedVariable, PanelFEInfo, SigmaInfo, ThetaInfo, compute_aic_bic,
};

// ======================== 结构体 ========================

fn serde_default_true() -> bool {
    true
}

fn serde_default_one_usize() -> usize {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelConfigure {
    pub constant: bool,
    pub cov_type: String,
    /// Event-study Wald on pre-policy `rel_time×treat` (Stata-style `test` on leads)
    #[serde(default = "serde_default_true")]
    pub did_parallel_trends: bool,
    /// Falsification: treated×fake pre window before adoption
    #[serde(default = "serde_default_true")]
    pub did_placebo: bool,
    /// Length of fake pre window [t*−H, t*−1] in time ordinal units
    #[serde(default = "serde_default_one_usize")]
    pub did_placebo_horizon: usize,
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
    pub mixed_ols: Option<OLSResult>,
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
    pub lsdv_twoway: Option<OLSResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fd: Option<OLSResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub re_fgls: Option<OLSResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub re_mle: Option<OLSResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub re_be: Option<OLSResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub re_fgls_time: Option<OLSResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub re_mle_time: Option<OLSResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub re_be_time: Option<OLSResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub re_fgls_twoway: Option<OLSResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub re_mle_twoway: Option<OLSResult>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub selection_tests: Vec<PanelSelectionTest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<PanelErrors>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelErrors {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mixed_ols: Option<String>,
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
    pub lsdv_twoway: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub re_fgls: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub re_mle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub re_be: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub re_fgls_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub re_mle_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub re_be_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub re_fgls_twoway: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub re_mle_twoway: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelSelectionTest {
    pub id: String,
    /// model_choice | effect_choice
    pub group: String,
    pub label: String,
    pub h0: String,
    pub stat_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stat: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub df1: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub df2: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p_value: Option<f64>,
    /// significant | not_significant | unavailable
    pub decision: String,
    pub recommendation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

// ======================== 辅助函数 ========================

pub(crate) fn series_to_group_indices(
    ctx: &mut dyn NodeExecutionContextTrait,
    series_id: &str,
) -> Result<Vec<usize>, String> {
    let series = ctx.get_data_series(series_id)?;
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
        return Err(
            "Panel: Entity ID and Time ID must be Categorical or Int64 DataSeries".to_string(),
        );
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
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::one_of(
                vec![DataType::Float64, DataType::Int64],
            )))),
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

fn build_panel_data(
    ctx: &mut dyn NodeExecutionContextTrait,
    constant: bool,
) -> Result<
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
    let endog_series = ctx.get_data_series(&endog_id)?;
    let endog_name = {
        let raw = endog_series.name().to_string();
        if raw.is_empty() { "y".to_string() } else { raw }
    };
    let endog_f64 = endog_series
        .cast(&polars::prelude::DataType::Float64)
        .map_err(|e| format!("Panel: Y cast: {}", e))?;

    let entity_value =
        ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("entity_id".to_string())))?;
    let entity_id_str = match &entity_value {
        DataValue::DataSeries(v) => v.id.clone(),
        _ => return Err("Panel: Entity ID must be a DataSeries".to_string()),
    };
    let entity_id = series_to_group_indices(ctx, &entity_id_str)?;

    let time_value =
        ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("time_id".to_string())))?;
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
        let series = ctx.get_data_series(&dsv.id)?;
        let name = {
            let raw = series.name().to_string();
            if raw.is_empty() {
                format!("x{}", i + 1)
            } else {
                raw
            }
        };
        if series.len() != n_raw {
            return Err(format!(
                "Panel: X '{}' len {} != Y len {}",
                name,
                series.len(),
                n_raw
            ));
        }
        let is_cat = matches!(
            series.dtype(),
            polars::prelude::DataType::Categorical(_, _) | polars::prelude::DataType::Enum(_, _)
        );
        let col_series = if is_cat {
            series
                .cast(&polars::prelude::DataType::String)
                .map_err(|e| e.to_string())?
        } else {
            series
                .cast(&polars::prelude::DataType::Float64)
                .map_err(|e| e.to_string())?
        };
        df_cols.push(Column::from(col_series.with_name(name.as_str().into())));
        exog_meta.push((name, is_cat, dsv));
    }

    let entity_series = ctx.get_data_series(&entity_id_str)?;
    let entity_series_name = {
        let raw = entity_series.name().to_string();
        if raw.is_empty() {
            "Entity ID".to_string()
        } else {
            raw
        }
    };
    let time_series = ctx.get_data_series(&time_id_str)?;
    let time_series_name = {
        let raw = time_series.name().to_string();
        if raw.is_empty() {
            "Time ID".to_string()
        } else {
            raw
        }
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
        let s = df
            .column("__entity__")
            .map_err(|e| format!("Panel: {}", e))?;
        let mut m: HashMap<String, usize> = HashMap::new();
        let mut idx_to_name: Vec<String> = Vec::new();
        let mut out = Vec::with_capacity(n);
        if matches!(
            s.dtype(),
            polars::prelude::DataType::Categorical(_, _) | polars::prelude::DataType::Enum(_, _)
        ) {
            let str_s = s
                .cast(&polars::prelude::DataType::String)
                .map_err(|e: polars::error::PolarsError| e.to_string())?;
            let ca = str_s
                .str()
                .map_err(|e: polars::error::PolarsError| e.to_string())?;
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
            let ca = s
                .i64()
                .map_err(|e: polars::error::PolarsError| e.to_string())?;
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
        if matches!(
            s.dtype(),
            polars::prelude::DataType::Categorical(_, _) | polars::prelude::DataType::Enum(_, _)
        ) {
            let str_s = s
                .cast(&polars::prelude::DataType::String)
                .map_err(|e: polars::error::PolarsError| e.to_string())?;
            let ca = str_s
                .str()
                .map_err(|e: polars::error::PolarsError| e.to_string())?;
            let values: Vec<String> = ca
                .into_iter()
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
            let m: HashMap<String, usize> = unique
                .iter()
                .enumerate()
                .map(|(i, k)| (k.clone(), i))
                .collect();
            let indices: Vec<usize> = values.iter().map(|k| *m.get(k).unwrap_or(&0)).collect();
            let vals: Vec<i64> = indices.iter().map(|&i| i as i64).collect();
            (indices, vals, unique)
        } else if s.dtype() == &polars::prelude::DataType::Int64 {
            let ca = s
                .i64()
                .map_err(|e: polars::error::PolarsError| e.to_string())?;
            let values: Vec<i64> = ca
                .into_iter()
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
            let ca = s
                .date()
                .map_err(|e: polars::error::PolarsError| e.to_string())?;
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

    for (series_name, is_categorical, _dsv) in exog_meta {
        let col = df
            .column(&series_name)
            .map_err(|e| format!("Panel: {}", e))?;
        if is_categorical {
            let str_ca = col
                .str()
                .map_err(|e: polars::error::PolarsError| e.to_string())?;
            let values: Vec<String> = str_ca
                .into_no_null_iter()
                .map(|s: &str| s.to_string())
                .collect();
            let mut unique: Vec<String> = Vec::new();
            for v in &values {
                if !unique.contains(v) {
                    unique.push(v.clone());
                }
            }
            if unique.len() < 2 {
                return Err(format!(
                    "Panel: categorical '{}' needs >= 2 unique values",
                    series_name
                ));
            }
            let drop_cat = unique[0].clone();
            for cat in unique.iter().filter(|c| **c != drop_cat) {
                let col_vec: Vec<f64> = values
                    .iter()
                    .map(|v| if v == cat { 1.0 } else { 0.0 })
                    .collect();
                exog_columns.push(col_vec);
                all_labels.push((series_name.clone(), Some(cat.clone())));
            }
        } else {
            let col_vec: Vec<f64> = col
                .f64()
                .map_err(|e: polars::error::PolarsError| e.to_string())?
                .into_no_null_iter()
                .collect();
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
    Ok((
        endog,
        exog,
        entity_after,
        time_after,
        time_values,
        all_labels,
        endog_name,
        has_constant,
        entity_names,
        entity_series_name,
        time_names,
        time_series_name,
    ))
}

pub(crate) fn panel_result_to_ols_result(
    pr: &yss_sci::regression::panel::PanelOLSResult,
    model_type: &str,
    method: &str,
    endog_name: &str,
    all_labels: &[(String, Option<String>)],
    label_offset: usize,
    num_groups_override: Option<usize>,
    omit_info: Option<&OmitInfo>,
) -> OLSResult {
    let panel_fe_info = pr.fe_stats.as_ref().map(|s| {
        let r2 = s.r2.as_ref();
        PanelFEInfo {
            r2_within: r2.map(|r| r.r2_within),
            r2_between: r2.map(|r| r.r2_between),
            r2_overall: r2.map(|r| r.r2_overall),
            num_groups: num_groups_override.unwrap_or(pr.num_entities),
            obs_per_group: ObsPerGroupInfo {
                min: s.obs_per_group.min,
                avg: s.obs_per_group.avg,
                max: s.obs_per_group.max,
            },
            sigma: SigmaInfo {
                sigma_u: s.sigma.sigma_u,
                sigma_e: s.sigma.sigma_e,
                rho: s.sigma.rho,
            },
            corr_u_i_Xb: s.corr_u_i_xb,
            theta: s.theta.as_ref().map(|t| ThetaInfo {
                min: t.min,
                avg: t.avg,
                max: t.max,
            }),
            chibar2: pr.chibar2,
            prob_chibar2: pr.prob_chibar2,
        }
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
            f_statistic: pr
                .lr_chi2
                .unwrap_or_else(|| pr.wald_chi2.unwrap_or(pr.fvalue)),
            prob_f_statistic: pr
                .prob_lr_chi2
                .unwrap_or_else(|| pr.prob_wald_chi2.unwrap_or(pr.f_p_value)),
            wald_chi2: pr.wald_chi2,
            prob_wald_chi2: pr.prob_wald_chi2,
            log_likelihood: pr.log_likelihood,
            lr_chi2: pr.lr_chi2,
            prob_lr_chi2: pr.prob_lr_chi2,
            chibar2: pr.chibar2,
            prob_chibar2: pr.prob_chibar2,
            mle_iter_log_lik_const: pr.mle_iter_log_lik_const.clone(),
            mle_iter_log_lik: pr.mle_iter_log_lik.clone(),
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
            omit_info: omit_info.cloned(),
        },
        betas: pr.betas.to_vec(),
        cov_beta: (0..pr.cov_beta.nrows())
            .map(|i| pr.cov_beta.row(i).iter().cloned().collect())
            .collect(),
        cov_beta_nonrobust: pr.cov_beta_nonrobust.as_ref().map(|m| {
            (0..m.nrows())
                .map(|i| m.row(i).iter().cloned().collect())
                .collect()
        }),
    }
}

fn linear_result_to_ols_result(
    pr: &yss_sci::regression::linear_model::OLSResult,
    model_type: &str,
    method: &str,
    endog_name: &str,
    all_labels: &[(String, Option<String>)],
    label_offset: usize,
    omit_info: Option<&OmitInfo>,
) -> OLSResult {
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
            log_likelihood: None,
            lr_chi2: None,
            prob_lr_chi2: None,
            chibar2: None,
            prob_chibar2: None,
            mle_iter_log_lik_const: None,
            mle_iter_log_lik: None,
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
            panel_fe_info: None,
            omit_info: omit_info.cloned(),
        },
        betas: pr.betas.to_vec(),
        cov_beta: (0..pr.cov_beta.nrows())
            .map(|i| pr.cov_beta.row(i).iter().cloned().collect())
            .collect(),
        cov_beta_nonrobust: None,
    }
}

fn unavailable_test(
    id: &str,
    group: &str,
    label: &str,
    h0: &str,
    stat_type: &str,
    recommendation: &str,
    note: &str,
) -> PanelSelectionTest {
    PanelSelectionTest {
        id: id.to_string(),
        group: group.to_string(),
        label: label.to_string(),
        h0: h0.to_string(),
        stat_type: stat_type.to_string(),
        stat: None,
        df1: None,
        df2: None,
        p_value: None,
        decision: "unavailable".to_string(),
        recommendation: recommendation.to_string(),
        note: Some(note.to_string()),
    }
}

fn pooled_vs_unrestricted_f_test(
    id: &str,
    group: &str,
    label: &str,
    h0: &str,
    pooled: Option<&OLSResult>,
    unrestricted: Option<&OLSResult>,
    sig_recommendation: &str,
    nsig_recommendation: &str,
) -> PanelSelectionTest {
    let Some(r) = pooled else {
        return unavailable_test(
            id,
            group,
            label,
            h0,
            "F",
            "Cannot decide because pooled model is unavailable.",
            "Pooled OLS result is missing",
        );
    };
    let Some(u) = unrestricted else {
        return unavailable_test(
            id,
            group,
            label,
            h0,
            "F",
            "Cannot decide because unrestricted model is unavailable.",
            "Unrestricted model result is missing",
        );
    };
    let rss_r = r.model_basic_info.ss_residual;
    let rss_u = u.model_basic_info.ss_residual;
    // Recover true regression df from ss_residual/ms_residual.
    // df_residual in model_basic_info may be cluster-adjusted (M-1) for LSDV/FE,
    // but the poolability F-test needs the actual OLS residual df.
    let df_r = if r.model_basic_info.ms_residual > 0.0 {
        (rss_r / r.model_basic_info.ms_residual).round() as usize
    } else {
        r.model_basic_info.df_residual
    };
    let df_u = if u.model_basic_info.ms_residual > 0.0 {
        (rss_u / u.model_basic_info.ms_residual).round() as usize
    } else {
        u.model_basic_info.df_residual
    };
    if df_r <= df_u || df_u == 0 {
        return unavailable_test(
            id,
            group,
            label,
            h0,
            "F",
            "Cannot decide due to invalid degrees of freedom.",
            "Need df_restricted > df_unrestricted > 0",
        );
    }
    let q = df_r - df_u;
    let num = (rss_r - rss_u).max(0.0) / q as f64;
    let den = rss_u / df_u as f64;
    if !num.is_finite() || !den.is_finite() || den <= 0.0 {
        return unavailable_test(
            id,
            group,
            label,
            h0,
            "F",
            "Cannot decide due to numerical instability.",
            "F-stat denominator is non-positive or non-finite",
        );
    }
    let f_stat = (num / den).max(0.0);
    let dist = match FisherSnedecor::new(q as f64, df_u as f64) {
        Ok(d) => d,
        Err(_) => {
            return unavailable_test(
                id,
                group,
                label,
                h0,
                "F",
                "Cannot decide because F distribution is invalid.",
                "Invalid F distribution degrees of freedom",
            );
        }
    };
    let p = (1.0 - dist.cdf(f_stat)).clamp(0.0, 1.0);
    let significant = p < 0.05;
    PanelSelectionTest {
        id: id.to_string(),
        group: group.to_string(),
        label: label.to_string(),
        h0: h0.to_string(),
        stat_type: "F".to_string(),
        stat: Some(f_stat),
        df1: Some(q),
        df2: Some(df_u),
        p_value: Some(p),
        decision: if significant {
            "significant".to_string()
        } else {
            "not_significant".to_string()
        },
        recommendation: if significant {
            sig_recommendation.to_string()
        } else {
            nsig_recommendation.to_string()
        },
        note: None,
    }
}

fn re_chibar2_test(
    id: &str,
    group: &str,
    label: &str,
    h0: &str,
    re_mle: Option<&OLSResult>,
    sig_recommendation: &str,
    nsig_recommendation: &str,
) -> PanelSelectionTest {
    let Some(re_res) = re_mle else {
        return unavailable_test(
            id,
            group,
            label,
            h0,
            "chibar2(01)",
            "Cannot decide because RE(MLE) result is unavailable.",
            "RE(MLE) result is missing",
        );
    };
    let Some(chibar2) = re_res.model_basic_info.chibar2 else {
        return unavailable_test(
            id,
            group,
            label,
            h0,
            "chibar2(01)",
            "Cannot decide because chibar2 statistic is missing.",
            "RE(MLE) did not report chibar2",
        );
    };
    let Some(p) = re_res.model_basic_info.prob_chibar2 else {
        return unavailable_test(
            id,
            group,
            label,
            h0,
            "chibar2(01)",
            "Cannot decide because p-value is missing.",
            "RE(MLE) did not report Prob>=chibar2",
        );
    };
    let significant = p < 0.05;
    PanelSelectionTest {
        id: id.to_string(),
        group: group.to_string(),
        label: label.to_string(),
        h0: h0.to_string(),
        stat_type: "chibar2(01)".to_string(),
        stat: Some(chibar2),
        df1: None,
        df2: None,
        p_value: Some(p),
        decision: if significant {
            "significant".to_string()
        } else {
            "not_significant".to_string()
        },
        recommendation: if significant {
            sig_recommendation.to_string()
        } else {
            nsig_recommendation.to_string()
        },
        note: None,
    }
}

/// Breusch-Pagan LM test for random effects (Stata xttest0).
/// Uses pooled OLS residuals to test H0: sigma_u^2 = 0.
/// For unbalanced panels: LM = N^2 / (2 * sum_i T_i(T_i-1)) * (A/B - 1)^2
/// where A = sum_i (sum_t e_it)^2, B = sum_i sum_t e_it^2.
fn breusch_pagan_lm_test(
    id: &str,
    group: &str,
    label: &str,
    h0: &str,
    residuals: Option<&Vec<f64>>,
    group_id: &[usize],
    sig_recommendation: &str,
    nsig_recommendation: &str,
) -> PanelSelectionTest {
    let Some(resid) = residuals else {
        return unavailable_test(
            id,
            group,
            label,
            h0,
            "chibar2(01)",
            "Cannot decide because pooled OLS residuals are unavailable.",
            "Pooled OLS fit failed",
        );
    };
    if resid.len() != group_id.len() {
        return unavailable_test(
            id,
            group,
            label,
            h0,
            "chibar2(01)",
            "Cannot decide due to length mismatch.",
            "residuals.len() != group_id.len()",
        );
    }

    let n_total = resid.len() as f64;
    let mut group_sums: std::collections::HashMap<usize, (f64, usize)> =
        std::collections::HashMap::new();
    let mut b_total = 0.0f64;
    for (i, &g) in group_id.iter().enumerate() {
        let e = resid[i];
        b_total += e * e;
        let entry = group_sums.entry(g).or_insert((0.0, 0));
        entry.0 += e;
        entry.1 += 1;
    }

    if b_total <= 0.0 || !b_total.is_finite() {
        return unavailable_test(
            id,
            group,
            label,
            h0,
            "chibar2(01)",
            "Cannot decide because RSS is zero or non-finite.",
            "e'e <= 0",
        );
    }

    let a_total: f64 = group_sums.values().map(|(s, _)| s * s).sum();
    let sum_ti_ti_minus_1: f64 = group_sums
        .values()
        .map(|(_, ti)| {
            let t = *ti as f64;
            t * (t - 1.0)
        })
        .sum();

    if sum_ti_ti_minus_1 <= 0.0 {
        return unavailable_test(
            id,
            group,
            label,
            h0,
            "chibar2(01)",
            "Cannot decide because all groups have only 1 observation.",
            "sum T_i(T_i-1) = 0",
        );
    }

    let ratio = a_total / b_total - 1.0;
    let lm = n_total * n_total / (2.0 * sum_ti_ti_minus_1) * ratio * ratio;

    use statrs::distribution::{ChiSquared, ContinuousCDF};
    let chi2_1 = match ChiSquared::new(1.0) {
        Ok(d) => d,
        Err(_) => {
            return unavailable_test(
                id,
                group,
                label,
                h0,
                "chibar2(01)",
                "Cannot compute p-value.",
                "Invalid chi2 distribution",
            );
        }
    };
    let p = (0.5 * (1.0 - chi2_1.cdf(lm))).clamp(0.0, 1.0);
    let significant = p < 0.05;

    PanelSelectionTest {
        id: id.to_string(),
        group: group.to_string(),
        label: label.to_string(),
        h0: h0.to_string(),
        stat_type: "chibar2(01)".to_string(),
        stat: Some(lm),
        df1: Some(1),
        df2: None,
        p_value: Some(p),
        decision: if significant {
            "significant".to_string()
        } else {
            "not_significant".to_string()
        },
        recommendation: if significant {
            sig_recommendation.to_string()
        } else {
            nsig_recommendation.to_string()
        },
        note: None,
    }
}

fn hausman_test(
    id: &str,
    group: &str,
    label: &str,
    h0: &str,
    fe: Option<&OLSResult>,
    re: Option<&OLSResult>,
    sig_recommendation: &str,
    nsig_recommendation: &str,
) -> PanelSelectionTest {
    let Some(fe_res) = fe else {
        return unavailable_test(
            id,
            group,
            label,
            h0,
            "Hausman chi2",
            "Cannot decide because FE result is unavailable.",
            "FE result is missing",
        );
    };
    let Some(re_res) = re else {
        return unavailable_test(
            id,
            group,
            label,
            h0,
            "Hausman chi2",
            "Cannot decide because RE result is unavailable.",
            "RE result is missing",
        );
    };
    // Include constant in the test (Stata hausman, constant sigmamore)
    let mut fe_index: HashMap<String, usize> = HashMap::new();
    for (i, c) in fe_res.coefficients.iter().enumerate() {
        let key = match &c.category {
            Some(cat) => format!("{}|{}", c.variable, cat),
            None => c.variable.clone(),
        };
        fe_index.insert(key, i);
    }
    let mut re_index: HashMap<String, usize> = HashMap::new();
    for (i, c) in re_res.coefficients.iter().enumerate() {
        let key = match &c.category {
            Some(cat) => format!("{}|{}", c.variable, cat),
            None => c.variable.clone(),
        };
        re_index.insert(key, i);
    }
    let mut common: Vec<(usize, usize)> = Vec::new();
    for (k, i_fe) in fe_index.iter() {
        if let Some(i_re) = re_index.get(k) {
            common.push((*i_fe, *i_re));
        }
    }
    common.sort_by_key(|(i_fe, _)| *i_fe);
    if common.is_empty() {
        return unavailable_test(
            id,
            group,
            label,
            h0,
            "Hausman chi2",
            "Cannot decide because no common slope coefficients were found.",
            "No overlapping FE and RE coefficient set",
        );
    }
    let m = common.len();
    let mut diff = Array1::<f64>::zeros(m);
    let mut vdiff = Array2::<f64>::zeros((m, m));
    for (i, (i_fe, i_re)) in common.iter().enumerate() {
        let b_fe = fe_res.betas.get(*i_fe).copied().unwrap_or(0.0);
        let b_re = re_res.betas.get(*i_re).copied().unwrap_or(0.0);
        diff[i] = b_fe - b_re;
    }
    // Use nonrobust VCE for Hausman test (Stata hausman, sigmamore).
    // With cluster-robust VCE, the standard Hausman formula is invalid because
    // V(b_FE - b_RE) ≠ V_FE - V_RE under robust variance estimation.
    let fe_vcov = fe_res
        .cov_beta_nonrobust
        .as_ref()
        .unwrap_or(&fe_res.cov_beta);
    let re_vcov = re_res
        .cov_beta_nonrobust
        .as_ref()
        .unwrap_or(&re_res.cov_beta);

    // sigmamore: rescale FE nonrobust VCE using RE's sigma² so V_FE - V_RE ≥ 0
    let sigma2_fe = fe_res.model_basic_info.ms_residual;
    let sigma2_re = re_res.model_basic_info.ms_residual;
    let sigmamore_scale = if sigma2_fe > 1e-300 {
        sigma2_re / sigma2_fe
    } else {
        1.0
    };

    for (i, (i_fe, i_re)) in common.iter().enumerate() {
        for (j, (j_fe, j_re)) in common.iter().enumerate() {
            let v_fe = fe_vcov
                .get(*i_fe)
                .and_then(|row| row.get(*j_fe))
                .copied()
                .unwrap_or(0.0)
                * sigmamore_scale;
            let v_re = re_vcov
                .get(*i_re)
                .and_then(|row| row.get(*j_re))
                .copied()
                .unwrap_or(0.0);
            vdiff[[i, j]] = v_fe - v_re;
        }
    }
    if !diff.iter().all(|v| v.is_finite()) || !vdiff.iter().all(|v| v.is_finite()) {
        return unavailable_test(
            id,
            group,
            label,
            h0,
            "Hausman chi2",
            "Cannot decide due to non-finite values in variance matrix.",
            "Encountered NaN/Inf in Hausman inputs",
        );
    }
    let v_faer = vdiff.view().into_faer();
    let v_faer_owned = v_faer.to_owned();
    let svd = match v_faer_owned.as_ref().svd() {
        Ok(s) => s,
        Err(_) => {
            return unavailable_test(
                id,
                group,
                label,
                h0,
                "Hausman chi2",
                "Cannot decide because SVD failed on variance-difference matrix.",
                "SVD decomposition failed for V_FE - V_RE",
            );
        }
    };
    let s = svd.S().column_vector();
    let u = svd.U();
    let v = svd.V();
    let max_s = s.iter().cloned().fold(0.0f64, f64::max);
    let tol = max_s * (m as f64) * f64::EPSILON;
    let rank = s.iter().filter(|&&si| si > tol).count();
    if rank == 0 {
        return unavailable_test(
            id,
            group,
            label,
            h0,
            "Hausman chi2",
            "Cannot decide because variance-difference matrix has zero effective rank.",
            "Effective rank(V_FE - V_RE) = 0",
        );
    }
    // Hausman via Moore-Penrose inverse:
    // H = d' * V^+ * d, V = U S V', V^+ = V S^+ U'
    let diff_col = diff.view().into_faer_col().to_owned();
    let ut_diff = u.get(.., ..m).transpose() * diff_col.as_ref();
    let ut_diff_nd = ut_diff.as_ref().into_ndarray().to_owned();
    let mut st_inv_ut_diff = faer::Mat::zeros(m, 1);
    for i in 0..m {
        let si = s[i];
        let val = if si > tol { ut_diff_nd[i] / si } else { 0.0 };
        st_inv_ut_diff.as_mut()[(i, 0)] = val;
    }
    let vinv_diff = v.get(.., ..m) * st_inv_ut_diff.as_ref();
    let vinv_diff_nd = vinv_diff.as_ref().into_ndarray().to_owned();
    let mut chi2 = 0.0f64;
    for i in 0..m {
        chi2 += diff[i] * vinv_diff_nd[[i, 0]];
    }
    let chi2 = chi2.max(0.0);
    let dist = match ChiSquared::new(rank as f64) {
        Ok(d) => d,
        Err(_) => {
            return unavailable_test(
                id,
                group,
                label,
                h0,
                "Hausman chi2",
                "Cannot decide because chi-square distribution is invalid.",
                "Invalid Hausman degrees of freedom after rank adjustment",
            );
        }
    };
    let p = (1.0 - dist.cdf(chi2)).clamp(0.0, 1.0);
    let significant = p < 0.05;
    PanelSelectionTest {
        id: id.to_string(),
        group: group.to_string(),
        label: label.to_string(),
        h0: h0.to_string(),
        stat_type: "Hausman chi2".to_string(),
        stat: Some(chi2),
        df1: Some(rank),
        df2: None,
        p_value: Some(p),
        decision: if significant {
            "significant".to_string()
        } else {
            "not_significant".to_string()
        },
        recommendation: if significant {
            sig_recommendation.to_string()
        } else {
            nsig_recommendation.to_string()
        },
        note: if rank < m {
            Some(format!(
                "Used generalized inverse because V_FE - V_RE is near-singular (effective rank {}/{}).",
                rank, m
            ))
        } else {
            None
        },
    }
}

// ======================== Panel Configure 节点 ========================

fn register_panel_configure(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "Panel Configure",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_localized_description(
        "面板回归配置 — 常数项与 VCE（默认按 entity 聚类）",
        "Panel regression configuration — Constant and VCE (cluster by entity default)",
    )
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
                Some(
                    if h.downcast_ref::<super::ols_nodes::VCENonRobust>().is_some() {
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
                    },
                )
            })
            .unwrap_or_else(|| "cluster".to_string());

        let config = PanelConfigure {
            constant,
            cov_type,
            did_parallel_trends: true,
            did_placebo: true,
            did_placebo_horizon: 1,
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
    .with_localized_description(
        "面板 VCE：按 Entity ID 聚类稳健（默认）",
        "Panel VCE: cluster-robust by Entity ID (default)",
    )
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
    slots.push(PinSlot::fixed(PinDefinition::exec_output(
        "Out",
        ExecRole::ExecOut,
    )));

    let definition = NodeDefinition::new(
        "Panel Summary",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_localized_description(
        "面板回归 — 固定效应（Within）、LSDV、一阶差分、随机效应；需 Entity ID 与 Time ID（类似 Stata xtset）；VCE 默认按 entity 聚类",
        "Panel data regression — FE (Within), LSDV, First Difference, Random Effects. Entity ID and Time ID required (like Stata xtset). VCE: cluster by entity.",
    )
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

        // ---- Drop strictly collinear columns (prefer non-dummy) ----
        let k = exog.ncols();
        let col_is_dummy: Vec<bool> = all_labels.iter().map(|(_, cat)| cat.is_some()).collect();
        let intercept_col = if constant { Some(0) } else { None };
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

        let cov_params: Option<yss_sci::regression::covariance::CovParams> = None;

        let mut fe_result = None;
        let mut mixed_ols_result = None;
        let mut fe_time_result = None;
        let mut fe_twoway_result = None;
        let mut lsdv_result = None;
        let mut lsdv_time_result = None;
        let mut lsdv_twoway_result = None;
        let mut fd_result = None;
        let mut re_fgls_result = None;
        let mut re_mle_result = None;
        let mut re_be_result = None;
        let mut re_fgls_time_result = None;
        let mut re_mle_time_result = None;
        let mut re_be_time_result = None;
        let mut re_fgls_twoway_result = None;
        let mut re_mle_twoway_result = None;
        let mut errors = PanelErrors {
            mixed_ols: None,
            fe: None,
            fe_time: None,
            fe_twoway: None,
            lsdv: None,
            lsdv_time: None,
            lsdv_twoway: None,
            fd: None,
            re_fgls: None,
            re_mle: None,
            re_be: None,
            re_fgls_time: None,
            re_mle_time: None,
            re_be_time: None,
            re_fgls_twoway: None,
            re_mle_twoway: None,
        };

        // Mixed (Pooled) regression: OLS with same VCE as FE/RE (cluster by entity by default)
        let mixed_cov_params = cov_params.clone().or_else(|| {
            if cov_type == "cluster" {
                Some(yss_sci::regression::covariance::CovParams::Cluster {
                    cluster_id: entity_id.clone(),
                    xtreg_fe_style: false,
                })
            } else {
                None
            }
        });
        let mixed_ols = OLS {
            endog: endog.clone(),
            exog: exog_use.clone(),
            config: OLSConfig {
                constant,
                cov_type: cov_type.to_string(),
                cov_params: mixed_cov_params,
            },
        };
        let mut pooled_ols_residuals: Option<Vec<f64>> = None;
        match mixed_ols.fit() {
            Ok(pr) => {
                // Compute pooled OLS residuals for Breusch-Pagan LM test
                let fitted = exog_use.dot(&pr.betas);
                let resid: Vec<f64> = endog.iter().zip(fitted.iter()).map(|(&y, &yh)| y - yh).collect();
                pooled_ols_residuals = Some(resid);

                mixed_ols_result = Some(linear_result_to_ols_result(
                    &pr,
                    "Panel:Mixed",
                    "Pooled OLS",
                    &endog_name,
                    &all_labels_use,
                    0,
                    omit_info.as_ref(),
                ));
            }
            Err(e) => errors.mixed_ols = Some(format!("Panel mixed OLS: {}", e)),
        }

        // FE (Within): entity fixed effects
        match fit_panel_fe(&endog, &exog_use, &entity_id, constant, cov_type, cov_params.clone()) {
            Ok(pr) => {
                let (kept_labels, fe_omit_info) = match &pr.omitted_indices {
                    Some(omitted) if !omitted.is_empty() => {
                        let kept: Vec<_> = (0..all_labels_use.len())
                            .filter(|i| !omitted.contains(i))
                            .filter_map(|i| all_labels_use.get(i).cloned())
                            .collect();
                        let omitted_vars: Vec<OmittedVariable> = omitted
                            .iter()
                            .filter_map(|&i| all_labels_use.get(i))
                            .map(|(var, cat)| OmittedVariable {
                                variable: var.clone(),
                                category: cat.clone(),
                                reason: "collinearity".to_string(),
                            })
                            .collect();
                        let fe_omit = Some(OmitInfo { omitted: omitted_vars });
                        let merged = match (omit_info.as_ref(), &fe_omit) {
                            (Some(a), Some(b)) => Some(OmitInfo {
                                omitted: a.omitted.iter().chain(b.omitted.iter()).cloned().collect(),
                            }),
                            (Some(a), None) => Some(a.clone()),
                            (None, Some(b)) => Some(b.clone()),
                            (None, None) => None,
                        };
                        (kept, merged)
                    }
                    _ => (all_labels_use.clone(), omit_info.clone()),
                };
                fe_result = Some(panel_result_to_ols_result(
                    &pr,
                    "Panel:FE",
                    "Fixed Effects (Within)",
                    &endog_name,
                    &kept_labels,
                    0,
                    None,
                    fe_omit_info.as_ref(),
                ));
            }
            Err(e) => errors.fe = Some(e),
        }

        // FE (Time): time fixed effects
        match fit_panel_fe_time(&endog, &exog_use, &entity_id, &time_id, constant, cov_type, cov_params.clone()) {
            Ok(pr) => {
                let (kept_labels, fe_time_omit_info) = match &pr.omitted_indices {
                    Some(omitted) if !omitted.is_empty() => {
                        let kept: Vec<_> = (0..all_labels_use.len())
                            .filter(|i| !omitted.contains(i))
                            .filter_map(|i| all_labels_use.get(i).cloned())
                            .collect();
                        let omitted_vars: Vec<OmittedVariable> = omitted
                            .iter()
                            .filter_map(|&i| all_labels_use.get(i))
                            .map(|(var, cat)| OmittedVariable {
                                variable: var.clone(),
                                category: cat.clone(),
                                reason: "collinearity".to_string(),
                            })
                            .collect();
                        let fe_omit = Some(OmitInfo { omitted: omitted_vars });
                        let merged = match (omit_info.as_ref(), &fe_omit) {
                            (Some(a), Some(b)) => Some(OmitInfo {
                                omitted: a.omitted.iter().chain(b.omitted.iter()).cloned().collect(),
                            }),
                            (Some(a), None) => Some(a.clone()),
                            (None, Some(b)) => Some(b.clone()),
                            (None, None) => None,
                        };
                        (kept, merged)
                    }
                    _ => (all_labels_use.clone(), omit_info.clone()),
                };
                fe_time_result = Some(panel_result_to_ols_result(
                    &pr,
                    "Panel:FE(Time)",
                    "Time Fixed Effects",
                    &endog_name,
                    &kept_labels,
                    0,
                    Some(pr.num_time_periods),
                    fe_time_omit_info.as_ref(),
                ));
            }
            Err(e) => errors.fe_time = Some(e),
        }

        // FE (Two-Way): entity + time fixed effects
        match fit_panel_fe_twoway(&endog, &exog_use, &entity_id, &time_id, constant, cov_type, cov_params.clone()) {
            Ok(pr) => {
                let (kept_labels, fe_twoway_omit_info) = match &pr.omitted_indices {
                    Some(omitted) if !omitted.is_empty() => {
                        let kept: Vec<_> = (0..all_labels_use.len())
                            .filter(|i| !omitted.contains(i))
                            .filter_map(|i| all_labels_use.get(i).cloned())
                            .collect();
                        let omitted_vars: Vec<OmittedVariable> = omitted
                            .iter()
                            .filter_map(|&i| all_labels_use.get(i))
                            .map(|(var, cat)| OmittedVariable {
                                variable: var.clone(),
                                category: cat.clone(),
                                reason: "collinearity".to_string(),
                            })
                            .collect();
                        let fe_omit = Some(OmitInfo { omitted: omitted_vars });
                        let merged = match (omit_info.as_ref(), &fe_omit) {
                            (Some(a), Some(b)) => Some(OmitInfo {
                                omitted: a.omitted.iter().chain(b.omitted.iter()).cloned().collect(),
                            }),
                            (Some(a), None) => Some(a.clone()),
                            (None, Some(b)) => Some(b.clone()),
                            (None, None) => None,
                        };
                        (kept, merged)
                    }
                    _ => (all_labels_use.clone(), omit_info.clone()),
                };
                fe_twoway_result = Some(panel_result_to_ols_result(
                    &pr,
                    "Panel:FE(Two-Way)",
                    "Two-Way Fixed Effects",
                    &endog_name,
                    &kept_labels,
                    0,
                    None,
                    fe_twoway_omit_info.as_ref(),
                ));
            }
            Err(e) => errors.fe_twoway = Some(e),
        }

        // LSDV (Two-Way): labels = [const, x1, x2, ..., entity_1, ..., entity_{n-1}, time_1, ..., time_{T-1}]
        match fit_panel_lsdv_twoway(&endog, &exog_use, &entity_id, &time_id, constant, cov_type, cov_params.clone()) {
            Ok(pr) => {
                let mut lsdv_twoway_labels = all_labels_use.clone();
                for i in 1..entity_names.len() {
                    lsdv_twoway_labels.push((entity_series_name.clone(), Some(entity_names[i].clone())));
                }
                for i in 1..time_names.len() {
                    lsdv_twoway_labels.push((time_series_name.clone(), Some(time_names[i].clone())));
                }
                let (kept_labels, lsdv_twoway_omit_info) = match &pr.omitted_indices {
                    Some(omitted) if !omitted.is_empty() => {
                        let kept: Vec<_> = (0..lsdv_twoway_labels.len())
                            .filter(|i| !omitted.contains(i))
                            .filter_map(|i| lsdv_twoway_labels.get(i).cloned())
                            .collect();
                        let omitted_vars: Vec<OmittedVariable> = omitted
                            .iter()
                            .filter_map(|&i| lsdv_twoway_labels.get(i))
                            .map(|(var, cat)| OmittedVariable {
                                variable: var.clone(),
                                category: cat.clone(),
                                reason: "collinearity".to_string(),
                            })
                            .collect();
                        let lsdv_omit = Some(OmitInfo { omitted: omitted_vars });
                        let merged = match (omit_info.as_ref(), &lsdv_omit) {
                            (Some(a), Some(b)) => Some(OmitInfo {
                                omitted: a.omitted.iter().chain(b.omitted.iter()).cloned().collect(),
                            }),
                            (Some(a), None) => Some(a.clone()),
                            (None, Some(b)) => Some(b.clone()),
                            (None, None) => None,
                        };
                        (kept, merged)
                    }
                    _ => (lsdv_twoway_labels.clone(), omit_info.clone()),
                };
                lsdv_twoway_result = Some(panel_result_to_ols_result(
                    &pr,
                    "Panel:LSDV(Two-Way)",
                    "LSDV (Two-Way Dummies)",
                    &endog_name,
                    &kept_labels,
                    0,
                    None,
                    lsdv_twoway_omit_info.as_ref(),
                ));
            }
            Err(e) => errors.lsdv_twoway = Some(e),
        }

        // LSDV: full labels = [const, x1, x2, ..., entity_1, entity_2, ...] (entity 0 is reference)
        match fit_panel_lsdv(&endog, &exog_use, &entity_id, constant, cov_type, cov_params.clone()) {
            Ok(pr) => {
                let mut lsdv_labels = all_labels_use.clone();
                for i in 1..entity_names.len() {
                    lsdv_labels.push((entity_series_name.clone(), Some(entity_names[i].clone())));
                }
                let (kept_labels, lsdv_omit_info) = match &pr.omitted_indices {
                    Some(omitted) if !omitted.is_empty() => {
                        let kept: Vec<_> = (0..lsdv_labels.len())
                            .filter(|i| !omitted.contains(i))
                            .filter_map(|i| lsdv_labels.get(i).cloned())
                            .collect();
                        let omitted_vars: Vec<OmittedVariable> = omitted
                            .iter()
                            .filter_map(|&i| lsdv_labels.get(i))
                            .map(|(var, cat)| OmittedVariable {
                                variable: var.clone(),
                                category: cat.clone(),
                                reason: "collinearity".to_string(),
                            })
                            .collect();
                        let lsdv_omit = Some(OmitInfo { omitted: omitted_vars });
                        let merged = match (omit_info.as_ref(), &lsdv_omit) {
                            (Some(a), Some(b)) => Some(OmitInfo {
                                omitted: a.omitted.iter().chain(b.omitted.iter()).cloned().collect(),
                            }),
                            (Some(a), None) => Some(a.clone()),
                            (None, Some(b)) => Some(b.clone()),
                            (None, None) => None,
                        };
                        (kept, merged)
                    }
                    _ => (lsdv_labels.clone(), omit_info.clone()),
                };
                lsdv_result = Some(panel_result_to_ols_result(
                    &pr,
                    "Panel:LSDV",
                    "Least Squares Dummy Variables",
                    &endog_name,
                    &kept_labels,
                    0,
                    None,
                    lsdv_omit_info.as_ref(),
                ));
            }
            Err(e) => errors.lsdv = Some(e),
        }

        // LSDV (Time): labels = [const, x1, x2, ..., time_1, time_2, ...] (time 0 is reference)
        match fit_panel_lsdv_time(&endog, &exog_use, &entity_id, &time_id, constant, cov_type, cov_params.clone()) {
            Ok(pr) => {
                let mut lsdv_time_labels = all_labels_use.clone();
                for i in 1..time_names.len() {
                    lsdv_time_labels.push((time_series_name.clone(), Some(time_names[i].clone())));
                }
                let (kept_labels, lsdv_time_omit_info) = match &pr.omitted_indices {
                    Some(omitted) if !omitted.is_empty() => {
                        let kept: Vec<_> = (0..lsdv_time_labels.len())
                            .filter(|i| !omitted.contains(i))
                            .filter_map(|i| lsdv_time_labels.get(i).cloned())
                            .collect();
                        let omitted_vars: Vec<OmittedVariable> = omitted
                            .iter()
                            .filter_map(|&i| lsdv_time_labels.get(i))
                            .map(|(var, cat)| OmittedVariable {
                                variable: var.clone(),
                                category: cat.clone(),
                                reason: "collinearity".to_string(),
                            })
                            .collect();
                        let lsdv_omit = Some(OmitInfo { omitted: omitted_vars });
                        let merged = match (omit_info.as_ref(), &lsdv_omit) {
                            (Some(a), Some(b)) => Some(OmitInfo {
                                omitted: a.omitted.iter().chain(b.omitted.iter()).cloned().collect(),
                            }),
                            (Some(a), None) => Some(a.clone()),
                            (None, Some(b)) => Some(b.clone()),
                            (None, None) => None,
                        };
                        (kept, merged)
                    }
                    _ => (lsdv_time_labels.clone(), omit_info.clone()),
                };
                lsdv_time_result = Some(panel_result_to_ols_result(
                    &pr,
                    "Panel:LSDV(Time)",
                    "LSDV (Time Dummies)",
                    &endog_name,
                    &kept_labels,
                    0,
                    None,
                    lsdv_time_omit_info.as_ref(),
                ));
            }
            Err(e) => errors.lsdv_time = Some(e),
        }

        match fit_panel_fd(&endog, &exog_use, &entity_id, &time_id, &time_values, constant, cov_type, cov_params.clone()) {
            Ok(pr) => {
                fd_result = Some(panel_result_to_ols_result(
                    &pr,
                    "Panel:FD",
                    "First Difference",
                    &endog_name,
                    &all_labels_use,
                    1,
                    None,
                    omit_info.as_ref(),
                ));
            }
            Err(e) => errors.fd = Some(e),
        }

        // Helper to build kept_labels and merged omit_info from panel omitted_indices
        let re_omit_handling = |pr: &yss_sci::regression::panel::PanelOLSResult| {
            match &pr.omitted_indices {
                Some(omitted) if !omitted.is_empty() => {
                    let kept: Vec<_> = (0..all_labels_use.len())
                        .filter(|i| !omitted.contains(i))
                        .filter_map(|i| all_labels_use.get(i).cloned())
                        .collect();
                    let omitted_vars: Vec<OmittedVariable> = omitted
                        .iter()
                        .filter_map(|&i| all_labels_use.get(i))
                        .map(|(var, cat)| OmittedVariable {
                            variable: var.clone(),
                            category: cat.clone(),
                            reason: "collinearity".to_string(),
                        })
                        .collect();
                    let re_omit = Some(OmitInfo { omitted: omitted_vars });
                    let merged = match (omit_info.as_ref(), &re_omit) {
                        (Some(a), Some(b)) => Some(OmitInfo {
                            omitted: a.omitted.iter().chain(b.omitted.iter()).cloned().collect(),
                        }),
                        (Some(a), None) => Some(a.clone()),
                        (None, Some(b)) => Some(b.clone()),
                        (None, None) => None,
                    };
                    (kept, merged)
                }
                _ => (all_labels_use.clone(), omit_info.clone()),
            }
        };

        // RE: FGLS (Swamy-Arora)
        match fit_panel_re_fgls(&endog, &exog_use, &entity_id, constant, cov_type, cov_params.clone()) {
            Ok(pr) => {
                let (kept_labels, re_omit_info) = re_omit_handling(&pr);
                re_fgls_result = Some(panel_result_to_ols_result(
                    &pr,
                    "Panel:RE(FGLS)",
                    "Random Effects (FGLS)",
                    &endog_name,
                    &kept_labels,
                    0,
                    None,
                    re_omit_info.as_ref(),
                ));
            }
            Err(e) => errors.re_fgls = Some(e),
        }

        // RE: MLE
        match fit_panel_re_mle(&endog, &exog_use, &entity_id, constant) {
            Ok(pr) => {
                let (kept_labels, re_omit_info) = re_omit_handling(&pr);
                re_mle_result = Some(panel_result_to_ols_result(
                    &pr,
                    "Panel:RE(MLE)",
                    "Random Effects (MLE)",
                    &endog_name,
                    &kept_labels,
                    0,
                    None,
                    re_omit_info.as_ref(),
                ));
            }
            Err(e) => errors.re_mle = Some(e),
        }

        // RE: Between estimator
        match fit_panel_re_be(&endog, &exog_use, &entity_id, constant, cov_type, cov_params.clone()) {
            Ok(pr) => {
                let (kept_labels, re_omit_info) = re_omit_handling(&pr);
                re_be_result = Some(panel_result_to_ols_result(
                    &pr,
                    "Panel:RE(BE)",
                    "Random Effects (Between)",
                    &endog_name,
                    &kept_labels,
                    0,
                    None,
                    re_omit_info.as_ref(),
                ));
            }
            Err(e) => errors.re_be = Some(e),
        }

        // RE (Time): FGLS
        match fit_panel_re_fgls_time(&endog, &exog_use, &entity_id, &time_id, constant, cov_type, cov_params.clone()) {
            Ok(pr) => {
                let (kept_labels, re_omit_info) = re_omit_handling(&pr);
                re_fgls_time_result = Some(panel_result_to_ols_result(
                    &pr,
                    "Panel:RE(Time,FGLS)",
                    "Time Random Effects (FGLS)",
                    &endog_name,
                    &kept_labels,
                    0,
                    Some(pr.num_time_periods),
                    re_omit_info.as_ref(),
                ));
            }
            Err(e) => errors.re_fgls_time = Some(e),
        }

        // RE (Time): MLE
        match fit_panel_re_mle_time(&endog, &exog_use, &entity_id, &time_id, constant) {
            Ok(pr) => {
                let (kept_labels, re_omit_info) = re_omit_handling(&pr);
                re_mle_time_result = Some(panel_result_to_ols_result(
                    &pr,
                    "Panel:RE(Time,MLE)",
                    "Time Random Effects (MLE)",
                    &endog_name,
                    &kept_labels,
                    0,
                    Some(pr.num_time_periods),
                    re_omit_info.as_ref(),
                ));
            }
            Err(e) => errors.re_mle_time = Some(e),
        }

        // RE (Time): Between
        match fit_panel_re_be_time(&endog, &exog_use, &entity_id, &time_id, constant, cov_type, cov_params.clone()) {
            Ok(pr) => {
                let (kept_labels, re_omit_info) = re_omit_handling(&pr);
                re_be_time_result = Some(panel_result_to_ols_result(
                    &pr,
                    "Panel:RE(Time,BE)",
                    "Time Random Effects (Between)",
                    &endog_name,
                    &kept_labels,
                    0,
                    Some(pr.num_time_periods),
                    re_omit_info.as_ref(),
                ));
            }
            Err(e) => errors.re_be_time = Some(e),
        }

        // RE (Two-Way): FGLS, MLE, BE (stubs - not yet implemented)
        match fit_panel_re_fgls_twoway(&endog, &exog_use, &entity_id, &time_id, constant, cov_type, cov_params.clone()) {
            Ok(pr) => {
                let (kept_labels, re_omit_info) = re_omit_handling(&pr);
                re_fgls_twoway_result = Some(panel_result_to_ols_result(
                    &pr,
                    "Panel:RE(Two-Way,FGLS)",
                    "Two-Way Random Effects (FGLS)",
                    &endog_name,
                    &kept_labels,
                    0,
                    None,
                    re_omit_info.as_ref(),
                ));
            }
            Err(e) => errors.re_fgls_twoway = Some(e),
        }
        match fit_panel_re_mle_twoway(&endog, &exog_use, &entity_id, &time_id, constant) {
            Ok(pr) => {
                let (kept_labels, re_omit_info) = re_omit_handling(&pr);
                re_mle_twoway_result = Some(panel_result_to_ols_result(
                    &pr,
                    "Panel:RE(Two-Way,MLE)",
                    "Two-Way Random Effects (MLE)",
                    &endog_name,
                    &kept_labels,
                    0,
                    None,
                    re_omit_info.as_ref(),
                ));
            }
            Err(e) => errors.re_mle_twoway = Some(e),
        }
        let has_any = mixed_ols_result.is_some() || fe_result.is_some() || fe_time_result.is_some() || fe_twoway_result.is_some()
            || lsdv_result.is_some() || lsdv_time_result.is_some() || lsdv_twoway_result.is_some()
            || fd_result.is_some() || re_fgls_result.is_some() || re_mle_result.is_some() || re_be_result.is_some()
            || re_fgls_time_result.is_some() || re_mle_time_result.is_some() || re_be_time_result.is_some()
            || re_fgls_twoway_result.is_some() || re_mle_twoway_result.is_some();
        if !has_any {
            return Err(format!(
                "Panel Summary: all models failed. Mixed(OLS): {:?}, FE: {:?}, FE(Time): {:?}, FE(Two-Way): {:?}, LSDV: {:?}, LSDV(Time): {:?}, LSDV(Two-Way): {:?}, FD: {:?}, RE(FGLS): {:?}, RE(MLE): {:?}, RE(BE): {:?}, RE(Time,FGLS): {:?}, RE(Time,MLE): {:?}, RE(Time,BE): {:?}, RE(Two-Way,FGLS): {:?}, RE(Two-Way,MLE): {:?}",
                errors.mixed_ols, errors.fe, errors.fe_time, errors.fe_twoway, errors.lsdv, errors.lsdv_time, errors.lsdv_twoway, errors.fd, errors.re_fgls, errors.re_mle, errors.re_be, errors.re_fgls_time, errors.re_mle_time, errors.re_be_time, errors.re_fgls_twoway, errors.re_mle_twoway
            ));
        }

        let has_errors = errors.mixed_ols.is_some() || errors.fe.is_some() || errors.fe_time.is_some() || errors.fe_twoway.is_some()
            || errors.lsdv.is_some() || errors.lsdv_time.is_some() || errors.lsdv_twoway.is_some()
            || errors.fd.is_some() || errors.re_fgls.is_some() || errors.re_mle.is_some() || errors.re_be.is_some()
            || errors.re_fgls_time.is_some() || errors.re_mle_time.is_some() || errors.re_be_time.is_some()
            || errors.re_fgls_twoway.is_some() || errors.re_mle_twoway.is_some();

        let selection_tests = vec![
            pooled_vs_unrestricted_f_test(
                "poolability_entity",
                "model_choice",
                "Pooled OLS vs FE (Entity)",
                "H0: no entity fixed effects (all entity effects = 0)",
                mixed_ols_result.as_ref(),
                lsdv_result.as_ref(),
                "Reject pooled OLS for entity dimension; FE(entity) is preferred to pooled.",
                "Entity fixed effects are not strongly supported against pooled OLS.",
            ),
            breusch_pagan_lm_test(
                "re_entity_vs_pooled",
                "model_choice",
                "RE(Entity) vs Pooled OLS",
                "H0: sigma_u = 0 (no entity random effect)",
                pooled_ols_residuals.as_ref(),
                &entity_id,
                "Reject pooled OLS for entity dimension; RE(entity) is supported.",
                "Entity random effect is weak; pooled OLS remains plausible.",
            ),
            hausman_test(
                "hausman_entity",
                "model_choice",
                "Hausman: FE(Entity) vs RE(Entity)",
                "H0: RE is consistent (Cov(alpha_i, X_it)=0)",
                fe_result.as_ref(),
                re_fgls_result.as_ref(),
                "Prefer FE(entity): RE appears inconsistent under Hausman test.",
                "Prefer RE(entity): fail to reject RE consistency.",
            ),
            pooled_vs_unrestricted_f_test(
                "poolability_entity_effect",
                "effect_choice",
                "Need Entity Effect?",
                "H0: no entity effect",
                mixed_ols_result.as_ref(),
                lsdv_result.as_ref(),
                "Entity effect is supported.",
                "Entity effect is not strongly supported.",
            ),
            pooled_vs_unrestricted_f_test(
                "poolability_time_effect",
                "effect_choice",
                "Need Time Effect?",
                "H0: no time effect",
                mixed_ols_result.as_ref(),
                lsdv_time_result.as_ref(),
                "Time effect is supported.",
                "Time effect is not strongly supported.",
            ),
            pooled_vs_unrestricted_f_test(
                "poolability_twoway_effect",
                "effect_choice",
                "Need Two-Way Effect?",
                "H0: no joint entity/time effects",
                mixed_ols_result.as_ref(),
                lsdv_twoway_result.as_ref(),
                "Joint entity/time effects are supported.",
                "Two-way effects are not strongly supported against pooled.",
            ),
            breusch_pagan_lm_test(
                "re_time_vs_pooled",
                "effect_choice",
                "RE(Time) vs Pooled OLS",
                "H0: sigma_lambda = 0 (no time random effect)",
                pooled_ols_residuals.as_ref(),
                &time_id,
                "Time random effect is supported.",
                "Time random effect is weak; pooled remains plausible.",
            ),
            breusch_pagan_lm_test(
                "re_twoway_vs_pooled",
                "effect_choice",
                "RE(Two-Way) vs Pooled OLS",
                "H0: no two-way random effects",
                pooled_ols_residuals.as_ref(),
                &entity_id,
                "Two-way random effects are supported.",
                "Two-way random effects are weak; pooled remains plausible.",
            ),
            hausman_test(
                "hausman_time",
                "effect_choice",
                "Hausman: FE(Time) vs RE(Time)",
                "H0: RE(Time) is consistent",
                fe_time_result.as_ref(),
                re_fgls_time_result.as_ref(),
                "Prefer FE(time): RE(time) appears inconsistent.",
                "Prefer RE(time): fail to reject RE consistency.",
            ),
            hausman_test(
                "hausman_twoway",
                "effect_choice",
                "Hausman: FE(Two-Way) vs RE(Two-Way)",
                "H0: RE(Two-Way) is consistent",
                fe_twoway_result.as_ref(),
                re_fgls_twoway_result.as_ref(),
                "Prefer FE(two-way): RE(two-way) appears inconsistent.",
                "Prefer RE(two-way): fail to reject RE consistency.",
            ),
        ];

        let summary = PanelSummaryResult {
            title: "Panel Regression Results".to_string(),
            endog_name,
            mixed_ols: mixed_ols_result,
            fe: fe_result,
            fe_time: fe_time_result,
            fe_twoway: fe_twoway_result,
            lsdv: lsdv_result,
            lsdv_time: lsdv_time_result,
            lsdv_twoway: lsdv_twoway_result,
            fd: fd_result,
            re_fgls: re_fgls_result,
            re_mle: re_mle_result,
            re_be: re_be_result,
            re_fgls_time: re_fgls_time_result,
            re_mle_time: re_mle_time_result,
            re_be_time: re_be_time_result,
            re_fgls_twoway: re_fgls_twoway_result,
            re_mle_twoway: re_mle_twoway_result,
            selection_tests,
            errors: if has_errors { Some(errors) } else { None },
        };

        let json_data = serde_json::to_string(&summary)
            .map_err(|e| format!("Panel Summary: serialize: {}", e))?;

        ctx.publish_report(ReportKind::PanelSummary, json_data);
        ctx.log("Panel Summary: completed".to_string());
        Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
    }));
    registry.register(definition);
}
