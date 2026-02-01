# Blueprint 执行模型重构计划

## 问题诊断

当前系统的核心问题：**Pure 节点（如 Divide, GetVariable）被当作可执行节点处理**

### 错误的执行流程（当前）
```
Event ──exec──▶ Print
                 ▲
                 │ value link
              Divide   ← 💥 Divide 被 execute() 调用，但它没有 exec pin！
```

### 正确的执行流程（Blueprint）
```
Event ──exec──▶ Print
                  │
                  ▼ Lazy Evaluate(Value)
               Divide  ← ✅ Divide 被 eval() 调用，按需计算
                /  \
          GetVarA  GetVarB
```

## 核心原则

1. **Exec 只管顺序** - 控制流节点决定执行顺序
2. **Data 只在被需要时才计算** - 数据流节点按需求值（Lazy Pull）
3. **Pure 节点永远不进执行队列** - 只能被 eval，不能被 execute

## 改造清单

### Step 1: 节点分型 ✅ (已完成)

现有的 `ExecutionModel` 枚举已经定义了节点类型：
- `Event` - 事件节点（执行起点）
- `ControlFlow` - 纯控制流节点
- `Hybrid` - 混合节点（有 exec 和 data）
- `DataFlow` - 纯数据流节点（Pure Node）

**判定规则**：
- 有 exec input 或 exec output → `ControlFlow` / `Hybrid` / `Event`
- 完全没有 exec pin → `DataFlow` (Pure)

### Step 2: 禁止 Pure 节点被 execute ⚠️ (需要实现)

在 `ExecutionContext::run_flow_internal()` 中添加防护：

```rust
fn run_flow_internal(&mut self, node_id: NodeId, output_exec_name: &str) -> Result<(), String> {
    let node = self.nodes.get(&node_id)?.clone();
    let node_guard = node.lock().unwrap();
    
    // 🚨 关键防线：Pure 节点不能被执行
    if node_guard.execution_model() == ExecutionModel::DataFlow {
        return Err(format!(
            "Pure DataFlow node '{}' cannot be executed directly. It should be evaluated lazily.",
            node_guard.name()
        ));
    }
    
    // ... 继续执行
}
```

### Step 3: Lazy Pull 机制 ✅ (已部分实现)

`ExecutionContext::get_pin_value()` 已经实现了 Lazy Pull：
1. 检查缓存
2. 找到上游节点
3. 调用 `process_data()` 求值
4. 缓存结果（仅 DataFlow 节点）

**需要改进**：
- 添加循环依赖检测
- 更清晰的错误信息

### Step 4: Pure 节点的 eval 实现 ✅ (已实现)

Pure 节点通过 `data_processor` 实现求值：

```rust
// Divide 节点示例
node.set_data_processor(Box::new(|ctx, _node, _pin_id| {
    let a = ctx.get_pin_value("input_a");
    let b = ctx.get_pin_value("input_b");
    // 计算并返回
}));
```

### Step 5: Exec 节点只关心需要的值 ✅ (已实现)

```rust
// Print 节点示例
node.set_flow_processor(Box::new(|ctx, node| {
    let value = ctx.get_pin_value(&node.inputs[0].id);  // Lazy Pull
    ctx.log(format!("Print: {:?}", value));
    Ok("Exec".to_string())
}));
```

### Step 6: 缓存机制 ✅ (已实现)

`ExecutionContext` 已有：
- `data_cache: HashMap<PinId, Value>` - 缓存计算结果
- 只缓存 `DataFlow` 节点（通过 `execution_model().is_cacheable()`）
- 每次执行周期开始和结束时清空缓存

**需要添加**：
- `evaluating: HashSet<NodeId>` - 防止循环依赖

### Step 7: 执行主循环 ✅ (已实现)

`ExecutionContext::run_flow_internal()` 已经实现了正确的执行流程：
- 只执行有 exec pin 的节点
- Pure 节点通过 `get_pin_value()` 按需求值

## 实施步骤

### 阶段 1: 添加防护（立即修复 crash）

1. ✅ 在 `run_flow_internal()` 中添加 Pure 节点检查
2. ✅ 添加循环依赖检测
3. ✅ 改进错误信息

### 阶段 2: 验证和测试

1. ✅ 测试 Divide 节点不再 crash
2. ✅ 测试 GetVariable 正确求值
3. ✅ 测试缓存机制工作正常
4. ✅ 测试循环依赖检测

### 阶段 3: 文档和示例

1. ✅ 更新 EXECUTOR_DESIGN.md
2. ✅ 添加 Pure 节点开发指南
3. ✅ 添加调试日志

## 节点分类表

| 节点类型 | ExecutionModel | 是否可 execute | 是否可 eval |
|---------|---------------|---------------|------------|
| event_on_run | Event | ✅ | ❌ |
| print | Hybrid | ✅ | ❌ |
| set_variable | Hybrid | ✅ | ❌ |
| divide | DataFlow | ❌ | ✅ |
| get_variable | DataFlow | ❌ | ✅ |
| add | DataFlow | ❌ | ✅ |
| sequence | ControlFlow | ✅ | ❌ |
| if_else | Hybrid | ✅ | ❌ |

## 预期效果

### 修复前（错误）
```
>>> Executing Event
>>> Executing Print
>>> Executing Divide  ← 💥 Panic! Divide 没有 exec pin
```

### 修复后（正确）
```
>>> Executing Event
>>> Executing Print
    [eval] Divide      ← ✅ Lazy evaluation
        [eval] GetVariable A
        [eval] GetVariable B
    Print: 5.0
```

## 兼容性

- ✅ 节点结构：不需要改动
- ✅ Pin / Link：完全复用
- ✅ 前端：不需要改动
- ⚠️ 后端：执行层需要添加防护

## 风险评估

- **低风险**：只添加防护逻辑，不改变现有工作的节点
- **高收益**：彻底修复 Pure 节点的执行问题
- **向后兼容**：现有节点继续工作

## 测试计划

1. **单元测试**：Pure 节点不能被 execute
2. **集成测试**：Print + Divide + GetVariable 完整流程
3. **性能测试**：缓存机制有效性
4. **边界测试**：循环依赖检测

## 总结

这是一次**语义修正**，不是重写：
- 核心架构保持不变
- 添加关键防护逻辑
- 明确节点执行模型
- 符合 Blueprint 语义
