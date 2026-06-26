mod common;

use std::sync::Arc;
use yssbi_lib::graph::{
    core::GraphInstance,
    pin::{DataRole, ExecRole, PinRole},
    register::NodeRegistry,
    value::DataValue,
};

/// 创建测试用的注册表
fn create_test_registry() -> Arc<NodeRegistry> {
    let registry = Arc::new(NodeRegistry::new());
    // 注册所有内置节点
    yssbi_lib::graph::register::catalog::register_builtin_nodes(&registry);
    registry
}

#[test]
fn test_complex_node_graph() {
    // 综合测试：sequence + branch + add + equal + print 节点的组合
    //
    // 图结构：
    // sequence1 (3 个输出)
    //   ├─ Step 0 -> sequence2 (3 个输出)
    //   │            ├─ Step 0 -> print("Sequence2-Step0")
    //   │            ├─ Step 1 -> print("Sequence2-Step1")
    //   │            └─ Step 2 -> print("Sequence2-Step2")
    //   ├─ Step 1 -> branch1 (condition=false)
    //   │            ├─ True -> print("Branch1-True")
    //   │            └─ False -> print("Branch1-False")
    //   └─ Step 2 -> branch2 (condition=add(10,10)==20)
    //                ├─ True -> print("Branch2-True")
    //                └─ False -> print("Branch2-False")

    let registry = create_test_registry();
    let graph = Arc::new(GraphInstance::new(
        "Complex Test Graph",
        yssbi_lib::graph::GraphKind::Event,
        registry.clone(),
    ));

    println!("\n=== Creating Nodes ===");

    // 创建第一个 Sequence 节点
    let seq1_node = graph
        .create_node("Control Flow:Sequence")
        .expect("Failed to create sequence1 node");
    println!("Created sequence1 node");

    // 创建第二个 Sequence 节点
    let seq2_node = graph
        .create_node("Control Flow:Sequence")
        .expect("Failed to create sequence2 node");
    println!("Created sequence2 node");

    // 创建两个 Branch 节点
    let branch1_node = graph
        .create_node("Control Flow:Branch")
        .expect("Failed to create branch1 node");
    println!("Created branch1 node");

    let branch2_node = graph
        .create_node("Control Flow:Branch")
        .expect("Failed to create branch2 node");
    println!("Created branch2 node");

    // 创建 Add 节点
    let add_node = graph
        .create_node("Math:Operators:Add (+)")
        .expect("Failed to create add node");
    println!("Created add node");

    // 创建 Equal 节点
    let equal_node = graph
        .create_node("Logic:Comparison:Equal (==)")
        .expect("Failed to create equal node");
    println!("Created equal node");

    // 创建 6 个 Print 节点
    let print_seq2_step0 = graph
        .create_node("Debug:Print")
        .expect("Failed to create print node");
    let print_seq2_step1 = graph
        .create_node("Debug:Print")
        .expect("Failed to create print node");
    let print_seq2_step2 = graph
        .create_node("Debug:Print")
        .expect("Failed to create print node");
    let print_branch1_true = graph
        .create_node("Debug:Print")
        .expect("Failed to create print node");
    let print_branch1_false = graph
        .create_node("Debug:Print")
        .expect("Failed to create print node");
    let print_branch2_true = graph
        .create_node("Debug:Print")
        .expect("Failed to create print node");
    let print_branch2_false = graph
        .create_node("Debug:Print")
        .expect("Failed to create print node");
    println!("Created 7 print nodes");

    println!("\n=== Setting Values ===");

    // 设置 Add 节点的输入值：10 + 10
    let add_pins = graph.get_pin_instances_by_node_id(add_node);
    let add_pin_a = add_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(0)))
        .expect("Add pin A not found");
    let add_pin_b = add_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(1)))
        .expect("Add pin B not found");

    graph
        .set_pin_user_value_by_pin_id(add_pin_a.id, DataValue::Int32(10))
        .expect("Failed to set add pin A");
    graph
        .set_pin_user_value_by_pin_id(add_pin_b.id, DataValue::Int32(10))
        .expect("Failed to set add pin B");
    println!("Set add node: 10 + 10");

    // 设置 Equal 节点的第二个输入值：20
    let equal_pins = graph.get_pin_instances_by_node_id(equal_node);
    let equal_pin_b = equal_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(1)))
        .expect("Equal pin B not found");

    graph
        .set_pin_user_value_by_pin_id(equal_pin_b.id, DataValue::Int32(20))
        .expect("Failed to set equal pin B");
    println!("Set equal node: ? == 20");

    // 设置 Branch1 的 condition 为 false
    let branch1_pins = graph.get_pin_instances_by_node_id(branch1_node);
    let branch1_condition = branch1_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Condition))
        .expect("Branch1 condition not found");

    graph
        .set_pin_user_value_by_pin_id(branch1_condition.id, DataValue::Boolean(false))
        .expect("Failed to set branch1 condition");
    println!("Set branch1 condition: false");

    // 设置 Print 节点的消息
    let print_messages = vec![
        (print_seq2_step0, "Sequence2-Step0"),
        (print_seq2_step1, "Sequence2-Step1"),
        (print_seq2_step2, "Sequence2-Step2"),
        (print_branch1_true, "Branch1-True"),
        (print_branch1_false, "Branch1-False"),
        (print_branch2_true, "Branch2-True"),
        (print_branch2_false, "Branch2-False"),
    ];

    for (print_node, message) in print_messages {
        let pins = graph.get_pin_instances_by_node_id(print_node);
        let message_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Inputs(0)))
            .expect("Print message pin not found");

        graph
            .set_pin_user_value_by_pin_id(message_pin.id, DataValue::String(message.to_string()))
            .expect("Failed to set print message");
    }
    println!("Set all print messages");

    println!("\n=== Connecting Data Pins ===");

    // 连接 Add.result -> Equal.A
    let add_result = add_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
        .expect("Add result not found");
    let equal_pin_a = equal_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Operands(0)))
        .expect("Equal pin A not found");

    graph
        .connect(add_result.id, equal_pin_a.id)
        .expect("Failed to connect add to equal");
    println!("Connected: Add.result -> Equal.A");

    // 连接 Equal.result -> Branch2.condition
    let equal_result = equal_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
        .expect("Equal result not found");
    let branch2_pins = graph.get_pin_instances_by_node_id(branch2_node);
    let branch2_condition = branch2_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Condition))
        .expect("Branch2 condition not found");

    graph
        .connect(equal_result.id, branch2_condition.id)
        .expect("Failed to connect equal to branch2");
    println!("Connected: Equal.result -> Branch2.condition");

    println!("\n=== Connecting Exec Pins ===");

    // 获取 Sequence1 的输出 pins
    let seq1_pins = graph.get_pin_instances_by_node_id(seq1_node);
    let seq1_step0 = seq1_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Exec(ExecRole::Steps(0)))
        .expect("Seq1 step0 not found");
    let seq1_step1 = seq1_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Exec(ExecRole::Steps(1)))
        .expect("Seq1 step1 not found");
    let seq1_step2 = seq1_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Exec(ExecRole::Steps(2)))
        .expect("Seq1 step2 not found");

    // 连接 Sequence1.Step0 -> Sequence2.In
    let seq2_pins = graph.get_pin_instances_by_node_id(seq2_node);
    let seq2_exec_in = seq2_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecIn))
        .expect("Seq2 exec in not found");

    graph
        .connect(seq1_step0.id, seq2_exec_in.id)
        .expect("Failed to connect seq1 to seq2");
    println!("Connected: Sequence1.Step0 -> Sequence2.In");

    // 连接 Sequence2 的输出到 Print 节点
    let seq2_step0 = seq2_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Exec(ExecRole::Steps(0)))
        .expect("Seq2 step0 not found");
    let seq2_step1 = seq2_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Exec(ExecRole::Steps(1)))
        .expect("Seq2 step1 not found");
    let seq2_step2 = seq2_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Exec(ExecRole::Steps(2)))
        .expect("Seq2 step2 not found");

    let print_seq2_step0_in = graph
        .get_pin_instances_by_node_id(print_seq2_step0)
        .iter()
        .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecIn))
        .expect("Print exec in not found")
        .id;
    let print_seq2_step1_in = graph
        .get_pin_instances_by_node_id(print_seq2_step1)
        .iter()
        .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecIn))
        .expect("Print exec in not found")
        .id;
    let print_seq2_step2_in = graph
        .get_pin_instances_by_node_id(print_seq2_step2)
        .iter()
        .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecIn))
        .expect("Print exec in not found")
        .id;

    graph
        .connect(seq2_step0.id, print_seq2_step0_in)
        .expect("Failed to connect");
    graph
        .connect(seq2_step1.id, print_seq2_step1_in)
        .expect("Failed to connect");
    graph
        .connect(seq2_step2.id, print_seq2_step2_in)
        .expect("Failed to connect");
    println!("Connected: Sequence2 outputs -> Print nodes");

    // 连接 Sequence1.Step1 -> Branch1.In
    let branch1_exec_in = branch1_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecIn))
        .expect("Branch1 exec in not found");

    graph
        .connect(seq1_step1.id, branch1_exec_in.id)
        .expect("Failed to connect seq1 to branch1");
    println!("Connected: Sequence1.Step1 -> Branch1.In");

    // 连接 Branch1 的输出到 Print 节点
    let branch1_true_out = branch1_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecTrue))
        .expect("Branch1 true out not found");
    let branch1_false_out = branch1_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecFalse))
        .expect("Branch1 false out not found");

    let print_branch1_true_in = graph
        .get_pin_instances_by_node_id(print_branch1_true)
        .iter()
        .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecIn))
        .expect("Print exec in not found")
        .id;
    let print_branch1_false_in = graph
        .get_pin_instances_by_node_id(print_branch1_false)
        .iter()
        .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecIn))
        .expect("Print exec in not found")
        .id;

    graph
        .connect(branch1_true_out.id, print_branch1_true_in)
        .expect("Failed to connect");
    graph
        .connect(branch1_false_out.id, print_branch1_false_in)
        .expect("Failed to connect");
    println!("Connected: Branch1 outputs -> Print nodes");

    // 连接 Sequence1.Step2 -> Branch2.In
    let branch2_exec_in = branch2_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecIn))
        .expect("Branch2 exec in not found");

    graph
        .connect(seq1_step2.id, branch2_exec_in.id)
        .expect("Failed to connect seq1 to branch2");
    println!("Connected: Sequence1.Step2 -> Branch2.In");

    // 连接 Branch2 的输出到 Print 节点
    let branch2_true_out = branch2_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecTrue))
        .expect("Branch2 true out not found");
    let branch2_false_out = branch2_pins
        .iter()
        .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecFalse))
        .expect("Branch2 false out not found");

    let print_branch2_true_in = graph
        .get_pin_instances_by_node_id(print_branch2_true)
        .iter()
        .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecIn))
        .expect("Print exec in not found")
        .id;
    let print_branch2_false_in = graph
        .get_pin_instances_by_node_id(print_branch2_false)
        .iter()
        .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecIn))
        .expect("Print exec in not found")
        .id;

    graph
        .connect(branch2_true_out.id, print_branch2_true_in)
        .expect("Failed to connect");
    graph
        .connect(branch2_false_out.id, print_branch2_false_in)
        .expect("Failed to connect");
    println!("Connected: Branch2 outputs -> Print nodes");

    println!("\n=== Executing Graph ===");

    // 使用 Executor 执行整个图
    use yssbi_lib::graph::core::GraphRuntime;
    let runtime = Arc::new(std::sync::Mutex::new(GraphRuntime::new_standalone(
        graph.clone(),
    )));
    let mut executor = common::executor_for_test(runtime);
    let result = executor.start(seq1_node);

    assert!(result.is_ok(), "Executor failed: {:?}", result.err());

    println!("\n=== Execution Logs ===");
    for log in executor.logs() {
        println!("{}", log);
    }

    println!("\n=== Test Completed Successfully ===");
    println!("Expected execution order:");
    println!("1. Sequence2-Step0");
    println!("2. Sequence2-Step1");
    println!("3. Sequence2-Step2");
    println!("4. Branch1-False (condition=false)");
    println!("5. Branch2-True (10+10==20 is true)");
}

#[test]
fn test_nested_sequence_tree() {
    // 测试嵌套的 sequence 树形结构
    //
    // 图结构：
    // root_sequence (3 个输出)
    //   ├─ Step 0 -> seq_1_0 (3 个输出)
    //   │            ├─ Step 0 -> seq_2_0 (3 个输出)
    //   │            │            ├─ Step 0 -> print("Position: 1-0, 2-0, Step-0")
    //   │            │            ├─ Step 1 -> print("Position: 1-0, 2-0, Step-1")
    //   │            │            └─ Step 2 -> print("Position: 1-0, 2-0, Step-2")
    //   │            ├─ Step 1 -> seq_2_1 (3 个输出)
    //   │            │            ├─ Step 0 -> print("Position: 1-0, 2-1, Step-0")
    //   │            │            ├─ Step 1 -> print("Position: 1-0, 2-1, Step-1")
    //   │            │            └─ Step 2 -> print("Position: 1-0, 2-1, Step-2")
    //   │            └─ Step 2 -> seq_2_2 (3 个输出)
    //   │                         ├─ Step 0 -> print("Position: 1-0, 2-2, Step-0")
    //   │                         ├─ Step 1 -> print("Position: 1-0, 2-2, Step-1")
    //   │                         └─ Step 2 -> print("Position: 1-0, 2-2, Step-2")
    //   ├─ Step 1 -> seq_1_1 (3 个输出)
    //   │            ├─ Step 0 -> seq_2_3 (3 个输出)
    //   │            │            ├─ Step 0 -> print("Position: 1-1, 2-3, Step-0")
    //   │            │            ├─ Step 1 -> print("Position: 1-1, 2-3, Step-1")
    //   │            │            └─ Step 2 -> print("Position: 1-1, 2-3, Step-2")
    //   │            ├─ Step 1 -> seq_2_4 (3 个输出)
    //   │            │            ├─ Step 0 -> print("Position: 1-1, 2-4, Step-0")
    //   │            │            ├─ Step 1 -> print("Position: 1-1, 2-4, Step-1")
    //   │            │            └─ Step 2 -> print("Position: 1-1, 2-4, Step-2")
    //   │            └─ Step 2 -> seq_2_5 (3 个输出)
    //   │                         ├─ Step 0 -> print("Position: 1-1, 2-5, Step-0")
    //   │                         ├─ Step 1 -> print("Position: 1-1, 2-5, Step-1")
    //   │                         └─ Step 2 -> print("Position: 1-1, 2-5, Step-2")
    //   └─ Step 2 -> seq_1_2 (3 个输出)
    //                ├─ Step 0 -> seq_2_6 (3 个输出)
    //                │            ├─ Step 0 -> print("Position: 1-2, 2-6, Step-0")
    //                │            ├─ Step 1 -> print("Position: 1-2, 2-6, Step-1")
    //                │            └─ Step 2 -> print("Position: 1-2, 2-6, Step-2")
    //                ├─ Step 1 -> seq_2_7 (3 个输出)
    //                │            ├─ Step 0 -> print("Position: 1-2, 2-7, Step-0")
    //                │            ├─ Step 1 -> print("Position: 1-2, 2-7, Step-1")
    //                │            └─ Step 2 -> print("Position: 1-2, 2-7, Step-2")
    //                └─ Step 2 -> seq_2_8 (3 个输出)
    //                             ├─ Step 0 -> print("Position: 1-2, 2-8, Step-0")
    //                             ├─ Step 1 -> print("Position: 1-2, 2-8, Step-1")
    //                             └─ Step 2 -> print("Position: 1-2, 2-8, Step-2")

    let registry = create_test_registry();
    let graph = Arc::new(GraphInstance::new(
        "Nested Sequence Tree Test",
        yssbi_lib::graph::GraphKind::Event,
        registry.clone(),
    ));

    println!("\n=== Creating Root Sequence Node ===");

    // 创建根 sequence 节点
    let root_seq = graph
        .create_node("Control Flow:Sequence")
        .expect("Failed to create root sequence node");
    println!("Created root sequence node");

    println!("\n=== Creating Level 1 Sequence Nodes (3 nodes) ===");

    // 创建第一层的 3 个 sequence 节点
    let mut level1_nodes = Vec::new();
    for i in 0..3 {
        let node = graph
            .create_node("Control Flow:Sequence")
            .expect(&format!("Failed to create level1 sequence node {}", i));
        level1_nodes.push(node);
        println!("Created level1 sequence node {}", i);
    }

    println!("\n=== Creating Level 2 Sequence Nodes (9 nodes) ===");

    // 创建第二层的 9 个 sequence 节点
    let mut level2_nodes = Vec::new();
    for i in 0..9 {
        let node = graph
            .create_node("Control Flow:Sequence")
            .expect(&format!("Failed to create level2 sequence node {}", i));
        level2_nodes.push(node);
        println!("Created level2 sequence node {}", i);
    }

    println!("\n=== Creating Print Nodes (27 nodes) ===");

    // 创建 27 个 print 节点（每个 level2 sequence 有 3 个输出）
    let mut print_nodes = Vec::new();
    for i in 0..27 {
        let node = graph
            .create_node("Debug:Print")
            .expect(&format!("Failed to create print node {}", i));
        print_nodes.push(node);
    }
    println!("Created 27 print nodes");

    println!("\n=== Setting Print Messages ===");

    // 设置 print 节点的消息
    for (idx, print_node) in print_nodes.iter().enumerate() {
        let level1_idx = idx / 9; // 0, 1, 2
        let level2_idx = idx / 3; // 0-8
        let step_idx = idx % 3; // 0, 1, 2

        let message = format!(
            "Position: 1-{}, 2-{}, Step-{}",
            level1_idx, level2_idx, step_idx
        );

        let pins = graph.get_pin_instances_by_node_id(*print_node);
        let message_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Inputs(0)))
            .expect("Print message pin not found");

        graph
            .set_pin_user_value_by_pin_id(message_pin.id, DataValue::String(message))
            .expect("Failed to set print message");
    }
    println!("Set all 27 print messages");

    println!("\n=== Connecting Root Sequence to Level 1 Sequences ===");

    // 连接 root sequence 到 level1 sequences
    let root_pins = graph.get_pin_instances_by_node_id(root_seq);
    for i in 0..3 {
        let root_step = root_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Exec(ExecRole::Steps(i)))
            .expect(&format!("Root step {} not found", i));

        let level1_pins = graph.get_pin_instances_by_node_id(level1_nodes[i]);
        let level1_exec_in = level1_pins
            .iter()
            .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecIn))
            .expect(&format!("Level1 node {} exec in not found", i));

        graph
            .connect(root_step.id, level1_exec_in.id)
            .expect(&format!("Failed to connect root to level1 node {}", i));
        println!("Connected: Root.Step{} -> Level1[{}].In", i, i);
    }

    println!("\n=== Connecting Level 1 Sequences to Level 2 Sequences ===");

    // 连接 level1 sequences 到 level2 sequences
    for i in 0..3 {
        let level1_pins = graph.get_pin_instances_by_node_id(level1_nodes[i]);

        for j in 0..3 {
            let level1_step = level1_pins
                .iter()
                .find(|p| p.definition.role == PinRole::Exec(ExecRole::Steps(j)))
                .expect(&format!("Level1[{}] step {} not found", i, j));

            let level2_idx = i * 3 + j;
            let level2_pins = graph.get_pin_instances_by_node_id(level2_nodes[level2_idx]);
            let level2_exec_in = level2_pins
                .iter()
                .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecIn))
                .expect(&format!("Level2 node {} exec in not found", level2_idx));

            graph
                .connect(level1_step.id, level2_exec_in.id)
                .expect(&format!(
                    "Failed to connect level1[{}] to level2[{}]",
                    i, level2_idx
                ));
            println!(
                "Connected: Level1[{}].Step{} -> Level2[{}].In",
                i, j, level2_idx
            );
        }
    }

    println!("\n=== Connecting Level 2 Sequences to Print Nodes ===");

    // 连接 level2 sequences 到 print 节点
    for i in 0..9 {
        let level2_pins = graph.get_pin_instances_by_node_id(level2_nodes[i]);

        for j in 0..3 {
            let level2_step = level2_pins
                .iter()
                .find(|p| p.definition.role == PinRole::Exec(ExecRole::Steps(j)))
                .expect(&format!("Level2[{}] step {} not found", i, j));

            let print_idx = i * 3 + j;
            let print_pins = graph.get_pin_instances_by_node_id(print_nodes[print_idx]);
            let print_exec_in = print_pins
                .iter()
                .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecIn))
                .expect(&format!("Print node {} exec in not found", print_idx));

            graph
                .connect(level2_step.id, print_exec_in.id)
                .expect(&format!(
                    "Failed to connect level2[{}] to print[{}]",
                    i, print_idx
                ));
            println!("Connected: Level2[{}].Step{} -> Print[{}]", i, j, print_idx);
        }
    }

    println!("\n=== Executing Graph ===");

    // 使用 Executor 执行整个图
    use yssbi_lib::graph::core::GraphRuntime;
    let runtime = Arc::new(std::sync::Mutex::new(GraphRuntime::new_standalone(
        graph.clone(),
    )));
    let mut executor = common::executor_for_test(runtime);
    let result = executor.start(root_seq);

    assert!(result.is_ok(), "Executor failed: {:?}", result.err());

    println!("\n=== Execution Logs ===");
    for log in executor.logs() {
        println!("{}", log);
    }

    println!("\n=== Test Completed Successfully ===");
    println!("Total nodes executed: 1 root + 3 level1 + 9 level2 + 27 prints = 40 nodes");
    println!("Expected 27 print outputs showing position information");
}

fn operand_pin_names(graph: &GraphInstance, node_id: yssbi_lib::graph::NodeId) -> Vec<String> {
    graph
        .get_pin_instances_by_pin_role_family(node_id, &PinRole::Data(DataRole::Operands(0)))
        .into_iter()
        .map(|p| p.definition.name.clone())
        .collect()
}

#[test]
fn test_repeatable_pin_reindex_after_remove() {
    let registry = create_test_registry();
    let graph = GraphInstance::new(
        "Repeatable Pin Naming",
        yssbi_lib::graph::GraphKind::Event,
        registry,
    );

    let add_node_id = graph
        .create_node("Math:Operators:Add (+)")
        .expect("create add node");
    assert_eq!(operand_pin_names(&graph, add_node_id), vec!["A", "B"]);

    graph
        .add_repeatable_pin(add_node_id, 0)
        .expect("add third operand");
    assert_eq!(
        operand_pin_names(&graph, add_node_id),
        vec!["A", "B", "C"]
    );

    let pin_b = graph
        .get_pin_instances_by_node_id(add_node_id)
        .into_iter()
        .find(|p| p.definition.name == "B")
        .expect("pin B exists")
        .id;

    graph
        .remove_repeatable_pin(add_node_id, pin_b)
        .expect("remove pin B");
    assert_eq!(
        operand_pin_names(&graph, add_node_id),
        vec!["A", "B"],
        "after removing B, former C should be reindexed to B"
    );

    graph
        .add_repeatable_pin(add_node_id, 0)
        .expect("add pin after reindex");
    assert_eq!(
        operand_pin_names(&graph, add_node_id),
        vec!["A", "B", "C"],
        "new pin should take the next letter, not duplicate C"
    );
}

#[test]
fn test_ts_align_schema_propagates_to_decompose() {
    use yssbi_lib::graph::node::{ColumnSchema, DataSchema, NodeInstanceParams};
    use yssbi_lib::graph::value::DataType;
    use yssbi_lib::graph::GraphKind;

    let registry = create_test_registry();
    let mut graph = GraphInstance::new("Schema Chain Test", GraphKind::Event, registry.clone());
    graph.set_schema_provider(Arc::new(|dataframe_id: &str| {
        if dataframe_id == "test_df" {
            Some(DataSchema {
                columns: vec![
                    ColumnSchema {
                        name: "time".to_string(),
                        data_type: DataType::Int64,
                    },
                    ColumnSchema {
                        name: "value".to_string(),
                        data_type: DataType::Float64,
                    },
                ],
            })
        } else {
            None
        }
    }));
    let graph = Arc::new(graph);

    let get_node = graph
        .create_node_with_position(
            "Data:Get DataFrame",
            0.0,
            0.0,
            Some(NodeInstanceParams::DataFrame {
                dataframe_id: "test_df".to_string(),
                dataframe_name: None,
            }),
        )
        .expect("create get dataframe");
    let align_node = graph
        .create_node("Data:Time Series:TS Align")
        .expect("create ts align");
    let decompose_node = graph
        .create_node("Data:Decompose DataFrame")
        .expect("create decompose");

    let get_out = graph
        .get_pin_instances_by_node_id(get_node)
        .into_iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Output))
        .expect("get output")
        .id;
    let align_in = graph
        .get_pin_instances_by_node_id(align_node)
        .into_iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Input))
        .expect("align input")
        .id;
    let align_out = graph
        .get_pin_instances_by_node_id(align_node)
        .into_iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Output))
        .expect("align output")
        .id;
    let decompose_in = graph
        .get_pin_instances_by_node_id(decompose_node)
        .into_iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Input))
        .expect("decompose input")
        .id;

    graph.connect(get_out, align_in).expect("connect get->align");
    graph
        .connect(align_out, decompose_in)
        .expect("connect align->decompose");

    let align_output = graph
        .get_pin_instances_by_node_id(align_node)
        .into_iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Output))
        .expect("align output pin");
    assert_eq!(
        align_output
            .resolved_schema
            .as_ref()
            .map(|schema| schema.columns.len()),
        Some(2),
        "TS Align output should inherit input schema"
    );

    let (_, _) = graph.materialize_dynamic_pins();

    let decompose_pins_before_reopen = graph
        .get_pin_instances_by_node_id(decompose_node)
        .into_iter()
        .filter(|p| p.is_output())
        .map(|p| p.definition.name.clone())
        .collect::<Vec<_>>();

    assert_eq!(decompose_pins_before_reopen, vec!["time", "value"]);

    // 模拟 DB schema 未就绪：清空 resolved_schema 后再 materialize，应保留磁盘 pin
    {
        let mut data_state = graph.data_state.write().unwrap();
        for pin in data_state.pins.values_mut() {
            pin.resolved_schema = None;
        }
    }
    let (change_sets, _) = graph.materialize_dynamic_pins();
    assert!(
        change_sets.is_empty(),
        "materialize should not strip persisted pins when schema is unavailable"
    );

    let mut output_names: Vec<String> = graph
        .get_pin_instances_by_node_id(decompose_node)
        .into_iter()
        .filter(|p| p.is_output())
        .map(|p| p.definition.name.clone())
        .collect();
    output_names.sort();
    assert_eq!(output_names, vec!["time", "value"]);
}

#[test]
fn test_ols_model_schema_propagates_to_predict_inputs() {
    use yssbi_lib::graph::node::{ColumnSchema, DataSchema, NodeInstanceParams};
    use yssbi_lib::graph::value::DataType;
    use yssbi_lib::graph::GraphKind;

    let registry = create_test_registry();
    let mut graph = GraphInstance::new("OLS Predict Schema", GraphKind::Event, registry.clone());
    graph.set_schema_provider(Arc::new(|dataframe_id: &str| {
        if dataframe_id == "reg_df" {
            Some(DataSchema {
                columns: vec![
                    ColumnSchema {
                        name: "y".to_string(),
                        data_type: DataType::Float64,
                    },
                    ColumnSchema {
                        name: "x1".to_string(),
                        data_type: DataType::Float64,
                    },
                ],
            })
        } else {
            None
        }
    }));
    let graph = Arc::new(graph);

    let get_node = graph
        .create_node_with_position(
            "Data:Get DataFrame",
            0.0,
            0.0,
            Some(NodeInstanceParams::DataFrame {
                dataframe_id: "reg_df".to_string(),
                dataframe_name: None,
            }),
        )
        .expect("get dataframe");
    let decompose_node = graph
        .create_node("Data:Decompose DataFrame")
        .expect("decompose");
    let ols_node = graph
        .create_node("Data:Statistics:OLS")
        .expect("ols");
    let predict_node = graph
        .create_node("Data:Statistics:Predict")
        .expect("predict");

    let get_out = graph
        .get_pin_instances_by_node_id(get_node)
        .into_iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Output))
        .expect("get out")
        .id;
    let decompose_in = graph
        .get_pin_instances_by_node_id(decompose_node)
        .into_iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Input))
        .expect("decompose in")
        .id;

    graph.connect(get_out, decompose_in).expect("get->decompose");

    let decompose_pins: std::collections::HashMap<String, yssbi_lib::graph::PinId> = graph
        .get_pin_instances_by_node_id(decompose_node)
        .into_iter()
        .filter(|p| p.is_output())
        .map(|p| (p.definition.name.clone(), p.id))
        .collect();

    let ols_y = graph
        .get_pin_instances_by_node_id(ols_node)
        .into_iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Custom("y".to_string())))
        .expect("ols y")
        .id;
    let ols_x = graph
        .get_pin_instances_by_node_id(ols_node)
        .into_iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Inputs(0)))
        .expect("ols x")
        .id;
    let ols_model_out = graph
        .get_pin_instances_by_node_id(ols_node)
        .into_iter()
        .find(|p| p.definition.role == PinRole::Data(DataRole::Custom("ols_model".to_string())))
        .expect("ols model out")
        .id;
    let predict_model_in = graph
        .get_pin_instances_by_node_id(predict_node)
        .into_iter()
        .find(|p| {
            p.definition.role
                == PinRole::Data(DataRole::Custom("prediction_model".to_string()))
        })
        .expect("predict model in")
        .id;

    graph
        .connect(decompose_pins["y"], ols_y)
        .expect("y -> ols");
    graph
        .connect(decompose_pins["x1"], ols_x)
        .expect("x1 -> ols");

    graph.propagate_schemas();

    let ols_model_pin = graph
        .get_pin_instances_by_node_id(ols_node)
        .into_iter()
        .find(|p| p.id == ols_model_out)
        .expect("ols model pin");
    assert_eq!(
        ols_model_pin
            .resolved_schema
            .as_ref()
            .map(|s| s.columns.iter().map(|c| c.name.clone()).collect::<Vec<_>>()),
        Some(vec!["x1".to_string()]),
        "OLS Model output should expose exog schema"
    );

    graph
        .connect(ols_model_out, predict_model_in)
        .expect("ols model -> predict");

    let predict_exog_names: Vec<String> = graph
        .get_pin_instances_by_node_id(predict_node)
        .into_iter()
        .filter(|p| {
            p.is_input()
                && matches!(
                    &p.definition.role,
                    PinRole::Data(DataRole::Custom(name)) if name.starts_with("pred_exog:")
                )
        })
        .map(|p| p.definition.name.clone())
        .collect();

    assert_eq!(predict_exog_names, vec!["x1"]);
}

/// 回归测试：Sequence 的 Then1 整条子树必须先于 Then2 执行
///
/// 结构：
///   Seq.Then1 -> A -> A2
///   Seq.Then2 -> B -> B2
///
/// 正确顺序应为 [Seq, A, A2, B, B2]；修复前因子帧错挂导致 Then2 在 Then1
/// 子树排空前插入，出现 [Seq, A, B, A2, B2] 之类的交错。
#[test]
fn test_sequence_runs_branch_fully_before_next() {
    use yssbi_lib::graph::core::GraphRuntime;

    let registry = create_test_registry();
    let graph = Arc::new(GraphInstance::new(
        "Sequence Order Test",
        yssbi_lib::graph::GraphKind::Event,
        registry.clone(),
    ));

    let seq = graph
        .create_node("Control Flow:Sequence")
        .expect("create sequence");
    let a = graph.create_node("Debug:Print").expect("create A");
    let a2 = graph.create_node("Debug:Print").expect("create A2");
    let b = graph.create_node("Debug:Print").expect("create B");
    let b2 = graph.create_node("Debug:Print").expect("create B2");

    // Print 的 Message 为可选输入，逐一设值避免执行期取值失败
    for (node, msg) in [(a, "A"), (a2, "A2"), (b, "B"), (b2, "B2")] {
        let pins = graph.get_pin_instances_by_node_id(node);
        let msg_pin = pins
            .iter()
            .find(|p| p.definition.role == PinRole::Data(DataRole::Inputs(0)))
            .expect("print message pin");
        graph
            .set_pin_user_value_by_pin_id(msg_pin.id, DataValue::String(msg.to_string()))
            .expect("set print message");
    }

    let exec_out = |node, role: ExecRole| {
        graph
            .get_pin_instances_by_node_id(node)
            .into_iter()
            .find(|p| p.definition.role == PinRole::Exec(role.clone()))
            .unwrap_or_else(|| panic!("exec output {:?} not found", role))
            .id
    };
    let exec_in = |node| {
        graph
            .get_pin_instances_by_node_id(node)
            .into_iter()
            .find(|p| p.definition.role == PinRole::Exec(ExecRole::ExecIn))
            .expect("exec input not found")
            .id
    };

    // Then1 -> A -> A2
    graph
        .connect(exec_out(seq, ExecRole::Steps(0)), exec_in(a))
        .expect("Then1 -> A");
    graph
        .connect(exec_out(a, ExecRole::ExecOut), exec_in(a2))
        .expect("A -> A2");
    // Then2 -> B -> B2
    graph
        .connect(exec_out(seq, ExecRole::Steps(1)), exec_in(b))
        .expect("Then2 -> B");
    graph
        .connect(exec_out(b, ExecRole::ExecOut), exec_in(b2))
        .expect("B -> B2");

    let runtime = Arc::new(std::sync::Mutex::new(GraphRuntime::new_standalone(
        graph.clone(),
    )));
    let (mut executor, recorder) = common::recording_executor_for_test(runtime);
    executor.start(seq).expect("execute sequence");

    let expected = vec![
        seq.to_string(),
        a.to_string(),
        a2.to_string(),
        b.to_string(),
        b2.to_string(),
    ];
    assert_eq!(
        recorder.order(),
        expected,
        "Then1 整条子树 (A, A2) 必须先于 Then2 (B, B2) 执行"
    );
}
