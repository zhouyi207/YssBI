# Graph 创建流程分析

## 概述

创建一个 Graph（Event/Function/Macro）的完整流程涉及前端、后端和事件系统的协作。整个流程采用**命令-事件分离（CQRS）**模式。

---

## 完整流程图

```
┌─────────────────────────────────────────────────────────────────────┐
│                           前端 (Frontend)                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                       │
│  1. 用户操作                                                          │
│     ├─ 点击 "Add Event" 按钮                                         │
│     └─ 触发 editor.addEvent()                                        │
│                                                                       │
│  2. useGraphManagement Hook                                          │
│     ├─ 生成唯一名称: getUniqueName("New Event", existingEvents)     │
│     ├─ 注册待处理操作到 pendingActionsRef                            │
│     │   └─ 存储回调: () => openGraph(id, name, type, data)          │
│     ├─ 调用 GraphService.createEvent(finalName)                     │
│     └─ 切换侧边栏到 'events' 标签                                    │
│                                                                       │
│  3. GraphService (Service Layer)                                     │
│     ├─ 封装后端调用                                                  │
│     └─ invoke("create_event", { graphName })                         │
│                                                                       │
└───────────────────────┬─────────────────────────────────────────────┘
                        │ Tauri IPC
                        ▼
┌─────────────────────────────────────────────────────────────────────┐
│                           后端 (Backend)                             │
├─────────────────────────────────────────────────────────────────────┤
│                                                                       │
│  4. Command Handler                                                  │
│     ├─ create_event(app, state, graph_name)                         │
│     ├─ state.add_event(graph_name)                                  │
│     │   ├─ 生成唯一 GraphId                                          │
│     │   ├─ 创建 GraphInstance                                        │
│     │   │   ├─ 初始化空的 nodes                                      │
│     │   │   ├─ 初始化空的 pins                                       │
│     │   │   ├─ 初始化空的 connections                                │
│     │   │   └─ 设置默认 position                                     │
│     │   └─ 存储到 ProjectState.graphs                               │
│     └─ emit_project_event(Event::EventCreated { id, data })         │
│                                                                       │
└───────────────────────┬─────────────────────────────────────────────┘
                        │ Tauri Event
                        ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      事件系统 (Event System)                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                       │
│  5. Event Emission                                                   │
│     ├─ 事件类型: "project-event"                                     │
│     └─ Payload: {                                                    │
│           type: "Event",                                             │
│           payload: {                                                 │
│               type: "EventCreated",                                  │
│               payload: { id: "...", data: {...} }                    │
│           }                                                           │
│        }                                                              │
│                                                                       │
└───────────────────────┬─────────────────────────────────────────────┘
                        │ Event Broadcast
                        ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    前端同步层 (Sync Layer)                           │
├─────────────────────────────────────────────────────────────────────┤
│                                                                       │
│  6. ProjectListener                                                  │
│     ├─ listen('project-event', callback)                            │
│     └─ 接收事件 payload                                              │
│                                                                       │
│  7. EventRegistry                                                    │
│     ├─ parseEvent(payload)                                           │
│     │   └─ 提取: type = "EventCreated", payload = { id, data }      │
│     └─ dispatch(event)                                               │
│                                                                       │
│  8. EventCreatedHandler                                              │
│     ├─ handle(payload, callbacks)                                    │
│     ├─ projectStore.addGraph(id, data)                              │
│     │   └─ 更新 Zustand Store: graphs[id] = data                    │
│     └─ callbacks?.onEventCreated?.(id, data)                        │
│                                                                       │
└───────────────────────┬─────────────────────────────────────────────┘
                        │ Callback
                        ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    业务逻辑层 (Business Logic)                       │
├─────────────────────────────────────────────────────────────────────┤
│                                                                       │
│  9. useGraphManagement.handleEventCreated                            │
│     ├─ 从 pendingActionsRef 中查找对应的回调                         │
│     ├─ 执行回调: action()                                            │
│     │   ├─ 从 projectStore 中查找新创建的 graph                      │
│     │   └─ openGraph(id, name, type, data)                          │
│     │       └─ 在编辑器中打开新的 tab                                │
│     └─ 清除待处理操作: pendingActions.delete(name)                   │
│                                                                       │
└───────────────────────┬─────────────────────────────────────────────┘
                        │ UI Update
                        ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         UI 层 (UI Layer)                             │
├─────────────────────────────────────────────────────────────────────┤
│                                                                       │
│  10. React 组件自动重渲染                                            │
│      ├─ Sidebar: 显示新的 Event 在列表中                            │
│      ├─ Editor: 打开新的 tab 显示 Event 编辑器                       │
│      └─ Canvas: 渲染空的画布（无 nodes）                             │
│                                                                       │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 详细步骤说明

### 步骤 1: 用户触发操作

**位置**: UI 组件（如 Sidebar）

```typescript
// 用户点击 "Add Event" 按钮
<button onClick={() => editor.addEvent()}>
    Add Event
</button>
```

---

### 步骤 2: 前端业务逻辑处理

**位置**: `src/features/application/editor/core/hooks/useGraphManagement.ts`

```typescript
const addEvent = useCallback(async (name?: string) => {
    // 2.1 获取当前所有 events
    const store = useProjectStore.getState();
    const events = Object.values(store.graphs).filter(g => g.type === 'event');
    
    // 2.2 生成唯一名称
    const finalName = getUniqueName(name || "New Event", events);
    // 例如: "New Event", "New Event 1", "New Event 2"
    
    // 2.3 注册待处理操作（重要！）
    // 当后端事件到达时，这个回调会被执行
    pendingActionsRef.current.events.set(finalName, () => {
        const updatedStore = useProjectStore.getState();
        const newEvent = Object.entries(updatedStore.graphs).find(
            ([_, graph]) => graph.type === 'event' && graph.name === finalName
        );
        
        if (newEvent) {
            const [id, event] = newEvent;
            openGraph(id, event.name, "event", event); // 打开编辑器
        }
    });
    
    // 2.4 调用后端 API
    await GraphService.createEvent(finalName);
    
    // 2.5 切换侧边栏标签
    switchSidebarTab('events');
}, [openGraph, switchSidebarTab]);
```

**关键点**:
- 使用 `pendingActionsRef` 存储待处理操作
- 通过 `name` 作为 key 关联前端请求和后端事件
- 不直接更新 Store，等待后端事件

---

### 步骤 3: Service 层封装

**位置**: `src/services/graph/graphService.ts`

```typescript
static async createEvent(graphName: string): Promise<void> {
    try {
        // 调用 Tauri 命令
        await invoke("create_event", { graphName });
        console.log(`Event '${graphName}' created successfully`);
    } catch (error) {
        console.error("Error creating event:", error);
        throw error;
    }
}
```

**职责**:
- 封装 Tauri IPC 调用
- 处理错误
- 不包含业务逻辑

---

### 步骤 4: 后端命令处理

**位置**: `src-tauri/src/commands/command_graph/command_graph.rs`

```rust
#[tauri::command]
pub fn create_event(
    app: AppHandle,
    state: State<ProjectState>,
    graph_name: &str,
) -> Result<(), String> {
    // 4.1 创建 Graph 实例
    let graph = state.add_event(graph_name);
    // 内部逻辑:
    // - 生成唯一 GraphId (UUID)
    // - 创建 GraphInstance
    //   - nodes: Vec::new()
    //   - pins: Vec::new()
    //   - connections: ConnectionGraph::new()
    //   - position: GraphPosition::default()
    // - 存储到 ProjectState.graphs
    
    // 4.2 发送事件
    emit_project_event(
        &app,
        Event::Event(EventEvent::EventCreated {
            id: graph.id,
            data: (&graph).into(), // 转换为 DTO
        }),
    );
    
    Ok(())
}
```

**关键点**:
- 后端负责生成 ID
- 后端创建完整的数据结构
- 立即发送事件通知前端

---

### 步骤 5: 事件发送

**位置**: 后端事件系统

```rust
// 事件结构
Event::Event(EventEvent::EventCreated {
    id: GraphId,
    data: GraphDTO,
})

// 序列化为 JSON
{
    "type": "Event",
    "payload": {
        "type": "EventCreated",
        "payload": {
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "data": {
                "id": "550e8400-e29b-41d4-a716-446655440000",
                "name": "New Event",
                "kind": "Event",
                "nodes": [],
                "pins": [],
                "connections": { "connections": {}, "reverse_connections": {} },
                "position": { "x": 0, "y": 0, "scale": 1 }
            }
        }
    }
}
```

---

### 步骤 6-8: 前端同步层处理

**位置**: `src/features/core/sync/`

#### 6. ProjectListener 接收事件

```typescript
// src/features/core/sync/listeners/ProjectListener.ts
this.unlisten = await listen('project-event', (event) => {
    console.log('[ProjectListener] Received event:', event.payload);
    this.registry.dispatch(event.payload);
});
```

#### 7. EventRegistry 分发事件

```typescript
// src/features/core/sync/registry/EventRegistry.ts
dispatch(event: any): void {
    // 解析嵌套事件
    const parsed = parseEvent(event);
    // parsed.type = "EventCreated"
    // parsed.payload = { id, data }
    
    // 查找对应的处理器
    const handler = this.handlers.get(parsed.type);
    
    // 执行处理器
    handler.handle(parsed.payload, this.callbacks);
}
```

#### 8. EventCreatedHandler 处理事件

```typescript
// src/features/core/sync/handlers/GraphEventHandler.ts
export class EventCreatedHandler extends BaseEventHandler {
    eventType = 'EventCreated';
    
    handle(payload: GraphCreatedPayload, callbacks?: EventCallbacks): void {
        this.log('Event created:', payload.id);
        
        // 8.1 更新 Store
        const projectStore = useProjectStore.getState();
        projectStore.addGraph(payload.id, payload.data);
        
        // 8.2 触发回调
        callbacks?.onEventCreated?.(payload.id, payload.data);
    }
}
```

---

### 步骤 9: 执行待处理操作

**位置**: `src/features/application/editor/core/hooks/useGraphManagement.ts`

```typescript
const handleEventCreated = useCallback((id: string, data: any) => {
    console.log("handleEventCreated:", id, data);
    
    // 9.1 查找待处理操作
    const action = pendingActionsRef.current.events.get(data.name);
    
    if (action) {
        // 9.2 执行操作（打开编辑器）
        action();
        // 内部会调用: openGraph(id, name, "event", data)
        
        // 9.3 清除待处理操作
        pendingActionsRef.current.events.delete(data.name);
    }
}, []);
```

---

### 步骤 10: UI 自动更新

**React 自动重渲染机制**:

```typescript
// Sidebar 组件
const graphs = useProjectStore(state => state.graphs);
const events = Object.values(graphs).filter(g => g.type === 'event');

// 当 projectStore.graphs 更新时，组件自动重渲染
return (
    <div>
        {events.map(event => (
            <EventItem key={event.id} event={event} />
        ))}
    </div>
);
```

---

## 关键设计模式

### 1. 命令-事件分离（CQRS）

```
命令流: UI → Service → Backend → 执行操作
事件流: Backend → Event → Sync Layer → Store → UI
```

**优点**:
- 前后端解耦
- 状态一致性
- 支持多窗口同步

### 2. 待处理操作（Pending Actions）

```typescript
pendingActionsRef.current.events.set(name, callback);
```

**作用**:
- 关联前端请求和后端事件
- 延迟执行 UI 操作（如打开编辑器）
- 处理异步创建流程

### 3. 单一数据源（Single Source of Truth）

```
后端 ProjectState → 事件 → 前端 ProjectStore → UI
```

**保证**:
- 后端是唯一的数据源
- 前端通过事件同步
- 避免状态不一致

---

## 时序图

```
用户          UI          GraphManagement    GraphService    Backend       EventSystem    SyncLayer      Store         UI
 │             │                 │                 │              │               │             │            │            │
 │  点击按钮   │                 │                 │              │               │             │            │            │
 ├────────────>│                 │                 │              │               │             │            │            │
 │             │  addEvent()     │                 │              │               │             │            │            │
 │             ├────────────────>│                 │              │               │             │            │            │
 │             │                 │ 生成唯一名称    │              │               │             │            │            │
 │             │                 │ 注册待处理操作  │              │               │             │            │            │
 │             │                 │                 │              │               │             │            │            │
 │             │                 │ createEvent()   │              │               │             │            │            │
 │             │                 ├────────────────>│              │               │             │            │            │
 │             │                 │                 │ invoke()     │               │             │            │            │
 │             │                 │                 ├─────────────>│               │             │            │            │
 │             │                 │                 │              │ add_event()   │             │            │            │
 │             │                 │                 │              │ emit_event()  │             │            │            │
 │             │                 │                 │              ├──────────────>│             │            │            │
 │             │                 │                 │              │               │ dispatch()  │            │            │
 │             │                 │                 │              │               ├────────────>│            │            │
 │             │                 │                 │              │               │             │ addGraph() │            │
 │             │                 │                 │              │               │             ├───────────>│            │
 │             │                 │                 │              │               │             │            │ 触发更新   │
 │             │                 │                 │              │               │             │            ├───────────>│
 │             │                 │                 │              │               │             │            │            │
 │             │                 │                 │              │               │ callback    │            │            │
 │             │                 │ handleEventCreated()           │               ├────────────>│            │            │
 │             │                 │<───────────────────────────────────────────────┘             │            │            │
 │             │                 │ 执行待处理操作  │              │               │             │            │            │
 │             │                 │ openGraph()     │              │               │             │            │            │
 │             │<────────────────┤                 │              │               │             │            │            │
 │             │ 打开编辑器      │                 │              │               │             │            │            │
 │<────────────┤                 │                 │              │               │             │            │            │
```

---

## 错误处理

### 前端错误

```typescript
try {
    await GraphService.createEvent(finalName);
} catch (error) {
    console.error("Failed to create event:", error);
    // 清除待处理操作
    pendingActionsRef.current.events.delete(finalName);
    throw error;
}
```

### 后端错误

```rust
pub fn create_event(...) -> Result<(), String> {
    // 如果失败，返回错误
    // 前端会捕获并处理
    Ok(())
}
```

---

## 性能优化

1. **异步非阻塞**: 前端发送命令后立即返回，不等待后端完成
2. **事件批处理**: 多个操作可以合并为一个事件
3. **增量更新**: 只更新变化的部分，不重新加载整个项目
4. **单例监听器**: 全局只有一个事件监听器，避免重复

---

## 扩展性

### 添加新的 Graph 类型

1. 后端添加命令: `create_xxx`
2. 后端添加事件: `XxxCreated`
3. 前端添加 Handler: `XxxCreatedHandler`
4. 前端添加业务逻辑: `addXxx`, `handleXxxCreated`

### 添加新的操作

1. 定义命令: `update_graph`, `delete_graph`
2. 定义事件: `GraphUpdated`, `GraphDeleted`
3. 添加处理器: `GraphUpdatedHandler`, `GraphDeletedHandler`
4. 更新业务逻辑

---

## 总结

创建 Graph 的流程体现了以下架构原则：

1. **关注点分离**: UI、业务逻辑、数据同步、状态管理各司其职
2. **单向数据流**: 命令向下，事件向上
3. **事件驱动**: 通过事件实现解耦和同步
4. **异步处理**: 不阻塞 UI，提升用户体验
5. **可扩展性**: 易于添加新功能和新类型

这种架构确保了前后端的一致性，支持多窗口同步，并且易于维护和扩展。
