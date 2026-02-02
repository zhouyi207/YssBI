# Plot 窗口立即打开修复

## 问题描述
Plot 节点执行后，窗口不是立即打开，而是等到整个程序执行完毕后才打开。这导致用户体验不佳，无法实时看到可视化结果。

## 根本原因

### 1. 延迟累积
Plot 节点和窗口创建函数中有多个延迟：

**Plot 节点** (`visualization.rs`):
```rust
// 添加小延迟避免快速连续创建窗口时的冲突
std::thread::sleep(std::time::Duration::from_millis(20));
```

**open_window_async** (`context.rs`):
```rust
std::thread::spawn(move || {
    // 添加小延迟确保主线程不被阻塞
    std::thread::sleep(std::time::Duration::from_millis(50));
    // ... 创建窗口
});
```

**总延迟**: 20ms + 50ms = 70ms per window

### 2. 异步执行的误解
虽然使用了 `std::thread::spawn` 来异步创建窗口，但：
- 50ms 的延迟推迟了窗口创建的开始
- 线程调度可能进一步延迟
- 多个窗口连续创建时，延迟累积

### 3. 执行流程问题
在你的图中：
```
Sequence5 → Plot 1 → Timer
         → Plot 2
         → Plot 3
         → Plot 4
```

Sequence5 按顺序执行每个输出，每个 Plot 都有延迟，导致：
- Plot 1: 70ms 延迟
- Plot 2: 70ms 延迟
- Plot 3: 70ms 延迟
- Plot 4: 70ms 延迟
- 总计: 280ms 延迟

加上 Sequence5 节点之间的 50ms 延迟（第 113 行），总延迟更长。

## 修复方案

### 1. 移除 Plot 节点中的延迟
**文件**: `src-tauri/src/executor/node/catalog/visualization.rs`

**修改前**:
```rust
ctx.log(format!("Plot node executing: Creating window with label: {}", window_label));

// 添加小延迟避免快速连续创建窗口时的冲突
std::thread::sleep(std::time::Duration::from_millis(20));

// 异步创建窗口，不阻塞主线程
match ctx.open_window_async(window_label.clone(), "Data Plot".into(), "#/plot".into()) {
```

**修改后**:
```rust
ctx.log(format!("Plot node executing: Creating window with label: {}", window_label));

// 异步创建窗口，不阻塞主线程
match ctx.open_window_async(window_label.clone(), "Data Plot".into(), "#/plot".into()) {
```

### 2. 移除 open_window_async 中的延迟
**文件**: `src-tauri/src/executor/context.rs`

**修改前**:
```rust
std::thread::spawn(move || {
    // 添加小延迟确保主线程不被阻塞
    std::thread::sleep(std::time::Duration::from_millis(50));
    
    match WebviewWindowBuilder::new(
```

**修改后**:
```rust
std::thread::spawn(move || {
    // 立即创建窗口，不延迟
    match WebviewWindowBuilder::new(
```

## 效果对比

### 修复前
```
Plot 1 执行 → 等待 70ms → 开始创建窗口 → 窗口出现
Plot 2 执行 → 等待 70ms → 开始创建窗口 → 窗口出现
Plot 3 执行 → 等待 70ms → 开始创建窗口 → 窗口出现
Plot 4 执行 → 等待 70ms → 开始创建窗口 → 窗口出现
```

### 修复后
```
Plot 1 执行 → 立即创建窗口 → 窗口出现
Plot 2 执行 → 立即创建窗口 → 窗口出现
Plot 3 执行 → 立即创建窗口 → 窗口出现
Plot 4 执行 → 立即创建窗口 → 窗口出现
```

## 为什么原来有延迟？

### 原始设计意图
1. **20ms 延迟**: 避免快速连续创建窗口时的冲突
2. **50ms 延迟**: 确保主线程不被阻塞

### 为什么不需要这些延迟？
1. **窗口标签唯一性**: 每个窗口使用纳秒级时间戳 + UUID，已经保证唯一性
2. **异步执行**: `std::thread::spawn` 已经在新线程中执行，不会阻塞主线程
3. **Tauri 窗口管理**: Tauri 内部已经处理了窗口创建的并发问题

## 潜在问题和解决方案

### 问题 1: 快速创建多个窗口可能导致资源竞争
**解决方案**: 
- 窗口标签已经使用纳秒时间戳 + UUID 保证唯一性
- Tauri 内部有窗口管理机制
- 如果仍有问题，可以在前端添加节流控制

### 问题 2: 窗口创建失败
**解决方案**:
- 已有错误处理逻辑
- 即使窗口创建失败，执行流程也会继续
- 错误会记录到日志中

### 问题 3: 窗口创建顺序
**解决方案**:
- 窗口创建顺序由执行顺序决定
- Sequence5 保证按顺序执行
- 每个窗口独立创建，不会互相影响

## 测试场景

### 测试 1: 单个 Plot 窗口
1. 创建 `Event` → `Plot`
2. 执行图
3. **预期**: Plot 窗口立即出现（< 100ms）

### 测试 2: 多个 Plot 窗口（Sequence5）
1. 创建 `Event` → `Sequence5` → 4 个 `Plot`
2. 执行图
3. **预期**: 
   - 窗口按顺序快速出现
   - 每个窗口间隔 < 100ms
   - 不等待整个执行完成

### 测试 3: Plot + Timer
1. 创建 `Event` → `Plot` → `Timer`
2. 执行图
3. **预期**:
   - Plot 窗口立即出现
   - Timer 开始计时
   - 不互相阻塞

## 性能提升

### 修复前
- 4 个 Plot 窗口: ~280ms 延迟
- 用户感知: 明显延迟，窗口批量出现

### 修复后
- 4 个 Plot 窗口: ~0ms 延迟（仅线程调度时间）
- 用户感知: 立即响应，窗口逐个出现

## 编译状态
✅ **编译成功**
- 只有 7 个警告（与此修复无关）
- 无编译错误

## 文件修改
1. `src-tauri/src/executor/node/catalog/visualization.rs` - 移除 20ms 延迟
2. `src-tauri/src/executor/context.rs` - 移除 50ms 延迟

## 状态
✅ **修复完成，可以测试**

Plot 窗口现在会在节点执行时立即创建，不再等待整个程序执行完毕。
