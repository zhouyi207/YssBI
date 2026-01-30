use std::sync::Arc;
use crate::executor::node::registry::NodeRegistry;
use crate::executor::node::implementation::GenericNode;
use crate::executor::pin::{GenericInDataPin, GenericOutExecPin, GenericInExecPin};
use crate::executor::value::ValueType;

pub fn register(registry: &NodeRegistry) {
    // 1. IfElse Node - 修复后的逻辑
    let if_else = GenericNode::new_prototype("if_else", "If Else");
    if_else.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    if_else.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Condition", ValueType::Boolean));
    if_else.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "True"));
    if_else.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "False"));
    
    if_else.set_flow_processor(Box::new(|ctx, node| {
        ctx.log("IfElse: Starting execution".to_string());
        
        // 现在可以安全地访问 inputs，因为 NodeData 已经正确填充
        let condition_value = if !node.inputs.is_empty() {
            let condition_pin_id = &node.inputs[0].id;
            ctx.log(format!("IfElse: Getting condition from pin {}", condition_pin_id));
            ctx.get_pin_value(condition_pin_id).as_bool().unwrap_or(false)
        } else {
            ctx.log("IfElse: No inputs found, using default false".to_string());
            false
        };
        
        ctx.log(format!("IfElse: Condition value is {}", condition_value));
        
        // 直接在闭包中写 if 逻辑
        if condition_value {
            ctx.log("IfElse: Condition is true, executing True branch".to_string());
            if let Err(e) = ctx.trigger_flow_by_pin(&node.id, "True") {
                ctx.log(format!("IfElse: Failed to execute True branch: {}", e));
                return Err(e);
            }
        } else {
            ctx.log("IfElse: Condition is false, executing False branch".to_string());
            if let Err(e) = ctx.trigger_flow_by_pin(&node.id, "False") {
                ctx.log(format!("IfElse: Failed to execute False branch: {}", e));
                return Err(e);
            }
        }
        
        ctx.log("IfElse: Execution completed".to_string());
        Ok("".into()) // 返回空字符串表示已经手动处理了流程
    }));
    
    let mut if_else = if_else;
    if_else.set_metadata(vec!["Control".into()], "default".into(), Some("Branch flow based on condition".into()));
    registry.register("if_else".into(), Arc::new(if_else));

    // 2. Sequence Node (2 outputs) - 重写为直接执行逻辑
    let seq = GenericNode::new_prototype("sequence", "Sequence");
    seq.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    seq.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Then 0"));
    seq.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Then 1"));

    seq.set_flow_processor(Box::new(|ctx, node| {
        ctx.log("Sequence node: starting execution".to_string());
        
        // 直接在闭包中写顺序执行逻辑
        ctx.log("Sequence node: executing Then 0".to_string());
        if let Err(e) = ctx.trigger_flow_by_pin(&node.id, "Then 0") {
            ctx.log(format!("Sequence node: Then 0 execution failed: {}", e));
            return Err(e);
        }
        
        // 添加小延迟
        std::thread::sleep(std::time::Duration::from_millis(50));
        
        ctx.log("Sequence node: executing Then 1".to_string());
        if let Err(e) = ctx.trigger_flow_by_pin(&node.id, "Then 1") {
            ctx.log(format!("Sequence node: Then 1 execution failed: {}", e));
            return Err(e);
        }
        
        ctx.log("Sequence node: execution completed".to_string());
        Ok("".into()) // 返回空字符串表示已经手动处理了流程
    }));
    
    let mut seq = seq;
    seq.set_metadata(vec!["Control".into()], "default".into(), Some("Execute 2 outputs in order".into()));
    registry.register("sequence".into(), Arc::new(seq));

    // 3. Sequence5 Node (5 outputs) - 重写为直接执行逻辑，使用循环
    let seq5 = GenericNode::new_prototype("sequence5", "Sequence 5");
    seq5.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    seq5.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Then 0"));
    seq5.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Then 1"));
    seq5.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Then 2"));
    seq5.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Then 3"));
    seq5.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Then 4"));

    seq5.set_flow_processor(Box::new(|ctx, node| {
        ctx.log("Sequence5 node: starting execution".to_string());
        
        // 直接在闭包中写循环逻辑
        let then_pins = ["Then 0", "Then 1", "Then 2", "Then 3", "Then 4"];
        
        for (index, pin_name) in then_pins.iter().enumerate() {
            ctx.log(format!("Sequence5 node: executing {}", pin_name));
            
            if let Err(e) = ctx.trigger_flow_by_pin(&node.id, pin_name) {
                ctx.log(format!("Sequence5 node: {} execution failed: {}", pin_name, e));
                return Err(e);
            }
            
            ctx.log(format!("Sequence5 node: {} completed", pin_name));
            
            // 在pin之间添加延迟（除了最后一个）
            if index < then_pins.len() - 1 {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
        
        ctx.log("Sequence5 node: execution completed".to_string());
        Ok("".into()) // 返回空字符串表示已经手动处理了流程
    }));
    
    let mut seq5 = seq5;
    seq5.set_metadata(vec!["Control".into()], "default".into(), Some("Execute 5 outputs in order".into()));
    registry.register("sequence5".into(), Arc::new(seq5));

    // 4. 新增：While Loop Node - 展示循环逻辑的例子
    let while_loop = GenericNode::new_prototype("while_loop", "While Loop");
    while_loop.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    while_loop.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Condition", ValueType::Boolean));
    while_loop.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "MaxIterations", ValueType::Float64));
    while_loop.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Loop Body"));
    while_loop.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Completed"));

    while_loop.set_flow_processor(Box::new(|ctx, node| {
        ctx.log("WhileLoop node: starting execution".to_string());
        
        let max_iterations = ctx.get_pin_value(&node.inputs[1].id).as_f64().unwrap_or(10.0) as i32;
        let mut iteration = 0;
        
        // 直接在闭包中写 while 循环逻辑
        while iteration < max_iterations {
            let condition = ctx.get_pin_value(&node.inputs[0].id).as_bool().unwrap_or(false);
            
            if !condition {
                ctx.log(format!("WhileLoop: Condition false at iteration {}, breaking", iteration));
                break;
            }
            
            ctx.log(format!("WhileLoop: Executing loop body, iteration {}", iteration));
            
            if let Err(e) = ctx.trigger_flow_by_pin(&node.id, "Loop Body") {
                ctx.log(format!("WhileLoop: Loop body execution failed at iteration {}: {}", iteration, e));
                return Err(e);
            }
            
            iteration += 1;
            
            // 防止无限循环的安全延迟
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        
        ctx.log(format!("WhileLoop: Loop completed after {} iterations", iteration));
        
        // 执行完成分支
        if let Err(e) = ctx.trigger_flow_by_pin(&node.id, "Completed") {
            ctx.log(format!("WhileLoop: Completed branch execution failed: {}", e));
            return Err(e);
        }
        
        Ok("".into()) // 返回空字符串表示已经手动处理了流程
    }));
    
    let mut while_loop = while_loop;
    while_loop.set_metadata(vec!["Control".into()], "default".into(), Some("Execute loop body while condition is true".into()));
    registry.register("while_loop".into(), Arc::new(while_loop));

    // 5. 新增：For Loop Node - 展示计数循环逻辑的例子
    let for_loop = GenericNode::new_prototype("for_loop", "For Loop");
    for_loop.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    for_loop.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Start", ValueType::Float64));
    for_loop.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "End", ValueType::Float64));
    for_loop.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Step", ValueType::Float64));
    for_loop.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Loop Body"));
    for_loop.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Completed"));

    for_loop.set_flow_processor(Box::new(|ctx, node| {
        ctx.log("ForLoop node: starting execution".to_string());
        
        let start = ctx.get_pin_value(&node.inputs[0].id).as_f64().unwrap_or(0.0) as i32;
        let end = ctx.get_pin_value(&node.inputs[1].id).as_f64().unwrap_or(10.0) as i32;
        let step = ctx.get_pin_value(&node.inputs[2].id).as_f64().unwrap_or(1.0) as i32;
        
        if step == 0 {
            return Err("ForLoop: Step cannot be zero".to_string());
        }
        
        // 直接在闭包中写 for 循环逻辑
        let mut current = start;
        let mut iteration = 0;
        
        while (step > 0 && current < end) || (step < 0 && current > end) {
            ctx.log(format!("ForLoop: Executing loop body, iteration {} (current={})", iteration, current));
            
            // 可以在这里设置循环变量到上下文中，供循环体使用
            ctx.set_variable("loop_index", serde_json::Value::Number(serde_json::Number::from(current)));
            
            if let Err(e) = ctx.trigger_flow_by_pin(&node.id, "Loop Body") {
                ctx.log(format!("ForLoop: Loop body execution failed at iteration {}: {}", iteration, e));
                return Err(e);
            }
            
            current += step;
            iteration += 1;
            
            // 防止无限循环的安全检查
            if iteration > 1000 {
                ctx.log("ForLoop: Maximum iteration limit reached (1000), breaking".to_string());
                break;
            }
            
            // 防止无限循环的安全延迟
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        
        ctx.log(format!("ForLoop: Loop completed after {} iterations", iteration));
        
        // 执行完成分支
        if let Err(e) = ctx.trigger_flow_by_pin(&node.id, "Completed") {
            ctx.log(format!("ForLoop: Completed branch execution failed: {}", e));
            return Err(e);
        }
        
        Ok("".into()) // 返回空字符串表示已经手动处理了流程
    }));
    
    let mut for_loop = for_loop;
    for_loop.set_metadata(vec!["Control".into()], "default".into(), Some("Execute loop body for a range of values".into()));
    registry.register("for_loop".into(), Arc::new(for_loop));
}
