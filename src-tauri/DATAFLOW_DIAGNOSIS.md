# DataFlow 执行诊断

## 你的三条铁律分析

### 铁律 1: Data 节点必须是纯函数 ✅

**当前状态**：✅ **已满足**

```rust
// divide 节点实现
reg_binary!("divide", "Divide (/)", math_cat.clone(), "A", "B", ValueType::Float64, |a: Value, b: Value| {
    let va = a.as_f64().unwrap_or(0.0);
    let vb = b.as_f64().unwrap_or(1.0);
    Value::from(va / vb)  // 纯函数：同样输入 → 同样输出
});
```

- ✅ 无副作用
- ✅ 不依赖执行顺序
- ✅ 确定性输出

### 铁律 2: eval 必须是递归拉取 + 缓存 ✅

**当前状态**：✅ **已正确实现**

```rust
// src-tauri/src/executor/context.rs
fn get_pin_value(&mut self, pin_id_str: &str) -> Value {
    // 1. 获取运行时 PinId
    let pin_id = self.data_pin_id_to_runtime_pin_id.get(pin_id_str)?;
    
    // 2. 查找上游连接
    let output_pin_id = self.connection_manager.get_upstream(pin_id)?;
    
    // 3. 检查缓存 ✅
    if let Some(cached_value) = self.data_cache.get(&output_pin_id) {
        return cached_value.clone();
    }
    
    // 4. 找到输出节点
    let node_id = self.pin_to_node.get(&output_pin_id)?;
    
    // 5. 循环依赖检测 ✅
    if self.evaluating_stack.contains(&node_id) {
        return Value::Null;
    }
    
    // 6. 递归求值 ✅
    self.evaluating_stack.push(node_id);
    let value = proto.process_data(self, &node_data, &output_pin_id_str);
    self.evaluating_stack.pop();
    
    // 7. 缓存结果 ✅
    if execution_model.is_cacheable() {
        self.data_cache.insert(output_pin_id, value.clone());
    }
    
    value
}
```

**divide 节点的 data_processor**：
```rust
node.set_data_processor(Box::new(|ctx, node, _pin_id| {
    let a = ctx.get_pin_value(&node.inputs[0].id);  // 🔁 递归拉取 A
    let b = ctx.get_pin_value(&node.inputs[1].id);  // 🔁 递归拉取 B
    // 计算
    Value::from(va / vb)
}));
```

**完整的递归链**：
```
Print.execute()
  └─> ctx.get_pin_value("print_value")
        └─> Divide.process_data()
              ├─> ctx.get_pin_value("divide_a")
              │     └─> GetVariable.process_data()
              │           └─> ctx.get_variable("var_a")
              └─> ctx.get_pin_value("divide_b")
                    └─> GetVariable.process_data()
                          └─> ctx.get_variable("var_b")
```

### 铁律 3: exec 节点 ≠ data 节点 ✅

**当前状态**：✅ **已正确分离**

```rust
// ExecutionModel 枚举已经明确区分
pub enum ExecutionModel {
    Event,        // 事件节点
    ControlFlow,  // 纯控制流
    DataFlow,     // 纯数据流（Pure）
    Hybrid,       // 混合节点
}
```

**节点分类**：
- `print` → `Hybrid` (有 exec + data)
- `divide` → `DataFlow` (只有 data)
- `get_variable` → `DataFlow` (只有 data)

## 当前实现的正确性验证

### ✅ 递归拉取已实现

**证据 1**: `get_pin_value` 会递归调用
```rust
// 在 divide 的 process_data 中
let a = ctx.get_pin_value(&node.inputs[0].id);  // 触发递归
```

**证据 2**: 缓存机制已实现
```rust
if let Some(cached_value) = self.data_cache.get(&output_pin_id) {
    return cached_value.clone();  // 缓存命中
}
```

**证据 3**: 循环依赖检测已实现
```rust
if self.evaluating_stack.contains(&node_id) {
    return Value::Null;  // 检测到循环
}
```

### ✅ Print 节点正确使用 eval

让我检查 print 节点的实现...

## 可能的问题点

### 问题 1: Print 节点实现

如果 print 节点直接访问 `values[0]` 而不是调用 `ctx.get_pin_value()`，就会出问题。

**需要检查**：
```rust
// ❌ 错误的实现
fn execute() {
    let v = values[0];  // 直接访问，values 可能为空
}

// ✅ 正确的实现
fn execute() {
    let v = ctx.get_pin_value(&node.inputs[0].id);  // 递归拉取
}
```

### 问题 2: NodeData 的 inputs 字段

在 `get_pin_value` 中构造 NodeData 时：
```rust
let node_data = {
    let node_guard = node_arc.lock().unwrap();
    NodeData {
        id: ...,
        node_type: ...,
        title: ...,
        inputs: vec![],   // ⚠️ 空的！
        outputs: vec![],  // ⚠️ 空的！
        variable_id: ...,
        sub_graph_id: None,
    }
};
```

**这可能导致问题**：如果节点的 `process_data` 依赖 `node.inputs[0].id`，但 `inputs` 是空的，就会 panic！

## 根本问题诊断

### 问题：NodeData.inputs 为空

**位置**：`src-tauri/src/executor/context.rs` 的 `get_pin_value()` 方法

**当前代码**：
```rust
let node_data = {
    let node_guard = node_arc.lock().unwrap();
    NodeData {
        id: self.runtime_id_to_data_id.get(&node_id).cloned().unwrap_or_default(),
        node_type: node_guard.node_type().to_string(),
        title: node_guard.name().to_string(),
        inputs: vec![],   // ❌ 问题：空的！
        outputs: vec![],  // ❌ 问题：空的！
        variable_id: node_guard.variable_id(),
        sub_graph_id: None,
    }
};
```

**问题分析**：
1. `divide` 节点的 `process_data` 需要访问 `node.inputs[0].id` 和 `node.inputs[1].id`
2. 但是传入的 `NodeData` 的 `inputs` 是空的
3. 导致 `node.inputs[0]` 访问越界或返回错误

### 解决方案

**方案 1：填充 NodeData 的 inputs/outputs**

```rust
let node_data = {
    let node_guard = node_arc.lock().unwrap();
    
    // 填充 inputs
    let mut inputs = Vec::new();
    for input_pin in node_guard.inputs().iter() {
        let frontend_pin_id = self.data_pin_id_to_runtime_pin_id
            .iter()
            .find(|(_, &runtime_id)| runtime_id == input_pin.id())
            .map(|(frontend_id, _)| frontend_id.clone())
            .unwrap_or_default();
        
        inputs.push(PinData {
            id: frontend_pin_id,
            name: input_pin.name().to_string(),
            pin_type: input_pin.data_type().to_string(),
            links: vec![],
            default_value: None,
            is_array: false,
        });
    }
    
    // 填充 outputs
    let mut outputs = Vec::new();
    for output_pin in node_guard.outputs().iter() {
        let frontend_pin_id = self.data_pin_id_to_runtime_pin_id
            .iter()
            .find(|(_, &runtime_id)| runtime_id == output_pin.id())
            .map(|(frontend_id, _)| frontend_id.clone())
            .unwrap_or_default();
        
        outputs.push(PinData {
            id: frontend_pin_id,
            name: output_pin.name().to_string(),
            pin_type: output_pin.data_type().to_string(),
            links: vec![],
            default_value: None,
            is_array: false,
        });
    }
    
    NodeData {
        id: self.runtime_id_to_data_id.get(&node_id).cloned().unwrap_or_default(),
        node_type: node_guard.node_type().to_string(),
        title: node_guard.name().to_string(),
        inputs,   // ✅ 填充了！
        outputs,  // ✅ 填充了！
        variable_id: node_guard.variable_id(),
        sub_graph_id: None,
    }
};
```

**方案 2：改变节点 API（更彻底）**

不传递 `NodeData`，而是直接传递 Pin ID：

```rust
// 新的 API
trait DataNode {
    fn compute(&self, ctx: &mut ExecutionContext, input_pins: &[PinId]) -> Value;
}

// divide 节点实现
node.set_data_processor(Box::new(|ctx, input_pins, _output_pin| {
    let a = ctx.get_pin_value_by_id(input_pins[0]);  // 直接用 PinId
    let b = ctx.get_pin_value_by_id(input_pins[1]);
    Value::from(a.as_f64()? / b.as_f64()?)
}));
```

## 推荐的修复方案

### 立即修复：填充 NodeData.inputs

这是最小改动，立即可用的方案。

**位置**：`src-tauri/src/executor/context.rs` 的 `get_pin_value()` 方法

**修改**：将空的 `inputs` 和 `outputs` 改为正确填充的数据

### 中期优化：统一 API

考虑重构节点 API，使其更清晰：
- `ExecNode::execute(ctx, node_id)` - 执行节点
- `DataNode::compute(ctx, input_pins)` - 计算数据

## 总结

**好消息**：
- ✅ 递归拉取机制已正确实现
- ✅ 缓存机制已正确实现
- ✅ 循环依赖检测已正确实现
- ✅ 节点分类已正确实现

**问题**：
- ⚠️ `NodeData.inputs` 在 `get_pin_value` 中为空
- ⚠️ 导致节点无法访问输入 Pin ID

**修复**：
- 🔧 填充 `NodeData.inputs` 和 `NodeData.outputs`
- 🔧 确保节点可以正确访问 Pin ID

**你的三条铁律**：
1. ✅ Data 节点是纯函数
2. ✅ eval 是递归拉取 + 缓存
3. ✅ exec 节点 ≠ data 节点

**架构是正确的，只需要修复 NodeData 的填充问题！**
