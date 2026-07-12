#[cfg(test)]
mod tests {
    use crate::execution::{ExecutionEffect, Executor, NoopEmitter, ResultSourceStore};
    use crate::graph::{
        GraphInstance, GraphRuntime,
        pin::{DataRole, ExecRole, PinRole},
        register::NodeRegistry,
        value::DataValue,
    };
    use std::sync::{Arc, Mutex};

    /// 创建测试用的注册表
    fn create_test_registry() -> Arc<NodeRegistry> {
        let registry = Arc::new(NodeRegistry::new());
        super::super::register(&registry);
        registry
    }

    /// 辅助函数：检查 ExecutionEffect 是否触发了指定的输出
    fn assert_triggers_output(effect: &ExecutionEffect, expected_role: ExecRole) {
        match effect {
            ExecutionEffect::TriggerOutput(role) => {
                assert_eq!(
                    role, &expected_role,
                    "Expected to trigger {:?}",
                    expected_role
                );
            }
            _ => panic!("Expected TriggerOutput, got {:?}", effect),
        }
    }

    /// 创建用于测试的执行器（无需 Tauri Channel）
    fn executor_for_test(graph: Arc<Mutex<GraphRuntime>>) -> Executor<NoopEmitter> {
        Executor::new(graph, NoopEmitter, ResultSourceStore::new())
    }

    /// 辅助函数：检查 ExecutionEffect 是否是 loop
    fn assert_is_loop(effect: &ExecutionEffect, should_continue: bool) {
        match effect {
            ExecutionEffect::Loop {
                body,
                completed,
                should_continue: cont,
            } => {
                assert_eq!(body, &ExecRole::ExecLoopBody);
                assert_eq!(completed, &ExecRole::ExecLoopComplete);
                assert_eq!(*cont, should_continue);
            }
            _ => panic!("Expected Loop, got {:?}", effect),
        }
    }

    /// 辅助函数：检查 ExecutionEffect 是否是 sequence
    fn assert_is_sequence(effect: &ExecutionEffect, expected_roles: Vec<ExecRole>) {
        match effect {
            ExecutionEffect::TriggerAndContinue { current, remaining } => {
                let mut all_roles = vec![current.clone()];
                all_roles.extend(remaining.clone());
                assert_eq!(
                    all_roles, expected_roles,
                    "Expected sequence {:?}",
                    expected_roles
                );
            }
            ExecutionEffect::TriggerOutput(role) if expected_roles.len() == 1 => {
                assert_eq!(
                    role, &expected_roles[0],
                    "Expected single trigger {:?}",
                    expected_roles[0]
                );
            }
            _ => panic!("Expected sequence effect, got {:?}", effect),
        }
    }

    // ==================== Branch 节点测试 ====================

    #[test]
    fn test_branch_node_true_path() {
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(
            "Test Graph",
            crate::graph::GraphKind::Event,
            registry.clone(),
        ));

        // 创建 Branch 节点
        let branch_node = graph
            .create_node("Control Flow:Branch")
            .expect("Failed to create branch node");

        // 获取所有 Pin
        let pins = graph.get_pin_instances_by_node_id(branch_node);

        // 找到 Condition 输入 Pin
        let condition_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Condition))
            .expect("Condition pin not found");

        // 设置 condition 为 true
        graph
            .set_pin_user_value_by_pin_id(condition_pin.id, DataValue::Boolean(true))
            .expect("Failed to set condition value");

        // 创建 GraphRuntime
        let runtime = Arc::new(std::sync::Mutex::new(GraphRuntime::new_standalone(
            graph.clone(),
        )));

        // 执行节点（使用 flow_processor）
        let definition = runtime
            .lock()
            .unwrap()
            .get_node_definition_by_node_id(branch_node);
        let flow_processor = definition
            .flow_processor
            .as_ref()
            .expect("Flow processor not found");

        let mut ctx = crate::execution::NodeExecutionContext::new(runtime.clone(), branch_node);
        let result = flow_processor(&mut ctx);

        assert!(result.is_ok(), "Flow processor failed: {:?}", result.err());

        // 验证返回的是 True 分支
        let effect = result.unwrap();
        assert_triggers_output(&effect, ExecRole::ExecTrue);
    }

    #[test]
    fn test_branch_node_false_path() {
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(
            "Test Graph",
            crate::graph::GraphKind::Event,
            registry.clone(),
        ));

        let branch_node = graph
            .create_node("Control Flow:Branch")
            .expect("Failed to create branch node");

        let pins = graph.get_pin_instances_by_node_id(branch_node);

        let condition_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Condition))
            .expect("Condition pin not found");

        // 设置 condition 为 false
        graph
            .set_pin_user_value_by_pin_id(condition_pin.id, DataValue::Boolean(false))
            .expect("Failed to set condition value");

        // 创建 GraphRuntime
        let runtime = Arc::new(std::sync::Mutex::new(GraphRuntime::new_standalone(
            graph.clone(),
        )));

        let definition = runtime
            .lock()
            .unwrap()
            .get_node_definition_by_node_id(branch_node);
        let flow_processor = definition
            .flow_processor
            .as_ref()
            .expect("Flow processor not found");

        let mut ctx = crate::execution::NodeExecutionContext::new(runtime.clone(), branch_node);
        let result = flow_processor(&mut ctx);

        assert!(result.is_ok(), "Flow processor failed: {:?}", result.err());

        // 验证返回的是 False 分支
        let effect = result.unwrap();
        assert_triggers_output(&effect, ExecRole::ExecFalse);
    }

    #[test]
    fn test_branch_node_default_value() {
        // 测试不设置值时使用默认值（false）
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(
            "Test Graph",
            crate::graph::GraphKind::Event,
            registry.clone(),
        ));

        let branch_node = graph
            .create_node("Control Flow:Branch")
            .expect("Failed to create branch node");

        // 创建 GraphRuntime
        let runtime = Arc::new(std::sync::Mutex::new(GraphRuntime::new_standalone(
            graph.clone(),
        )));

        let definition = runtime
            .lock()
            .unwrap()
            .get_node_definition_by_node_id(branch_node);
        let flow_processor = definition
            .flow_processor
            .as_ref()
            .expect("Flow processor not found");

        let mut ctx = crate::execution::NodeExecutionContext::new(runtime.clone(), branch_node);
        let result = flow_processor(&mut ctx);

        assert!(result.is_ok(), "Flow processor failed: {:?}", result.err());

        // 默认值是 false，应该走 False 分支
        let effect = result.unwrap();
        assert_triggers_output(&effect, ExecRole::ExecFalse);
    }

    // ==================== Sequence 节点测试 ====================

    #[test]
    fn test_sequence_node_basic() {
        let registry = create_test_registry();
        let graph = GraphInstance::new(
            "Test Graph",
            crate::graph::GraphKind::Event,
            registry.clone(),
        );

        // 创建 Sequence 节点
        let seq_node = graph
            .create_node("Control Flow:Sequence")
            .expect("Failed to create sequence node");

        // 验证有 3 个默认步骤
        let pins = graph.get_pin_instances_by_node_id(seq_node);
        let step_pins: Vec<_> = pins
            .iter()
            .filter(|p| matches!(p.definition.role, PinRole::Exec(ExecRole::Steps(_))))
            .collect();

        assert_eq!(step_pins.len(), 3, "Should have 3 default steps");

        // 验证步骤索引
        assert!(
            step_pins
                .iter()
                .any(|p| p.definition.role == PinRole::Exec(ExecRole::Steps(0))),
            "Should have Step 0"
        );
        assert!(
            step_pins
                .iter()
                .any(|p| p.definition.role == PinRole::Exec(ExecRole::Steps(1))),
            "Should have Step 1"
        );
        assert!(
            step_pins
                .iter()
                .any(|p| p.definition.role == PinRole::Exec(ExecRole::Steps(2))),
            "Should have Step 2"
        );
    }

    #[test]
    fn test_sequence_node_execution() {
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(
            "Test Graph",
            crate::graph::GraphKind::Event,
            registry.clone(),
        ));

        let seq_node = graph
            .create_node("Control Flow:Sequence")
            .expect("Failed to create sequence node");

        // 创建 GraphRuntime
        let runtime = Arc::new(std::sync::Mutex::new(GraphRuntime::new_standalone(
            graph.clone(),
        )));

        // 执行节点
        let definition = runtime
            .lock()
            .unwrap()
            .get_node_definition_by_node_id(seq_node);
        let flow_processor = definition
            .flow_processor
            .as_ref()
            .expect("Flow processor not found");

        let mut ctx = crate::execution::NodeExecutionContext::new(runtime.clone(), seq_node);
        let result = flow_processor(&mut ctx);

        assert!(result.is_ok(), "Flow processor failed: {:?}", result.err());

        // 验证返回 sequence effect
        let effect = result.unwrap();
        assert_is_sequence(
            &effect,
            vec![ExecRole::Steps(0), ExecRole::Steps(1), ExecRole::Steps(2)],
        );
    }

    #[test]
    fn test_sequence_node_with_four_steps() {
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(
            "Test Graph",
            crate::graph::GraphKind::Event,
            registry.clone(),
        ));

        let seq_node = graph
            .create_node("Control Flow:Sequence")
            .expect("Failed to create sequence node");
        graph
            .add_repeatable_pin(seq_node, 1)
            .expect("Failed to add fourth Then pin");

        let runtime = Arc::new(std::sync::Mutex::new(GraphRuntime::new_standalone(
            graph.clone(),
        )));

        let definition = runtime
            .lock()
            .unwrap()
            .get_node_definition_by_node_id(seq_node);
        let flow_processor = definition
            .flow_processor
            .as_ref()
            .expect("Flow processor not found");

        let mut ctx = crate::execution::NodeExecutionContext::new(runtime.clone(), seq_node);
        let effect = flow_processor(&mut ctx).expect("Flow processor failed");

        assert_is_sequence(
            &effect,
            vec![
                ExecRole::Steps(0),
                ExecRole::Steps(1),
                ExecRole::Steps(2),
                ExecRole::Steps(3),
            ],
        );
    }

    // ==================== Branch + Sequence 连接测试（使用 Executor 自动执行）====================

    #[test]
    fn test_branch_chain_with_executor() {
        // 测试场景（使用 Executor 自动执行）：
        // branch1 (condition=false) -> false output -> branch2 exec input
        // branch2 (condition=false) -> false output -> sequence exec input
        // 验证：Executor 自动执行整个链路，最终触发 sequence

        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(
            "Test Graph",
            crate::graph::GraphKind::Event,
            registry.clone(),
        ));

        // 创建节点
        let branch1_node = graph
            .create_node("Control Flow:Branch")
            .expect("Failed to create branch1 node");

        let branch2_node = graph
            .create_node("Control Flow:Branch")
            .expect("Failed to create branch2 node");

        let seq_node = graph
            .create_node("Control Flow:Sequence")
            .expect("Failed to create sequence node");

        // === 设置 branch1 的 condition 为 false ===
        let branch1_pins = graph.get_pin_instances_by_node_id(branch1_node);
        let branch1_condition = branch1_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Condition))
            .expect("Branch1 condition pin not found");

        graph
            .set_pin_user_value_by_pin_id(branch1_condition.id, DataValue::Boolean(false))
            .expect("Failed to set branch1 condition");

        // === 设置 branch2 的 condition 为 false ===
        let branch2_pins = graph.get_pin_instances_by_node_id(branch2_node);
        let branch2_condition = branch2_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Condition))
            .expect("Branch2 condition pin not found");

        graph
            .set_pin_user_value_by_pin_id(branch2_condition.id, DataValue::Boolean(false))
            .expect("Failed to set branch2 condition");

        // === 连接：branch1.false -> branch2.exec_in ===
        let branch1_false_out = branch1_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecFalse))
            .expect("Branch1 false output not found");

        let branch2_exec_in = branch2_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecIn))
            .expect("Branch2 exec input not found");

        graph
            .connect(branch1_false_out.id, branch2_exec_in.id)
            .expect("Failed to connect branch1 false to branch2 exec in");

        // === 连接：branch2.false -> sequence.exec_in ===
        let branch2_false_out = branch2_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecFalse))
            .expect("Branch2 false output not found");

        let seq_pins = graph.get_pin_instances_by_node_id(seq_node);
        let seq_exec_in = seq_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecIn))
            .expect("Sequence exec input not found");

        graph
            .connect(branch2_false_out.id, seq_exec_in.id)
            .expect("Failed to connect branch2 false to sequence exec in");

        // === 使用 Executor 自动执行 ===
        let runtime = Arc::new(std::sync::Mutex::new(GraphRuntime::new_standalone(
            graph.clone(),
        )));
        let mut executor = executor_for_test(runtime);
        let result = executor.start(branch1_node);

        assert!(result.is_ok(), "Executor failed: {:?}", result.err());

        // 打印执行日志
        println!("\n=== Execution Logs ===");
        for log in executor.logs() {
            println!("{}", log);
        }

        // 验证执行结果：所有节点都应该被执行
        // 可以通过检查节点状态或输出值来验证
        println!("\n=== Stack Debug Info ===");
        println!("{}", executor.debug_stack());
    }

    #[test]
    fn test_branch_true_path_with_executor() {
        // 测试场景（使用 Executor）：
        // branch1 (condition=true) -> true output -> sequence1 exec input
        // branch1 (condition=true) -> false output -> branch2 exec input (不会执行)
        // 验证：只有 sequence1 会被执行

        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(
            "Test Graph",
            crate::graph::GraphKind::Event,
            registry.clone(),
        ));

        // 创建节点
        let branch1_node = graph
            .create_node("Control Flow:Branch")
            .expect("Failed to create branch1 node");

        let seq1_node = graph
            .create_node("Control Flow:Sequence")
            .expect("Failed to create sequence1 node");

        let branch2_node = graph
            .create_node("Control Flow:Branch")
            .expect("Failed to create branch2 node");

        // === 设置 branch1 的 condition 为 true ===
        let branch1_pins = graph.get_pin_instances_by_node_id(branch1_node);
        let branch1_condition = branch1_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Condition))
            .expect("Branch1 condition pin not found");

        graph
            .set_pin_user_value_by_pin_id(branch1_condition.id, DataValue::Boolean(true))
            .expect("Failed to set branch1 condition");

        // === 连接：branch1.true -> sequence1.exec_in ===
        let branch1_true_out = branch1_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecTrue))
            .expect("Branch1 true output not found");

        let seq1_pins = graph.get_pin_instances_by_node_id(seq1_node);
        let seq1_exec_in = seq1_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecIn))
            .expect("Sequence1 exec input not found");

        graph
            .connect(branch1_true_out.id, seq1_exec_in.id)
            .expect("Failed to connect branch1 true to sequence1 exec in");

        // === 连接：branch1.false -> branch2.exec_in ===
        let branch1_false_out = branch1_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecFalse))
            .expect("Branch1 false output not found");

        let branch2_pins = graph.get_pin_instances_by_node_id(branch2_node);
        let branch2_exec_in = branch2_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecIn))
            .expect("Branch2 exec input not found");

        graph
            .connect(branch1_false_out.id, branch2_exec_in.id)
            .expect("Failed to connect branch1 false to branch2 exec in");

        // === 使用 Executor 自动执行 ===
        let runtime = Arc::new(std::sync::Mutex::new(GraphRuntime::new_standalone(
            graph.clone(),
        )));
        let mut executor = executor_for_test(runtime);
        let result = executor.start(branch1_node);

        assert!(result.is_ok(), "Executor failed: {:?}", result.err());

        // 打印执行日志
        println!("\n=== Execution Logs ===");
        for log in executor.logs() {
            println!("{}", log);
        }

        // 验证：sequence 应该被执行（检查是否有 TriggerAndContinue 效果）
        let logs_str = executor.logs().join("\n");
        assert!(
            logs_str.contains("TriggerAndContinue") || logs_str.contains("Steps"),
            "Sequence should be executed (expected TriggerAndContinue or Steps in logs)"
        );

        println!("\n=== Stack Debug Info ===");
        println!("{}", executor.debug_stack());
    }

    // ==================== Do 节点测试 ====================

    #[test]
    fn test_do_node_triggers_out() {
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(
            "Test Graph",
            crate::graph::GraphKind::Event,
            registry.clone(),
        ));

        let do_node = graph
            .create_node("Control Flow:Do")
            .expect("Failed to create Do node");

        let runtime = Arc::new(std::sync::Mutex::new(GraphRuntime::new_standalone(
            graph.clone(),
        )));

        let definition = runtime
            .lock()
            .unwrap()
            .get_node_definition_by_node_id(do_node);
        let flow_processor = definition
            .flow_processor
            .as_ref()
            .expect("Flow processor not found");

        let mut ctx = crate::execution::NodeExecutionContext::new(runtime.clone(), do_node);
        let effect = flow_processor(&mut ctx).expect("Flow processor failed");
        assert_triggers_output(&effect, ExecRole::ExecOut);
    }

    // ==================== Merge 节点测试 ====================

    #[test]
    fn test_merge_node_has_repeatable_inputs() {
        let registry = create_test_registry();
        let graph = GraphInstance::new(
            "Test Graph",
            crate::graph::GraphKind::Event,
            registry.clone(),
        );

        let merge_node = graph
            .create_node("Control Flow:Merge")
            .expect("Failed to create Merge node");

        let pins = graph.get_pin_instances_by_node_id(merge_node);
        let input_pins: Vec<_> = pins
            .iter()
            .filter(|p| matches!(p.definition.role, PinRole::Exec(ExecRole::ExecInputs(_))))
            .collect();

        assert_eq!(input_pins.len(), 2, "Merge should have 2 default In pins");
    }

    #[test]
    fn test_merge_node_triggers_out() {
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(
            "Test Graph",
            crate::graph::GraphKind::Event,
            registry.clone(),
        ));

        let merge_node = graph
            .create_node("Control Flow:Merge")
            .expect("Failed to create Merge node");

        let runtime = Arc::new(std::sync::Mutex::new(GraphRuntime::new_standalone(
            graph.clone(),
        )));

        let definition = runtime
            .lock()
            .unwrap()
            .get_node_definition_by_node_id(merge_node);
        let flow_processor = definition
            .flow_processor
            .as_ref()
            .expect("Flow processor not found");

        let mut ctx = crate::execution::NodeExecutionContext::new(runtime.clone(), merge_node);
        let effect = flow_processor(&mut ctx).expect("Flow processor failed");
        assert_triggers_output(&effect, ExecRole::ExecOut);
    }

    // ==================== Sleep 节点测试 ====================

    #[test]
    fn test_sleep_node_short_duration() {
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(
            "Test Graph",
            crate::graph::GraphKind::Event,
            registry.clone(),
        ));

        let sleep_node = graph
            .create_node("Control Flow:Sleep")
            .expect("Failed to create Sleep node");

        let pins = graph.get_pin_instances_by_node_id(sleep_node);
        let duration_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Input))
            .expect("Duration pin not found");

        graph
            .set_pin_user_value_by_pin_id(duration_pin.id, DataValue::Float64(0.05))
            .expect("Failed to set duration");

        let runtime = Arc::new(std::sync::Mutex::new(GraphRuntime::new_standalone(
            graph.clone(),
        )));

        let definition = runtime
            .lock()
            .unwrap()
            .get_node_definition_by_node_id(sleep_node);
        let flow_processor = definition
            .flow_processor
            .as_ref()
            .expect("Flow processor not found");

        let start = std::time::Instant::now();
        let mut ctx = crate::execution::NodeExecutionContext::new(runtime.clone(), sleep_node);
        let effect = flow_processor(&mut ctx).expect("Flow processor failed");
        let elapsed = start.elapsed();

        assert_triggers_output(&effect, ExecRole::ExecOut);
        assert!(
            elapsed.as_secs_f64() >= 0.04,
            "Sleep should wait at least 0.04s, got {:?}",
            elapsed
        );
    }

    #[test]
    fn test_sleep_node_rejects_negative_duration() {
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(
            "Test Graph",
            crate::graph::GraphKind::Event,
            registry.clone(),
        ));

        let sleep_node = graph
            .create_node("Control Flow:Sleep")
            .expect("Failed to create Sleep node");

        let pins = graph.get_pin_instances_by_node_id(sleep_node);
        let duration_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Input))
            .expect("Duration pin not found");

        graph
            .set_pin_user_value_by_pin_id(duration_pin.id, DataValue::Float64(-1.0))
            .expect("Failed to set duration");

        let runtime = Arc::new(std::sync::Mutex::new(GraphRuntime::new_standalone(
            graph.clone(),
        )));

        let definition = runtime
            .lock()
            .unwrap()
            .get_node_definition_by_node_id(sleep_node);
        let flow_processor = definition
            .flow_processor
            .as_ref()
            .expect("Flow processor not found");

        let mut ctx = crate::execution::NodeExecutionContext::new(runtime.clone(), sleep_node);
        let result = flow_processor(&mut ctx);
        assert!(result.is_err(), "Negative duration should fail");
    }

    // ==================== For Loop 节点测试 ====================

    #[test]
    fn test_for_loop_first_iteration() {
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(
            "Test Graph",
            crate::graph::GraphKind::Event,
            registry.clone(),
        ));

        let loop_node = graph
            .create_node("Control Flow:For Loop")
            .expect("Failed to create For Loop node");

        let pins = graph.get_pin_instances_by_node_id(loop_node);
        let count_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Input))
            .expect("Count pin not found");
        graph
            .set_pin_user_value_by_pin_id(count_pin.id, DataValue::Int64(3))
            .expect("Failed to set count");

        let runtime = Arc::new(std::sync::Mutex::new(GraphRuntime::new_standalone(
            graph.clone(),
        )));

        let definition = runtime
            .lock()
            .unwrap()
            .get_node_definition_by_node_id(loop_node);
        let flow_processor = definition
            .flow_processor
            .as_ref()
            .expect("Flow processor not found");

        let mut ctx = crate::execution::NodeExecutionContext::new(runtime.clone(), loop_node);
        let effect = flow_processor(&mut ctx).expect("Flow processor failed");
        assert_is_loop(&effect, true);
        assert_eq!(runtime.lock().unwrap().get_loop_counter(loop_node), 1);

        let index_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Custom("index".to_string())))
            .expect("Index pin not found");
        let index_value = runtime
            .lock()
            .unwrap()
            .get_pin_data_value_by_pin_id(index_pin.id)
            .expect("Index value missing");
        assert_eq!(index_value, DataValue::Int64(0));
    }

    #[test]
    fn test_for_loop_completes_after_count() {
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(
            "Test Graph",
            crate::graph::GraphKind::Event,
            registry.clone(),
        ));

        let loop_node = graph
            .create_node("Control Flow:For Loop")
            .expect("Failed to create For Loop node");

        let pins = graph.get_pin_instances_by_node_id(loop_node);
        let count_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Input))
            .expect("Count pin not found");
        graph
            .set_pin_user_value_by_pin_id(count_pin.id, DataValue::Int64(2))
            .expect("Failed to set count");

        let runtime = Arc::new(std::sync::Mutex::new(GraphRuntime::new_standalone(
            graph.clone(),
        )));
        runtime.lock().unwrap().set_loop_counter(loop_node, 2);

        let definition = runtime
            .lock()
            .unwrap()
            .get_node_definition_by_node_id(loop_node);
        let flow_processor = definition
            .flow_processor
            .as_ref()
            .expect("Flow processor not found");

        let mut ctx = crate::execution::NodeExecutionContext::new(runtime.clone(), loop_node);
        let effect = flow_processor(&mut ctx).expect("Flow processor failed");
        assert_is_loop(&effect, false);
        assert_eq!(runtime.lock().unwrap().get_loop_counter(loop_node), 0);
    }

    // ==================== Switch 节点测试 ====================

    #[test]
    fn test_switch_triggers_matching_case() {
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(
            "Test Graph",
            crate::graph::GraphKind::Event,
            registry.clone(),
        ));

        let switch_node = graph
            .create_node("Control Flow:Switch")
            .expect("Failed to create Switch node");

        let pins = graph.get_pin_instances_by_node_id(switch_node);
        let selector_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Input))
            .expect("Selector pin not found");
        graph
            .set_pin_user_value_by_pin_id(selector_pin.id, DataValue::Int64(1))
            .expect("Failed to set selector");

        let runtime = Arc::new(std::sync::Mutex::new(GraphRuntime::new_standalone(
            graph.clone(),
        )));

        let definition = runtime
            .lock()
            .unwrap()
            .get_node_definition_by_node_id(switch_node);
        let flow_processor = definition
            .flow_processor
            .as_ref()
            .expect("Flow processor not found");

        let mut ctx = crate::execution::NodeExecutionContext::new(runtime.clone(), switch_node);
        let effect = flow_processor(&mut ctx).expect("Flow processor failed");
        assert_triggers_output(&effect, ExecRole::Cases(1));
    }

    #[test]
    fn test_switch_falls_back_to_default() {
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(
            "Test Graph",
            crate::graph::GraphKind::Event,
            registry.clone(),
        ));

        let switch_node = graph
            .create_node("Control Flow:Switch")
            .expect("Failed to create Switch node");

        let pins = graph.get_pin_instances_by_node_id(switch_node);
        let selector_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Input))
            .expect("Selector pin not found");
        graph
            .set_pin_user_value_by_pin_id(selector_pin.id, DataValue::Int64(99))
            .expect("Failed to set selector");

        let runtime = Arc::new(std::sync::Mutex::new(GraphRuntime::new_standalone(
            graph.clone(),
        )));

        let definition = runtime
            .lock()
            .unwrap()
            .get_node_definition_by_node_id(switch_node);
        let flow_processor = definition
            .flow_processor
            .as_ref()
            .expect("Flow processor not found");

        let mut ctx = crate::execution::NodeExecutionContext::new(runtime.clone(), switch_node);
        let effect = flow_processor(&mut ctx).expect("Flow processor failed");
        assert_triggers_output(&effect, ExecRole::ExecFalse);
    }

    // ==================== While Loop 节点测试 ====================

    #[test]
    fn test_while_loop_continues_when_condition_true() {
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(
            "Test Graph",
            crate::graph::GraphKind::Event,
            registry.clone(),
        ));

        let while_node = graph
            .create_node("Control Flow:While Loop")
            .expect("Failed to create While Loop node");

        let pins = graph.get_pin_instances_by_node_id(while_node);
        let condition_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Condition))
            .expect("Condition pin not found");
        graph
            .set_pin_user_value_by_pin_id(condition_pin.id, DataValue::Boolean(true))
            .expect("Failed to set condition");

        let runtime = Arc::new(std::sync::Mutex::new(GraphRuntime::new_standalone(
            graph.clone(),
        )));

        let definition = runtime
            .lock()
            .unwrap()
            .get_node_definition_by_node_id(while_node);
        let flow_processor = definition
            .flow_processor
            .as_ref()
            .expect("Flow processor not found");

        let mut ctx = crate::execution::NodeExecutionContext::new(runtime.clone(), while_node);
        let effect = flow_processor(&mut ctx).expect("Flow processor failed");
        assert_is_loop(&effect, true);
        assert_eq!(runtime.lock().unwrap().get_loop_counter(while_node), 1);
    }

    #[test]
    fn test_while_loop_exits_when_condition_false() {
        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(
            "Test Graph",
            crate::graph::GraphKind::Event,
            registry.clone(),
        ));

        let while_node = graph
            .create_node("Control Flow:While Loop")
            .expect("Failed to create While Loop node");

        let runtime = Arc::new(std::sync::Mutex::new(GraphRuntime::new_standalone(
            graph.clone(),
        )));

        let definition = runtime
            .lock()
            .unwrap()
            .get_node_definition_by_node_id(while_node);
        let flow_processor = definition
            .flow_processor
            .as_ref()
            .expect("Flow processor not found");

        let mut ctx = crate::execution::NodeExecutionContext::new(runtime.clone(), while_node);
        let effect = flow_processor(&mut ctx).expect("Flow processor failed");
        assert_is_loop(&effect, false);
    }

    // ==================== Branch + Sequence 连接测试（手动触发，保留用于单元测试）====================

    #[test]
    fn test_branch_chain_to_sequence() {
        // 测试场景（使用 Executor）：
        // branch1 (condition=false) -> false output -> branch2 exec input
        // branch2 (condition=true) -> false output -> sequence exec input
        // 验证：branch1 走 false 分支，branch2 走 true 分支（不会触发 sequence）

        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(
            "Test Graph",
            crate::graph::GraphKind::Event,
            registry.clone(),
        ));

        // 创建第一个 Branch 节点
        let branch1_node = graph
            .create_node("Control Flow:Branch")
            .expect("Failed to create branch1 node");

        // 创建第二个 Branch 节点
        let branch2_node = graph
            .create_node("Control Flow:Branch")
            .expect("Failed to create branch2 node");

        // 创建 Sequence 节点
        let seq_node = graph
            .create_node("Control Flow:Sequence")
            .expect("Failed to create sequence node");

        // === 设置 branch1 的 condition 为 false ===
        let branch1_pins = graph.get_pin_instances_by_node_id(branch1_node);
        let branch1_condition = branch1_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Condition))
            .expect("Branch1 condition pin not found");

        graph
            .set_pin_user_value_by_pin_id(branch1_condition.id, DataValue::Boolean(false))
            .expect("Failed to set branch1 condition");

        // === 设置 branch2 的 condition 为 true ===
        let branch2_pins = graph.get_pin_instances_by_node_id(branch2_node);
        let branch2_condition = branch2_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Condition))
            .expect("Branch2 condition pin not found");

        graph
            .set_pin_user_value_by_pin_id(branch2_condition.id, DataValue::Boolean(true))
            .expect("Failed to set branch2 condition");

        // === 连接：branch1.false -> branch2.exec_in ===
        let branch1_false_out = branch1_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecFalse))
            .expect("Branch1 false output not found");

        let branch2_exec_in = branch2_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecIn))
            .expect("Branch2 exec input not found");

        graph
            .connect(branch1_false_out.id, branch2_exec_in.id)
            .expect("Failed to connect branch1 false to branch2 exec in");

        // === 连接：branch2.false -> sequence.exec_in ===
        let branch2_false_out = branch2_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecFalse))
            .expect("Branch2 false output not found");

        let seq_pins = graph.get_pin_instances_by_node_id(seq_node);
        let seq_exec_in = seq_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecIn))
            .expect("Sequence exec input not found");

        graph
            .connect(branch2_false_out.id, seq_exec_in.id)
            .expect("Failed to connect branch2 false to sequence exec in");

        // === 使用 Executor 自动执行 ===
        let runtime = Arc::new(std::sync::Mutex::new(GraphRuntime::new_standalone(
            graph.clone(),
        )));
        let mut executor = executor_for_test(runtime);
        let result = executor.start(branch1_node);

        assert!(result.is_ok(), "Executor failed: {:?}", result.err());

        // 打印执行日志
        println!("\n=== Execution Logs ===");
        for log in executor.logs() {
            println!("{}", log);
        }

        // 验证：branch2 走 true 分支，所以 sequence 不应该被执行
        let logs_str = executor.logs().join("\n");
        assert!(
            logs_str.contains("ExecTrue"),
            "Branch2 should trigger true path"
        );

        // 验证连接关系
        let connections = graph.all_connections();
        assert!(
            connections
                .iter()
                .any(|c| c.from_pin == branch1_false_out.id && c.to_pin == branch2_exec_in.id),
            "Connection from branch1 false to branch2 exec in should exist"
        );
        assert!(
            connections
                .iter()
                .any(|c| c.from_pin == branch2_false_out.id && c.to_pin == seq_exec_in.id),
            "Connection from branch2 false to sequence exec in should exist"
        );

        println!("\n=== Stack Debug Info ===");
        println!("{}", executor.debug_stack());
    }

    #[test]
    fn test_branch_chain_reaches_sequence() {
        // 测试场景（使用 Executor）：
        // branch1 (condition=false) -> false output -> branch2 exec input
        // branch2 (condition=false) -> false output -> sequence exec input
        // 验证：branch1 走 false 分支，branch2 也走 false 分支，最终触发 sequence

        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(
            "Test Graph",
            crate::graph::GraphKind::Event,
            registry.clone(),
        ));

        // 创建节点
        let branch1_node = graph
            .create_node("Control Flow:Branch")
            .expect("Failed to create branch1 node");

        let branch2_node = graph
            .create_node("Control Flow:Branch")
            .expect("Failed to create branch2 node");

        let seq_node = graph
            .create_node("Control Flow:Sequence")
            .expect("Failed to create sequence node");

        // === 设置 branch1 的 condition 为 false ===
        let branch1_pins = graph.get_pin_instances_by_node_id(branch1_node);
        let branch1_condition = branch1_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Condition))
            .expect("Branch1 condition pin not found");

        graph
            .set_pin_user_value_by_pin_id(branch1_condition.id, DataValue::Boolean(false))
            .expect("Failed to set branch1 condition");

        // === 设置 branch2 的 condition 为 false ===
        let branch2_pins = graph.get_pin_instances_by_node_id(branch2_node);
        let branch2_condition = branch2_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Condition))
            .expect("Branch2 condition pin not found");

        graph
            .set_pin_user_value_by_pin_id(branch2_condition.id, DataValue::Boolean(false))
            .expect("Failed to set branch2 condition");

        // === 连接：branch1.false -> branch2.exec_in ===
        let branch1_false_out = branch1_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecFalse))
            .expect("Branch1 false output not found");

        let branch2_exec_in = branch2_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecIn))
            .expect("Branch2 exec input not found");

        graph
            .connect(branch1_false_out.id, branch2_exec_in.id)
            .expect("Failed to connect branch1 false to branch2 exec in");

        // === 连接：branch2.false -> sequence.exec_in ===
        let branch2_false_out = branch2_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecFalse))
            .expect("Branch2 false output not found");

        let seq_pins = graph.get_pin_instances_by_node_id(seq_node);
        let seq_exec_in = seq_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecIn))
            .expect("Sequence exec input not found");

        graph
            .connect(branch2_false_out.id, seq_exec_in.id)
            .expect("Failed to connect branch2 false to sequence exec in");

        // === 使用 Executor 自动执行 ===
        let runtime = Arc::new(std::sync::Mutex::new(GraphRuntime::new_standalone(
            graph.clone(),
        )));
        let mut executor = executor_for_test(runtime);
        let result = executor.start(branch1_node);

        assert!(result.is_ok(), "Executor failed: {:?}", result.err());

        // 打印执行日志
        println!("\n=== Execution Logs ===");
        for log in executor.logs() {
            println!("{}", log);
        }

        // 验证：sequence 应该被执行
        let logs_str = executor.logs().join("\n");
        assert!(
            logs_str.contains("TriggerAndContinue") || logs_str.contains("Steps"),
            "Sequence should be executed (expected TriggerAndContinue or Steps in logs)"
        );

        println!("\n=== Stack Debug Info ===");
        println!("{}", executor.debug_stack());
    }

    #[test]
    fn test_complex_branch_sequence_flow() {
        // 测试更复杂的场景（使用 Executor）：
        // branch1 (condition=true) -> true output -> sequence1 exec input
        // branch1 (condition=true) -> false output -> branch2 exec input (不会执行)
        // branch2 (condition=false) -> false output -> sequence2 exec input (不会执行)
        // 验证：只有 sequence1 会被触发

        let registry = create_test_registry();
        let graph = Arc::new(GraphInstance::new(
            "Test Graph",
            crate::graph::GraphKind::Event,
            registry.clone(),
        ));

        // 创建节点
        let branch1_node = graph
            .create_node("Control Flow:Branch")
            .expect("Failed to create branch1 node");

        let branch2_node = graph
            .create_node("Control Flow:Branch")
            .expect("Failed to create branch2 node");

        let seq1_node = graph
            .create_node("Control Flow:Sequence")
            .expect("Failed to create sequence1 node");

        let seq2_node = graph
            .create_node("Control Flow:Sequence")
            .expect("Failed to create sequence2 node");

        // === 设置 branch1 的 condition 为 true ===
        let branch1_pins = graph.get_pin_instances_by_node_id(branch1_node);
        let branch1_condition = branch1_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Condition))
            .expect("Branch1 condition pin not found");

        graph
            .set_pin_user_value_by_pin_id(branch1_condition.id, DataValue::Boolean(true))
            .expect("Failed to set branch1 condition");

        // === 设置 branch2 的 condition 为 false ===
        let branch2_pins = graph.get_pin_instances_by_node_id(branch2_node);
        let branch2_condition = branch2_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Condition))
            .expect("Branch2 condition pin not found");

        graph
            .set_pin_user_value_by_pin_id(branch2_condition.id, DataValue::Boolean(false))
            .expect("Failed to set branch2 condition");

        // === 连接：branch1.true -> sequence1.exec_in ===
        let branch1_true_out = branch1_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecTrue))
            .expect("Branch1 true output not found");

        let seq1_pins = graph.get_pin_instances_by_node_id(seq1_node);
        let seq1_exec_in = seq1_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecIn))
            .expect("Sequence1 exec input not found");

        graph
            .connect(branch1_true_out.id, seq1_exec_in.id)
            .expect("Failed to connect branch1 true to sequence1 exec in");

        // === 连接：branch1.false -> branch2.exec_in ===
        let branch1_false_out = branch1_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecFalse))
            .expect("Branch1 false output not found");

        let branch2_exec_in = branch2_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecIn))
            .expect("Branch2 exec input not found");

        graph
            .connect(branch1_false_out.id, branch2_exec_in.id)
            .expect("Failed to connect branch1 false to branch2 exec in");

        // === 连接：branch2.false -> sequence2.exec_in ===
        let branch2_false_out = branch2_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecFalse))
            .expect("Branch2 false output not found");

        let seq2_pins = graph.get_pin_instances_by_node_id(seq2_node);
        let seq2_exec_in = seq2_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecIn))
            .expect("Sequence2 exec input not found");

        graph
            .connect(branch2_false_out.id, seq2_exec_in.id)
            .expect("Failed to connect branch2 false to sequence2 exec in");

        // === 使用 Executor 自动执行 ===
        let runtime = Arc::new(std::sync::Mutex::new(GraphRuntime::new_standalone(
            graph.clone(),
        )));
        let mut executor = executor_for_test(runtime);
        let result = executor.start(branch1_node);

        assert!(result.is_ok(), "Executor failed: {:?}", result.err());

        // 打印执行日志
        println!("\n=== Execution Logs ===");
        for log in executor.logs() {
            println!("{}", log);
        }

        // 验证：只有 sequence1 应该被执行
        let logs_str = executor.logs().join("\n");
        assert!(
            logs_str.contains("TriggerAndContinue") || logs_str.contains("Steps"),
            "Sequence1 should be executed"
        );

        // 验证 branch1 走了 true 分支
        assert!(
            logs_str.contains("ExecTrue"),
            "Branch1 should trigger true path"
        );

        println!("\n=== Stack Debug Info ===");
        println!("{}", executor.debug_stack());
    }
}
