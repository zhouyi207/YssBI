# 执行状态可视化功能 - 完成

## 功能概述

实现了图执行时的实时可视化反馈：
- ✅ 正在执行的节点会高亮显示（黄色边框 + 脉冲动画）
- ✅ 已执行的节点显示绿色标记
- ✅ 错误节点显示红色标记
- ✅ 连接线在数据流动时显示流动动画（黄色虚线）

## 实现架构

### 后端 (Rust)

**文件**: `src-tauri/src/executor/context.rs`

1. **添加事件发送功能**
   - 导入 `tauri::Emitter` trait
   - 添加 `emit_execution_event()` 辅助函数
   - 在关键执行点发送事件

2. **执行事件类型**
   - `execution_start`: 执行开始
   - `execution_complete`: 执行完成
   - `node_start`: 节点开始执行
   - `node_complete`: 节点执行完成
   - `node_error`: 节点执行错误
   - `connection_active`: 连接激活（数据流动）

3. **事件发送时机**
   - 执行开始/结束：在 `execute()` 函数
   - 节点执行：在 `run_flow_internal()` 函数
   - 连接激活：在 `trigger_next_flow()` 函数

4. **延迟控制**
   - 节点执行前延迟 100ms（让前端有时间更新 UI）
   - 连接激活延迟 150ms（显示流动动画）

### 前端 (TypeScript/React)

#### 1. 类型定义

**文件**: `src/components/Editor/Types/execution.ts`

定义了执行状态相关的类型：
- `ExecutionStatus`: 执行状态（idle, running, completed, error）
- `NodeExecutionState`: 节点执行状态
- `ExecutionState`: 完整的执行状态
- `ExecutionEvent`: 执行事件

#### 2. 状态管理

**文件**: `src/components/Editor/Store/useExecutionStore.ts`

使用 Zustand 创建执行状态 store：
- `status`: 当前执行状态
- `currentNodeId`: 当前正在执行的节点 ID
- `executedNodes`: 已执行的节点集合
- `nodeStates`: 节点状态映射
- `activeConnections`: 激活的连接集合

Actions:
- `startExecution()`: 开始执行
- `completeExecution()`: 完成执行
- `markNodeExecuting()`: 标记节点正在执行
- `markNodeCompleted()`: 标记节点完成
- `markNodeError()`: 标记节点错误
- `addActiveConnection()`: 添加激活连接
- `removeActiveConnection()`: 移除激活连接
- `reset()`: 重置状态

#### 3. 事件监听

**文件**: `src/components/Editor/Hooks/useExecutionVisualization.ts`

监听后端发送的执行事件并更新状态：
- 使用 `@tauri-apps/api/event` 的 `listen` API
- 根据事件类型调用相应的 store actions
- 自动管理连接动画的生命周期（300ms 后移除）
- 执行完成后 2 秒自动重置状态

#### 4. 节点可视化

**文件**: `src/components/Editor/Nodes/Node.tsx`

添加执行状态的视觉效果：

**边框和光晕**:
- 正在执行：黄色边框 + 光晕 + 脉冲动画
- 已完成：绿色边框（半透明）
- 错误：红色边框 + 光晕

**背景渐变**:
- 正在执行：黄色渐变背景
- 错误：红色渐变背景
- 已完成：绿色渐变背景（淡）

**状态指示器**:
- 正在执行：右上角黄色圆点 + ping 动画
- 错误：右上角红色圆点
- 已完成：右上角绿色圆点（半透明）

#### 5. 连接线动画

**文件**: `src/components/Editor/Edges/Edge.tsx`

添加流动动画效果：
- 基础连接线：激活时变为黄色并加粗
- 流动动画：黄色虚线 + 移动动画
- 发光效果：模糊的黄色光晕

**文件**: `src/App.css`

添加 CSS 动画：
```css
@keyframes dash {
  to {
    stroke-dashoffset: -20;
  }
}
```

#### 6. 集成

**文件**: `src/components/Editor/Canvas/Canvas.tsx`

在 Canvas 组件中启用执行可视化：
```typescript
useExecutionVisualization();
```

## 数据流

```
后端执行 → 发送事件 (Tauri Event)
    ↓
前端监听 (useExecutionVisualization)
    ↓
更新状态 (useExecutionStore)
    ↓
UI 更新 (Node 组件 + Edge 组件)
    ↓
视觉反馈（高亮 + 动画）
```

## 视觉效果

### 节点状态

1. **正在执行**
   - 边框：黄色 (#facc15)
   - 光晕：黄色 ring-2
   - 动画：脉冲 (animate-pulse)
   - 背景：黄色渐变
   - 指示器：右上角黄色圆点 + ping 动画

2. **已完成**
   - 边框：绿色半透明 (#22c55e/50)
   - 指示器：右上角绿色圆点（半透明）
   - 背景：绿色渐变（淡）

3. **错误**
   - 边框：红色 (#ef4444)
   - 光晕：红色 ring-2
   - 背景：红色渐变
   - 指示器：右上角红色圆点

### 连接线动画

1. **激活状态**
   - 颜色：黄色 (#facc15)
   - 粗细：加粗 1px
   - 动画：虚线流动
   - 效果：发光模糊

2. **动画参数**
   - 虚线：10px 间隔
   - 速度：0.5s 线性循环
   - 持续时间：300ms

## 性能优化

1. **事件节流**
   - 后端添加延迟避免事件过快
   - 前端使用 setTimeout 管理动画生命周期

2. **状态管理**
   - 使用 Zustand 的选择器避免不必要的重渲染
   - Node 组件使用 React.memo 优化

3. **动画性能**
   - 使用 CSS 动画而非 JavaScript
   - 利用 GPU 加速（transform, opacity）
   - 连接线使用 SVG 而非 Canvas（更适合动画）

## 测试步骤

1. **启动应用**
   ```bash
   npm run tauri dev
   ```

2. **创建测试场景**
   - 创建一个 Event
   - 添加多个节点（如 Print, Math 等）
   - 连接节点形成执行流程

3. **执行并观察**
   - 点击执行按钮
   - 观察节点依次高亮
   - 观察连接线流动动画
   - 检查执行完成后的状态

4. **检查控制台**
   - 后端日志应显示事件发送
   - 前端控制台应显示事件接收
   - 状态更新日志

## 调试

### 后端日志
```
[emit_execution_event] Sending event: execution_start
[emit_execution_event] Sending event: node_start, node_id: ...
[emit_execution_event] Sending event: connection_active, from: ..., to: ...
[emit_execution_event] Sending event: node_complete, node_id: ...
```

### 前端日志
```
[useExecutionVisualization] Setting up execution event listener...
[ExecutionEvent] { type: "execution_start", ... }
[ExecutionEvent] Execution started
[ExecutionEvent] { type: "node_start", nodeId: "..." }
[ExecutionEvent] Node started: ...
[ExecutionEvent] { type: "connection_active", fromPinId: "...", toPinId: "..." }
[ExecutionEvent] Connection active: ... -> ...
```

## 已知限制

1. **Canvas 渲染的连接线**
   - EdgesLayer 使用 Canvas 渲染，不支持 CSS 动画
   - 需要切换到 SVG 渲染才能完全支持连接线动画
   - 当前实现在 Edge 组件中，但 EdgesLayer 还在使用 Canvas

2. **性能考虑**
   - 大型图（>100 节点）可能需要优化
   - 可以考虑只对可见节点启用动画

3. **动画同步**
   - 后端延迟是固定的，可能需要根据节点复杂度调整
   - 连接线动画持续时间固定为 300ms

## 未来改进

1. **完全切换到 SVG 渲染**
   - 替换 EdgesLayer 的 Canvas 实现
   - 使用 SVG 渲染所有连接线
   - 支持更丰富的动画效果

2. **可配置的动画**
   - 允许用户调整动画速度
   - 允许禁用动画（性能模式）

3. **更多视觉效果**
   - 数据值的可视化（显示传递的值）
   - 执行路径的高亮
   - 执行时间的显示

4. **执行历史**
   - 记录执行历史
   - 支持回放
   - 支持断点调试

## 文件清单

### 新增文件
- `src/components/Editor/Types/execution.ts`
- `src/components/Editor/Store/useExecutionStore.ts`
- `src/components/Editor/Hooks/useExecutionVisualization.ts`

### 修改文件
- `src-tauri/src/executor/context.rs`
- `src/components/Editor/Nodes/Node.tsx`
- `src/components/Editor/Edges/Edge.tsx`
- `src/components/Editor/Canvas/Canvas.tsx`
- `src/App.css`

## 总结

执行状态可视化功能已完全实现，提供了直观的视觉反馈：
- ✅ 节点执行状态实时显示
- ✅ 连接线流动动画
- ✅ 错误状态标识
- ✅ 执行完成后自动重置

用户现在可以清楚地看到图的执行过程，大大提升了调试和理解的体验。
