# 测试执行可视化功能

## 快速测试步骤

### 1. 启动应用
```bash
npm run tauri dev
```

### 2. 创建测试场景

1. **创建一个新的 Event**
   - 点击 "New Event" 或打开现有 Event

2. **添加节点**
   - 从节点面板拖拽以下节点到画布：
     - Event On Run（自动创建）
     - Print 节点 x 3
     - Math Add 节点 x 1

3. **连接节点**
   ```
   Event On Run (Exec) 
       → Print 1 (In)
       → Print 2 (In)  
       → Math Add (In)
       → Print 3 (In)
   ```

4. **设置值**
   - 在 Print 节点的 Value pin 上设置不同的值
   - 例如："Step 1", "Step 2", "Step 3"

### 3. 执行并观察

1. **点击执行按钮**（播放图标）

2. **观察效果**
   - ✅ Event On Run 节点首先高亮（黄色边框 + 脉冲）
   - ✅ 连接线从 Event 到 Print 1 显示流动动画（黄色虚线）
   - ✅ Print 1 节点高亮
   - ✅ Print 1 完成后显示绿色标记
   - ✅ 连接线流动到 Print 2
   - ✅ 依次执行所有节点
   - ✅ 执行完成后，所有节点显示绿色标记
   - ✅ 2 秒后状态自动重置

### 4. 测试错误状态

1. **创建一个会出错的场景**
   - 例如：除以零、类型不匹配等

2. **执行并观察**
   - ✅ 出错的节点显示红色边框和标记
   - ✅ 执行停止

## 预期视觉效果

### 节点状态

#### 正在执行
- 边框：黄色发光
- 动画：脉冲效果
- 背景：黄色渐变
- 指示器：右上角黄色圆点 + ping 动画

#### 已完成
- 边框：绿色（半透明）
- 背景：淡绿色渐变
- 指示器：右上角绿色圆点

#### 错误
- 边框：红色发光
- 背景：红色渐变
- 指示器：右上角红色圆点

### 连接线动画

#### 激活时
- 颜色：从灰色变为黄色
- 粗细：加粗
- 动画：虚线流动（从起点到终点）
- 效果：发光模糊

#### 持续时间
- 300ms（与后端延迟同步）

## 控制台日志检查

### 前端控制台（F12）

应该看到：
```
[useExecutionVisualization] Setting up execution event listener...
[ExecutionEvent] { type: "execution_start", timestamp: ... }
[ExecutionEvent] Execution started
[ExecutionEvent] { type: "node_start", nodeId: "node-xxx", timestamp: ... }
[ExecutionEvent] Node started: node-xxx
[ExecutionEvent] { type: "connection_active", fromPinId: "pin-xxx", toPinId: "pin-yyy", timestamp: ... }
[ExecutionEvent] Connection active: pin-xxx -> pin-yyy
[ExecutionEvent] { type: "node_complete", nodeId: "node-xxx", timestamp: ... }
[ExecutionEvent] Node completed: node-xxx
...
[ExecutionEvent] { type: "execution_complete", timestamp: ... }
[ExecutionEvent] Execution completed
```

### 后端控制台

应该看到：
```
[INFO] Starting execution from node: ...
[INFO] >>> Executing Node: Event On Run (event_on_run)
[INFO]   -> Node returned next exec: 'Exec'
[INFO] [trigger_next_flow] Looking for pin 'Exec' in node 'Event On Run'
[INFO] [trigger_next_flow] Found pin 'Exec'
[INFO] >>> Executing Node: Print (print)
[INFO] [Print] Step 1
[INFO]   -> Node returned next exec: 'Out'
...
[INFO] Execution finished
```

## 故障排除

### 节点不高亮

1. **检查事件监听器**
   - 打开浏览器控制台
   - 确认看到 `[useExecutionVisualization] Setting up execution event listener...`

2. **检查后端事件发送**
   - 查看后端控制台
   - 确认没有 "Failed to emit execution event" 错误

3. **检查 Tauri 事件系统**
   - 确认 Tauri 版本支持事件系统
   - 检查是否有权限问题

### 连接线不流动

1. **检查 Edge 组件**
   - 确认 `isActive` 属性被正确传递
   - 检查 CSS 动画是否加载

2. **检查 EdgesLayer**
   - 当前 EdgesLayer 使用 Canvas 渲染
   - 连接线动画可能不会显示（已知限制）
   - 需要切换到 SVG 渲染

### 动画太快或太慢

1. **调整后端延迟**
   - 修改 `context.rs` 中的 `std::thread::sleep` 时间
   - 节点执行前：100ms
   - 连接激活：150ms

2. **调整前端动画**
   - 修改 `App.css` 中的动画持续时间
   - 修改 `useExecutionVisualization.ts` 中的 setTimeout 时间

### 状态不重置

1. **检查自动重置**
   - 执行完成后应该 2 秒后自动重置
   - 检查 `useExecutionVisualization.ts` 中的 setTimeout

2. **手动重置**
   - 刷新页面
   - 或在控制台执行：
     ```javascript
     useExecutionStore.getState().reset()
     ```

## 性能测试

### 小型图（< 10 节点）
- ✅ 应该流畅无卡顿
- ✅ 动画应该平滑

### 中型图（10-50 节点）
- ✅ 应该基本流畅
- ⚠️ 可能有轻微延迟

### 大型图（> 50 节点）
- ⚠️ 可能需要优化
- 💡 考虑禁用动画或只对可见节点启用

## 成功标准

✅ 节点按执行顺序依次高亮
✅ 连接线显示流动动画
✅ 执行完成后节点显示绿色标记
✅ 错误节点显示红色标记
✅ 状态在执行完成后自动重置
✅ 控制台日志正确显示事件
✅ 动画流畅无卡顿
✅ 视觉效果清晰易懂

## 已知问题

1. **EdgesLayer 使用 Canvas**
   - 连接线动画可能不显示
   - 需要切换到 SVG 渲染

2. **大型图性能**
   - 可能需要优化
   - 考虑添加性能模式

3. **动画同步**
   - 后端延迟是固定的
   - 可能需要根据节点复杂度调整
