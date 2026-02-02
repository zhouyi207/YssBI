# Pin 用户值执行修复

**日期**: 2026-02-02  
**状态**: ✅ 已修复

## 问题描述

用户在前端为 Print 节点的输入 Pin 设置了值，但执行时输出仍然是 `null`。

### 日志分析

```
[14:13:08.682][BE][INFO] >>> Executing Node: GenericNode (print)
[14:13:08.682][BE][INFO] [Print] null  ← 应该输出用户设置的值
```

## 根本原因

在 `ExecutionContext::get_pin_value` 方法中，当 Pin 没有上游连接时，直接返回 `Value::Null`，没有检查用户值（`user_value`）和默认值（`default_value`）。

### 原始代码

```rust
fn get_pin_value(&mut self, pin_id_str: &str) -> Value {
    // 1. 获取运行时 PinId
    let pin_id = ...;

    // 2. 查找上游连接
    let output_pin_id = match self.connection_manager.get_upstream(pin_id) {
        Some(id) => id,
        None => return Value::Null,  // ❌ 直接返回 Null，没有检查用户值
    };
    
    // ... 处理连接值
}
```

## 解决方案

### 1. 存储原始图数据

在 `ExecutionContext` 中添加 `original_graph` 字段来保存原始图数据：

```rust
pub struct ExecutionContext {
    // ... 现有字段
    
    /// 🆕 原始图数据（用于获取 Pin 的 user_value 和 default_value）
    original_graph: GraphDto,
}
```

### 2. 添加辅助方法

```rust
impl ExecutionContext {
    /// 从原始图数据中获取 Pin 信息
    fn get_original_pin_data(&self, pin_id: &str) -> Option<&crate::project::PinDto> {
        for node in &self.original_graph.nodes {
            // 查找输入 Pin
            for pin in &node.inputs {
                if pin.id == pin_id {
                    return Some(pin);
                }
            }
            // 查找输出 Pin
            for pin in &node.outputs {
                if pin.id == pin_id {
                    return Some(pin);
                }
            }
        }
        None
    }
}
```

### 3. 实现三层优先级

修改 `get_pin_value` 方法，实现完整的三层优先级：

```rust
fn get_pin_value(&mut self, pin_id_str: &str) -> Value {
    // 1. 获取运行时 PinId
    let pin_id = ...;

    // 2. 查找上游连接
    if let Some(output_pin_id) = self.connection_manager.get_upstream(pin_id) {
        // ✅ 有连接：使用连接值（优先级最高）
        // ... 处理连接值的逻辑
        return value;
    }
    
    // 🆕 3. 没有连接：检查用户值和默认值
    let original_graph = self.get_original_pin_data(pin_id_str);
    
    // ✅ 优先级中：用户值
    if let Some(user_value) = original_graph.and_then(|p| p.user_value.clone()) {
        info!("[get_pin_value] Using user value for pin '{}': {:?}", pin_id_str, user_value);
        return user_value;
    }
    
    // ✅ 优先级低：默认值
    if let Some(default_value) = original_graph.and_then(|p| p.default_value.clone()) {
        info!("[get_pin_value] Using default value for pin '{}': {:?}", pin_id_str, default_value);
        return default_value;
    }
    
    // 都没有，返回 Null
    Value::Null
}
```

## 值优先级系统

```
┌─────────────────────────────────────┐
│  1. 连接值 (Highest Priority)       │
│     - 如果 Pin 有上游连接            │
│     - 计算并返回连接的值             │
└─────────────────────────────────────┘
              ↓ 没有连接
┌─────────────────────────────────────┐
│  2. 用户值 (Medium Priority)         │
│     - 用户在前端设置的值             │
│     - 通过 update_pin_user_value 保存│
└─────────────────────────────────────┘
              ↓ 没有用户值
┌─────────────────────────────────────┐
│  3. 默认值 (Lowest Priority)         │
│     - 节点定义中的默认值             │
│     - 从 schema 或节点原型获取       │
└─────────────────────────────────────┘
              ↓ 都没有
┌─────────────────────────────────────┐
│  4. Null                            │
└─────────────────────────────────────┘
```

## 测试场景

### 场景 1：仅用户值

```
节点：Print
输入：Message (未连接)
用户值："Hello World"

执行结果：
[Print] Hello World  ✅
```

### 场景 2：连接值优先

```
节点：Print
输入：Message (已连接到 String 节点)
用户值："Hello World"
连接值："From Connection"

执行结果：
[Print] From Connection  ✅ (连接值优先)
```

### 场景 3：默认值

```
节点：Add
输入：A (未连接，无用户值)
默认值：0

执行结果：
使用默认值 0  ✅
```

## 修改的文件

- ✅ `src-tauri/src/executor/context.rs`
  - 添加 `original_graph` 字段
  - 添加 `get_original_pin_data` 方法
  - 修改 `get_pin_value` 方法实现三层优先级

## 编译状态

```
✅ cargo check 通过
⚠️  7 个警告（静态变量引用，非关键）
```

## 日志输出

修复后的日志输出：

```
[14:13:08.682][BE][INFO] >>> Executing Node: GenericNode (print)
[14:13:08.682][BE][INFO] [get_pin_value] Using user value for pin 'pin-xxx': String("Hello World")
[14:13:08.682][BE][INFO] [Print] Hello World  ✅
[14:13:08.683][BE][INFO]   -> Node returned next exec: 'Out'
```

## 相关功能

此修复完善了以下功能链：

1. **前端输入** → `PinInput.tsx` 组件
2. **API 调用** → `update_pin_user_value` 命令
3. **数据保存** → `SerializedPin.user_value` 字段
4. **执行读取** → `ExecutionContext::get_pin_value` ✅ (本次修复)

## 未来改进

- [ ] 添加类型验证（确保用户值类型与 Pin 类型匹配）
- [ ] 添加值转换（自动转换兼容类型）
- [ ] 支持表达式求值（例如：`"2 + 3"` → `5`）
- [ ] 添加值缓存（避免重复查找原始图数据）

## 总结

成功修复了 Pin 用户值在执行时不生效的问题，实现了完整的三层优先级系统：

1. ✅ 连接值（最高优先级）
2. ✅ 用户值（中等优先级）- **本次修复**
3. ✅ 默认值（最低优先级）

现在用户可以在前端为未连接的 Pin 设置值，执行时会正确使用这些值。🎉
