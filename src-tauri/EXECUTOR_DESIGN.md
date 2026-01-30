# Executor 设计文档

## 概述

本执行器采用 **ExecFlow（控制流）+ DataFlow（数据流）** 的混合架构，清晰分离控制逻辑和数据计算。

## 核心概念

### 1. 执行模型（ExecutionModel）

每个节点都有明确的执行模型，定义其在执行过程中的角色：

```rust
pub enum ExecutionModel {
    Event,       // 事件节点：执行起点
    ControlFlow, // 控制流节点：决定执行顺序
    DataFlow,    // 数据流节点：纯数据计算
    Hybrid,      // 混合节点：同时参与控制流和数据流
}
```

#### 节点分类示例

| 执行模型 | 特征 | 示例节点 |
|---------|------|---------|
| **Event** | 只有输出 ExecPin | OnRun, OnClick, OnTimer |
| **ControlFlow** | 有 ExecPin，无 DataPin | Sequence, Delay |
| **DataFlow** | 只有 DataPin | Constant, Add, Multiply, GetVariable |
| **Hybrid** | 同时有 ExecPin 和 DataPin | IfElse, Print, SetVariable, ForLoop |

### 2. Pin 类型

#### ExecPin（执行针脚）
- **作用**：控制节点的执行顺序
- **流动方式**：主动推送（Push-based）
- **连接规则**：ExecPin → ExecPin

```
[OnRun] ──exec──> [Print] ──exec──> [SetVariable]
   ↓ 控制流决定执行顺序
```

#### DataPin（数据针脚）
- **作用**：传递数据
- **流动方式**：按需拉取（Pull-based）
- **连接规则**：DataPin → DataPin

```
[Constant: 42] ──data──> [Add] ──data──> [Print]
                            ↑ 当 Add 需要数据时，回溯到 Constant
```

## 执行流程

### 控制流执行（ExecFlow）

```rust
// 1. 从事件节点开始
execute() {
    find_event_node("event_on_run")
    run_flow_internal(event_node)
}

// 2. 递归执行控制流
run_flow_internal(node) {
    // 执行节点逻辑
    let next_pin = node.process_flow(ctx)
    
    // 触发下一个节点
    if !next_pin.is_empty() {
        trigger_next_flow(node, next_pin)
    }
}
```

**执行示例**：
```
[OnRun] → [Sequence] → [Print "A"] → [Print "B"]
  ↓         ↓            ↓             ↓
 执行      执行         执行          执行
```

### 数据流执行（DataFlow）

```rust
// 按需拉取 + 缓存优化
get_pin_value(input_pin) {
    // 1. 检查缓存
    if let Some(cached) = data_cache.get(output_pin) {
        return cached
    }
    
    // 2. 回溯到上游节点
    let upstream_node = find_upstream(input_pin)
    
    // 3. 执行数据计算
    let value = upstream_node.process_data(ctx)
    
    // 4. 如果是纯数据节点，缓存结果
    if upstream_node.execution_model().is_cacheable() {
        data_cache.insert(output_pin, value)
    }
    
    return value
}
```

**执行示例**：
```
[Constant: 10] ──┬──> [Add] ──> [Print]
                 └──> [Multiply] ──> [Print]

执行流程：
1. Print 需要 Add 的结果
2. Add 回溯到 Constant，获取 10（计算并缓存）
3. Print 需要 Multiply 的结果
4. Multiply 回溯到 Constant，直接使用缓存的 10（避免重复计算）
```

## 关键优化

### 1. 数据缓存（Data Cache）

**目的**：避免重复计算纯数据节点

**实现**：
```rust
pub struct ExecutionContext {
    // 数据流缓存（在单次执行周期内有效）
    data_cache: HashMap<PinId, Value>,
}

// 执行开始时清空
execute() {
    data_cache.clear()
    // ... 执行逻辑
    data_cache.clear()
}
```

**缓存策略**：
- ✅ **缓存**：ExecutionModel::DataFlow 节点（纯函数式）
- ❌ **不缓存**：ExecutionModel::Hybrid 节点（可能有副作用）
- ❌ **不缓存**：ExecutionModel::ControlFlow 节点（不产生数据）

### 2. 循环检测

**ExecFlow 循环检测**：
```rust
execution_stack: Vec<NodeId>  // 当前执行路径

run_flow_internal(node) {
    if execution_stack.contains(node) {
        return Err("Cycle detected")
    }
    execution_stack.push(node)
    // ... 执行
    execution_stack.pop()
}
```

**DataFlow 循环检测**：
- 在 ConnectionManager 中，建立连接时检测
- 使用 DFS 算法检测是否会形成环

## 节点实现指南

### 纯控制流节点（ControlFlow）

```rust
// 示例：Sequence 节点
let seq = GenericNode::new_prototype("sequence", "Sequence");
seq.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
seq.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Then 0"));
seq.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Then 1"));

seq.set_flow_processor(Box::new(|ctx, node| {
    // 顺序执行所有输出
    ctx.trigger_flow_by_pin(&node.id, "Then 0")?;
    ctx.trigger_flow_by_pin(&node.id, "Then 1")?;
    Ok("".into())
}));
```

### 纯数据流节点（DataFlow）

```rust
// 示例：Add 节点
let add = GenericNode::new_prototype("add", "Add");
add.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "A", "number"));
add.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "B", "number"));
add.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Result", "number"));

add.set_data_processor(Box::new(|ctx, node, _pin_id| {
    let a = ctx.get_pin_value(&node.inputs[0].id).as_f64().unwrap_or(0.0);
    let b = ctx.get_pin_value(&node.inputs[1].id).as_f64().unwrap_or(0.0);
    json!(a + b)
}));
```

### 混合节点（Hybrid）

```rust
// 示例：IfElse 节点
let if_else = GenericNode::new_prototype("if_else", "If Else");
if_else.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
if_else.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Condition", "bool"));
if_else.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "True"));
if_else.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "False"));

if_else.set_flow_processor(Box::new(|ctx, node| {
    // 读取数据
    let condition = ctx.get_pin_value(&node.inputs[0].id).as_bool().unwrap_or(false);
    
    // 根据数据决定控制流
    if condition {
        ctx.trigger_flow_by_pin(&node.id, "True")?;
    } else {
        ctx.trigger_flow_by_pin(&node.id, "False")?;
    }
    Ok("".into())
}));
```

## 最佳实践

### 1. 节点设计原则

- **单一职责**：每个节点只做一件事
- **明确模型**：清楚节点属于哪种执行模型
- **纯函数式**：DataFlow 节点应该是纯函数，无副作用
- **避免副作用**：Hybrid 节点的副作用应该在 flow_processor 中

### 2. 性能优化

- **缓存友好**：尽量使用 DataFlow 节点，可以被缓存
- **避免重复计算**：相同的数据节点在一次执行中只计算一次
- **延迟计算**：数据只在需要时才计算（Pull-based）

### 3. 调试技巧

```rust
// 查看节点执行模型
let model = node.execution_model();
println!("Node execution model: {:?}", model);

// 查看是否可缓存
if model.is_cacheable() {
    println!("This node's results will be cached");
}

// 查看缓存命中
// 在 get_pin_value 中取消注释日志
// [DataFlow Cache Hit] Pin ...
// [DataFlow Cache Store] Pin ...
```

## 架构图

```
┌─────────────────────────────────────────────────────────┐
│                   ExecutionContext                       │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌──────────────┐         ┌──────────────┐             │
│  │  ExecFlow    │         │  DataFlow    │             │
│  │  (Push)      │         │  (Pull)      │             │
│  └──────────────┘         └──────────────┘             │
│         │                         │                     │
│         ↓                         ↓                     │
│  run_flow_internal()      get_pin_value()              │
│         │                         │                     │
│         ↓                         ↓                     │
│  ┌──────────────┐         ┌──────────────┐             │
│  │ execution_   │         │ data_cache   │             │
│  │ stack        │         │              │             │
│  └──────────────┘         └──────────────┘             │
│                                                          │
└─────────────────────────────────────────────────────────┘
                          │
                          ↓
        ┌─────────────────────────────────┐
        │     ConnectionManager            │
        ├─────────────────────────────────┤
        │  - ExecPin connections           │
        │  - DataPin connections           │
        │  - Cycle detection               │
        └─────────────────────────────────┘
                          │
                          ↓
        ┌─────────────────────────────────┐
        │         GenericNode              │
        ├─────────────────────────────────┤
        │  - execution_model()             │
        │  - flow_processor                │
        │  - data_processor                │
        └─────────────────────────────────┘
```

## 总结

这个设计的核心优势：

1. **清晰的职责分离**：ExecFlow 管控制，DataFlow 管数据
2. **高效的执行**：数据缓存避免重复计算
3. **灵活的扩展**：通过 ExecutionModel 轻松识别节点类型
4. **安全的执行**：循环检测防止无限递归
5. **易于调试**：明确的执行模型和日志系统

这个架构既保持了可视化编程的直观性，又提供了高性能的执行效率。
