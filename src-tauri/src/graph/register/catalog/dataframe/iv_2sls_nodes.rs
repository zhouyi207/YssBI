//! IV:2SLS (Instrumental Variables Two-Stage Least Squares) 回归节点
//!
//! 与 GLS 类似，但包含两个 DataFrame 输入：x_exdogs 和 x_instructions。
//! DataSeries: Y, 可重复的 X:endog（内生变量），以及 DataFrame: x_exdogs, x_instructions。

use crate::execution::ExecutionEffect;
use crate::execution::context::NodeExecutionContextTrait;
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use std::sync::Arc;

// ======================== 共享辅助函数 ========================

fn iv_2sls_input_slots() -> Vec<PinSlot> {
    let x_endog_type = crate::graph::value::DataType::DataSeries(Box::new(
        crate::graph::value::DataType::one_of(vec![
            crate::graph::value::DataType::Float64,
            crate::graph::value::DataType::Categorical,
        ]),
    ));

    vec![
        PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
        PinSlot::fixed(PinDefinition::data_input(
            "Y",
            DataRole::Custom("y".to_string()),
            PinDataTypeDefinition::concrete(crate::graph::value::DataType::DataSeries(Box::new(
                crate::graph::value::DataType::Float64,
            ))),
        )),
        PinSlot::repeatable(
            PinDefinition::data_input(
                "",
                DataRole::Inputs(0),
                PinDataTypeDefinition::concrete(x_endog_type),
            ),
            "X:endog",
            1,
            None,
        ),
        PinSlot::fixed(PinDefinition::data_input(
            "x_exogs",
            DataRole::Custom("x_exogs".to_string()),
            PinDataTypeDefinition::concrete(crate::graph::value::DataType::DataFrame),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "x_instruments",
            DataRole::Custom("x_instruments".to_string()),
            PinDataTypeDefinition::concrete(crate::graph::value::DataType::DataFrame),
        )),
    ]
}

// ======================== 注册入口 ========================

pub fn register(registry: &NodeRegistry) {
    register_iv_2sls_summary(registry);
}

// ======================== IV:2SLS Summary 节点 ========================

fn register_iv_2sls_summary(registry: &NodeRegistry) {
    let mut slots = iv_2sls_input_slots();
    slots.push(PinSlot::fixed(PinDefinition::data_output(
        "Result",
        DataRole::Result,
        PinDataTypeDefinition::concrete(crate::graph::value::DataType::Struct(
            "IV2SLSResult".to_string(),
        )),
    )));
    slots.push(PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)));

    let definition = NodeDefinition::new(
        "IV:2SLS Summary",
        vec!["Data".to_string(), "Statistics".to_string()],
    )
    .with_ui_style("dataframe")
    .with_description(
        "Instrumental Variables Two-Stage Least Squares regression — outputs results and opens the summary window",
    )
    .with_pin_slots(slots)
    .with_flow_processor(Arc::new(|ctx| {
        // 验证输入已连接
        let _y = ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("y".to_string())))?;
        let x_endog_values = ctx.get_inputs_by_family(&PinRole::Data(DataRole::Inputs(0)))?;
        if x_endog_values.is_empty() {
            return Err("IV:2SLS: at least one X:endog input is required".to_string());
        }
        let _x_exdogs =
            ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("x_exdogs".to_string())))?;
        let _x_instructions = ctx
            .get_input_by_role(&PinRole::Data(DataRole::Custom("x_instructions".to_string())))?;

        // TODO: 实现 2SLS 回归逻辑，目前返回占位错误
        Err("IV:2SLS Summary: 2SLS regression not yet implemented. Node structure is ready."
            .to_string())
    }));
    registry.register(definition);
}
