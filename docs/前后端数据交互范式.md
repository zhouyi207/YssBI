# 前后端数据交互范式

## 核心架构模式

应用采用 **命令-事件分离（CQRS-like）** 模式：

### 1. 命令流（Command Flow）- 查询类操作
**特征：不改变项目状态，直接返回数据**

```
前端 UI/Hook 
  → Service Layer (invoke) 
  → 后端 Command Handler 
  → 直接返回数据 
  → 前端处理响应
```

**适用场景：**
- 数据查询（getProjectState, getNodes, getConnections）
- Schema 获取（getNodeDefinition）
- 项目路径查询（getProjectPath）

**范式规则：**
- Service 方法使用 `async/await` 直接返回数据
- 不触发任何事件
- 前端直接使用返回值更新 UI 或临时状态
- 不更新全局 Store

**示例：**
```typescript
// Service Layer
static async getProjectState(): Promise<ProjectData> {
    const data = await invoke("get_project_data");
    return convertData(data);
}

// 使用方
const projectData = await ProjectService.getProjectState();
// 直接使用 projectData，不触发状态更新
```

---

### 2. 事件流（Event Flow）- 修改类操作
**特征：改变项目状态，通过事件推送更新**

```
前端 UI/Hook 
  → Service Layer (invoke) 
  → 后端 Command Handler 
  → 修改状态 
  → emit Event 
  → 前端 Event Listener (useProjectSync) 
  → 更新 Store 
  → UI 自动响应
```

**适用场景：**
- 项目操作（newProject, loadProject, saveProject）
- Graph 创建/删除（createEvent, createFunction, removeGraph）
- Node 创建/删除（createNode, deleteNode）
- Connection 创建/删除（createConnection, deleteConnection）
- Variable 创建/更新/删除

**范式规则：**
- Service 方法调用后端，通常返回 `void` 或简单确认
- 后端执行操作后 emit 事件（如 `EventCreated`, `NodeDeleted`）
- 前端通过 `useProjectSync` 全局监听事件
- 事件处理器更新 Zustand Store
- React 组件通过 Store 订阅自动重渲染

**示例：**
```typescript
// Service Layer - 只负责发送命令
static async createEvent(graphName: string): Promise<void> {
    await invoke("create_event", { graphName });
    // 不返回数据，等待事件
}

// Event Listener - 全局单例监听
useEffect(() => {
    const unlisten = await listen('project-event', (event) => {
        switch (event.payload.type) {
            case 'EventCreated':
                projectStore.addGraph(payload.id, payload.data);
                break;
        }
    });
}, []);

// UI Component - 通过 Store 订阅
const graphs = useProjectStore(state => state.graphs);
```

---

## 分层架构

### Layer 1: Service Layer（服务层）
**职责：封装后端 API 调用**

**规范：**
- 使用 `invoke()` 调用 Tauri 命令
- 只负责数据转换（DTO → Domain）
- 不包含业务逻辑
- 不直接操作 Store
- 命名：`XxxService.methodName()`

**文件位置：**
```
src/services/
  ├── project/projectService.ts
  ├── graph/graphService.ts
  ├── graph/node/nodeService.ts
  ├── graph/connection/connectionService.ts
  ├── schema/SchemaService.ts
  └── executor/executorService.ts
```

---

### Layer 2: Store Layer（状态层）
**职责：管理全局应用状态**

**规范：**
- 使用 Zustand 创建 Store
- 提供状态 + 操作方法
- 只负责状态管理，不调用后端
- 命名：`useXxxStore`

**Store 类型：**

1. **Project Store（项目级状态）**
   - 管理：variables, graphs, databases
   - 同步：通过事件监听器自动更新
   - 生命周期：应用全局

2. **Node Store（编辑器状态）**
   - 管理：每个 tab 的 nodes 和 variables
   - 同步：手动管理，支持 undo/redo
   - 生命周期：编辑器会话

3. **UI Store（界面状态）**
   - 管理：选中状态、模态框、Toast
   - 同步：纯前端状态
   - 生命周期：UI 交互

**文件位置：**
```
src/features/core/
  ├── dataStore/          # 项目数据（projectIO, graphMeta, graphData, variable, database）
  ├── editor/             # 编辑器状态（useEditorStore, useClipboardStore）
  ├── layout/layoutStore  # 布局状态
  ├── ui/UIStore.ts
  ├── execution/          # useExecutionStore
  └── settings/settingsStore.ts
```

---

### Layer 3: Sync Layer（同步层）
**职责：监听后端事件，同步状态到 Store**

**规范：**
- 使用 `listen()` 监听 Tauri 事件
- 全局单例模式（防止重复监听）
- 解析事件类型，调用 Store 方法
- 支持可选回调（onEventCreated 等）

**核心实现：**
```typescript
// Core: ProjectListener + EventRegistry + Handlers（直接更新 Store）
// Handlers 负责更新 Store，callbacks 为可选的 UI 扩展（如打开新 Tab）

// Application: useProjectSync 协调监听器与 useEditor 回调
export function useProjectSync() {
  const editor = useEditor();
  const callbacks = useMemo(() => ({
    onNodeCreated: editor.handleNodeCreated,
    onNodeDeleted: editor.handleNodeDeleted,
    // ...
  }), [editor.handleNodeCreated, ...]);

  useEffect(() => {
    const listener = await SingletonManager.getInstance('project-listener', async () => {
      const l = new ProjectListener(callbacks);
      await l.start();
      return l;
    });
    return () => SingletonManager.decrementRef('project-listener', l => l.stop());
  }, []);
}
```

**文件位置：**
```
src/features/core/sync/           # ProjectListener、EventRegistry、handlers（更新 Store）
src/features/application/initialization/useProjectSync.ts   # 协调监听器与编辑器回调
src/features/domain/execution/hooks/useExecutionVisualization.ts  # 执行事件监听
```

---

### Layer 4: Hook Layer（业务逻辑层）
**职责：组合 Service + Store，提供业务逻辑**

**规范：**
- 组合多个 Service 调用
- 处理错误和加载状态
- 协调多个 Store 更新
- 命名：`useXxx`

**示例：**
```typescript
export function useProjectOperations() {
    const projectStore = useProjectStore();
    
    const createNewEvent = async (name: string) => {
        try {
            // 1. 调用 Service
            await GraphService.createEvent(name);
            // 2. 等待事件自动更新 Store
            // 3. UI 自动响应
        } catch (error) {
            // 错误处理
        }
    };
    
    return { createNewEvent };
}
```

---

## 事件命名规范

### 后端事件结构
```rust
// 嵌套事件结构
Event::Event(EventEvent::EventCreated { id, data })

// 序列化为 JSON
{
    "type": "Event",
    "payload": {
        "type": "EventCreated",
        "payload": { "id": "...", "data": {...} }
    }
}
```

### 前端事件解析
```typescript
// 提取实际事件类型
const eventType = eventData.type; // "Event"
const eventPayload = eventData.payload;

let type: string;
let payload: any;

if (eventPayload && 'type' in eventPayload) {
    // 嵌套事件
    type = eventPayload.type; // "EventCreated"
    payload = eventPayload.payload; // { id, data }
} else {
    // 直接事件
    type = eventType;
    payload = eventPayload;
}
```

### 事件类型清单

**项目级事件：**
- `ProjectLoaded` - 项目加载完成
- `ProjectCleared` - 项目清空
- `ProjectSaved` - 项目保存完成

**Graph 事件：**
- `EventCreated` / `EventUpdated` / `EventDeleted`
- `FunctionCreated` / `FunctionUpdated` / `FunctionDeleted`
- `MacroCreated` / `MacroUpdated` / `MacroDeleted`

**Variable 事件：**
- `GlobalVariableCreated` / `GlobalVariableUpdated` / `GlobalVariableDeleted`

**DataFrame 事件：**
- `DataFrameCreated` / `DataFrameDeleted`

**执行事件：**
- `execution_start` / `execution_complete`
- `node_start` / `node_complete` / `node_error`
- `connection_active`

---

## 最佳实践

### ✅ DO（推荐做法）

1. **查询操作直接返回**
   ```typescript
   const data = await Service.getData();
   setLocalState(data);
   ```

2. **修改操作等待事件**
   ```typescript
   await Service.createItem(name);
   // Store 会自动更新，无需手动处理
   ```

3. **全局状态用 Store**
   ```typescript
   const items = useGraphMetaStore(state => state.graphs);
   const nodes = useGraphDataStore(state => state.nodes);
   ```

4. **局部状态用 useState**
   ```typescript
   const [isOpen, setIsOpen] = useState(false);
   ```

5. **事件监听全局单例**
   ```typescript
   let globalUnlisten: (() => void) | null = null;
   ```

### ❌ DON'T（避免做法）

1. **不要在 Service 中操作 Store**
   ```typescript
   // ❌ 错误
   static async createEvent() {
       await invoke(...);
       useGraphMetaStore.getState().addGraph(...); // 不要这样
   }
   ```

2. **不要重复监听事件**
   ```typescript
   // ❌ 错误 - 每个组件都监听
   useEffect(() => {
       listen('project-event', ...);
   }, []);
   ```

3. **不要混淆命令和事件**
   ```typescript
   // ❌ 错误 - 查询操作不应该触发事件
   static async getNodes() {
       await invoke("get_nodes"); // 应该直接返回
   }
   ```

4. **不要在事件处理器中调用 Service**
   ```typescript
   // ❌ 错误 - 可能导致循环
   listen('project-event', async (event) => {
       await Service.updateData(...); // 不要这样
   });
   ```

---

## 数据流图示

```
┌─────────────────────────────────────────────────────────────┐
│                         前端应用                              │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐              │
│  │ UI Layer │───▶│  Hooks   │───▶│ Services │──┐           │
│  └──────────┘    └──────────┘    └──────────┘  │           │
│       ▲               │                          │           │
│       │               │                          ▼           │
│       │               │                    ┌──────────┐     │
│       │               └───────────────────▶│  invoke  │     │
│       │                                    └──────────┘     │
│       │                                          │           │
│  ┌──────────┐                                   │           │
│  │  Stores  │◀──────────────┐                   │           │
│  └──────────┘               │                   │           │
│       ▲                     │                   │           │
│       │                ┌──────────┐             │           │
│       │                │  Sync    │             │           │
│       │                │  Layer   │             │           │
│       │                └──────────┘             │           │
│       │                     ▲                   │           │
└───────┼─────────────────────┼───────────────────┼───────────┘
        │                     │                   │
        │                ┌──────────┐             │
        │                │  listen  │             │
        │                └──────────┘             │
        │                     ▲                   │
════════╪═════════════════════╪═══════════════════╪════════════
        │                     │                   ▼
┌───────┼─────────────────────┼───────────────────────────────┐
│       │                     │          后端                  │
│       │                     │                                │
│       │                ┌──────────┐      ┌──────────┐       │
│       │                │  emit    │◀─────│ Commands │       │
│       │                │  event   │      └──────────┘       │
│       │                └──────────┘            │            │
│       │                     ▲                  │            │
│       │                     │                  ▼            │
│       │                     │           ┌──────────┐       │
│       │                     └───────────│  State   │       │
│       │                                 └──────────┘       │
│       │                                       │            │
│       │                                       │            │
│       └───────────────────────────────────────┘            │
│                      直接返回（查询）                        │
└───────────────────────────────────────────────────────────┘
```


## 项目与图的生命周期约定

整个生命周期（开 App → 开项目 → 开图 → 修改 → 关闭/保存）遵循
**后端权威 + CQRS 推送** 模式：

- **后端 `ProjectState.project_data` 是唯一权威**；前端 store 只是投影。
- **App 启动**：拉一次 schema/node 注册表（全局只读）。
- **打开项目**：`load_project` 命令先 `state.clear()` 再 `set_data(...)`，
  emit `ProjectLoaded`；前端按阶段拉取 `databases+variables` 与
  图索引（图体懒加载）。前端 `resetClientProjectState()` 同步清除
  layout 图 tab、viewport、history、数据视图缓存。
- **打开图**：`load_project_graph` 命令把图反序列化进
  `project_data.graphs` 并通过 **统一入口 `ProjectState::insert_graph`**
  绑定 registry / schema provider / schema 传播 / 动态 pin 解析。
- **修改图**：所有修改走后端命令（mutate `project_data` → emit 事件 →
  前端 handler 更新 store）。高频写命令（如节点拖拽）使用
  `trackPending` + `isPending` 抑制自回声。
- **关闭/保存图**：单图 `save_project_graph` / `unload_project_graph`；
  关 tab 与关窗口都走应用内三态确认（Save All / Don't Save / Cancel）。

> 完整规范见 `.cursor/rules/project-graph-lifecycle.mdc`。

## 科学计算相关

