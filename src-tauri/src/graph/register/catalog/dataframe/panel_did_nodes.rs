//! 单期 DID（2×2）— **双向固定效应**（entity + time），与 Stata `reghdfe Y x i.treat#i.post, absorb(id t)` 一类设定一致。
//! 回归项：**可选 X + Treat×Post**（及截距）。`treat`、`post` 主效应在 TWFE 下通常被个体/时间 FE 吸收，不放入设计矩阵以免 within 后全零列。

use super::info_nodes::{OLSResult, OmitInfo, OmittedVariable};
use super::panel_did_auxiliary::{
    DidEventStudyPoint, adoption_time_ord, run_parallel_trends_test, run_placebo_test,
};
use super::panel_did_engine::{DidFakeGroupEnginePayload, ExogLabelEntry};
use super::panel_nodes::{PanelConfigure, panel_result_to_ols_result, series_to_group_indices};
use crate::execution::ExecutionEffect;
use crate::execution::context::NodeExecutionContextTrait;
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{DataSeriesValue, DataType, DataValue};
use ndarray::{Array1, Array2};
use polars::prelude::{Column, DataFrame, DataType as PDataType, NamedFrom, Series};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use yss_sci::regression::collinearity;
use yss_sci::regression::panel::fit_panel_fe_twoway;

/// Parallel trends: Wald χ² on pre-policy event-study interactions (Stata `test` on leads).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DidParallelTrendsBlock {
    pub available: bool,
    pub chi2: Option<f64>,
    pub df: Option<usize>,
    pub p_value: Option<f64>,
    pub reference_rel: Option<i32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tested_rel_periods: Vec<i32>,
    /// Event-study coefficients (same TWFE as Wald); reference period coef = 0 (omitted).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub event_study: Vec<DidEventStudyPoint>,
    pub method_note: String,
}

/// Placebo timing: coef on treated×fake pre window (should be ≈0 under parallel trends).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DidPlaceboTimingBlock {
    pub available: bool,
    pub coef: Option<f64>,
    pub std_err: Option<f64>,
    pub t_value: Option<f64>,
    pub p_value: Option<f64>,
    pub horizon: usize,
    pub method_note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelDidResult {
    pub kind: String,
    pub title: String,
    pub endog_name: String,
    pub treat_name: String,
    pub post_name: String,
    pub fe_twoway: Option<OLSResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_trends: Option<DidParallelTrendsBlock>,
    /// 虚构政策时点：政策前 H 期 × 真实处理组
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placebo: Option<DidPlaceboTimingBlock>,
    /// 供结果页「虚构处理组」按钮按需调用后端；不含置换结果本身
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fake_group_engine: Option<DidFakeGroupEnginePayload>,
}

fn bool_series_to_f64(series: &Series) -> Result<Vec<f64>, String> {
    if series.dtype() != &PDataType::Boolean {
        return Err(format!(
            "DID: Treat and Post must be Boolean DataSeries, got {:?}",
            series.dtype()
        ));
    }
    let ca = series.bool().map_err(|e| e.to_string())?;
    ca.into_iter()
        .enumerate()
        .map(|(i, o)| {
            o.ok_or_else(|| format!("DID: Treat/Post contains null at row {}", i))
                .map(|b| if b { 1.0 } else { 0.0 })
        })
        .collect()
}

fn panel_did_input_slots() -> Vec<PinSlot> {
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
            0,
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
        PinSlot::fixed(PinDefinition::data_input(
            "Treat",
            DataRole::Custom("treat".to_string()),
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Boolean))),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "Post",
            DataRole::Custom("post".to_string()),
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Boolean))),
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

/// 与 Panel Summary 相同的数据对齐；Treat/Post 转为 0/1 后构造 **Treat×Post** 列参与回归（不单独放 treat/post 主效应，见模块说明）。
/// 返回 `t_adopt`：首个出现 `post==1` 的日历时间序号（与 `time_after` 序一致）；`treat_f` 为处理组 0/1。
fn build_panel_did_data(
    ctx: &mut dyn NodeExecutionContextTrait,
    constant: bool,
) -> Result<
    (
        Array1<f64>,
        Array2<f64>,
        Vec<usize>,
        Vec<usize>,
        Vec<(String, Option<String>)>,
        String,
        bool,
        String,
        String,
        usize,
        Vec<f64>,
        Vec<f64>,
    ),
    String,
> {
    let endog_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("y".to_string())))?;
    let endog_id = match &endog_value {
        DataValue::DataSeries(v) => v.id.clone(),
        _ => return Err("DID: Y must be a DataSeries".to_string()),
    };
    let endog_series = ctx.get_series(&endog_id)?;
    let endog_name = {
        let raw = endog_series.name().to_string();
        if raw.is_empty() { "y".to_string() } else { raw }
    };
    let endog_f64 = endog_series
        .cast(&PDataType::Float64)
        .map_err(|e| format!("DID: Y cast: {}", e))?;

    let entity_value =
        ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("entity_id".to_string())))?;
    let entity_id_str = match &entity_value {
        DataValue::DataSeries(v) => v.id.clone(),
        _ => return Err("DID: Entity ID must be a DataSeries".to_string()),
    };
    let entity_id = series_to_group_indices(ctx, &entity_id_str)?;

    let time_value =
        ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("time_id".to_string())))?;
    let time_id_str = match &time_value {
        DataValue::DataSeries(v) => v.id.clone(),
        _ => return Err("DID: Time ID must be a DataSeries".to_string()),
    };
    let time_id = series_to_group_indices(ctx, &time_id_str)?;

    let treat_value =
        ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("treat".to_string())))?;
    let treat_id = match &treat_value {
        DataValue::DataSeries(v) => v.id.clone(),
        _ => return Err("DID: Treat must be a DataSeries".to_string()),
    };
    let treat_series = ctx.get_series(&treat_id)?;
    let treat_name = {
        let raw = treat_series.name().to_string();
        if raw.is_empty() {
            "treat".to_string()
        } else {
            raw
        }
    };

    let post_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("post".to_string())))?;
    let post_id = match &post_value {
        DataValue::DataSeries(v) => v.id.clone(),
        _ => return Err("DID: Post must be a DataSeries".to_string()),
    };
    let post_series = ctx.get_series(&post_id)?;
    let post_name = {
        let raw = post_series.name().to_string();
        if raw.is_empty() {
            "post".to_string()
        } else {
            raw
        }
    };

    let n_raw = endog_f64.len();
    if entity_id.len() != n_raw || time_id.len() != n_raw {
        return Err(format!(
            "DID: Entity ID ({}), Time ID ({}), Y ({}) must have same length",
            entity_id.len(),
            time_id.len(),
            n_raw
        ));
    }
    if treat_series.len() != n_raw || post_series.len() != n_raw {
        return Err(format!(
            "DID: Treat ({}), Post ({}), Y ({}) must have same length",
            treat_series.len(),
            post_series.len(),
            n_raw
        ));
    }

    let exog_values = ctx.get_inputs_by_family(&PinRole::Data(DataRole::Inputs(0)))?;

    let mut df_cols: Vec<Column> = vec![Column::from(endog_f64.with_name("__endog__".into()))];
    let mut exog_meta: Vec<(String, bool, DataSeriesValue)> = Vec::new();
    for (i, val) in exog_values.iter().enumerate() {
        let dsv = match val {
            DataValue::DataSeries(v) => v.clone(),
            _ => return Err(format!("DID: X {} is not a DataSeries", i)),
        };
        let series = ctx.get_series(&dsv.id)?;
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
                "DID: X '{}' len {} != Y len {}",
                name,
                series.len(),
                n_raw
            ));
        }
        let is_cat = matches!(
            series.dtype(),
            PDataType::Categorical(_, _) | PDataType::Enum(_, _)
        );
        let col_series = if is_cat {
            series.cast(&PDataType::String).map_err(|e| e.to_string())?
        } else {
            series
                .cast(&PDataType::Float64)
                .map_err(|e| e.to_string())?
        };
        df_cols.push(Column::from(col_series.with_name(name.as_str().into())));
        exog_meta.push((name, is_cat, dsv));
    }

    let treat_f = bool_series_to_f64(&treat_series)?;
    let post_f = bool_series_to_f64(&post_series)?;
    df_cols.push(Column::from(Series::new("__treat__".into(), treat_f)));
    df_cols.push(Column::from(Series::new("__post__".into(), post_f)));

    df_cols.push(Column::from(
        ctx.get_series(&entity_id_str)?
            .with_name("__entity__".into()),
    ));
    df_cols.push(Column::from(
        ctx.get_series(&time_id_str)?.with_name("__time__".into()),
    ));

    let df = DataFrame::new(n_raw, df_cols)
        .map_err(|e| format!("DID: {}", e))?
        .drop_nulls::<&str>(None)
        .map_err(|e| format!("DID: drop_nulls: {}", e))?
        .sort(["__entity__", "__time__"], Default::default())
        .map_err(|e| format!("DID: sort: {}", e))?;

    let n = df.height();
    if n == 0 {
        return Err("DID: no valid observations after dropping nulls".to_string());
    }

    let endog_vec: Vec<f64> = df
        .column("__endog__")
        .map_err(|e| format!("DID: {}", e))?
        .f64()
        .map_err(|e| format!("DID: {}", e))?
        .into_no_null_iter()
        .collect();

    let entity_after: Vec<usize> = {
        let s = df.column("__entity__").map_err(|e| format!("DID: {}", e))?;
        let mut m: HashMap<String, usize> = HashMap::new();
        let mut idx_to_name: Vec<String> = Vec::new();
        let mut out = Vec::with_capacity(n);
        if matches!(
            s.dtype(),
            PDataType::Categorical(_, _) | PDataType::Enum(_, _)
        ) {
            let str_s = s
                .cast(&PDataType::String)
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
        out
    };

    let time_after: Vec<usize> = {
        let s = df.column("__time__").map_err(|e| format!("DID: {}", e))?;
        if matches!(
            s.dtype(),
            PDataType::Categorical(_, _) | PDataType::Enum(_, _)
        ) {
            let str_s = s
                .cast(&PDataType::String)
                .map_err(|e: polars::error::PolarsError| e.to_string())?;
            let ca = str_s
                .str()
                .map_err(|e: polars::error::PolarsError| e.to_string())?;
            let values: Vec<String> = ca
                .into_iter()
                .map(|opt| opt.ok_or("null").map(|s: &str| s.to_string()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| "DID: time contains null")?;
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
            values.iter().map(|k| *m.get(k).unwrap_or(&0)).collect()
        } else if s.dtype() == &PDataType::Int64 {
            let ca = s
                .i64()
                .map_err(|e: polars::error::PolarsError| e.to_string())?;
            let values: Vec<i64> = ca
                .into_iter()
                .map(|opt| opt.ok_or("DID: time contains null"))
                .collect::<Result<Vec<_>, _>>()?;
            let mut unique: Vec<i64> = Vec::new();
            for v in &values {
                if !unique.contains(v) {
                    unique.push(*v);
                }
            }
            unique.sort_unstable();
            let m: HashMap<i64, usize> = unique.iter().enumerate().map(|(i, &k)| (k, i)).collect();
            values.iter().map(|k| *m.get(k).unwrap_or(&0)).collect()
        } else if s.dtype() == &PDataType::Date {
            let ca = s
                .date()
                .map_err(|e: polars::error::PolarsError| e.to_string())?;
            let physical = ca.physical();
            let values: Vec<i32> = physical
                .into_iter()
                .map(|opt| opt.ok_or("DID: time contains null"))
                .collect::<Result<Vec<_>, _>>()?;
            let mut unique: Vec<i32> = Vec::new();
            for v in &values {
                if !unique.contains(v) {
                    unique.push(*v);
                }
            }
            unique.sort_unstable();
            let m: HashMap<i32, usize> = unique.iter().enumerate().map(|(i, &k)| (k, i)).collect();
            values.iter().map(|k| *m.get(k).unwrap_or(&0)).collect()
        } else {
            return Err("DID: Time ID must be Categorical, Int64, or Date".to_string());
        }
    };

    let has_constant = constant;
    let mut exog_columns: Vec<Vec<f64>> = Vec::new();
    let mut all_labels: Vec<(String, Option<String>)> = vec![("const".to_string(), None)];

    for (series_name, is_categorical, _) in exog_meta {
        let col = df.column(&series_name).map_err(|e| format!("DID: {}", e))?;
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
                    "DID: categorical '{}' needs >= 2 unique values",
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

    let t_col: Vec<f64> = df
        .column("__treat__")
        .map_err(|e| format!("DID: {}", e))?
        .f64()
        .map_err(|e| format!("DID: {}", e))?
        .into_no_null_iter()
        .collect();
    let p_col: Vec<f64> = df
        .column("__post__")
        .map_err(|e| format!("DID: {}", e))?
        .f64()
        .map_err(|e| format!("DID: {}", e))?
        .into_no_null_iter()
        .collect();
    let did_col: Vec<f64> = t_col.iter().zip(p_col.iter()).map(|(a, b)| a * b).collect();
    let did_min = did_col.iter().copied().fold(f64::INFINITY, f64::min);
    let did_max = did_col.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if did_max - did_min <= 1e-12 {
        return Err(
            "DID: Treat×Post is constant on the estimation sample; cannot identify a treatment effect"
                .to_string(),
        );
    }

    let t_adopt = adoption_time_ord(&time_after, &p_col).ok_or_else(|| {
        "DID: cannot define adoption date (no observation with post==1)".to_string()
    })?;
    let treat_f: Vec<f64> = t_col.clone();

    // TWFE absorbs unit-invariant Treat and common Post; mains would be zero after within transform.
    exog_columns.push(did_col);
    all_labels.push((format!("{}×{}", treat_name, post_name), None));

    let k = exog_columns.len() + 1;
    let mut exog_raw = Vec::with_capacity(n * k);
    for i in 0..n {
        exog_raw.push(1.0);
        for col in &exog_columns {
            exog_raw.push(col[i]);
        }
    }
    let exog = Array2::from_shape_vec((n, k), exog_raw)
        .map_err(|e| format!("DID: exog shape: {:?}", e))?;

    let endog = Array1::from(endog_vec);
    let post_f = p_col;
    Ok((
        endog,
        exog,
        entity_after,
        time_after,
        all_labels,
        endog_name,
        has_constant,
        treat_name,
        post_name,
        t_adopt,
        treat_f,
        post_f,
    ))
}

pub fn register(registry: &NodeRegistry) {
    let mut slots = panel_did_input_slots();
    slots.push(PinSlot::fixed(PinDefinition::data_output(
        "Result",
        DataRole::Result,
        PinDataTypeDefinition::concrete(DataType::Struct("PanelDidResult".to_string())),
    )));
    slots.push(PinSlot::fixed(PinDefinition::exec_output(
        "Out",
        ExecRole::ExecOut,
    )));

    let definition = NodeDefinition::new(
        "Panel DID (TWFE)",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_localized_description(
        "2×2 DID 双向固定效应（entity + time）。对 Y 回归可选 X 与 Treat×Post（主效应被 FE 吸收）；Treat×Post 系数为 DID 估计量。",
        "2×2 DID with two-way FE (entity + time). Regresses Y on optional X and Treat×Post only (main effects absorbed by FE). Coef on Treat×Post is the DID estimate.",
    )
    .with_pin_slots(slots)
    .with_flow_processor(Arc::new(|ctx| {
        let config = match ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("panel_config".to_string()))) {
            Ok(v) => v
                .as_handle_id()
                .and_then(|id| ctx.get_handle(id).ok())
                .and_then(|h| h.downcast_ref::<PanelConfigure>().cloned()),
            Err(_) => None,
        };
        let constant = config.as_ref().map(|c| c.constant).unwrap_or(true);
        let cov_type = config
            .as_ref()
            .map(|c| c.cov_type.as_str())
            .unwrap_or("cluster");

        let (endog, exog, entity_id, time_id, all_labels, endog_name, _, treat_name, post_name, t_adopt, treat_f, post_f) =
            match build_panel_did_data(ctx, constant) {
                Ok(v) => v,
                Err(e) => {
                    let err_result = PanelDidResult {
                        kind: "panel_did".to_string(),
                        title: "Panel DID (TWFE)".to_string(),
                        endog_name: "y".to_string(),
                        treat_name: "treat".to_string(),
                        post_name: "post".to_string(),
                        fe_twoway: None,
                        error: Some(e.clone()),
                        parallel_trends: None,
                        placebo: None,
                        fake_group_engine: None,
                    };
                    let json = serde_json::to_string(&err_result)
                        .map_err(|se| format!("DID: serialize: {}", se))?;
                    ctx.open_window("panel_did".to_string(), json);
                    return Err(e);
                }
            };

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

        let x_ncols = exog_use.ncols().saturating_sub(2);
        let x_labels: Vec<(String, Option<String>)> = all_labels_use
            .iter()
            .skip(1)
            .take(x_ncols)
            .cloned()
            .collect();

        let fe_out = match fit_panel_fe_twoway(
            &endog,
            &exog_use,
            &entity_id,
            &time_id,
            constant,
            cov_type,
            cov_params,
        ) {
            Ok(pr) => {
                let (kept_labels, fe_omit) = match &pr.omitted_indices {
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
                Some(panel_result_to_ols_result(
                    &pr,
                    "Panel:DID",
                    "Two-Way FE (DID)",
                    &endog_name,
                    &kept_labels,
                    0,
                    None,
                    fe_omit.as_ref(),
                ))
            }
            Err(e) => {
                let err_result = PanelDidResult {
                    kind: "panel_did".to_string(),
                    title: "Panel DID (TWFE)".to_string(),
                    endog_name: endog_name.clone(),
                    treat_name: treat_name.clone(),
                    post_name: post_name.clone(),
                    fe_twoway: None,
                    error: Some(e.clone()),
                    parallel_trends: None,
                    placebo: None,
                    fake_group_engine: None,
                };
                let json = serde_json::to_string(&err_result)
                    .map_err(|se| format!("DID: serialize: {}", se))?;
                ctx.open_window("panel_did".to_string(), json);
                return Err(e);
            }
        };

        let run_parallel = config.as_ref().map(|c| c.did_parallel_trends).unwrap_or(true);
        let run_placebo = config.as_ref().map(|c| c.did_placebo).unwrap_or(true);
        let placebo_horizon_cfg = config
            .as_ref()
            .map(|c| c.did_placebo_horizon.max(1))
            .unwrap_or(1);

        let parallel_trends = if run_parallel {
            Some(match run_parallel_trends_test(
                &endog,
                &exog_use,
                x_ncols,
                &x_labels,
                &entity_id,
                &time_id,
                &time_id,
                t_adopt,
                &treat_f,
                constant,
                cov_type,
            ) {
                Ok((chi2, df, p, k_ref, tested, method_note, event_study)) => DidParallelTrendsBlock {
                    available: true,
                    chi2: Some(chi2),
                    df: Some(df),
                    p_value: Some(p),
                    reference_rel: Some(k_ref),
                    tested_rel_periods: tested,
                    event_study,
                    method_note,
                },
                Err(msg) => DidParallelTrendsBlock {
                    available: false,
                    chi2: None,
                    df: None,
                    p_value: None,
                    reference_rel: None,
                    tested_rel_periods: Vec::new(),
                    event_study: Vec::new(),
                    method_note: msg,
                },
            })
        } else {
            Some(DidParallelTrendsBlock {
                available: false,
                chi2: None,
                df: None,
                p_value: None,
                reference_rel: None,
                tested_rel_periods: Vec::new(),
                event_study: Vec::new(),
                method_note: "Disabled: set did_parallel_trends=true in Panel Configure.".to_string(),
            })
        };

        let placebo_horizon = placebo_horizon_cfg;
        let placebo = if run_placebo {
            Some(match run_placebo_test(
                &endog,
                &exog_use,
                x_ncols,
                &x_labels,
                &entity_id,
                &time_id,
                &time_id,
                t_adopt,
                &treat_f,
                placebo_horizon,
                constant,
                cov_type,
                &treat_name,
            ) {
                Ok((coef, std_err, t_value, p_value, method_note)) => DidPlaceboTimingBlock {
                    available: true,
                    coef: Some(coef),
                    std_err: Some(std_err),
                    t_value: Some(t_value),
                    p_value: Some(p_value),
                    horizon: placebo_horizon,
                    method_note,
                },
                Err(msg) => DidPlaceboTimingBlock {
                    available: false,
                    coef: None,
                    std_err: None,
                    t_value: None,
                    p_value: None,
                    horizon: placebo_horizon,
                    method_note: msg,
                },
            })
        } else {
            Some(DidPlaceboTimingBlock {
                available: false,
                coef: None,
                std_err: None,
                t_value: None,
                p_value: None,
                horizon: placebo_horizon,
                method_note: "Disabled: set did_placebo=true in Panel Configure (虚构政策时点).".to_string(),
            })
        };

        let did_label = format!("{}×{}", treat_name, post_name);
        let observed_did_coef = fe_out.as_ref().and_then(|ols| {
            ols.coefficients
                .iter()
                .find(|c| c.variable == did_label && c.category.is_none())
                .map(|c| c.coef)
        });

        let fake_group_engine = match observed_did_coef {
            Some(coef_obs) => {
                let ncols = exog_use.ncols();
                let exog_row_major: Vec<f64> = exog_use.iter().copied().collect();
                let all_labels: Vec<ExogLabelEntry> = all_labels_use
                    .iter()
                    .map(|(variable, category)| ExogLabelEntry {
                        variable: variable.clone(),
                        category: category.clone(),
                    })
                    .collect();
                Some(DidFakeGroupEnginePayload {
                    endog: endog.to_vec(),
                    exog_row_major,
                    ncols,
                    all_labels,
                    entity_id: entity_id.clone(),
                    time_id: time_id.clone(),
                    post: post_f.clone(),
                    treat: treat_f.clone(),
                    did_label,
                    observed_coef: coef_obs,
                    constant,
                    cov_type: cov_type.to_string(),
                })
            }
            None => None,
        };

        let result = PanelDidResult {
            kind: "panel_did".to_string(),
            title: "Panel DID (TWFE)".to_string(),
            endog_name,
            treat_name,
            post_name,
            fe_twoway: fe_out,
            error: None,
            parallel_trends,
            placebo,
            fake_group_engine,
        };

        let json_data =
            serde_json::to_string(&result).map_err(|e| format!("DID: serialize: {}", e))?;
        ctx.open_window("panel_did".to_string(), json_data);
        ctx.log("Panel DID (TWFE): completed".to_string());
        Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
    }));
    registry.register(definition);
}
