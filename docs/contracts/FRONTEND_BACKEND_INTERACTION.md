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
**特征：改变项目状态，使用带 identity/revision 的直接回包与同源事件**

```
前端 UI/Hook
  → Application Coordinator
  → Service Layer (invoke)
  → 后端 Command Handler
  → 修改权威状态
  → 返回 canonical mutation result，同时 emit project-event
  → 发起方校验并应用直接回包；ProjectListener 分发远端事件/回声
  → Application Port / Coordinator 更新前端投影
  → UI 自动响应
```

**适用场景：**
- 项目操作（newProject, loadProject, saveProject）
- Graph 创建/删除（createEvent, createFunction, removeGraph）
- Node 创建/删除（createNode, deleteNode）
- Connection 创建/删除（createConnection, deleteConnection）
- Variable 创建/更新/删除

**范式规则：**
- Service 只负责 invoke、DTO 解析与错误 wire 映射；修改命令返回带项目 identity、operation ID 和 revision 的结果
- Graph 文档修改 emit `GraphDelta`；资源/变量/数据库等发布 emit `ResourceMutationCommitted`
- 发起方由 application coordinator 校验并应用直接回包；matching `GraphDelta` 回声只按 pending operation 抑制
- `ProjectListener` + `EventRegistry` 处理其他窗口、外部变更和先到达的事件，并通过窄 application port 协调投影
- React 组件只订阅前端投影 Store

**示例：**
```typescript
// 发起方：application coordinator 消费 canonical GraphMutationResultDto
const outcome = await executeEditorMutation({ graphPath, locale, mutation });

// 全局单例 listener：registry 负责解包、校验类型并路由 handler
const listener = new ProjectListener();
await listener.start();

// GraphDeltaHandler 对非 matching echo 的新 revision 请求权威投影刷新
syncApplicationEventPort().graphDelta(delta.graphPath);
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
- 使用 `listen()` 监听 Tauri 的 `project-event`
- 全局单例模式（防止重复监听）
- 只接受当前 Rust `Event` 的 `Project` / `Resource` 外层 envelope
- `EventRegistry` 解包并校验 leaf type；handler 校验 identity/revision 后调用窄 application port
- handler 不调用 Service，也不保留已无 Rust producer 的兼容事件

**核心实现：**
```typescript
// Core：全局 listener 将原始 wire 交给 registry。
const listener = new ProjectListener(callbacks);
await listener.start();

// Registry：parseEvent → isValidEventType → 对应 handler。
registry.dispatch(event.payload as RawBackendEvent);

// Application：启动时注册 core 所需的协调器适配。
registerCoreApplicationPorts();
```

**文件位置：**
```
src/features/core/sync/                                      # listener、registry、parser、handlers、ports
src/features/application/initialization/useProjectSync.ts    # 管理全局 listener 生命周期
src/features/application/initialization/registerCoreApplicationPorts.ts
src/features/domain/execution/hooks/useExecutionVisualization.ts  # 执行事件监听（独立于 project-event）
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
export async function mutateOpenGraph(graphPath: string, mutation: EditorGraphMutationDto) {
    return executeEditorMutation({
        graphPath,
        locale: i18n.resolvedLanguage ?? 'en-US',
        mutation,
    });
}
```

Application coordinator 负责 project identity、operation ID、revision、pending echo 与回包应用；Hook 不直接拼装 event，也不直接写后端权威状态。

**Editor 窗口编排（单例 provider + caller-shaped Canvas）：**

```tsx
// EditorWindow.tsx
<EditorSessionProvider>
  <EditorWindowReady />
</EditorSessionProvider>

// Canvas.tsx — useEditorCanvas 的唯一 caller
const canvas = useEditorCanvas({ mode });

// 只有 interactive Canvas 挂 pointer loop/drop/overlay；Overlay 不读 session
return mode === 'interactive'
  ? <CanvasOverlays model={overlayModel} />
  : null;

// Detail / 资源列表按 caller 使用窄 slice
const resources = useEditorSessionResources();
```

`useEditorCanvas` 只暴露 Canvas 实际使用的 commands/workspace/resources/interaction。variables/functions 使用现有窄 hooks；shared resource context 不承担 Dockview group topology，Dockview 仍是唯一 authority。

---

## 事件命名规范

### 后端事件结构
```rust
// 当前仅有 Project / Resource 两种外层 envelope。
Event::Project(EventProject::GraphDelta {
    project_instance_id,
    delta,
})

// 序列化为 JSON
{
    "type": "Project",
    "payload": {
        "type": "GraphDelta",
        "payload": {
            "projectInstanceId": "...",
            "delta": { "graphPath": "events/Main.yssbi-event", "fromRevision": 1, "toRevision": 2 }
        }
    }
}
```

### 前端事件解析
```typescript
// ProjectListener 将完整外层 wire 交给 EventRegistry。
registry.dispatch(event.payload as RawBackendEvent);

// EventRegistry 内部统一解包和校验，不在视图中 switch event type。
const parsed = parseEvent(event);
if (isValidEventType(parsed.type)) {
    handlers.get(parsed.type)?.handle(parsed.payload, callbacks);
}
```

### `project-event` leaf type 清单

**`Project` envelope：**
- `ProjectLoaded`
- `ProjectCleared`
- `ProjectLifecycleCommitted`
- `ProjectSaved`
- `ComputationSettingsChanged`
- `GraphDelta`
- `ResourceMutationCommitted`

**`Resource` envelope：**
- `ProjectIndexInvalidated`

Graph/Function/Variable/DataFrame 不再各自拥有 CRUD leaf event；其 revisioned 变更统一由 `GraphDelta` 或 `ResourceMutationCommitted` 表达。`EventUpdated`、`EventDeleted`、`FunctionUpdated`、`FunctionDeleted`、`ResourceChanged` 以及旧 Variable/DataFrame leaf 均不是有效 wire。

**执行事件（独立于 `project-event`）：**
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
│                 直接返回（查询结果 / 修改 receipt）            │
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
- **修改图**：所有修改走后端命令；命令返回 `GraphMutationResultDto`，同时
  emit `GraphDelta`。发起方按 identity/operation/revision 应用直接回包，matching
  pending echo 被抑制；其他窗口或更新 revision 的 `GraphDelta` 请求权威投影刷新。
  资源、变量、数据库和 worksheet 发布统一经 `ResourceMutationCommitted` 进入
  `projectPublicationCoordinator`。
- **关闭/保存图**：单图 `save_project_graph` / `unload_project_graph`；
  关 tab 与关窗口都走应用内三态确认（Save All / Don't Save / Cancel）。

> 项目级生命周期规则见根目录 `AGENTS.md` 的 “Graph lifecycle and synchronization” 章节。

## 科学计算相关

