//! 面板数据对齐节点
//!
//! - XT Align: 按 (entity, time) 对齐面板数据
//! - XT Diff: 在 align 后的数据上按 entity 做一阶差分

use crate::graph::node::{passthrough_input_schema_resolver, NodeDefinition};
use crate::graph::pin::{DataRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{DataType, DataValue};
use std::sync::Arc;
use yss_sci::panel::{align_dataframe, diff_dataframe};

pub fn register(registry: &NodeRegistry) {
    register_xt_align(registry);
    register_xt_diff(registry);
}

fn register_xt_align(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "XT Align",
        vec!["Data".to_string(), "Panel".to_string()],
    )
    .with_ui_style("dataframe")
    .with_localized_description(
        "按 (entity, time) 对齐面板数据：补齐缺失时间点，缺失为 null。实体列支持 Categorical、Int64、String；时间列支持 Int64 或 Date。",
        "Align panel data by (entity, time): fill missing time points with null. Entity column supports Categorical, Int64, String; time column supports Int64 or Date.",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "DataFrame",
            DataRole::Input,
            PinDataTypeDefinition::concrete(DataType::DataFrame),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "Entity Col",
            DataRole::Custom("entity_col".to_string()),
            PinDataTypeDefinition::concrete(DataType::String),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "Time Col",
            DataRole::Custom("time_col".to_string()),
            PinDataTypeDefinition::concrete(DataType::String),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "Interval",
            DataRole::Custom("interval".to_string()),
            PinDataTypeDefinition::concrete(DataType::Int64),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Aligned",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataFrame),
        )),
    ])
    .with_output_schema_resolver(passthrough_input_schema_resolver(PinRole::Data(
        DataRole::Input,
    )))
    .with_data_evaluator(Arc::new(|ctx| {
        let df_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Input))?;
        let df_id = match &df_value {
            DataValue::DataFrame(id) => id.clone(),
            DataValue::Null => return Err("XT Align: 请连接 DataFrame 输入".to_string()),
            _ => return Err("XT Align: 输入必须是 DataFrame".to_string()),
        };

        let entity_value =
            ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("entity_col".to_string())))?;
        let entity_col = match &entity_value {
            DataValue::String(s) if !s.is_empty() => s.clone(),
            DataValue::String(_) => return Err("XT Align: 实体列名不能为空".to_string()),
            DataValue::Null => return Err("XT Align: 请提供实体列名（Entity Col）".to_string()),
            _ => return Err("XT Align: 实体列名必须是 String".to_string()),
        };

        let time_value =
            ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("time_col".to_string())))?;
        let time_col = match &time_value {
            DataValue::String(s) if !s.is_empty() => s.clone(),
            DataValue::String(_) => return Err("XT Align: 时间列名不能为空".to_string()),
            DataValue::Null => return Err("XT Align: 请提供时间列名（Time Col）".to_string()),
            _ => return Err("XT Align: 时间列名必须是 String".to_string()),
        };

        let interval = match ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("interval".to_string()))) {
            Ok(DataValue::Int64(i)) if i > 0 => Some(i),
            Ok(DataValue::Int64(_)) => return Err("XT Align: Interval 必须为正整数".to_string()),
            _ => None,
        };

        let df = ctx.get_dataframe(&df_id)?;
        let aligned = align_dataframe(&df, &entity_col, &time_col, interval)
            .map_err(|e| format!("XT Align: {}", e))?;

        let result_id = ctx.put_dataframe(aligned)?;
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Output),
            DataValue::DataFrame(result_id),
        )?;

        Ok(())
    }));
    registry.register(definition);
}

fn register_xt_diff(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "XT Diff",
        vec!["Data".to_string(), "Panel".to_string()],
    )
    .with_ui_style("dataframe")
    .with_localized_description(
        "在 XT Align 后的 DataFrame 上按 entity 做一阶差分（Stata D. 语义）。仅保留有有效差分的行。",
        "First difference by entity on XT-aligned DataFrame (Stata D. semantics). Keeps only rows with valid differences.",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Aligned DataFrame",
            DataRole::Input,
            PinDataTypeDefinition::concrete(DataType::DataFrame),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "Entity Col",
            DataRole::Custom("entity_col".to_string()),
            PinDataTypeDefinition::concrete(DataType::String),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "Time Col",
            DataRole::Custom("time_col".to_string()),
            PinDataTypeDefinition::concrete(DataType::String),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Diff",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataFrame),
        )),
    ])
    .with_output_schema_resolver(passthrough_input_schema_resolver(PinRole::Data(
        DataRole::Input,
    )))
    .with_data_evaluator(Arc::new(|ctx| {
        let df_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Input))?;
        let df_id = match &df_value {
            DataValue::DataFrame(id) => id.clone(),
            DataValue::Null => return Err("XT Diff: 请连接 Aligned DataFrame 输入".to_string()),
            _ => return Err("XT Diff: 输入必须是 DataFrame".to_string()),
        };

        let entity_value =
            ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("entity_col".to_string())))?;
        let entity_col = match &entity_value {
            DataValue::String(s) if !s.is_empty() => s.clone(),
            DataValue::String(_) => return Err("XT Diff: 实体列名不能为空".to_string()),
            DataValue::Null => return Err("XT Diff: 请提供实体列名（Entity Col）".to_string()),
            _ => return Err("XT Diff: 实体列名必须是 String".to_string()),
        };

        let time_value =
            ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("time_col".to_string())))?;
        let time_col = match &time_value {
            DataValue::String(s) if !s.is_empty() => s.clone(),
            DataValue::String(_) => return Err("XT Diff: 时间列名不能为空".to_string()),
            DataValue::Null => return Err("XT Diff: 请提供时间列名（Time Col）".to_string()),
            _ => return Err("XT Diff: 时间列名必须是 String".to_string()),
        };

        let df = ctx.get_dataframe(&df_id)?;
        let diff_df = diff_dataframe(&df, &entity_col, &time_col)
            .map_err(|e| format!("XT Diff: {}", e))?;

        let result_id = ctx.put_dataframe(diff_df)?;
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Output),
            DataValue::DataFrame(result_id),
        )?;

        Ok(())
    }));
    registry.register(definition);
}
