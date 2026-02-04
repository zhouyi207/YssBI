//! Debug 节点

use crate::executor::execution::ExecutionEffect;
use crate::executor::node::NodeDefinition;
use crate::executor::register::NodeRegistry;
use crate::executor::pin::{DataRole, ExecRole, PinDefinition, PinRole, PinTypeDesc};
use crate::executor::value::{DataValue, ValueType};
use std::sync::Arc;

pub fn register(registry: &NodeRegistry) {
    register_print(registry);
}

/// Print 节点 - 打印字符串到控制台
/// 
/// 这是一个混合节点：
/// - 有 exec input 和 exec output（控制流）
/// - 有 data input（字符串数据）
fn register_print(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("debug.print", "Print")
        .with_category(vec!["Debug".to_string()])
        .with_ui_style("debug")
        .with_description("Print a string to the console")
        // Exec 输入
        .add_pin(PinDefinition::exec_input("In", ExecRole::ExecIn))
        // Data 输入（字符串）
        .add_pin(
            PinDefinition::data_input(
                "Message",
                DataRole::Input(0),
                PinTypeDesc::concrete(ValueType::String),
            )
            .with_default(Some(DataValue::String("Hello, World!".to_string()))),
        )
        // Exec 输出
        .add_pin(PinDefinition::exec_output("Out", ExecRole::ExecOut))
        // 🧱 第一层：控制流处理器
        // Print 节点需要先执行数据求值（打印），然后触发输出
        .with_flow_processor(Arc::new(|ctx| {
            // 获取输入字符串
            let input_value = ctx
                .get_input_by_role(&PinRole::Data(DataRole::Input(0)))?;
            
            let message = input_value
                .as_string()
                .ok_or_else(|| "Message must be a string".to_string())?;

            // 打印到控制台
            ctx.log(format!("Print: {}", message));

            // 触发输出 exec pin
            Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
        }));

    registry.register(definition);
}
