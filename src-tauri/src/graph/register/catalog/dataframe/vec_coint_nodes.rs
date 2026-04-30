//! VEC 协整分析节点
//!
//! 对应 Stata vec x y z, lags(#) rank(#) trend(none|constant|trend) sindicators(varlist)

use crate::execution::context::NodeExecutionContextTrait;
use crate::execution::ExecutionEffect;
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{DataType, DataValue};
use ndarray::Array2;
use polars::prelude::DataType as PolarsDataType;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use yss_sci::ts::vec::{vec_estimate, VECConfig, VecLmarRow, VecStableRow, VecTrendSpec};

// ======================== 结构体 ========================

/// VEC 协整分析结果（Stata vec 风格）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VECSummaryResult {
    pub title: String,
    pub var_names: Vec<String>,
    pub num_observation: usize,
    pub log_likelihood: f64,
    pub aic: f64,
    pub hqic: f64,
    pub sbic: f64,
    pub det_sigma_ml: f64,
    pub rank: usize,
    pub lags: usize,
    pub trend_spec: String,
    pub equations: Vec<VECEquationDisplay>,
    pub coefficients: Vec<VECCoefDisplay>,
    pub beta: Vec<Vec<f64>>,
    /// beta 表变量名，与 beta 列对应（含 const）
    pub beta_var_names: Vec<String>,
    pub cointegrating_equations: Vec<VECCointegratingEquationDisplay>,
    /// beta 表 Stata 风格：Std. err., z, P>|z|, [95% conf. interval]
    pub beta_std_err: Vec<Vec<Option<f64>>>,
    pub beta_z_value: Vec<Vec<Option<f64>>>,
    pub beta_p_value: Vec<Vec<Option<f64>>>,
    pub beta_ci_lower: Vec<Vec<Option<f64>>>,
    pub beta_ci_upper: Vec<Vec<Option<f64>>>,
    /// veclmar: LM 残差自相关检验
    pub veclmar: Vec<VecLmarRow>,
    /// vecstable: 特征值平稳性检验
    pub vecstable: Vec<VecStableRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VECCointegratingEquationDisplay {
    pub eq_name: String,
    pub parms: usize,
    pub chi2: f64,
    pub p_chi2: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VECEquationDisplay {
    pub eq_name: String,
    pub parms: usize,
    pub rmse: f64,
    pub r_sq: f64,
    pub chi2: f64,
    pub p_chi2: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VECCoefDisplay {
    pub eq_name: String,
    pub variable: String,
    pub coef: f64,
    pub std_err: f64,
    pub z_value: f64,
    pub p_value: f64,
    pub ci_lower: f64,
    pub ci_upper: f64,
}

// ======================== 辅助 ========================

fn sindicators_dataframe_to_array2(
    df: &polars::prelude::DataFrame,
    expected_rows: usize,
) -> Result<(Vec<Vec<f64>>, Vec<String>), String> {
    let nrows = df.height();
    if nrows != expected_rows {
        return Err(format!(
            "VEC: sindicators DataFrame has {} rows, expected {} (must match Variables length)",
            nrows, expected_rows
        ));
    }
    let numeric_dtypes = [
        PolarsDataType::Float32,
        PolarsDataType::Float64,
        PolarsDataType::Int32,
        PolarsDataType::Int64,
        PolarsDataType::UInt32,
        PolarsDataType::UInt64,
    ];
    let mut columns: Vec<Vec<f64>> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    for col in df.columns() {
        if !numeric_dtypes.contains(&col.dtype()) {
            continue;
        }
        let s = col.cast(&PolarsDataType::Float64).map_err(|e| {
            format!(
                "VEC: sindicators column '{}' cast failed: {}",
                col.name(),
                e
            )
        })?;
        let f64_ca = s.f64().map_err(|e| format!("VEC: sindicators: {}", e))?;
        if f64_ca.null_count() > 0 {
            return Err(format!(
                "VEC: sindicators column '{}' contains nulls",
                col.name()
            ));
        }
        let vec: Vec<f64> = f64_ca.into_no_null_iter().collect();
        columns.push(vec);
        names.push(col.name().to_string());
    }
    if columns.is_empty() {
        return Err("VEC: sindicators DataFrame has no numeric columns".to_string());
    }
    Ok((columns, names))
}

fn vec_input_slots() -> Vec<PinSlot> {
    vec![
        PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
        PinSlot::repeatable(
            PinDefinition::data_input(
                "",
                DataRole::Inputs(0),
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
            ),
            "Variables",
            2,
            None,
        ),
        PinSlot::fixed(
            PinDefinition::data_input(
                "Lags",
                DataRole::Custom("lags".to_string()),
                PinDataTypeDefinition::concrete(DataType::Int64),
            )
            .with_optional(true)
            .with_default_value(DataValue::Int64(2)),
        ),
        PinSlot::fixed(
            PinDefinition::data_input(
                "Rank",
                DataRole::Custom("rank".to_string()),
                PinDataTypeDefinition::concrete(DataType::Int64),
            )
            .with_optional(true)
            .with_default_value(DataValue::Int64(1)),
        ),
        PinSlot::fixed(
            PinDefinition::data_input(
                "Constant",
                DataRole::Custom("constant".to_string()),
                PinDataTypeDefinition::concrete(DataType::Boolean),
            )
            .with_optional(true)
            .with_default_value(DataValue::Boolean(true)),
        ),
        PinSlot::fixed(
            PinDefinition::data_input(
                "Trend",
                DataRole::Custom("trend".to_string()),
                PinDataTypeDefinition::concrete(DataType::Boolean),
            )
            .with_optional(true)
            .with_default_value(DataValue::Boolean(false)),
        ),
        PinSlot::fixed(
            PinDefinition::data_input(
                "Sindicators",
                DataRole::Custom("sindicators".to_string()),
                PinDataTypeDefinition::concrete(DataType::DataFrame),
            )
            .with_optional(true),
        ),
        PinSlot::fixed(PinDefinition::data_output(
            "Result",
            DataRole::Result,
            PinDataTypeDefinition::concrete(DataType::Struct("VECSummaryResult".to_string())),
        )),
        PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)),
    ]
}

fn run_vec(ctx: &mut dyn NodeExecutionContextTrait) -> Result<VECSummaryResult, String> {
    let lags_val = ctx
        .get_input_by_role(&PinRole::Data(DataRole::Custom("lags".to_string())))
        .ok()
        .and_then(|v| v.as_i64())
        .unwrap_or(2);
    let lags_p = lags_val as usize;
    if lags_p < 1 {
        return Err("VEC: lags must be >= 1".to_string());
    }

    let rank_val = ctx
        .get_input_by_role(&PinRole::Data(DataRole::Custom("rank".to_string())))
        .ok()
        .and_then(|v| v.as_i64())
        .unwrap_or(1);
    let rank = rank_val as usize;

    let constant = ctx
        .get_input_by_role(&PinRole::Data(DataRole::Custom("constant".to_string())))
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let trend = ctx
        .get_input_by_role(&PinRole::Data(DataRole::Custom("trend".to_string())))
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let var_inputs = ctx.get_inputs_by_family(&PinRole::Data(DataRole::Inputs(0)))?;
    if var_inputs.len() < 2 {
        return Err("VEC: at least 2 variables required".to_string());
    }

    let mut series_list = Vec::new();
    let mut var_names = Vec::new();
    for v in &var_inputs {
        let dsv = match v {
            DataValue::DataSeries(s) => s.clone(),
            _ => return Err("VEC: each variable must be a DataSeries".to_string()),
        };
        let s = ctx.get_series(&dsv.id)?;
        let name = s.name().to_string();
        if name.is_empty() {
            var_names.push(format!("y{}", series_list.len()));
        } else {
            var_names.push(name);
        }
        let vals: Vec<f64> = s
            .cast(&PolarsDataType::Float64)
            .map_err(|e| format!("VEC: cannot cast to Float64: {}", e))?
            .f64()
            .map_err(|e| format!("VEC: {}", e))?
            .into_no_null_iter()
            .collect();
        series_list.push(vals);
    }

    let n = series_list[0].len();
    for (i, s) in series_list.iter().enumerate() {
        if s.len() != n {
            return Err(format!(
                "VEC: variable '{}' has {} rows, expected {}",
                var_names
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| format!("y{}", i)),
                s.len(),
                n
            ));
        }
    }

    let k = series_list.len();
    if rank >= k {
        return Err(format!(
            "VEC: rank({}) must be less than number of variables ({})",
            rank, k
        ));
    }

    let _sindicators =
        match ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("sindicators".to_string()))) {
            Ok(DataValue::DataFrame(id)) => {
                let df = ctx.get_dataframe(&id)?;
                let (cols, _names) = sindicators_dataframe_to_array2(df.as_ref(), n)?;
                Some(cols)
            }
            Ok(DataValue::Null) | Err(_) => None,
            _ => None,
        };

    if _sindicators.is_some() && !constant && !trend {
        return Err("VEC: sindicators cannot be specified with trend(none)".to_string());
    }

    let trend_spec = match (constant, trend) {
        (false, false) => "none",
        (true, false) => "constant",
        (true, true) => "trend",
        (false, true) => "trend",
    }
    .to_string();

    let trend_spec_enum = match (constant, trend) {
        (false, false) => VecTrendSpec::None,
        (true, false) => VecTrendSpec::Constant,
        (true, true) | (false, true) => VecTrendSpec::Trend,
    };

    let y = Array2::from_shape_fn((n, k), |(i, j)| series_list[j][i]);

    let sindicators_arr = _sindicators.as_ref().map(|cols| {
        let n_si = cols.len();
        Array2::from_shape_fn((n, n_si), |(i, j)| cols[j][i])
    });

    let config = VECConfig {
        trend_spec: trend_spec_enum,
        lags: lags_p,
        rank,
        mlag: 2,
    };

    let result = vec_estimate(
        &y,
        &config,
        Some(var_names.clone()),
        sindicators_arr.as_ref(),
    )?;

    let equations: Vec<VECEquationDisplay> = result
        .equations
        .iter()
        .map(|e| VECEquationDisplay {
            eq_name: e.eq_name.clone(),
            parms: e.parms,
            rmse: e.rmse,
            r_sq: e.r_sq,
            chi2: e.chi2,
            p_chi2: e.p_chi2,
        })
        .collect();

    let mut coefficients = Vec::new();
    for (eq_idx, eq_name) in result.equations.iter().map(|e| &e.eq_name).enumerate() {
        for (j, label) in result.coef_labels[eq_idx].iter().enumerate() {
            coefficients.push(VECCoefDisplay {
                eq_name: eq_name.clone(),
                variable: label.clone(),
                coef: result.coefficients[eq_idx][j],
                std_err: result.std_errs[eq_idx][j],
                z_value: result.z_values[eq_idx][j],
                p_value: result.p_values[eq_idx][j],
                ci_lower: result.ci_lower[eq_idx][j],
                ci_upper: result.ci_upper[eq_idx][j],
            });
        }
    }

    let beta_transposed: Vec<Vec<f64>> = (0..result.rank)
        .map(|j| (0..result.beta.len()).map(|i| result.beta[i][j]).collect())
        .collect();

    let beta_var_names: Vec<String> = (0..result.beta.len())
        .map(|i| {
            if i < result.var_names.len() {
                result.var_names[i].clone()
            } else {
                "const".to_string()
            }
        })
        .collect();

    let cointegrating_equations: Vec<VECCointegratingEquationDisplay> = result
        .cointegrating_equations
        .iter()
        .map(|ce| VECCointegratingEquationDisplay {
            eq_name: ce.eq_name.clone(),
            parms: ce.parms,
            chi2: ce.chi2,
            p_chi2: ce.p_chi2,
        })
        .collect();

    // beta_std_err 等与 beta 同结构，转置为 r×n_vars（n_vars 含 const）
    let beta_std_err_t: Vec<Vec<Option<f64>>> = (0..result.rank)
        .map(|j| {
            (0..result.beta_std_err.len())
                .map(|i| result.beta_std_err[i][j])
                .collect()
        })
        .collect();
    let beta_z_value_t: Vec<Vec<Option<f64>>> = (0..result.rank)
        .map(|j| {
            (0..result.beta_z_value.len())
                .map(|i| result.beta_z_value[i][j])
                .collect()
        })
        .collect();
    let beta_p_value_t: Vec<Vec<Option<f64>>> = (0..result.rank)
        .map(|j| {
            (0..result.beta_p_value.len())
                .map(|i| result.beta_p_value[i][j])
                .collect()
        })
        .collect();
    let beta_ci_lower_t: Vec<Vec<Option<f64>>> = (0..result.rank)
        .map(|j| {
            (0..result.beta_ci_lower.len())
                .map(|i| result.beta_ci_lower[i][j])
                .collect()
        })
        .collect();
    let beta_ci_upper_t: Vec<Vec<Option<f64>>> = (0..result.rank)
        .map(|j| {
            (0..result.beta_ci_upper.len())
                .map(|i| result.beta_ci_upper[i][j])
                .collect()
        })
        .collect();

    Ok(VECSummaryResult {
        title: "Vector Error-Correction Model".to_string(),
        var_names: result.var_names,
        num_observation: result.num_observation,
        log_likelihood: result.log_likelihood,
        aic: result.aic,
        hqic: result.hqic,
        sbic: result.sbic,
        det_sigma_ml: result.det_sigma_ml,
        rank: result.rank,
        lags: result.lags,
        trend_spec,
        equations,
        coefficients,
        beta: beta_transposed,
        beta_var_names,
        cointegrating_equations,
        beta_std_err: beta_std_err_t,
        beta_z_value: beta_z_value_t,
        beta_p_value: beta_p_value_t,
        beta_ci_lower: beta_ci_lower_t,
        beta_ci_upper: beta_ci_upper_t,
        veclmar: result.veclmar,
        vecstable: result.vecstable,
    })
}

pub fn register(registry: &NodeRegistry) {
    let slots = vec_input_slots();

    let definition = NodeDefinition::new(
        "VEC (Cointegration)",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Vector Error-Correction model — Johansen cointegration (Stata vec)")
    .with_pin_slots(slots)
    .with_flow_processor(Arc::new(|ctx| {
        let result = run_vec(ctx)?;

        let json_data =
            serde_json::to_string(&result).map_err(|e| format!("VEC: serialize failed: {}", e))?;

        let result_handle_id = ctx.put_handle(Box::new(result.clone()));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Result),
            DataValue::new_struct("VECSummaryResult", result_handle_id),
        )?;

        ctx.open_window("vec_summary".to_string(), json_data);
        ctx.log("VEC: completed".to_string());
        Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
    }));
    registry.register(definition);
}
