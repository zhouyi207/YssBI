//! VAR 向量自回归节点
//!
//! 实现与 Stata varbasic 一致：VAR(p) 估计、正交化 IRF、FEVD。

use crate::execution::ExecutionEffect;
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::value::DataValue;
use ndarray::Array2;
use polars::prelude::DataType;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use yss_sci::ts::var::{VAR, VARConfig};

// ======================== 结构体 ========================

/// varlmar 单行（LM 残差自相关检验）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VARLmarDisplay {
    pub lag: usize,
    pub chi2: f64,
    pub df: usize,
    pub p_value: f64,
}

/// varwle 单行（Wald lag-exclusion）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VARWleDisplay {
    pub eq_name: String,
    pub lag: usize,
    pub chi2: f64,
    pub df: usize,
    pub p_value: f64,
}

/// varstable 单行（特征值平稳性检验，Stata varstable 命令）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VARStableDisplay {
    pub re: f64,
    pub im: f64,
    pub modulus: f64,
}

/// vargranger 单行（格兰杰因果 Wald 检验，Stata vargranger 命令）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VARGrangerDisplay {
    pub eq_name: String,
    pub excluded: String,
    pub chi2: f64,
    pub df: usize,
    pub p_value: f64,
}

/// VAR Summary 窗口展示用（与 OLS Summary 形式类似）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VARSummaryResult {
    pub title: String,
    pub var_names: Vec<String>,
    pub num_observation: usize,
    pub log_likelihood: f64,
    pub aic: f64,
    pub fpe: f64,
    pub hqic: f64,
    pub sbic: f64,
    pub det_sigma_ml: f64,
    pub equations: Vec<VAREquationDisplay>,
    pub coefficients: Vec<VARCoefDisplay>,
    pub sigma: Vec<Vec<f64>>,
    pub oirf: Vec<Vec<Vec<f64>>>,
    pub fevd: Vec<Vec<Vec<f64>>>,
    pub varwle: Vec<VARWleDisplay>,
    pub varlmar: Vec<VARLmarDisplay>,
    pub varstable: Vec<VARStableDisplay>,
    pub vargranger: Vec<VARGrangerDisplay>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VAREquationDisplay {
    pub eq_name: String,
    pub parms: usize,
    pub rmse: f64,
    pub r_sq: f64,
    pub chi2: f64,
    pub p_chi2: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VARCoefDisplay {
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

/// 从 DataFrame 提取数值列作为外生变量矩阵 (T × M)，列名作为 exog_names
fn exog_dataframe_to_array2(
    df: &polars::prelude::DataFrame,
    expected_rows: usize,
) -> Result<(Array2<f64>, Vec<String>), String> {
    let nrows = df.height();
    if nrows != expected_rows {
        return Err(format!(
            "VAR: exog DataFrame has {} rows, expected {} (must match Variables length)",
            nrows, expected_rows
        ));
    }
    let numeric_dtypes = [
        DataType::Float32,
        DataType::Float64,
        DataType::Int32,
        DataType::Int64,
        DataType::UInt32,
        DataType::UInt64,
    ];
    let mut columns: Vec<Vec<f64>> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    for col in df.columns() {
        if !numeric_dtypes.contains(&col.dtype()) {
            continue;
        }
        let s = col
            .cast(&polars::prelude::DataType::Float64)
            .map_err(|e| format!("VAR: exog column '{}' cast failed: {}", col.name(), e))?;
        let f64_ca = s.f64().map_err(|e| format!("VAR: exog: {}", e))?;
        if f64_ca.null_count() > 0 {
            return Err(format!(
                "VAR: exog column '{}' contains nulls, fill or drop nulls first",
                col.name()
            ));
        }
        let vec: Vec<f64> = f64_ca.into_no_null_iter().collect();
        if vec.len() != nrows {
            return Err(format!(
                "VAR: exog column '{}' has {} rows, expected {}",
                col.name(),
                vec.len(),
                nrows
            ));
        }
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

    let mut series_list = Vec::new();
    let mut var_names = Vec::new();
    for v in &var_inputs {
        let dsv = match v {
            crate::graph::value::DataValue::DataSeries(s) => s.clone(),
            _ => return Err("VAR: each variable must be a DataSeries".to_string()),
        };
        let s = ctx.get_series(&dsv.id)?;
        let name = s.name().to_string();
        if name.is_empty() {
            var_names.push(format!("y{}", series_list.len()));
        } else {
            var_names.push(name);
        }
        let vals: Vec<f64> = s
            .cast(&polars::prelude::DataType::Float64)
            .map_err(|e| format!("VAR: cannot cast to Float64: {}", e))?
            .f64()
            .map_err(|e| format!("VAR: {}", e))?
            .into_no_null_iter()
            .collect();
        series_list.push(vals);
    }

    let n = series_list[0].len();
    for (i, s) in series_list.iter().enumerate() {
        if s.len() != n {
            return Err(format!(
                "VAR: variable '{}' has {} rows, expected {}",
                var_names.get(i).cloned().unwrap_or_else(|| format!("y{}", i)),
                s.len(),
                n
            ));
        }
    }

    let k = series_list.len();
    let mut y_data = Vec::with_capacity(n * k);
    for i in 0..n {
        for j in 0..k {
            y_data.push(series_list[j][i]);
        }
    }
    let y = Array2::from_shape_vec((n, k), y_data)
        .map_err(|e| format!("VAR: failed to build Y matrix: {}", e))?;

    let (exog, exog_names) = match ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("exog_df".to_string()))) {
        Ok(DataValue::DataFrame(id)) => {
            let df = ctx.get_dataframe(&id)?;
            let (arr, names) = exog_dataframe_to_array2(df.as_ref(), n)?;
            (Some(arr), Some(names))
        }
        Ok(DataValue::Null) | Err(_) => (None, None),
        _ => (None, None),
    };

    let var_config = VARConfig {
        constant: true,
        lags,
        step: 8,
        dfk: false,
        mlag: 2,
    };

    let var = VAR {
        y,
        exog,
        config: var_config,
        var_names: Some(var_names.clone()),
        exog_names,
    };
    let result = var.fit()?;

    let mut coefficients = Vec::new();
    for eq in 0..result.var_names.len() {
        for (j, label) in result.coef_labels.get(eq).unwrap_or(&vec![]).iter().enumerate() {
            if j < result.coefficients[eq].len() {
                coefficients.push(VARCoefDisplay {
                    eq_name: result.equations.get(eq).map(|e| e.eq_name.clone()).unwrap_or_else(|| format!("eq{}", eq)),
                    variable: label.clone(),
                    coef: result.coefficients[eq][j],
                    std_err: result.std_errs.get(eq).and_then(|se| se.get(j)).copied().unwrap_or(0.0),
                    z_value: result.z_values.get(eq).and_then(|zv| zv.get(j)).copied().unwrap_or(0.0),
                    p_value: result.p_values.get(eq).and_then(|pv| pv.get(j)).copied().unwrap_or(1.0),
                    ci_lower: result.ci_lower.get(eq).and_then(|c| c.get(j)).copied().unwrap_or(0.0),
                    ci_upper: result.ci_upper.get(eq).and_then(|c| c.get(j)).copied().unwrap_or(0.0),
                });
            }
        }
    }

    let equations = result
        .equations
        .iter()
        .enumerate()
        .map(|(i, e)| VAREquationDisplay {
            eq_name: var_names.get(i).cloned().unwrap_or_else(|| e.eq_name.clone()),
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

pub fn register(registry: &NodeRegistry) {
    register_var_summary(registry);
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
    slots.push(PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)));

    let definition = NodeDefinition::new(
        "VAR Summary",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description("Vector Autoregression — VAR(p) with OIRF and FEVD (Stata varbasic style)")
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

        ctx.open_window("var_summary".to_string(), json_data);

        ctx.log("VAR Summary: completed".to_string());
        Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
    }));
    registry.register(definition);
}
