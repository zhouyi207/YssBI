//! VECRANK — Johansen 协整秩检验（Stata vecrank）

use crate::execution::{ExecutionEffect, ReportKind};
use crate::execution::context::NodeExecutionContextTrait;
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::register::catalog::docs;
use crate::graph::value::{DataType, DataValue};
use ndarray::Array2;
use polars::prelude::DataType as PolarsDataType;
use std::sync::Arc;
use yss_sci::ts::vec::{VecRankResult, VecTrendSpec, vec_vecrank_stats};

fn vecrank_input_slots() -> Vec<PinSlot> {
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
                "Max eigen",
                DataRole::Custom("max_eigen".to_string()),
                PinDataTypeDefinition::concrete(DataType::Boolean),
            )
            .with_optional(true)
            .with_default_value(DataValue::Boolean(false)),
        ),
        PinSlot::fixed(PinDefinition::data_output(
            "Result",
            DataRole::Result,
            PinDataTypeDefinition::concrete(DataType::Struct("VecRankResult".to_string())),
        )),
        PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)),
    ]
}

fn run_vecrank(ctx: &mut dyn NodeExecutionContextTrait) -> Result<VecRankResult, String> {
    let lags_val = ctx
        .get_input_by_role(&PinRole::Data(DataRole::Custom("lags".to_string())))
        .ok()
        .and_then(|v| v.as_i64())
        .unwrap_or(2);
    let lags = lags_val as usize;

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

    let show_max = ctx
        .get_input_by_role(&PinRole::Data(DataRole::Custom("max_eigen".to_string())))
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let trend_spec = match (constant, trend) {
        (false, false) => VecTrendSpec::None,
        (true, false) => VecTrendSpec::Constant,
        (true, true) | (false, true) => VecTrendSpec::Trend,
    };

    let var_inputs = ctx.get_inputs_by_family(&PinRole::Data(DataRole::Inputs(0)))?;
    if var_inputs.len() < 2 {
        return Err("VECRANK: at least 2 variables required".to_string());
    }

    let mut series_list = Vec::new();
    let mut var_names = Vec::new();
    for v in &var_inputs {
        let dsv = match v {
            DataValue::DataSeries(s) => s.clone(),
            _ => return Err("VECRANK: each variable must be a DataSeries".to_string()),
        };
        let s = ctx.get_data_series(&dsv.id)?;
        let name = s.name().to_string();
        if name.is_empty() {
            var_names.push(format!("y{}", series_list.len()));
        } else {
            var_names.push(name);
        }
        let vals: Vec<f64> = s
            .cast(&PolarsDataType::Float64)
            .map_err(|e| format!("VECRANK: cannot cast to Float64: {}", e))?
            .f64()
            .map_err(|e| format!("VECRANK: {}", e))?
            .into_no_null_iter()
            .collect();
        series_list.push(vals);
    }

    let n = series_list[0].len();
    for (i, s) in series_list.iter().enumerate() {
        if s.len() != n {
            return Err(format!(
                "VECRANK: variable '{}' has {} rows, expected {}",
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
    let y = Array2::from_shape_fn((n, k), |(i, j)| series_list[j][i]);

    let out = vec_vecrank_stats(
        &y,
        lags,
        trend_spec,
        None,
        show_max,
        Some(var_names.clone()),
    )?;

    ctx.log(format!(
        "VECRANK: T={} lags={} trend={} · trace@5% rank={} · max@5% rank={}",
        out.num_observation,
        out.n_lags,
        out.trend_spec,
        out.selected_rank_trace_95,
        out.selected_rank_max_95
    ));

    Ok(out)
}

pub fn register(registry: &NodeRegistry) {
    let slots = vecrank_input_slots();

    let definition = NodeDefinition::new(
        "VECRANK (Johansen)",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
        .with_documentation(docs::vec::VECRANK_ZH, docs::vec::VECRANK_EN)
    .with_pin_slots(slots)
    .with_flow_processor(Arc::new(|ctx| {
        let result = run_vecrank(ctx)?;

        let json_data = serde_json::to_string(&result)
            .map_err(|e| format!("VECRANK: serialize failed: {}", e))?;

        let result_handle_id = ctx.put_handle(Box::new(result.clone()));
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Result),
            DataValue::new_struct("VecRankResult", result_handle_id),
        )?;

        ctx.publish_report(ReportKind::VecRankSummary, json_data);
        ctx.log("VECRANK: completed".to_string());
        Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
    }));
    registry.register(definition);
}
