//! DataFrame 节点定义

use crate::graph::node::{NodeDefinition, PinResolverContext};
use crate::graph::pin::{DataRole, PinDefinition, PinDataTypeDefinition, PinRole};
use crate::graph::register::NodeRegistry;
use crate::graph::value::DataType;
use std::sync::Arc;

pub fn register(registry: &NodeRegistry) {
    register_get_dataframe(registry);
    register_get_column(registry);
    register_decompose_dataframe(registry);
}

/// Get DataFrame 节点 - 获取数据帧引用
/// 需要 dataframe_id 绑定具体数据帧
fn register_get_dataframe(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Get DataFrame", vec!["Data".to_string()])
        .with_node_type("get_dataframe")
        .with_ui_style("dataframe")
        .with_description("Get a DataFrame by ID")
        .with_pin_generator(Arc::new(|| {
            Ok(vec![PinDefinition::data_output(
                "DataFrame",
                DataRole::Output,
                PinDataTypeDefinition::concrete(DataType::DataFrame),
            )])
        }));

    registry.register(definition);
}

/// Get Column 节点 - 从数据帧获取列
/// 输入：DataFrame，输出：列数据（Array 类型）
/// 需要 column_name 指定列名，可选 column_type
fn register_get_column(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Get Column", vec!["Data".to_string()])
        .with_node_type("get_column")
        .with_ui_style("dataframe")
        .with_description("Get a column from a DataFrame")
        .with_pin_generator(Arc::new(|| {
            Ok(vec![
                PinDefinition::data_input(
                    "DataFrame",
                    DataRole::Input,
                    PinDataTypeDefinition::concrete(DataType::DataFrame),
                ),
                PinDefinition::data_output(
                    "Column",
                    DataRole::Output,
                    PinDataTypeDefinition::concrete(DataType::Array(Box::new(DataType::Any))),
                ),
            ])
        }));

    registry.register(definition);
}

/// Decompose DataFrame 节点 - 将 DataFrame 分解为各列的 DataSeries
///
/// 静态 pins: 仅一个 DataFrame 输入
/// 动态 pins: 根据连接的 DataFrame schema 自动生成各列的输出 pin
fn register_decompose_dataframe(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Decompose DataFrame", vec!["Data".to_string()])
        .with_node_type("decompose_dataframe")
        .with_ui_style("dataframe")
        .with_description("Decompose a DataFrame into individual columns")
        .with_pin_generator(Arc::new(|| {
            Ok(vec![PinDefinition::data_input(
                "DataFrame",
                DataRole::Input,
                PinDataTypeDefinition::concrete(DataType::DataFrame),
            )])
        }))
        .with_pin_resolver(Arc::new(|ctx: &PinResolverContext| {
            // 静态输入 pin 始终保留
            let mut pins = vec![PinDefinition::data_input(
                "DataFrame",
                DataRole::Input,
                PinDataTypeDefinition::concrete(DataType::DataFrame),
            )];

            // 从输入 schema 动态生成输出 pins
            if let Some(schema) = ctx.input_schemas.get(&PinRole::Data(DataRole::Input)) {
                for col in &schema.columns {
                    pins.push(
                        PinDefinition::data_output(
                            &col.name,
                            DataRole::Custom(col.name.clone()),
                            PinDataTypeDefinition::concrete(col.data_type.clone()),
                        )
                        .with_dynamic(true),
                    );
                }
            }

            Ok(pins)
        }));

    registry.register(definition);
}
