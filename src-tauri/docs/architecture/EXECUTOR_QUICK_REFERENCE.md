# Executor 快速参考

## 执行模型速查表

| 模型 | ExecPin | DataPin | 特征 | 缓存 | 示例 |
|-----|---------|---------|------|------|------|
| **Event** | 只有输出 | 可选 | 执行起点 | ❌ | OnRun, OnClick |
| **ControlFlow** | 有输入 | 无 | 控制顺序 | ❌ | Sequence, Delay |
| **DataFlow** | 无 | 有 | 纯计算 | ✅ | Constant, Add, Math |
| **Hybrid** | 有 | 有 | 混合逻辑 | ❌ | IfElse, Print, SetVariable |

## 节点创建模板

### DataFlow 节点（推荐用于纯计算）

```rust
let node = GenericNode::new_prototype("node_type", "Display Name");
node.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "Input", "type"));
node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Output", "type"));

node.set_data_processor(Box::new(|ctx, node, _pin_id| {
    let input = ctx.get_pin_value(&node.inputs[0].id);
    // 纯函数计算
    json!(result)
}));
```

### ControlFlow 节点

```rust
let node = GenericNode::new_prototype("node_type", "Display Name");
node.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
node.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Out"));

node.set_flow_processor(Box::new(|ctx, node| {
    // 控制流逻辑
    ctx.trigger_flow_by_pin(&node.id, "Out")?;
    Ok("".into())
}));
```

### Hybrid 节点

```rust
let node = GenericNode::new_prototype("node_type", "Display Name");
node.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
node.add_in_data_pin(GenericInDataPin::new(uuid::Uuid::nil(), "Data", "type"));
node.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Out"));

node.set_flow_processor(Box::new(|ctx, node| {
    let data = ctx.get_pin_value(&node.inputs[0].id);
    // 使用数据 + 控制流
    ctx.trigger_flow_by_pin(&node.id, "Out")?;
    Ok("".into())
}));
```

## 常用 API

### ExecutionContext

```rust
// 获取数据（自动缓存）
let value = ctx.get_pin_value(pin_id);

// 触发控制流
ctx.trigger_flow_by_pin(node_id, pin_name)?;

// 变量操作
ctx.get_variable(var_id);
ctx.set_variable(var_id, value);

// 日志
ctx.log(message);
```

### GenericNode

```rust
// 查询执行模型
let model = node.execution_model();
let cacheable = model.is_cacheable();

// 添加 Pin
node.add_in_data_pin(pin);
node.add_output(pin);
node.add_in_exec_pin(pin);
node.add_out_exec_pin(pin);

// 设置处理器
node.set_data_processor(processor);
node.set_flow_processor(processor);
```

## 数据类型

### Pin 类型

```rust
"number"   // f64
"string"   // String
"bool"     // bool
"any"      // 任意类型
"exec"     // 执行流
```

### Value 操作

```rust
// 获取值
value.as_f64().unwrap_or(0.0)
value.as_str().unwrap_or("")
value.as_bool().unwrap_or(false)

// 创建值
json!(42)
json!("hello")
json!(true)
json!([1, 2, 3])
json!({"key": "value"})
```

## 性能优化清单

- ✅ 使用 DataFlow 节点进行纯计算
- ✅ 避免在循环中重复计算常量
- ✅ 将计算密集型逻辑放在 DataFlow 节点
- ✅ 使用 Sequence 确保必要的执行顺序
- ❌ 不要在 DataFlow 节点中产生副作用
- ❌ 不要过度使用 Hybrid 节点

## 调试命令

```rust
// 查看节点模型
println!("Model: {:?}", node.execution_model());

// 查看缓存状态（在 context.rs 中取消注释）
// [DataFlow Cache Hit] Pin ...
// [DataFlow Cache Store] Pin ...

// 查看执行路径
// >>> Executing Node: ...
```

## 常见错误

### 错误 1：数据节点有副作用

```rust
// ❌ 错误：DataFlow 节点修改全局状态
let bad = GenericNode::new_prototype("bad", "Bad");
bad.add_output(pin);
bad.set_data_processor(Box::new(|ctx, _, _| {
    ctx.set_variable("x", json!(1));  // 副作用！
    json!(1)
}));

// ✅ 正确：使用 Hybrid 节点
let good = GenericNode::new_prototype("good", "Good");
good.add_in_exec_pin(exec_pin);
good.add_out_exec_pin(exec_pin);
good.set_flow_processor(Box::new(|ctx, _| {
    ctx.set_variable("x", json!(1));  // 在 flow_processor 中
    Ok("".into())
}));
```

### 错误 2：忘记触发下一个流程

```rust
// ❌ 错误：控制流中断
node.set_flow_processor(Box::new(|ctx, node| {
    // 做了一些事情
    Ok("".into())  // 没有触发下一个节点！
}));

// ✅ 正确：触发下一个节点
node.set_flow_processor(Box::new(|ctx, node| {
    // 做了一些事情
    ctx.trigger_flow_by_pin(&node.id, "Out")?;
    Ok("".into())
}));
```

### 错误 3：循环中的缓存问题

```rust
// ⚠️ 注意：缓存在整个执行周期有效
ForLoop {
    for i in 0..10 {
        let x = ctx.get_pin_value("constant");  // 第一次计算，后续使用缓存
        // 如果 constant 需要根据循环变量变化，这会有问题
    }
}

// ✅ 解决：使用变量或动态计算
ForLoop {
    for i in 0..10 {
        ctx.set_variable("loop_index", json!(i));
        let x = ctx.get_pin_value("dynamic_value");  // 每次重新计算
    }
}
```

## 架构图（简化版）

```
┌─────────────────────────────────────┐
│      ExecutionContext               │
├─────────────────────────────────────┤
│                                     │
│  ExecFlow (Push)  DataFlow (Pull)  │
│       ↓                ↑            │
│   run_flow      get_pin_value       │
│       ↓                ↑            │
│  execution_     data_cache          │
│  stack          (优化)              │
│                                     │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│      ConnectionManager              │
│  - ExecPin connections              │
│  - DataPin connections              │
│  - Cycle detection                  │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│      GenericNode                    │
│  - execution_model()                │
│  - flow_processor                   │
│  - data_processor                   │
└─────────────────────────────────────┘
```

## 下一步

1. 阅读 `EXECUTOR_DESIGN.md` 了解详细设计
2. 查看 `EXECUTOR_EXAMPLES.md` 学习使用示例
3. 参考 `src-tauri/src/executor/node/catalog/` 中的节点实现
4. 开始创建你自己的节点！

---

**记住**：DataFlow 不是独立流动的，而是在 ExecFlow 驱动下按需拉取的！
