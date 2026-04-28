# EventCreated 超时问题分析

## 现象

创建 Event/Function 时偶尔出现：
```
[useGraphManagement] Timeout waiting for EventCreated event for <uuid>
```

## 流程回顾

1. 用户点击 "Add Event" → `addEvent()` 调用 `GraphService.createEvent()`
2. 后端 `create_event` 创建 graph，emit `EventCreated`，返回 ID
3. 前端注册 `pendingActionsRef[id]`，等待 `handleEventCreated(id, data)` 回调
4. `ProjectListener` 监听 `project-event`，收到后由 `EventCreatedHandler` 处理
5. Handler 更新 Store 并调用 `onEventCreated` → `handleEventCreated` 执行 callback（打开 Tab）

## 可能原因

### 1. 监听器就绪竞态（最可能）

- `useProjectSyncCore` 的 `setup()` 是异步且不阻塞渲染
- `initProjectSync` 与 listener 的 `listen()` 并行执行
- 若 `syncFromBackend` 先完成，用户会看到 UI 并可点击 "Add Event"
- 此时 listener 若尚未完成 `listen()`，事件会丢失
- 在慢速环境或初始化较重时更容易复现

### 2. 窗口切换导致 listener 重建

- 从 EditorWindow 切到 DataViewWindow 时，`decrementRef` 会停止并销毁 listener
- 再切回 EditorWindow 时创建新的 listener
- 若在切换前刚点了 Add Event，pending action 会随旧 listener 一起失效
- 新 listener 收到事件时，callbacks 可能尚未正确绑定

### 3. 事件在 Tauri 层丢失

- 若 emit 时没有活跃的 listener，事件可能不会被投递
- 概率较低，但无法完全排除

## 修复策略

**不依赖事件的主流程**：创建成功后立即用 `get_graph` 拉取数据并打开 Tab，事件仅作为补充同步，超时作为兜底。

这样即使 EventCreated 未到达，用户也能正常打开新创建的 graph。
