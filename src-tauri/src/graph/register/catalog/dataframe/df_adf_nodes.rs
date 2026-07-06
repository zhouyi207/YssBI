//! DF & ADF 单位根检验节点
//!
//! 对应 Stata dfuller y, lags(p) regress noconstant drift trend

use crate::execution::{ExecutionEffect, ReportKind};
use crate::execution::context::NodeExecutionContextTrait;
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::register::catalog::docs;
use crate::graph::value::{DataType, DataValue};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use yss_sci::ts::unit_root::{AdfRegression, adf_test};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DFADFRegRow {
    pub variable: String,
    pub coef: f64,
    pub std_err: f64,
    pub t: f64,
    pub p_value: f64,
    pub ci_lower: f64,
    pub ci_upper: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DFADFSummaryResult {
    pub title: String,
    pub var_name: String,
    pub h0: String,
    pub test_statistic: f64,
    pub critical_value_1pct: f64,
    pub critical_value_5pct: f64,
    pub critical_value_10pct: f64,
    pub p_value: f64,
    pub use_t_distribution: bool,
    pub num_obs: usize,
    pub lags: usize,
    pub regression: String,
    pub coef_lagged: f64,
    pub std_err_lagged: f64,
    pub regression_table: Vec<DFADFRegRow>,
}

/// DF & ADF Summary 列表结果：遍历 constant/trend/lags 所有组合
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DFADFSummaryListResult {
    pub title: String,
    pub var_name: String,
    pub items: Vec<DFADFSummaryResult>,
}

pub fn register(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "DF & ADF",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
        .with_documentation(docs::adf::DF_ADF_ZH, docs::adf::DF_ADF_EN)
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
        PinSlot::fixed(PinDefinition::data_input(
            "Y",
            DataRole::Custom("y".to_string()),
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
        PinSlot::fixed(
            PinDefinition::data_input(
                "Lags",
                DataRole::Custom("lags".to_string()),
                PinDataTypeDefinition::concrete(DataType::Int64),
            )
            .with_optional(true)
            .with_default_value(DataValue::Int64(0)),
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
        PinSlot::fixed(PinDefinition::data_output(
            "Result",
            DataRole::Result,
            PinDataTypeDefinition::concrete(DataType::Struct("DFADFSummaryResult".to_string())),
        )),
        PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)),
    ])
    .with_flow_processor(Arc::new(|ctx| {
        let result = run_df_adf(ctx)?;

        let json_data = serde_json::to_string(&result)
            .map_err(|e| format!("DF & ADF: serialize failed: {}", e))?;

        let result_handle_id = ctx.put_handle(Box::new(result.clone()));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Result),
            DataValue::new_struct("DFADFSummaryResult", result_handle_id),
        )?;

        ctx.publish_report(ReportKind::DfAdfSummary, json_data);
        ctx.log("DF & ADF: completed".to_string());
        Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
    }));
    registry.register(definition);

    // DF & ADF Summary: 遍历所有 constant/trend/lags 组合，以列表展示
    let summary_def = NodeDefinition::new(
        "DF & ADF Summary",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
        .with_documentation(docs::adf::DF_ADF_SUMMARY_ZH, docs::adf::DF_ADF_SUMMARY_EN)
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
        PinSlot::fixed(PinDefinition::data_input(
            "Y",
            DataRole::Custom("y".to_string()),
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Result",
            DataRole::Result,
            PinDataTypeDefinition::concrete(DataType::Struct("DFADFSummaryListResult".to_string())),
        )),
        PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)),
    ])
    .with_flow_processor(Arc::new(|ctx| {
        let result = run_df_adf_summary(ctx)?;

        let json_data = serde_json::to_string(&result)
            .map_err(|e| format!("DF & ADF Summary: serialize failed: {}", e))?;

        let result_handle_id = ctx.put_handle(Box::new(result.clone()));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Result),
            DataValue::new_struct("DFADFSummaryListResult", result_handle_id),
        )?;

        ctx.publish_report(ReportKind::DfAdfSummaryList, json_data);
        ctx.log("DF & ADF Summary: completed".to_string());
        Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
    }));
    registry.register(summary_def);
}

fn run_df_adf(ctx: &mut dyn NodeExecutionContextTrait) -> Result<DFADFSummaryResult, String> {
    let y_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("y".to_string())))?;
    let dsv = match &y_value {
        DataValue::DataSeries(s) => s.clone(),
        _ => return Err("DF & ADF: Y must be a Float64 DataSeries".to_string()),
    };

    let series = ctx.get_data_series(&dsv.id)?;
    let var_name = series.name().to_string();
    let var_name = if var_name.is_empty() {
        "y".to_string()
    } else {
        var_name
    };

    let y: Vec<f64> = series
        .cast(&polars::prelude::DataType::Float64)
        .map_err(|e| format!("DF & ADF: cannot cast to Float64: {}", e))?
        .f64()
        .map_err(|e| format!("DF & ADF: {}", e))?
        .into_no_null_iter()
        .collect();

    let lags = ctx
        .get_input_by_role(&PinRole::Data(DataRole::Custom("lags".to_string())))
        .ok()
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let lags = lags.max(0) as usize;

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

    let r = adf_test(&y, lags, constant, trend)?;

    let regression = match r.regression {
        AdfRegression::NoConstant => "noconstant",
        AdfRegression::Drift => "drift",
        AdfRegression::Trend => "trend",
    }
    .to_string();

    let title = if lags == 0 {
        "Dickey-Fuller test for unit root"
    } else {
        "Augmented Dickey-Fuller test for unit root"
    };

    let h0 = match r.regression {
        AdfRegression::NoConstant => "H0: Random walk without drift, d = 0",
        AdfRegression::Drift => "H0: Random walk with drift, d = 0",
        AdfRegression::Trend => "H0: Random walk with or without drift",
    }
    .to_string();

    let regression_table: Vec<DFADFRegRow> = r
        .regression_table
        .iter()
        .map(|row| DFADFRegRow {
            variable: row.variable.clone(),
            coef: row.coef,
            std_err: row.std_err,
            t: row.t,
            p_value: row.p_value,
            ci_lower: row.ci_lower,
            ci_upper: row.ci_upper,
        })
        .collect();

    Ok(DFADFSummaryResult {
        title: title.to_string(),
        var_name,
        h0,
        test_statistic: r.test_statistic,
        critical_value_1pct: r.critical_value_1pct,
        critical_value_5pct: r.critical_value_5pct,
        critical_value_10pct: r.critical_value_10pct,
        p_value: r.p_value,
        use_t_distribution: r.use_t_distribution,
        num_obs: r.num_obs,
        lags: r.lags,
        regression,
        coef_lagged: r.coef_lagged,
        std_err_lagged: r.std_err_lagged,
        regression_table,
    })
}

fn adf_result_to_summary(
    r: &yss_sci::ts::unit_root::AdfResult,
    var_name: &str,
) -> DFADFSummaryResult {
    let regression = match r.regression {
        AdfRegression::NoConstant => "noconstant",
        AdfRegression::Drift => "drift",
        AdfRegression::Trend => "trend",
    }
    .to_string();

    let title = if r.lags == 0 {
        "Dickey-Fuller test for unit root"
    } else {
        "Augmented Dickey-Fuller test for unit root"
    };

    let h0 = match r.regression {
        AdfRegression::NoConstant => "H0: Random walk without drift, d = 0",
        AdfRegression::Drift => "H0: Random walk with drift, d = 0",
        AdfRegression::Trend => "H0: Random walk with or without drift",
    }
    .to_string();

    let regression_table: Vec<DFADFRegRow> = r
        .regression_table
        .iter()
        .map(|row| DFADFRegRow {
            variable: row.variable.clone(),
            coef: row.coef,
            std_err: row.std_err,
            t: row.t,
            p_value: row.p_value,
            ci_lower: row.ci_lower,
            ci_upper: row.ci_upper,
        })
        .collect();

    DFADFSummaryResult {
        title: title.to_string(),
        var_name: var_name.to_string(),
        h0,
        test_statistic: r.test_statistic,
        critical_value_1pct: r.critical_value_1pct,
        critical_value_5pct: r.critical_value_5pct,
        critical_value_10pct: r.critical_value_10pct,
        p_value: r.p_value,
        use_t_distribution: r.use_t_distribution,
        num_obs: r.num_obs,
        lags: r.lags,
        regression,
        coef_lagged: r.coef_lagged,
        std_err_lagged: r.std_err_lagged,
        regression_table,
    }
}

/// max_lags = floor(12 * (T/100)^(1/4))，Stata 默认
fn max_lags_stata(t: usize) -> usize {
    let t_f = t as f64;
    (12.0 * (t_f / 100.0).powf(0.25)).floor() as usize
}

fn run_df_adf_summary(
    ctx: &mut dyn NodeExecutionContextTrait,
) -> Result<DFADFSummaryListResult, String> {
    let y_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("y".to_string())))?;
    let dsv = match &y_value {
        DataValue::DataSeries(s) => s.clone(),
        _ => return Err("DF & ADF Summary: Y must be a Float64 DataSeries".to_string()),
    };

    let series = ctx.get_data_series(&dsv.id)?;
    let var_name = series.name().to_string();
    let var_name = if var_name.is_empty() {
        "y".to_string()
    } else {
        var_name
    };

    let y: Vec<f64> = series
        .cast(&polars::prelude::DataType::Float64)
        .map_err(|e| format!("DF & ADF Summary: cannot cast to Float64: {}", e))?
        .f64()
        .map_err(|e| format!("DF & ADF Summary: {}", e))?
        .into_no_null_iter()
        .collect();

    let t = y.len();
    let max_lags = max_lags_stata(t);

    let mut items = Vec::new();

    // 遍历 constant, trend, lags
    // (constant, trend): (false, false)=noconstant, (true, false)=drift, (true, true)=trend
    for (constant, trend) in [(false, false), (true, false), (true, true)] {
        for lags in 0..=max_lags {
            match adf_test(&y, lags, constant, trend) {
                Ok(r) => {
                    let summary = adf_result_to_summary(&r, &var_name);
                    items.push(summary);
                }
                Err(e) => {
                    // 某些 lags 可能因样本不足失败，跳过
                    ctx.log(format!(
                        "DF & ADF Summary: skip constant={} trend={} lags={}: {}",
                        constant, trend, lags, e
                    ));
                }
            }
        }
    }

    Ok(DFADFSummaryListResult {
        title: format!("DF & ADF Summary: {}", var_name),
        var_name,
        items,
    })
}
