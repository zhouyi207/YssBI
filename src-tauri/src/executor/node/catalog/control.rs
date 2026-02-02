use std::sync::Arc;
use crate::executor::node::registry::NodeRegistry;
use crate::executor::node::implementation::{GenericNode, DynamicPinConfig, DynamicPinType, PinDirection, NodeDynamicCapability};
use crate::executor::pin::{GenericInDataPin, GenericOutExecPin, GenericInExecPin, GenericOutDataPin};
use crate::executor::value::{ValueType, PinTypeDesc};

pub fn register(registry: &NodeRegistry) {
    // 1. IfElse Node - 修复后的逻辑
    let if_else = GenericNode::new_prototype("if_else", "If Else");
    if_else.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    if_else.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "Condition", PinTypeDesc::concrete(ValueType::Boolean)));
    if_else.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "True"));
    if_else.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "False"));
    
    if_else.set_flow_processor(Box::new(|ctx, node| {
        ctx.log("IfElse: Starting execution".to_string());
        
        // 现在可以安全地访问 inputs，因为 NodeDto 已经正确填充
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
    while_loop.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "Condition", PinTypeDesc::concrete(ValueType::Boolean)));
    while_loop.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "MaxIterations", PinTypeDesc::concrete(ValueType::Float64)));
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
    for_loop.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "Start", PinTypeDesc::concrete(ValueType::Float64)));
    for_loop.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "End", PinTypeDesc::concrete(ValueType::Float64)));
    for_loop.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "Step", PinTypeDesc::concrete(ValueType::Float64)));
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

    // 6. Switch Node - 多分支选择
    let switch = GenericNode::new_prototype("switch", "Switch");
    switch.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    switch.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "Value", PinTypeDesc::concrete(ValueType::Float64)));
    switch.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Case 0"));
    switch.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Case 1"));
    switch.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Case 2"));
    switch.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Case 3"));
    switch.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Default"));

    switch.set_flow_processor(Box::new(|ctx, node| {
        ctx.log("Switch node: starting execution".to_string());
        
        let value = ctx.get_pin_value(&node.inputs[0].id).as_f64().unwrap_or(0.0) as i32;
        ctx.log(format!("Switch: Value is {}", value));
        
        let case_name = match value {
            0 => "Case 0",
            1 => "Case 1", 
            2 => "Case 2",
            3 => "Case 3",
            _ => "Default",
        };
        
        ctx.log(format!("Switch: Executing {}", case_name));
        if let Err(e) = ctx.trigger_flow_by_pin(&node.id, case_name) {
            ctx.log(format!("Switch: {} execution failed: {}", case_name, e));
            return Err(e);
        }
        
        ctx.log("Switch: execution completed".to_string());
        Ok("".into())
    }));
    
    let mut switch = switch;
    switch.set_metadata(vec!["Control".into()], "default".into(), Some("Execute different branches based on integer value".into()));
    registry.register("switch".into(), Arc::new(switch));

    // 7. Try-Catch Node - 异常处理
    let try_catch = GenericNode::new_prototype("try_catch", "Try Catch");
    try_catch.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    try_catch.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Try"));
    try_catch.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Catch"));
    try_catch.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Finally"));

    try_catch.set_flow_processor(Box::new(|ctx, node| {
        ctx.log("TryCatch node: starting execution".to_string());
        
        let mut had_error = false;
        
        // 执行 Try 分支
        ctx.log("TryCatch: Executing Try branch".to_string());
        if let Err(e) = ctx.trigger_flow_by_pin(&node.id, "Try") {
            ctx.log(format!("TryCatch: Try branch failed: {}", e));
            had_error = true;
            
            // 执行 Catch 分支
            ctx.log("TryCatch: Executing Catch branch".to_string());
            if let Err(catch_e) = ctx.trigger_flow_by_pin(&node.id, "Catch") {
                ctx.log(format!("TryCatch: Catch branch also failed: {}", catch_e));
                // 继续执行 Finally，但记录错误
            }
        }
        
        // 总是执行 Finally 分支
        ctx.log("TryCatch: Executing Finally branch".to_string());
        if let Err(e) = ctx.trigger_flow_by_pin(&node.id, "Finally") {
            ctx.log(format!("TryCatch: Finally branch failed: {}", e));
            return Err(e);
        }
        
        if had_error {
            ctx.log("TryCatch: Completed with error handled".to_string());
        } else {
            ctx.log("TryCatch: Completed successfully".to_string());
        }
        
        Ok("".into())
    }));
    
    let mut try_catch = try_catch;
    try_catch.set_metadata(vec!["Control".into()], "default".into(), Some("Handle errors with try-catch-finally pattern".into()));
    registry.register("try_catch".into(), Arc::new(try_catch));

    // 8. Delay Node - 延迟执行
    let delay = GenericNode::new_prototype("delay", "Delay");
    delay.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    delay.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "Milliseconds", PinTypeDesc::concrete(ValueType::Float64)));
    delay.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Out"));

    delay.set_flow_processor(Box::new(|ctx, node| {
        ctx.log("Delay node: starting execution".to_string());
        
        let ms = ctx.get_pin_value(&node.inputs[0].id).as_f64().unwrap_or(1000.0) as u64;
        ctx.log(format!("Delay: Waiting for {} milliseconds", ms));
        
        // 限制最大延迟时间为10秒，防止过长等待
        let safe_ms = ms.min(10000);
        std::thread::sleep(std::time::Duration::from_millis(safe_ms));
        
        ctx.log("Delay: Wait completed, continuing execution".to_string());
        if let Err(e) = ctx.trigger_flow_by_pin(&node.id, "Out") {
            ctx.log(format!("Delay: Output execution failed: {}", e));
            return Err(e);
        }
        
        Ok("".into())
    }));
    
    let mut delay = delay;
    delay.set_metadata(vec!["Control".into()], "default".into(), Some("Delay execution for specified milliseconds".into()));
    registry.register("delay".into(), Arc::new(delay));

    // 9. Gate Node - 条件门控
    let gate = GenericNode::new_prototype("gate", "Gate");
    gate.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    gate.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "Condition", PinTypeDesc::concrete(ValueType::Boolean)));
    gate.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "CloseOnFalse", PinTypeDesc::concrete(ValueType::Boolean)));
    gate.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Out"));
    gate.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Closed"));

    gate.set_flow_processor(Box::new(|ctx, node| {
        ctx.log("Gate node: starting execution".to_string());
        
        let condition = ctx.get_pin_value(&node.inputs[0].id).as_bool().unwrap_or(false);
        let close_on_false = ctx.get_pin_value(&node.inputs[1].id).as_bool().unwrap_or(true);
        
        ctx.log(format!("Gate: Condition={}, CloseOnFalse={}", condition, close_on_false));
        
        if condition {
            ctx.log("Gate: Condition is true, opening gate".to_string());
            if let Err(e) = ctx.trigger_flow_by_pin(&node.id, "Out") {
                ctx.log(format!("Gate: Output execution failed: {}", e));
                return Err(e);
            }
        } else if close_on_false {
            ctx.log("Gate: Condition is false and CloseOnFalse is true, gate closed".to_string());
            if let Err(e) = ctx.trigger_flow_by_pin(&node.id, "Closed") {
                ctx.log(format!("Gate: Closed output execution failed: {}", e));
                return Err(e);
            }
        } else {
            ctx.log("Gate: Condition is false but CloseOnFalse is false, passing through".to_string());
            if let Err(e) = ctx.trigger_flow_by_pin(&node.id, "Out") {
                ctx.log(format!("Gate: Output execution failed: {}", e));
                return Err(e);
            }
        }
        
        Ok("".into())
    }));
    
    let mut gate = gate;
    gate.set_metadata(vec!["Control".into()], "default".into(), Some("Control flow based on condition with optional closed output".into()));
    registry.register("gate".into(), Arc::new(gate));

    // 10. MultiGate Node - 多路门控（轮流执行）
    let multi_gate = GenericNode::new_prototype("multi_gate", "Multi Gate");
    multi_gate.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    multi_gate.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "Reset", PinTypeDesc::concrete(ValueType::Boolean)));
    multi_gate.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "StartIndex", PinTypeDesc::concrete(ValueType::Float64)));
    multi_gate.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Out 0"));
    multi_gate.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Out 1"));
    multi_gate.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Out 2"));
    multi_gate.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Out 3"));

    multi_gate.set_flow_processor(Box::new(|ctx, node| {
        ctx.log("MultiGate node: starting execution".to_string());
        
        let reset = ctx.get_pin_value(&node.inputs[0].id).as_bool().unwrap_or(false);
        let start_index = ctx.get_pin_value(&node.inputs[1].id).as_f64().unwrap_or(0.0) as usize;
        
        // 使用静态变量来保持状态（在实际实现中，这应该存储在节点实例中）
        static mut CURRENT_INDEX: usize = 0;
        
        unsafe {
            if reset {
                CURRENT_INDEX = start_index;
                ctx.log(format!("MultiGate: Reset to index {}", CURRENT_INDEX));
            }
            
            let output_names = ["Out 0", "Out 1", "Out 2", "Out 3"];
            let current_output = output_names[CURRENT_INDEX % output_names.len()];
            
            ctx.log(format!("MultiGate: Executing {} (index {})", current_output, CURRENT_INDEX));
            
            if let Err(e) = ctx.trigger_flow_by_pin(&node.id, current_output) {
                ctx.log(format!("MultiGate: {} execution failed: {}", current_output, e));
                return Err(e);
            }
            
            // 移动到下一个输出
            CURRENT_INDEX = (CURRENT_INDEX + 1) % output_names.len();
            ctx.log(format!("MultiGate: Next index will be {}", CURRENT_INDEX));
        }
        
        Ok("".into())
    }));
    
    let mut multi_gate = multi_gate;
    multi_gate.set_metadata(vec!["Control".into()], "default".into(), Some("Cycle through multiple outputs on each execution".into()));
    registry.register("multi_gate".into(), Arc::new(multi_gate));

    // 11. DoOnce Node - 只执行一次
    let do_once = GenericNode::new_prototype("do_once", "Do Once");
    do_once.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    do_once.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "Reset", PinTypeDesc::concrete(ValueType::Boolean)));
    do_once.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Out"));
    do_once.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Already Done"));

    do_once.set_flow_processor(Box::new(|ctx, node| {
        ctx.log("DoOnce node: starting execution".to_string());
        
        let reset = ctx.get_pin_value(&node.inputs[0].id).as_bool().unwrap_or(false);
        
        // 使用静态变量来保持状态（在实际实现中，这应该存储在节点实例中）
        static mut HAS_EXECUTED: bool = false;
        
        unsafe {
            if reset {
                HAS_EXECUTED = false;
                ctx.log("DoOnce: Reset flag, will execute next time".to_string());
                return Ok("".into());
            }
            
            if !HAS_EXECUTED {
                ctx.log("DoOnce: First execution, proceeding".to_string());
                HAS_EXECUTED = true;
                
                if let Err(e) = ctx.trigger_flow_by_pin(&node.id, "Out") {
                    ctx.log(format!("DoOnce: Output execution failed: {}", e));
                    return Err(e);
                }
            } else {
                ctx.log("DoOnce: Already executed, skipping".to_string());
                if let Err(e) = ctx.trigger_flow_by_pin(&node.id, "Already Done") {
                    ctx.log(format!("DoOnce: Already Done output execution failed: {}", e));
                    return Err(e);
                }
            }
        }
        
        Ok("".into())
    }));
    
    let mut do_once = do_once;
    do_once.set_metadata(vec!["Control".into()], "default".into(), Some("Execute output only once, with reset capability".into()));
    registry.register("do_once".into(), Arc::new(do_once));

    // 12. FlipFlop Node - 触发器（交替执行）
    let flip_flop = GenericNode::new_prototype("flip_flop", "Flip Flop");
    flip_flop.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    flip_flop.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "Reset", PinTypeDesc::concrete(ValueType::Boolean)));
    flip_flop.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "A"));
    flip_flop.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "B"));

    flip_flop.set_flow_processor(Box::new(|ctx, node| {
        ctx.log("FlipFlop node: starting execution".to_string());
        
        let reset = ctx.get_pin_value(&node.inputs[0].id).as_bool().unwrap_or(false);
        
        // 使用静态变量来保持状态（在实际实现中，这应该存储在节点实例中）
        static mut IS_A: bool = true;
        
        unsafe {
            if reset {
                IS_A = true;
                ctx.log("FlipFlop: Reset to A".to_string());
                return Ok("".into());
            }
            
            let output_name = if IS_A { "A" } else { "B" };
            ctx.log(format!("FlipFlop: Executing output {}", output_name));
            
            if let Err(e) = ctx.trigger_flow_by_pin(&node.id, output_name) {
                ctx.log(format!("FlipFlop: {} execution failed: {}", output_name, e));
                return Err(e);
            }
            
            // 切换状态
            IS_A = !IS_A;
            ctx.log(format!("FlipFlop: Next output will be {}", if IS_A { "A" } else { "B" }));
        }
        
        Ok("".into())
    }));
    
    let mut flip_flop = flip_flop;
    flip_flop.set_metadata(vec!["Control".into()], "default".into(), Some("Alternate between two outputs on each execution".into()));
    registry.register("flip_flop".into(), Arc::new(flip_flop));

    // 13. Branch Node - 简单的条件分支（类似三元运算符）
    let branch = GenericNode::new_prototype("branch", "Branch");
    branch.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    branch.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "Condition", PinTypeDesc::concrete(ValueType::Boolean)));
    branch.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "TrueValue", PinTypeDesc::any()));
    branch.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "FalseValue", PinTypeDesc::any()));
    branch.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Out"));
    branch.add_out_data_pin(GenericOutDataPin::new(uuid::Uuid::nil(), "Result", PinTypeDesc::any()));

    branch.set_flow_processor(Box::new(|ctx, node| {
        ctx.log("Branch node: starting execution".to_string());
        
        let condition = ctx.get_pin_value(&node.inputs[0].id).as_bool().unwrap_or(false);
        let result = if condition {
            ctx.log("Branch: Condition is true, using TrueValue".to_string());
            ctx.get_pin_value(&node.inputs[1].id)
        } else {
            ctx.log("Branch: Condition is false, using FalseValue".to_string());
            ctx.get_pin_value(&node.inputs[2].id)
        };
        
        // 设置输出值
        ctx.set_pin_value(&node.outputs[0].id, result);
        
        if let Err(e) = ctx.trigger_flow_by_pin(&node.id, "Out") {
            ctx.log(format!("Branch: Output execution failed: {}", e));
            return Err(e);
        }
        
        Ok("".into())
    }));
    
    let mut branch = branch;
    branch.set_metadata(vec!["Control".into()], "default".into(), Some("Select between two values based on condition".into()));
    registry.register("branch".into(), Arc::new(branch));

    // 14. ForEach Node - 遍历数组
    let for_each = GenericNode::new_prototype("for_each", "For Each");
    for_each.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    for_each.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "Array", PinTypeDesc::any().array()));
    for_each.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Loop Body"));
    for_each.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Completed"));
    for_each.add_out_data_pin(GenericOutDataPin::new(uuid::Uuid::nil(), "Item", PinTypeDesc::any()));
    for_each.add_out_data_pin(GenericOutDataPin::new(uuid::Uuid::nil(), "Index", PinTypeDesc::concrete(ValueType::Float64)));

    for_each.set_flow_processor(Box::new(|ctx, node| {
        ctx.log("ForEach node: starting execution".to_string());
        
        let array_value = ctx.get_pin_value(&node.inputs[0].id);
        let array = match array_value.as_array() {
            Some(arr) => arr.clone(), // Clone the array to avoid borrow issues
            None => {
                ctx.log("ForEach: Input is not an array, treating as single item".to_string());
                vec![array_value.clone()]
            }
        };
        
        ctx.log(format!("ForEach: Processing {} items", array.len()));
        
        for (index, item) in array.iter().enumerate() {
            ctx.log(format!("ForEach: Processing item {} (index {})", index, index));
            
            // 设置当前项和索引
            ctx.set_pin_value(&node.outputs[0].id, item.clone());
            ctx.set_pin_value(&node.outputs[1].id, serde_json::Value::Number(serde_json::Number::from(index)));
            
            if let Err(e) = ctx.trigger_flow_by_pin(&node.id, "Loop Body") {
                ctx.log(format!("ForEach: Loop body execution failed at index {}: {}", index, e));
                return Err(e);
            }
            
            // 防止过长执行的安全延迟
            if index % 100 == 99 {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
        
        ctx.log("ForEach: All items processed, executing Completed".to_string());
        if let Err(e) = ctx.trigger_flow_by_pin(&node.id, "Completed") {
            ctx.log(format!("ForEach: Completed execution failed: {}", e));
            return Err(e);
        }
        
        Ok("".into())
    }));
    
    let mut for_each = for_each;
    for_each.set_metadata(vec!["Control".into()], "default".into(), Some("Execute loop body for each item in array".into()));
    registry.register("for_each".into(), Arc::new(for_each));

    // 15. Parallel Node - 并行执行
    let parallel = GenericNode::new_prototype("parallel", "Parallel");
    parallel.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    parallel.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "WaitForAll", PinTypeDesc::concrete(ValueType::Boolean)));
    parallel.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Branch A"));
    parallel.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Branch B"));
    parallel.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Branch C"));
    parallel.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "All Complete"));

    parallel.set_flow_processor(Box::new(|ctx, node| {
        ctx.log("Parallel node: starting execution".to_string());
        
        let wait_for_all = ctx.get_pin_value(&node.inputs[0].id).as_bool().unwrap_or(true);
        
        if wait_for_all {
            ctx.log("Parallel: Executing all branches and waiting for completion".to_string());
            
            // 注意：这是简化的并行实现，实际应该使用线程池
            let branches = ["Branch A", "Branch B", "Branch C"];
            let mut results = Vec::new();
            
            for branch in &branches {
                ctx.log(format!("Parallel: Executing {}", branch));
                match ctx.trigger_flow_by_pin(&node.id, branch) {
                    Ok(_) => {
                        ctx.log(format!("Parallel: {} completed successfully", branch));
                        results.push(Ok(()));
                    }
                    Err(e) => {
                        ctx.log(format!("Parallel: {} failed: {}", branch, e));
                        results.push(Err(e));
                    }
                }
            }
            
            // 检查是否有失败的分支
            for (i, result) in results.iter().enumerate() {
                if let Err(e) = result {
                    return Err(format!("Parallel: {} failed: {}", branches[i], e));
                }
            }
            
            ctx.log("Parallel: All branches completed, executing All Complete".to_string());
            if let Err(e) = ctx.trigger_flow_by_pin(&node.id, "All Complete") {
                ctx.log(format!("Parallel: All Complete execution failed: {}", e));
                return Err(e);
            }
        } else {
            ctx.log("Parallel: Fire-and-forget mode, executing all branches without waiting".to_string());
            
            let branches = ["Branch A", "Branch B", "Branch C"];
            for branch in &branches {
                ctx.log(format!("Parallel: Triggering {}", branch));
                if let Err(e) = ctx.trigger_flow_by_pin(&node.id, branch) {
                    ctx.log(format!("Parallel: {} failed: {}", branch, e));
                    // 在 fire-and-forget 模式下，继续执行其他分支
                }
            }
            
            ctx.log("Parallel: All branches triggered".to_string());
        }
        
        Ok("".into())
    }));
    
    let mut parallel = parallel;
    parallel.set_metadata(vec!["Control".into()], "default".into(), Some("Execute multiple branches in parallel".into()));
    registry.register("parallel".into(), Arc::new(parallel));

    // 16. Throttle Node - 限流控制
    let throttle = GenericNode::new_prototype("throttle", "Throttle");
    throttle.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    throttle.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "IntervalMs", PinTypeDesc::concrete(ValueType::Float64)));
    throttle.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "Reset", PinTypeDesc::concrete(ValueType::Boolean)));
    throttle.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Out"));
    throttle.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Throttled"));

    throttle.set_flow_processor(Box::new(|ctx, node| {
        ctx.log("Throttle node: starting execution".to_string());
        
        let interval_ms = ctx.get_pin_value(&node.inputs[0].id).as_f64().unwrap_or(1000.0) as u64;
        let reset = ctx.get_pin_value(&node.inputs[1].id).as_bool().unwrap_or(false);
        
        use std::time::{SystemTime, UNIX_EPOCH};
        static mut LAST_EXECUTION: u64 = 0;
        
        unsafe {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            
            if reset {
                LAST_EXECUTION = 0;
                ctx.log("Throttle: Reset timestamp".to_string());
                return Ok("".into());
            }
            
            let time_since_last = now.saturating_sub(LAST_EXECUTION);
            
            if time_since_last >= interval_ms {
                ctx.log(format!("Throttle: Allowing execution ({}ms since last)", time_since_last));
                LAST_EXECUTION = now;
                
                if let Err(e) = ctx.trigger_flow_by_pin(&node.id, "Out") {
                    ctx.log(format!("Throttle: Output execution failed: {}", e));
                    return Err(e);
                }
            } else {
                let remaining = interval_ms - time_since_last;
                ctx.log(format!("Throttle: Throttling execution ({}ms remaining)", remaining));
                
                if let Err(e) = ctx.trigger_flow_by_pin(&node.id, "Throttled") {
                    ctx.log(format!("Throttle: Throttled output execution failed: {}", e));
                    return Err(e);
                }
            }
        }
        
        Ok("".into())
    }));
    
    let mut throttle = throttle;
    throttle.set_metadata(vec!["Control".into()], "default".into(), Some("Limit execution frequency to specified interval".into()));
    registry.register("throttle".into(), Arc::new(throttle));

    // 17. Counter Node - 计数器
    let counter = GenericNode::new_prototype("counter", "Counter");
    counter.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "Increment"));
    counter.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "Decrement"));
    counter.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "Reset"));
    counter.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "Step", PinTypeDesc::concrete(ValueType::Float64)));
    counter.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "ResetValue", PinTypeDesc::concrete(ValueType::Float64)));
    counter.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Out"));
    counter.add_out_data_pin(GenericOutDataPin::new(uuid::Uuid::nil(), "Count", PinTypeDesc::concrete(ValueType::Float64)));

    counter.set_flow_processor(Box::new(|ctx, node| {
        ctx.log("Counter node: starting execution".to_string());
        
        let step = ctx.get_pin_value(&node.inputs[0].id).as_f64().unwrap_or(1.0);
        let reset_value = ctx.get_pin_value(&node.inputs[1].id).as_f64().unwrap_or(0.0);
        
        static mut COUNT: f64 = 0.0;
        
        unsafe {
            // 检查哪个执行引脚被触发（这里简化处理）
            // 在实际实现中，应该通过执行上下文知道是哪个引脚触发的
            
            // 假设通过某种方式知道触发的引脚
            let triggered_pin = "Increment"; // 这里需要从上下文获取
            
            match triggered_pin {
                "Increment" => {
                    COUNT += step;
                    ctx.log(format!("Counter: Incremented by {}, new count: {}", step, COUNT));
                }
                "Decrement" => {
                    COUNT -= step;
                    ctx.log(format!("Counter: Decremented by {}, new count: {}", step, COUNT));
                }
                "Reset" => {
                    COUNT = reset_value;
                    ctx.log(format!("Counter: Reset to {}", COUNT));
                }
                _ => {
                    COUNT += step; // 默认为增加
                    ctx.log(format!("Counter: Default increment, new count: {}", COUNT));
                }
            }
            
            // 设置输出值
            ctx.set_pin_value(&node.outputs[0].id, serde_json::Value::Number(
                serde_json::Number::from_f64(COUNT).unwrap_or(serde_json::Number::from(0))
            ));
            
            if let Err(e) = ctx.trigger_flow_by_pin(&node.id, "Out") {
                ctx.log(format!("Counter: Output execution failed: {}", e));
                return Err(e);
            }
        }
        
        Ok("".into())
    }));
    
    let mut counter = counter;
    counter.set_metadata(vec!["Control".into()], "default".into(), Some("Count up, down, or reset with configurable step".into()));
    registry.register("counter".into(), Arc::new(counter));

    // 18. Timer Node - 定时器
    let timer = GenericNode::new_prototype("timer", "Timer");
    timer.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "Start"));
    timer.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "Stop"));
    timer.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "Reset"));
    timer.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "Duration", PinTypeDesc::concrete(ValueType::Float64)));
    timer.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "Loop", PinTypeDesc::concrete(ValueType::Boolean)));
    timer.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Tick"));
    timer.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Finished"));
    timer.add_out_data_pin(GenericOutDataPin::new(uuid::Uuid::nil(), "ElapsedTime", PinTypeDesc::concrete(ValueType::Float64)));

    timer.set_flow_processor(Box::new(|ctx, node| {
        ctx.log("Timer node: starting execution".to_string());
        
        let duration = ctx.get_pin_value(&node.inputs[0].id).as_f64().unwrap_or(1000.0);
        let should_loop = ctx.get_pin_value(&node.inputs[1].id).as_bool().unwrap_or(false);
        
        use std::time::{SystemTime, UNIX_EPOCH};
        static mut START_TIME: u64 = 0;
        static mut IS_RUNNING: bool = false;
        
        unsafe {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            
            // 简化的状态机处理
            let triggered_pin = "Start"; // 这里需要从上下文获取实际触发的引脚
            
            match triggered_pin {
                "Start" => {
                    if !IS_RUNNING {
                        START_TIME = now;
                        IS_RUNNING = true;
                        ctx.log(format!("Timer: Started with duration {}ms", duration));
                    }
                }
                "Stop" => {
                    IS_RUNNING = false;
                    ctx.log("Timer: Stopped".to_string());
                    return Ok("".into());
                }
                "Reset" => {
                    START_TIME = now;
                    IS_RUNNING = false;
                    ctx.log("Timer: Reset".to_string());
                    return Ok("".into());
                }
                _ => {}
            }
            
            if IS_RUNNING {
                let elapsed = now - START_TIME;
                ctx.set_pin_value(&node.outputs[0].id, serde_json::Value::Number(
                    serde_json::Number::from(elapsed)
                ));
                
                if elapsed >= duration as u64 {
                    ctx.log(format!("Timer: Finished after {}ms", elapsed));
                    
                    if should_loop {
                        START_TIME = now;
                        ctx.log("Timer: Looping, restarting".to_string());
                    } else {
                        IS_RUNNING = false;
                    }
                    
                    if let Err(e) = ctx.trigger_flow_by_pin(&node.id, "Finished") {
                        ctx.log(format!("Timer: Finished execution failed: {}", e));
                        return Err(e);
                    }
                } else {
                    if let Err(e) = ctx.trigger_flow_by_pin(&node.id, "Tick") {
                        ctx.log(format!("Timer: Tick execution failed: {}", e));
                        return Err(e);
                    }
                }
            }
        }
        
        Ok("".into())
    }));
    
    let mut timer = timer;
    timer.set_metadata(vec!["Control".into()], "default".into(), Some("Timer with start/stop/reset and optional looping".into()));
    registry.register("timer".into(), Arc::new(timer));

    // ==================== 动态节点示例 ====================

    // 19. Dynamic Sequence Node - 可动态添加输出的序列节点
    register_dynamic_sequence(registry);
}

/// 注册动态 Sequence 节点
fn register_dynamic_sequence(registry: &NodeRegistry) {
    let sequence = GenericNode::new_prototype("sequence_dynamic", "Dynamic Sequence");
    
    // 添加基础 Pin
    sequence.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    sequence.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Then 0"));
    sequence.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Then 1"));
    
    // 设置动态能力
    let dynamic_config = DynamicPinConfig {
        pin_type: DynamicPinType::Exec,
        direction: PinDirection::Output,
        name_template: "Then {}".to_string(),
        data_type: PinTypeDesc::unknown(), // 执行 Pin 不需要具体的数据类型
        min_count: 2,
        max_count: Some(10),
        can_reorder: true,
    };
    
    let capability = NodeDynamicCapability {
        can_add_pins: true,
        dynamic_configs: vec![dynamic_config],
        processor_generator: Some(Box::new(generate_dynamic_sequence_processor)),
    };
    
    sequence.set_dynamic_capability(capability);
    
    // 设置初始处理器
    let initial_processor = generate_dynamic_sequence_processor(&sequence);
    sequence.set_flow_processor(initial_processor);
    
    let mut sequence = sequence;
    sequence.set_metadata(vec!["Control".into()], "default".into(), Some("Dynamic sequence with configurable outputs".into()));
    registry.register("sequence_dynamic".into(), Arc::new(sequence));
}

/// 动态序列处理器生成器
fn generate_dynamic_sequence_processor(node: &GenericNode) -> Box<dyn Fn(&mut dyn crate::executor::ExecutionContextTrait, &crate::executor::NodeDto) -> Result<String, String> + Send + Sync + 'static> {
    // 获取当前所有输出执行 Pin 的名称
    let output_names = node.get_dynamic_exec_output_names();
    
    Box::new(move |ctx, node_data| {
        ctx.log("Dynamic Sequence: starting execution".to_string());
        
        for (index, pin_name) in output_names.iter().enumerate() {
            ctx.log(format!("Dynamic Sequence: executing {}", pin_name));
            
            if let Err(e) = ctx.trigger_flow_by_pin(&node_data.id, pin_name) {
                ctx.log(format!("Dynamic Sequence: {} execution failed: {}", pin_name, e));
                return Err(e);
            }
            
            ctx.log(format!("Dynamic Sequence: {} completed", pin_name));
            
            // 在输出之间添加延迟（除了最后一个）
            if index < output_names.len() - 1 {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
        
        ctx.log("Dynamic Sequence: execution completed".to_string());
        Ok("".into())
    })
}
