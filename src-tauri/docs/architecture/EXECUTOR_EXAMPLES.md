# Executor 使用示例

## 示例 1：纯数据流计算

### 场景：计算 (10 + 5) * 2

```
[Constant: 10] ──┐
                 ├──> [Add] ──> [Multiply] ──> [Print]
[Constant: 5] ───┘                  ↑
                                    │
[Constant: 2] ──────────────────────┘
```

### 执行流程

```rust
// 1. ExecFlow 到达 Print 节点
Print.process_flow() {
    // 2. Print 需要 Multiply 的结果
    let value = ctx.get_pin_value("multiply_output")
    
    // 3. 回溯到 Multiply 节点
    Multiply.process_data() {
        // 4. Multiply 需要 Add 的结果
        let a = ctx.get_pin_value("add_output")
        
        // 5. 回溯到 Add 节点
        Add.process_data() {
            // 6. Add 需要两个 Constant
            let x = ctx.get_pin_value("const_10")  // 计算并缓存
            let y = ctx.get_pin_value("const_5")   // 计算并缓存
            return x + y  // = 15，缓存结果
        }
        
        // 7. Multiply 需要第二个输入
        let b = ctx.get_pin_value("const_2")  // 计算并缓存
        return a * b  // = 30，缓存结果
    }
    
    // 8. Print 输出结果
    println!("Result: {}", value)  // Result: 30
}
```

### 关键点

- ✅ 所有 Constant 节点只计算一次（缓存）
- ✅ Add 和 Multiply 的结果也被缓存
- ✅ 如果有其他节点也需要这些值，直接使用缓存

---

## 示例 2：控制流分支

### 场景：根据条件打印不同消息

```
[OnRun] ──> [GetVariable: "age"] ──> [GreaterThan: 18] ──> [IfElse]
                                                              ├──True──> [Print "Adult"]
                                                              └──False─> [Print "Minor"]
```

### 执行流程

```rust
// 1. ExecFlow 从 OnRun 开始
OnRun.process_flow() {
    return "Exec"  // 触发下一个节点
}

// 2. ExecFlow 到达 IfElse
IfElse.process_flow() {
    // 3. IfElse 需要条件数据
    let condition = ctx.get_pin_value("greater_than_output")
    
    // 4. 回溯到 GreaterThan
    GreaterThan.process_data() {
        let age = ctx.get_pin_value("get_variable_output")
        
        // 5. 回溯到 GetVariable
        GetVariable.process_data() {
            return ctx.get_variable("age")  // = 20
        }
        
        return age > 18  // = true
    }
    
    // 6. 根据条件决定控制流
    if condition {
        ctx.trigger_flow_by_pin("True")  // 执行 Print "Adult"
    } else {
        ctx.trigger_flow_by_pin("False")
    }
}

// 7. ExecFlow 到达 Print "Adult"
Print.process_flow() {
    println!("Adult")
}
```

### 关键点

- ✅ DataFlow（GetVariable, GreaterThan）按需计算
- ✅ ExecFlow（IfElse）根据数据决定分支
- ✅ 只有一个分支被执行

---

## 示例 3：循环执行

### 场景：打印 0 到 4

```
[OnRun] ──> [ForLoop: 0..5] ──┬──Loop Body──> [Print Index]
                              └──Completed──> [Print "Done"]
```

### 执行流程

```rust
// 1. ExecFlow 到达 ForLoop
ForLoop.process_flow() {
    let start = ctx.get_pin_value("start_input")  // = 0
    let end = ctx.get_pin_value("end_input")      // = 5
    
    // 2. 循环执行 Loop Body
    for i in start..end {
        ctx.set_variable("loop_index", i)
        ctx.trigger_flow_by_pin("Loop Body")
        
        // 3. ExecFlow 到达 Print
        Print.process_flow() {
            let index = ctx.get_variable("loop_index")
            println!("Index: {}", index)
        }
    }
    
    // 4. 循环结束，触发 Completed
    ctx.trigger_flow_by_pin("Completed")
}

// 5. ExecFlow 到达 Print "Done"
Print.process_flow() {
    println!("Done")
}
```

### 输出

```
Index: 0
Index: 1
Index: 2
Index: 3
Index: 4
Done
```

### 关键点

- ✅ ForLoop 内部管理循环逻辑
- ✅ Loop Body 被执行多次
- ✅ Completed 在循环结束后执行一次

---

## 示例 4：数据缓存优化

### 场景：复用计算结果

```
[Constant: 10] ──┬──> [Add: +5] ──> [Print]
                 ├──> [Multiply: *2] ──> [Print]
                 └──> [Divide: /2] ──> [Print]
```

### 执行流程（无缓存）

```rust
// 假设没有缓存
Print1: Constant 计算 → 返回 10
Print2: Constant 计算 → 返回 10  // 重复计算！
Print3: Constant 计算 → 返回 10  // 重复计算！
```

### 执行流程（有缓存）✅

```rust
// 1. Print1 需要 Add 的结果
Add.process_data() {
    let x = ctx.get_pin_value("constant_output")
    // Constant 计算 → 返回 10，缓存到 data_cache
    return x + 5  // = 15
}

// 2. Print2 需要 Multiply 的结果
Multiply.process_data() {
    let x = ctx.get_pin_value("constant_output")
    // 直接从 data_cache 获取 10，不重复计算！
    return x * 2  // = 20
}

// 3. Print3 需要 Divide 的结果
Divide.process_data() {
    let x = ctx.get_pin_value("constant_output")
    // 直接从 data_cache 获取 10，不重复计算！
    return x / 2  // = 5
}
```

### 性能对比

| 场景 | 无缓存 | 有缓存 |
|-----|-------|-------|
| Constant 计算次数 | 3 次 | 1 次 |
| 性能提升 | - | 3x |

### 关键点

- ✅ Constant 是 DataFlow 节点，结果可缓存
- ✅ 一次执行周期内，相同的输出 Pin 只计算一次
- ✅ 大幅减少重复计算，提升性能

---

## 示例 5：混合节点的副作用

### 场景：设置变量并打印

```
[OnRun] ──> [SetVariable: "count" = 0] ──> [Print Variable]
```

### 为什么 SetVariable 不缓存？

```rust
// SetVariable 是 Hybrid 节点
SetVariable.execution_model() == ExecutionModel::Hybrid

// 原因：有副作用
SetVariable.process_flow() {
    let value = ctx.get_pin_value("value_input")
    ctx.set_variable("count", value)  // 副作用：修改全局状态
    return "Exec"
}

// 如果缓存，可能导致：
// 1. 第二次调用时不执行，变量不更新
// 2. 副作用被跳过，导致错误
```

### 缓存策略

| 执行模型 | 是否缓存 | 原因 |
|---------|---------|------|
| DataFlow | ✅ 是 | 纯函数，无副作用 |
| Hybrid | ❌ 否 | 可能有副作用 |
| ControlFlow | ❌ 否 | 不产生数据 |
| Event | ❌ 否 | 不产生数据 |

---

## 示例 6：复杂图的执行

### 场景：完整的业务逻辑

```
[OnRun] ──> [GetVariable: "user_age"] ──┐
                                         ├──> [GreaterThan: 18] ──> [IfElse]
[Constant: 18] ──────────────────────────┘                            ├──True──> [Sequence]
                                                                      │            ├──> [SetVariable: "status" = "adult"]
                                                                      │            └──> [Print "Welcome"]
                                                                      └──False─> [Print "Access Denied"]
```

### 执行流程

```rust
// 1. ExecFlow: OnRun → IfElse
IfElse.process_flow() {
    // 2. DataFlow: 回溯获取条件
    let condition = ctx.get_pin_value("greater_than_output")
    
    GreaterThan.process_data() {
        let age = ctx.get_pin_value("get_variable_output")  // = 20
        let threshold = ctx.get_pin_value("constant_output")  // = 18，缓存
        return age > threshold  // = true
    }
    
    // 3. ExecFlow: 根据条件分支
    if condition {
        ctx.trigger_flow_by_pin("True")  // → Sequence
    }
}

// 4. ExecFlow: Sequence
Sequence.process_flow() {
    ctx.trigger_flow_by_pin("Then 0")  // → SetVariable
    ctx.trigger_flow_by_pin("Then 1")  // → Print
}

// 5. ExecFlow: SetVariable
SetVariable.process_flow() {
    ctx.set_variable("status", "adult")
}

// 6. ExecFlow: Print
Print.process_flow() {
    println!("Welcome")
}
```

### 关键点

- ✅ ExecFlow 和 DataFlow 协同工作
- ✅ Constant 18 被缓存，可被多次使用
- ✅ 控制流根据数据动态决定路径
- ✅ Sequence 确保操作顺序

---

## 调试技巧

### 1. 查看节点执行模型

```rust
let node = /* ... */;
let model = node.execution_model();
println!("Node: {}, Model: {:?}", node.node_type(), model);

// 输出：
// Node: constant, Model: DataFlow
// Node: if_else, Model: Hybrid
// Node: sequence, Model: ControlFlow
```

### 2. 启用缓存日志

在 `context.rs` 的 `get_pin_value` 中取消注释：

```rust
// 缓存命中
if let Some(cached_value) = self.data_cache.get(&output_pin_id) {
    self.log(format!("[DataFlow Cache Hit] Pin {:?}", output_pin_id));
    return cached_value.clone();
}

// 缓存存储
if execution_model.is_cacheable() {
    self.data_cache.insert(output_pin_id, value.clone());
    self.log(format!("[DataFlow Cache Store] Pin {:?}", output_pin_id));
}
```

### 3. 追踪执行路径

```rust
// 在 run_flow_internal 中
self.logs.push(format!(">>> Executing Node: {} ({})", node_name, node_type));

// 在 get_pin_value 中
self.logs.push(format!("<<< DataFlow: Getting value from {}", node_type));
```

---

## 性能优化建议

### 1. 优先使用 DataFlow 节点

```rust
// ❌ 不好：每次都重新计算
Hybrid节点 {
    flow_processor: 计算 + 副作用
}

// ✅ 好：分离计算和副作用
DataFlow节点 {
    data_processor: 纯计算（可缓存）
}
Hybrid节点 {
    flow_processor: 只处理副作用
}
```

### 2. 避免在循环中重复计算

```rust
// ❌ 不好：每次循环都计算 threshold
ForLoop {
    for i in 0..100 {
        let threshold = calculate_threshold()  // 重复计算 100 次
        if i > threshold { ... }
    }
}

// ✅ 好：在循环外计算一次
[Calculate Threshold] ──> [ForLoop]
                            ↑ 在循环内使用缓存的值
```

### 3. 合理使用 Sequence

```rust
// ❌ 不好：不必要的顺序
[A] ──> [B] ──> [C]  // 如果 A、B、C 没有依赖关系

// ✅ 好：并行执行
[OnRun] ──┬──> [A]
          ├──> [B]
          └──> [C]
```

---

## 总结

这个执行器设计的核心优势：

1. **清晰的执行模型**：每个节点都有明确的角色
2. **高效的数据缓存**：避免重复计算
3. **灵活的控制流**：支持分支、循环等复杂逻辑
4. **易于调试**：完整的日志和追踪系统

通过这些示例，你应该能够理解如何设计和实现各种类型的节点，以及如何优化执行性能。
