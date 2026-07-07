//! 数学运算节点
//!
//! 支持基础类型与 DataSeries<基础类型> 的运算：
//! - scalar + scalar → scalar
//! - DataSeries + DataSeries → DataSeries（逐元素，长度需一致）
//! - DataSeries + scalar / scalar + DataSeries → DataSeries（标量广播）

use crate::graph::node::NodeDefinition;
use crate::graph::pin::{DataRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot};
use crate::graph::register::NodeRegistry;
use crate::graph::register::catalog::docs;
use crate::graph::value::{DataSeriesValue, DataType, DataValue};
use polars::prelude::Series;
use std::sync::Arc;

/// 可参与数学运算的输入类型：标量数值 + DataSeries<数值类型>
fn operable_input_type() -> DataType {
    DataType::one_of(vec![DataType::number(), DataType::number_series()])
}

/// 运算结果类型：标量或 DataSeries<Float64>
fn operable_output_type() -> DataType {
    DataType::one_of(vec![
        DataType::Float64,
        DataType::DataSeries(Box::new(DataType::Float64)),
    ])
}

fn scalar_to_f64(v: &DataValue) -> Option<f64> {
    v.as_f64()
}

fn has_any_series(operands: &[DataValue]) -> bool {
    operands
        .iter()
        .any(|v| matches!(v, DataValue::DataSeries(_)))
}

/// 将 DataValue 转为 Float64 Series（用于运算）
fn value_to_f64_series(
    v: &DataValue,
    len: Option<usize>,
    ctx: &mut dyn crate::execution::NodeExecutionContextTrait,
) -> Result<Series, String> {
    match v {
        DataValue::DataSeries(dsv) => {
            let s = ctx.get_data_series(&dsv.id)?;
            let cast = s
                .cast(&polars::prelude::DataType::Float64)
                .map_err(|e| format!("Math: cannot cast series to Float64: {}", e))?;
            Ok(cast)
        }
        _ => {
            let scalar = scalar_to_f64(v)
                .ok_or_else(|| format!("Math: cannot convert {:?} to scalar", v.value_type()))?;
            let n = len.unwrap_or(1);
            Ok(Series::from_iter(std::iter::repeat(scalar).take(n)))
        }
    }
}

/// 获取首个 DataSeries 的长度，用于构造标量 Series
fn get_data_series_len(
    operands: &[DataValue],
    ctx: &dyn crate::execution::NodeExecutionContextTrait,
) -> Option<usize> {
    for v in operands {
        if let DataValue::DataSeries(dsv) = v {
            if let Ok(s) = ctx.get_data_series(&dsv.id) {
                return Some(s.len());
            }
        }
    }
    None
}

pub fn register(registry: &NodeRegistry) {
    register_add(registry);
    register_subtract(registry);
    register_multiply(registry);
    register_divide(registry);
}

fn register_add(registry: &NodeRegistry) {
    let definition = docs::math::apply_docs(
        NodeDefinition::new("Add (+)", vec!["Math".to_string(), "Operators".to_string()])
            .with_ui_style("math")
            .with_pin_slots(vec![
                PinSlot::repeatable(
                    PinDefinition::data_input(
                        "",
                        DataRole::Operands(0),
                        PinDataTypeDefinition::concrete(operable_input_type()),
                    )
                    .with_optional(true),
                    "",
                    2,
                    None,
                ),
                PinSlot::fixed(PinDefinition::data_output(
                    "Result",
                    DataRole::Result,
                    PinDataTypeDefinition::concrete(operable_output_type()),
                )),
            ])
            .with_data_evaluator(Arc::new(|ctx| {
                let operands = ctx.get_inputs_by_family(&PinRole::Data(DataRole::Operands(0)))?;
                let result = if has_any_series(&operands) {
                    let len = get_data_series_len(&operands, ctx);
                    let mut acc: Option<Series> = None;
                    for v in operands {
                        let s = value_to_f64_series(&v, len, ctx)?;
                        acc = Some(match acc {
                            None => s,
                            Some(a) => {
                                let out = (&a + &s).map_err(|e| format!("Add: {}", e))?;
                                out
                            }
                        });
                    }
                    let series = acc.ok_or("Add: no operands".to_string())?;
                    let id = ctx.put_data_series(series)?;
                    DataValue::DataSeries(DataSeriesValue::with_element_type(id, DataType::Float64))
                } else {
                    operands
                        .into_iter()
                        .reduce(|acc, v| (acc.clone() + v).unwrap_or(acc))
                        .ok_or_else(|| "Add: no operands".to_string())?
                };
                ctx.emit_output_by_role(&PinRole::Data(DataRole::Result), result)?;
                Ok(())
            })),
        "Add (+)",
    );
    registry.register(definition);
}

fn register_subtract(registry: &NodeRegistry) {
    let definition = docs::math::apply_docs(
        NodeDefinition::new(
            "Subtract (-)",
            vec!["Math".to_string(), "Operators".to_string()],
        )
        .with_ui_style("math")
        .with_pin_slots(vec![
            PinSlot::fixed(
                PinDefinition::data_input(
                    "A",
                    DataRole::Operands(0),
                    PinDataTypeDefinition::concrete(operable_input_type()),
                )
                .with_optional(true),
            ),
            PinSlot::fixed(
                PinDefinition::data_input(
                    "B",
                    DataRole::Operands(1),
                    PinDataTypeDefinition::concrete(operable_input_type()),
                )
                .with_optional(true),
            ),
            PinSlot::fixed(PinDefinition::data_output(
                "Result",
                DataRole::Result,
                PinDataTypeDefinition::concrete(operable_output_type()),
            )),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let a = ctx.get_input_by_role(&PinRole::Data(DataRole::Operands(0)))?;
            let b = ctx.get_input_by_role(&PinRole::Data(DataRole::Operands(1)))?;
            let result = if has_any_series(&[a.clone(), b.clone()]) {
                let len = get_data_series_len(&[a.clone(), b.clone()], ctx);
                let sa = value_to_f64_series(&a, len, ctx)?;
                let sb = value_to_f64_series(&b, len, ctx)?;
                let out = (&sa - &sb).map_err(|e| format!("Subtract: {}", e))?;
                let id = ctx.put_data_series(out)?;
                DataValue::DataSeries(DataSeriesValue::with_element_type(id, DataType::Float64))
            } else {
                (a - b)?
            };
            ctx.emit_output_by_role(&PinRole::Data(DataRole::Result), result)?;
            Ok(())
        })),
        "Subtract (-)",
    );
    registry.register(definition);
}

fn register_multiply(registry: &NodeRegistry) {
    let definition = docs::math::apply_docs(
        NodeDefinition::new(
            "Multiply (*)",
            vec!["Math".to_string(), "Operators".to_string()],
        )
        .with_ui_style("math")
        .with_pin_slots(vec![
            PinSlot::fixed(
                PinDefinition::data_input(
                    "A",
                    DataRole::Operands(0),
                    PinDataTypeDefinition::concrete(operable_input_type()),
                )
                .with_optional(true),
            ),
            PinSlot::fixed(
                PinDefinition::data_input(
                    "B",
                    DataRole::Operands(1),
                    PinDataTypeDefinition::concrete(operable_input_type()),
                )
                .with_optional(true),
            ),
            PinSlot::fixed(PinDefinition::data_output(
                "Result",
                DataRole::Result,
                PinDataTypeDefinition::concrete(operable_output_type()),
            )),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let a = ctx.get_input_by_role(&PinRole::Data(DataRole::Operands(0)))?;
            let b = ctx.get_input_by_role(&PinRole::Data(DataRole::Operands(1)))?;
            let result = if has_any_series(&[a.clone(), b.clone()]) {
                let len = get_data_series_len(&[a.clone(), b.clone()], ctx);
                let sa = value_to_f64_series(&a, len, ctx)?;
                let sb = value_to_f64_series(&b, len, ctx)?;
                let out = (&sa * &sb).map_err(|e| format!("Multiply: {}", e))?;
                let id = ctx.put_data_series(out)?;
                DataValue::DataSeries(DataSeriesValue::with_element_type(id, DataType::Float64))
            } else {
                (a * b)?
            };
            ctx.emit_output_by_role(&PinRole::Data(DataRole::Result), result)?;
            Ok(())
        })),
        "Multiply (*)",
    );
    registry.register(definition);
}

fn register_divide(registry: &NodeRegistry) {
    let definition = docs::math::apply_docs(
        NodeDefinition::new(
            "Divide (/)",
            vec!["Math".to_string(), "Operators".to_string()],
        )
        .with_ui_style("math")
        .with_pin_slots(vec![
            PinSlot::fixed(
                PinDefinition::data_input(
                    "A",
                    DataRole::Operands(0),
                    PinDataTypeDefinition::concrete(operable_input_type()),
                )
                .with_optional(true),
            ),
            PinSlot::fixed(
                PinDefinition::data_input(
                    "B",
                    DataRole::Operands(1),
                    PinDataTypeDefinition::concrete(operable_input_type()),
                )
                .with_optional(true),
            ),
            PinSlot::fixed(PinDefinition::data_output(
                "Result",
                DataRole::Result,
                PinDataTypeDefinition::concrete(operable_output_type()),
            )),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let a = ctx.get_input_by_role(&PinRole::Data(DataRole::Operands(0)))?;
            let b = ctx.get_input_by_role(&PinRole::Data(DataRole::Operands(1)))?;
            let result = if has_any_series(&[a.clone(), b.clone()]) {
                let len = get_data_series_len(&[a.clone(), b.clone()], ctx);
                let sa = value_to_f64_series(&a, len, ctx)?;
                let sb = value_to_f64_series(&b, len, ctx)?;
                let out = (&sa / &sb).map_err(|e| format!("Divide: {}", e))?;
                let id = ctx.put_data_series(out)?;
                DataValue::DataSeries(DataSeriesValue::with_element_type(id, DataType::Float64))
            } else {
                (a / b)?
            };
            ctx.emit_output_by_role(&PinRole::Data(DataRole::Result), result)?;
            Ok(())
        })),
        "Divide (/)",
    );
    registry.register(definition);
}
