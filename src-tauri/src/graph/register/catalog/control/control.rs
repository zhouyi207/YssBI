//! 控制流节点

use crate::execution::ExecutionEffect;
use crate::graph::node::NodeDefinition;
use crate::graph::register::catalog::docs;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{DataType, DataValue};
use std::sync::Arc;
use std::time::Duration;

/// Maximum Sleep duration in seconds (safety cap).
pub const MAX_SLEEP_SECONDS: f64 = 60.0;

/// Default While Loop iteration cap when MaxIterations is unconnected.
pub const DEFAULT_WHILE_MAX_ITERATIONS: i64 = 1000;

pub fn register(registry: &NodeRegistry) {
    register_if_else(registry);
    register_sequence(registry);
    register_do(registry);
    register_merge(registry);
    register_sleep(registry);
    register_for_loop(registry);
    register_switch(registry);
    register_while_loop(registry);
}

fn register_if_else(registry: &NodeRegistry) {
    let definition = docs::control::apply_docs(
        NodeDefinition::new("Branch", vec!["Control Flow".to_string()])
            .with_ui_style("control")
            .with_pin_slots(vec![
                PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
                PinSlot::fixed(
                    PinDefinition::data_input(
                        "Condition",
                        DataRole::Condition,
                        PinDataTypeDefinition::concrete(DataType::Boolean),
                    )
                    .with_optional(true),
                ),
                PinSlot::fixed(PinDefinition::exec_output("True", ExecRole::ExecTrue)),
                PinSlot::fixed(PinDefinition::exec_output("False", ExecRole::ExecFalse)),
            ])
            .with_flow_processor(Arc::new(|ctx| {
                let condition = ctx
                    .get_input_by_role(&PinRole::Data(DataRole::Condition))?
                    .as_bool()
                    .ok_or_else(|| "Condition must be a boolean value".to_string())?;
                if condition {
                    Ok(ExecutionEffect::trigger(ExecRole::ExecTrue))
                } else {
                    Ok(ExecutionEffect::trigger(ExecRole::ExecFalse))
                }
            })),
        "Branch",
    );
    registry.register(definition);
}

fn register_sequence(registry: &NodeRegistry) {
    let definition = docs::control::apply_docs(
        NodeDefinition::new("Sequence", vec!["Control Flow".to_string()])
            .with_ui_style("control")
            .with_pin_slots(vec![
                PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
                PinSlot::repeatable(
                    PinDefinition::exec_output("", ExecRole::Steps(0)),
                    "Then",
                    3,
                    None,
                ),
            ])
            .with_flow_processor(Arc::new(|ctx| {
                let roles = ctx.get_exec_step_outputs();
                ctx.log(format!("Sequence: scheduling {} steps", roles.len()));
                Ok(ExecutionEffect::sequence(roles))
            })),
        "Sequence",
    );
    registry.register(definition);
}

fn register_do(registry: &NodeRegistry) {
    let definition = docs::control::apply_docs(
        NodeDefinition::new("Do", vec!["Control Flow".to_string()])
            .with_ui_style("control")
            .with_pin_slots(vec![
                PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
                PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)),
            ])
            .with_flow_processor(Arc::new(|_| Ok(ExecutionEffect::trigger(ExecRole::ExecOut)))),
        "Do",
    );
    registry.register(definition);
}

fn register_merge(registry: &NodeRegistry) {
    let definition = docs::control::apply_docs(
        NodeDefinition::new("Merge", vec!["Control Flow".to_string()])
            .with_ui_style("control")
            .with_pin_slots(vec![
                PinSlot::repeatable(
                    PinDefinition::exec_input("", ExecRole::ExecInputs(0)),
                    "In",
                    2,
                    Some(8),
                ),
                PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)),
            ])
            .with_flow_processor(Arc::new(|ctx| {
                ctx.log("Merge: forwarding execution".to_string());
                Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
            })),
        "Merge",
    );
    registry.register(definition);
}

fn register_sleep(registry: &NodeRegistry) {
    let definition = docs::control::apply_docs(
        NodeDefinition::new("Sleep", vec!["Control Flow".to_string()])
            .with_ui_style("control")
            .with_pin_slots(vec![
                PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
                PinSlot::fixed(
                    PinDefinition::data_input(
                        "Duration",
                        DataRole::Input,
                        PinDataTypeDefinition::concrete(DataType::Float64),
                    )
                    .with_optional(true)
                    .with_default_value(DataValue::Float64(1.0)),
                ),
                PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)),
            ])
            .with_flow_processor(Arc::new(|ctx| {
                let duration = ctx
                    .get_input_by_role(&PinRole::Data(DataRole::Input))?
                    .as_f64()
                    .unwrap_or(1.0);
                if duration < 0.0 {
                    return Err("Duration must be non-negative".to_string());
                }
                let secs = duration.min(MAX_SLEEP_SECONDS);
                ctx.log(format!("Sleep: waiting {:.3}s", secs));
                std::thread::sleep(Duration::from_secs_f64(secs));
                Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
            })),
        "Sleep",
    );
    registry.register(definition);
}

fn register_for_loop(registry: &NodeRegistry) {
    let definition = docs::control::apply_docs(
        NodeDefinition::new("For Loop", vec!["Control Flow".to_string()])
            .with_ui_style("control")
            .with_pin_slots(vec![
                PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
                PinSlot::fixed(
                    PinDefinition::data_input(
                        "Count",
                        DataRole::Input,
                        PinDataTypeDefinition::concrete(DataType::Int64),
                    )
                    .with_optional(true)
                    .with_default_value(DataValue::Int64(1)),
                ),
                PinSlot::fixed(
                    PinDefinition::data_output(
                        "Index",
                        DataRole::Custom("index".to_string()),
                        PinDataTypeDefinition::concrete(DataType::Int64),
                    ),
                ),
                PinSlot::fixed(PinDefinition::exec_output("Body", ExecRole::ExecLoopBody)),
                PinSlot::fixed(PinDefinition::exec_output("Completed", ExecRole::ExecLoopComplete)),
            ])
            .with_flow_processor(Arc::new(|ctx| {
                let count = ctx
                    .get_input_by_role(&PinRole::Data(DataRole::Input))?
                    .as_i64()
                    .unwrap_or(1)
                    .max(0);
                let index = ctx.get_loop_counter();
                if index < count {
                    ctx.emit_output_by_role(
                        &PinRole::Data(DataRole::Custom("index".to_string())),
                        DataValue::Int64(index),
                    )?;
                    ctx.set_loop_counter(index + 1);
                    ctx.log(format!("For Loop: iteration {}/{}", index, count));
                    Ok(ExecutionEffect::loop_effect(
                        ExecRole::ExecLoopBody,
                        ExecRole::ExecLoopComplete,
                        true,
                    ))
                } else {
                    ctx.reset_loop_counter();
                    ctx.log(format!("For Loop: completed {} iterations", count));
                    Ok(ExecutionEffect::loop_effect(
                        ExecRole::ExecLoopBody,
                        ExecRole::ExecLoopComplete,
                        false,
                    ))
                }
            })),
        "For Loop",
    );
    registry.register(definition);
}

fn register_switch(registry: &NodeRegistry) {
    let definition = docs::control::apply_docs(
        NodeDefinition::new("Switch", vec!["Control Flow".to_string()])
            .with_ui_style("control")
            .with_pin_slots(vec![
                PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
                PinSlot::fixed(
                    PinDefinition::data_input(
                        "Selector",
                        DataRole::Input,
                        PinDataTypeDefinition::concrete(DataType::Int64),
                    )
                    .with_optional(true)
                    .with_default_value(DataValue::Int64(0)),
                ),
                PinSlot::repeatable(
                    PinDefinition::exec_output("", ExecRole::Cases(0)),
                    "Case",
                    2,
                    Some(16),
                ),
                PinSlot::fixed(PinDefinition::exec_output("Default", ExecRole::ExecFalse)),
            ])
            .with_flow_processor(Arc::new(|ctx| {
                let selector = ctx
                    .get_input_by_role(&PinRole::Data(DataRole::Input))?
                    .as_i64()
                    .unwrap_or(0);
                let cases = ctx.get_exec_case_outputs();
                if selector >= 0 {
                    let idx = selector as usize;
                    if idx < cases.len() {
                        ctx.log(format!("Switch: selector {} -> case {}", selector, idx));
                        return Ok(ExecutionEffect::trigger(cases[idx].clone()));
                    }
                }
                ctx.log(format!(
                    "Switch: selector {} out of range, using Default",
                    selector
                ));
                Ok(ExecutionEffect::trigger(ExecRole::ExecFalse))
            })),
        "Switch",
    );
    registry.register(definition);
}

fn register_while_loop(registry: &NodeRegistry) {
    let definition = docs::control::apply_docs(
        NodeDefinition::new("While Loop", vec!["Control Flow".to_string()])
            .with_ui_style("control")
            .with_pin_slots(vec![
                PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
                PinSlot::fixed(
                    PinDefinition::data_input(
                        "Condition",
                        DataRole::Condition,
                        PinDataTypeDefinition::concrete(DataType::Boolean),
                    )
                    .with_optional(true),
                ),
                PinSlot::fixed(
                    PinDefinition::data_input(
                        "MaxIterations",
                        DataRole::Custom("maxIterations".to_string()),
                        PinDataTypeDefinition::concrete(DataType::Int64),
                    )
                    .with_optional(true)
                    .with_default_value(DataValue::Int64(DEFAULT_WHILE_MAX_ITERATIONS)),
                ),
                PinSlot::fixed(PinDefinition::exec_output("Body", ExecRole::ExecLoopBody)),
                PinSlot::fixed(PinDefinition::exec_output("Completed", ExecRole::ExecLoopComplete)),
            ])
            .with_flow_processor(Arc::new(|ctx| {
                let max_iterations = ctx
                    .get_input_by_role(&PinRole::Data(DataRole::Custom(
                        "maxIterations".to_string(),
                    )))?
                    .as_i64()
                    .unwrap_or(DEFAULT_WHILE_MAX_ITERATIONS)
                    .max(1);
                let iteration = ctx.get_loop_counter();
                if iteration >= max_iterations {
                    ctx.reset_loop_counter();
                    ctx.log(format!(
                        "While Loop: max iterations {} reached",
                        max_iterations
                    ));
                    return Ok(ExecutionEffect::loop_effect(
                        ExecRole::ExecLoopBody,
                        ExecRole::ExecLoopComplete,
                        false,
                    ));
                }

                let condition = ctx
                    .get_input_by_role(&PinRole::Data(DataRole::Condition))?
                    .as_bool()
                    .unwrap_or(false);
                if condition {
                    ctx.set_loop_counter(iteration + 1);
                    ctx.log(format!(
                        "While Loop: iteration {} (max {})",
                        iteration + 1,
                        max_iterations
                    ));
                    Ok(ExecutionEffect::loop_effect(
                        ExecRole::ExecLoopBody,
                        ExecRole::ExecLoopComplete,
                        true,
                    ))
                } else {
                    ctx.reset_loop_counter();
                    ctx.log("While Loop: condition false, exiting".to_string());
                    Ok(ExecutionEffect::loop_effect(
                        ExecRole::ExecLoopBody,
                        ExecRole::ExecLoopComplete,
                        false,
                    ))
                }
            })),
        "While Loop",
    );
    registry.register(definition);
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_WHILE_MAX_ITERATIONS, MAX_SLEEP_SECONDS};

    #[test]
    fn max_sleep_cap_is_sixty_seconds() {
        assert_eq!(MAX_SLEEP_SECONDS, 60.0);
    }

    #[test]
    fn default_while_max_iterations_is_one_thousand() {
        assert_eq!(DEFAULT_WHILE_MAX_ITERATIONS, 1000);
    }
}
