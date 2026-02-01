# Executor 设计总结

## 你的问题回顾

> "dataflow 不是依据 execflow 流动的吗，如果分开了那怎么判断流动呢？"

**答案**：你说得对！DataFlow **不是**独立流动的，而是在 ExecFlow 驱动下**按需拉取**的。

## 核心设计

### 1. 执行模型（不是分离，而是协同）

```
ExecFlow (控制流)          DataFlow (数据流)
     ↓                          ↑
  主动推送                    按需拉取
     ↓                          ↑
决定执行顺序                提供计算结果
     ↓                          ↑
     └──────── 协同工作 ────────┘
```

### 2. 实际执行流程

```rust
// ExecFlow 驱动整个执行
run_flow_internal(node) {
    // 当节点需要数据时...
    let value = ctx.get_pin_value(pin)
    
    // DataFlow 回溯计算
    // ↓ 这里才触发数据流动
    upstream_node.process_data()
}
```

## 改进内容

### 改进前（你的原始代码）✅

```rust
// 已经是正确的 Pull-based 模型
get_pin_value() {
    找到上游节点 → 执行计算 → 返回结果
}
```

**优点**：
- ✅ 架构正确
- ✅ 逻辑清晰
- ✅ 按需计算

**缺点**：
- ❌ 没有缓存，重复计算
- ❌ 没有明确的执行模型标记
- ❌ 难以优化性能

### 改进后 🚀

```rust
// 1. 添加执行模型枚举
enum ExecutionModel {
    Event, ControlFlow, DataFlow, Hybrid
}

// 2. 节点自动识别模型
node.execution_model() → DataFlow  // 可缓存

// 3. 添加数据缓存
get_pin_value() {
    if cached { return cached }  // 缓存命中
    计算 → 缓存 → 返回
}
```

**新增优势**：
- ✅ 自动缓存纯数据节点
- ✅ 明确的执行模型
- ✅ 更好的性能
- ✅ 更易调试

## 关键文件变更

### 1. `types.rs` - 新增执行模型

```rust
pub enum ExecutionModel {
    Event,       // 执行起点
    ControlFlow, // 控制顺序
    DataFlow,    // 纯计算（可缓存）
    Hybrid,      // 混合
}
```

### 2. `implementation.rs` - 自动识别模型

```rust
impl GenericNode {
    pub fn execution_model(&self) -> ExecutionModel {
        // 根据 Pin 类型自动判断
    }
}
```

### 3. `context.rs` - 添加缓存

```rust
pub struct ExecutionContext {
    data_cache: HashMap<PinId, Value>,  // 新增
}

impl ExecutionContextTrait {
    fn get_pin_value(&mut self, pin_id: &str) -> Value {
        // 1. 检查缓存
        // 2. 回溯计算
        // 3. 缓存结果（如果是 DataFlow 节点）
    }
}
```

## 性能对比

### 场景：一个 Constant 连接到 3 个节点

```
[Constant: 42] ──┬──> [Add]
                 ├──> [Multiply]
                 └──> [Divide]
```

| 指标 | 改进前 | 改进后 | 提升 |
|-----|-------|-------|------|
| Constant 计算次数 | 3 次 | 1 次 | 3x |
| 内存使用 | 低 | 略高 | +1 HashMap |
| 代码复杂度 | 简单 | 中等 | +100 行 |

## 使用指南

### 创建纯数据节点（推荐）

```rust
let add = GenericNode::new_prototype("add", "Add");
add.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "A", "number"));
add.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "B", "number"));
add.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Result", "number"));

add.set_data_processor(Box::new(|ctx, node, _| {
    let a = ctx.get_pin_value(&node.inputs[0].id).as_f64().unwrap_or(0.0);
    let b = ctx.get_pin_value(&node.inputs[1].id).as_f64().unwrap_or(0.0);
    json!(a + b)
}));

// 自动识别为 DataFlow，结果会被缓存 ✅
```

### 创建混合节点

```rust
let if_else = GenericNode::new_prototype("if_else", "If Else");
if_else.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
if_else.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Condition", "bool"));
if_else.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "True"));
if_else.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "False"));

if_else.set_flow_processor(Box::new(|ctx, node| {
    let condition = ctx.get_pin_value(&node.inputs[0].id).as_bool().unwrap_or(false);
    if condition {
        ctx.trigger_flow_by_pin(&node.id, "True")?;
    } else {
        ctx.trigger_flow_by_pin(&node.id, "False")?;
    }
    Ok("".into())
}));

// 自动识别为 Hybrid，不会缓存（可能有副作用）
```

## 调试技巧

### 查看节点执行模型

```rust
let model = node.execution_model();
println!("Model: {:?}, Cacheable: {}", model, model.is_cacheable());
```

### 启用缓存日志

在 `context.rs` 中取消注释：

```rust
// self.log(format!("[DataFlow Cache Hit] Pin {:?}", output_pin_id));
// self.log(format!("[DataFlow Cache Store] Pin {:?}", output_pin_id));
```

## 常见问题

### Q1: DataFlow 和 ExecFlow 是分离的吗？

**A**: 不是分离，而是**协同**：
- ExecFlow 决定**什么时候**执行
- DataFlow 提供**需要的数据**
- DataFlow 在 ExecFlow 驱动下按需计算

### Q2: 为什么要区分执行模型？

**A**: 为了优化：
- DataFlow 节点是纯函数，可以安全缓存
- Hybrid 节点可能有副作用，不能缓存
- 明确模型有助于调试和性能分析

### Q3: 缓存会导致数据不一致吗？

**A**: 不会：
- 缓存只在**单次执行周期**内有效
- 每次 `execute()` 开始和结束都会清空缓存
- 只缓存纯数据节点（DataFlow）

### Q4: 如何处理有副作用的节点？

**A**: 使用 Hybrid 模型：
- 副作用在 `flow_processor` 中处理
- 不会被缓存
- 每次 ExecFlow 到达都会执行

## 下一步

### 可选的进一步优化

1. **增量计算**：只重新计算变化的部分
2. **并行执行**：独立的数据分支可以并行计算
3. **持久化缓存**：跨执行周期的缓存（需要失效策略）
4. **依赖追踪**：自动分析数据依赖关系

### 当前设计已经足够

对于大多数可视化编程场景，当前的设计已经：
- ✅ 性能优秀（缓存避免重复计算）
- ✅ 架构清晰（明确的执行模型）
- ✅ 易于扩展（新增节点类型简单）
- ✅ 易于调试（完整的日志系统）

## 总结

你的原始设计**已经是正确的**！我们只是：

1. **添加了执行模型标记**：让节点类型更明确
2. **添加了数据缓存**：避免重复计算
3. **完善了文档**：帮助理解和使用

核心的 **Pull-based DataFlow + Push-based ExecFlow** 架构保持不变，这是正确的设计！

---

## 参考文档

- `EXECUTOR_DESIGN.md` - 详细的设计文档
- `EXECUTOR_EXAMPLES.md` - 使用示例和调试技巧
- `src-tauri/src/executor/` - 实现代码

祝你的可视化编程系统开发顺利！🚀
