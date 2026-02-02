# Exec Pin 连接动画修复

## 问题描述
执行 pin（exec pin）的连接线没有显示数据流动画，只有数据 pin（data pin）的连接线有动画。

例如：
- `Sequence5` → `Plot` 的 exec 连接：❌ 无动画
- `Constant` → `Print` 的 data 连接：✅ 有动画

## 根本原因

后端有两个触发下游流程的函数：

### 1. `trigger_next_flow` (context.rs:494)
- 用于普通节点的流程控制
- **✅ 发送 connection_active 事件**
- 代码位置：第 536-549 行

### 2. `execute_pin_downstream` (context.rs:874)
- 用于控制流节点（如 Sequence, IfElse）的流程控制
- **❌ 没有发送 connection_active 事件** ← 问题所在！
- 被 `trigger_flow_by_pin` 调用
- 被 Sequence5, IfElse, WhileLoop 等控制流节点使用

## 修复方案

在 `execute_pin_downstream` 函数中添加连接激活事件的发送逻辑，与 `trigger_next_flow` 保持一致。

### 修改位置
文件：`src-tauri/src/executor/context.rs`
函数：`execute_pin_downstream` (第 874 行)

### 添加的代码
```rust
// 执行所有下游连接
for (index, &next_pin_id) in downstream_pins.iter().enumerate() {
    // 🆕 发送连接激活事件
    let from_pin_id_str = self.data_pin_id_to_runtime_pin_id
        .iter()
        .find(|(_, &v)| v == pin_id)
        .map(|(k, _)| k.clone());
    let to_pin_id_str = self.data_pin_id_to_runtime_pin_id
        .iter()
        .find(|(_, &v)| v == next_pin_id)
        .map(|(k, _)| k.clone());
        
    if let (Some(from), Some(to)) = (from_pin_id_str, to_pin_id_str) {
        self.emit_execution_event("connection_active", None, Some(&from), Some(&to));
        // 添加小延迟让前端显示连接动画
        std::thread::sleep(std::time::Duration::from_millis(150));
    }
    
    let next_node_id = self
        .pin_to_node
        .get(&next_pin_id)
        .ok_or("Target node not found")?;
    
    info!("[execute_pin_downstream] Executing downstream node #{}: {:?}", index + 1, next_node_id);
    self.run_flow_internal(*next_node_id, "Out")?;
}
```

## 影响范围

### 受益的节点类型
所有使用 `trigger_flow_by_pin` 的控制流节点：

1. **Sequence** - 顺序执行节点
2. **Sequence5** - 5 路顺序执行节点
3. **IfElse** - 条件分支节点
4. **WhileLoop** - 循环节点
5. **Dynamic Sequence** - 动态顺序节点
6. 其他自定义控制流节点

### 连接类型
- ✅ Exec Pin → Exec Pin 连接（现在有动画了）
- ✅ Data Pin → Data Pin 连接（已有动画）

## 测试场景

### 测试 1: Sequence5 → Plot
1. 创建 `Event` → `Sequence5` → 多个 `Plot` 节点
2. 执行图
3. **预期**:
   - Sequence5 的每个 "Then X" 输出连接先变黄色（300ms）
   - 然后显示绿色粒子流动
   - 按顺序依次激活每条连接

### 测试 2: IfElse 分支
1. 创建 `Event` → `IfElse` → `Plot` (True) 和 `Plot` (False)
2. 执行图
3. **预期**:
   - 根据条件，True 或 False 分支的连接显示动画
   - 未执行的分支不显示动画

### 测试 3: WhileLoop 循环
1. 创建包含 WhileLoop 的图
2. 执行图
3. **预期**:
   - Loop Body 连接每次循环都显示动画
   - Completed 连接在循环结束后显示动画

## 技术细节

### 事件发送时机
```
节点开始执行
  ↓
调用 trigger_flow_by_pin("Then 0")
  ↓
execute_pin_downstream 查找下游连接
  ↓
🆕 发送 connection_active 事件
  ↓
等待 150ms（后端延迟）
  ↓
执行下游节点
```

### 前端处理流程
```
接收 connection_active 事件
  ↓
添加到 activeConnections（黄色高亮）
  ↓
300ms 后移除 activeConnections
  ↓
添加到 completedConnections（绿色粒子）
  ↓
粒子持续流动直到执行结束
```

### 时间线
- t=0ms: 连接激活，变黄色
- t=150ms: 后端开始执行下游节点
- t=300ms: 连接完成，恢复原色，开始显示绿色粒子
- t=...: 粒子持续流动
- t=end: 执行结束
- t=end+2s: 状态重置

## 代码一致性

现在两个触发流程的函数都发送连接激活事件：

### trigger_next_flow
```rust
// 普通节点的流程控制
if let (Some(from), Some(to)) = (from_pin_id_str, to_pin_id_str) {
    self.emit_execution_event("connection_active", None, Some(&from), Some(&to));
    std::thread::sleep(std::time::Duration::from_millis(150));
}
```

### execute_pin_downstream
```rust
// 控制流节点的流程控制
if let (Some(from), Some(to)) = (from_pin_id_str, to_pin_id_str) {
    self.emit_execution_event("connection_active", None, Some(&from), Some(&to));
    std::thread::sleep(std::time::Duration::from_millis(150));
}
```

## 编译状态
✅ **编译成功**
- 只有 7 个警告（与此修复无关）
- 无编译错误

## 文件修改
1. `src-tauri/src/executor/context.rs` - 在 `execute_pin_downstream` 中添加事件发送

## 状态
✅ **修复完成，可以测试**

现在所有类型的连接（exec pin 和 data pin）都会显示数据流动画。
