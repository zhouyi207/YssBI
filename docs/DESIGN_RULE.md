# 设计规则

本文档定义 YssBI 项目的核心设计原则与约定，供开发与评审时参考。

---

## 1. 整体设计

### 1.1 数据主权

- **强类型设计**：不能隐式转换。
- **后端为唯一真实来源**：所有持久化状态（图、节点、连接、变量、项目等）均存储在后端 Rust 中。
- **前端为视图层**：通过 Tauri `invoke` 调用后端命令，将获取的数据写入 Zustand Store，驱动 UI 渲染与交互。

### 1.2 整体数据流

采用 **命令-事件分离（CQRS-like）** 模式:

- **读取数据（Command Flow）**：用户操作 → 前端 invoke 后端命令 → 后端直接返回数据。
- **修改数据（Event Flow）**：用户操作 → 前端 invoke 后端命令 → 后端更新状态 → emit event 至前端 → 前端更新 Store → UI 重渲染。

---

## 2. 前端设计

### 2.1 目录与模块边界

```
src/
├─ app/                 # 应用入口与全局配置
│   ├─ main.tsx         # 入口文件
│   ├─ App.tsx          # 根组件
│   └─ providers/       # 全局 Provider（Settings、Theme 等）
│
├─ views/               # 纯 UI 视图层
│   ├─ EditorView/      # 编辑器主视图
│   │   ├─ Layout/      # 布局组件（Workspace、Sidebar、Menubar、TabBar）
│   │   ├─ Canvas/      # 画布组件（Canvas、Edge、SelectionBox、Grid）
│   │   ├─ Nodes/       # 节点渲染组件（Node、NodeContainer、Layout）
│   │   ├─ Pins/        # Pin 渲染组件（Pin、PinInput）
│   │   └─ Renderer/    # 布局渲染（LayoutNodeRenderer、Sash）
│   ├─ DataView/        # 数据视图
│   ├─ LogView/         # 日志视图
│   └─ PlotView/        # 图表视图
│
├─ features/            # 业务逻辑（三层架构）
│   ├─ domain/          # 领域层：纯业务模型与纯函数
│   │   ├─ graph/       # 图相关纯逻辑（类型定义、校验、转换）
│   │   ├─ node/        # 节点纯逻辑（分类、样式映射、className 工具）
│   │   ├─ pin/         # Pin 纯逻辑（类型映射、颜色映射）
│   │   ├─ connection/  # 连接纯逻辑（校验、兼容性判断）
│   │   └─ variable/    # 变量纯逻辑
│   │
│   ├─ application/     # 应用层：用例编排与 Hook 协调
│   │   ├─ editor/      # 编辑器用例（useEditor、useEditorGroup、keyboard）
│   │   ├─ project/     # 项目用例（save/load/execute、useProjectSync）
│   │   ├─ menubar/     # 菜单栏用例
│   │   └─ initialization/ # 应用初始化
│   │
│   └─ core/            # 核心层：基础设施与共享能力
│       ├─ dataStore/   # Zustand Store（graphData、graphMeta、variable、database）
│       ├─ sync/        # 事件同步（EventHandler、EventRegistry、Listener）
│       ├─ editor/      # 编辑器基础状态（editorStore、clipboardStore）
│       ├─ canvas/      # 画布交互 Hook（useCanvasInteraction、viewport、gesture）
│       ├─ nodeRegister/ # 节点注册表
│       └─ schema/      # 前端 Schema 缓存
│
├─ services/            # 服务层：封装所有后端调用
│   ├─ graph/           # GraphService、ConnectionService、NodeService、PinService
│   ├─ project/         # ProjectService
│   ├─ variable/        # VariableService
│   ├─ database/        # DatabaseService
│   ├─ schema/          # SchemaService
│   ├─ executor/        # ExecutorService
│   └─ settings/        # SettingsService
│
└─ shared/              # 共享资源（无业务逻辑）
    ├─ types/           # 类型定义（domain/dto/store/ui/state/settings）
    ├─ utils/           # 工具函数（math、dtoConverters、connections、internalNodes）
    ├─ ui/              # 通用 UI 组件（Modal、Toast、Select）
    └─ assets/          # 静态资源
```

**各层依赖规则**：

| 层级                      | 可依赖                                              | 禁止                                                                     |
| ------------------------- | --------------------------------------------------- | ------------------------------------------------------------------------ |
| **views/**                | features/application, features/core（Hook）, shared | ❌ 直接 invoke；❌ 直接 import services；❌ 直接操作 Store                  |
| **features/domain/**      | shared/types, shared/utils                          | ❌ import React；❌ import Zustand；❌ invoke 后端；❌ 依赖 core/application |
| **features/application/** | domain, core, services, shared                      | ❌ 写复杂 JSX；❌ 直接定义 Store                                           |
| **features/core/**        | services, shared                                    | ❌ 依赖 domain 或 application；❌ 包含业务判断逻辑                         |
| **services/**             | shared/types                                        | ❌ import React/Zustand；❌ 操作 Store；❌ 依赖 features                    |
| **shared/**               | 无外部依赖                                          | ❌ 依赖 features/services/views                                           |

### 2.2 命名与领域约定

**文件命名**：

| 分类 | 文件名格式 | 示例 |
|------|-----------|------|
| 视图组件 | `PascalCase.tsx` | `Canvas.tsx`、`NodeContainer.tsx`、`PinInput.tsx` |
| 视图目录 | `PascalCase/` | `EditorView/`、`Canvas/`、`Nodes/` |
| Hook | `useXxx.ts`（camelCase） | `useEditor.ts`、`useEditorGraphData.ts` |
| Store | `xxxStore.ts`（camelCase） | `graphDataStore.ts`、`variableStore.ts` |
| Service | `xxxService.ts`（camelCase） | `connectionService.ts`、`graphService.ts` |
| 类型文件 | `camelCase.ts` | `graph.ts`、`node.ts`、`editor.ts` |
| 纯函数 / 工具 | `camelCase.ts` | `nodeClassNames.ts`、`dtoConverters.ts` |

**命名模式**：

| 分类 | 命名约定 | 示例 |
|------|---------|------|
| React 组件 | `PascalCase` | `Canvas`、`NodeContainer`、`EditorWindow` |
| 自定义 Hook | `use` + `PascalCase` | `useEditor`、`useCanvasViewport`、`useEditorOperations` |
| Zustand Store | `use` + `PascalCase` + `Store` | `useGraphDataStore`、`useEditorStore`、`useVariableStore` |
| Store 接口 | `PascalCase` + `Store` | `GraphDataStore`、`EditorStore`、`VariableStore` |
| Service 类 | `PascalCase` + `Service` | `ConnectionService`、`GraphService`、`NodeService` |
| Service 方法 | `camelCase` 动词 | `connectPins()`、`getGraph()`、`deleteNode()` |
| 变量 / 函数 | `camelCase` | `activeTabId`、`addNode()`、`findConnectionsByPin()` |
| 常量 | `UPPER_SNAKE_CASE` | `PIN_COLORS`、`DEFAULT_THEME`、`GRID` |

**类型命名**：

| 类型分类 | 后缀约定 | 示例 |
|---------|---------|------|
| 领域模型 | 无后缀 | `Node`、`Pin`、`Graph`、`Connection`、`Variable` |
| DTO | `DTO` 后缀 | `GraphInstanceDTO`、`NodeInstanceDTO`、`PinInstanceDTO` |
| Store 数据 | `Data` 后缀 | `NodeData`、`PinData`、`ConnectionData`、`GraphData` |
| Store 输入 | `Input` / `Like` 后缀 | `GraphDataInput`、`GraphDataLike` |
| UI 专用 | 描述性 | `EditorGroup`、`EditorGesture`、`LayoutTab` |

**事件命名**（对应后端 Event）：

| 分类 | 约定 | 示例 |
|------|------|------|
| 事件类型字符串 | `PascalCase` 过去式 | `"EventCreated"`、`"NodeDeleted"`、`"ConnectionsUpdated"` |
| 事件处理器类 | `PascalCase` + `Handler` | `EventCreatedHandler`、`NodeEventHandler` |
| 处理器方法 | `handle()` | `handler.handle(payload)` |

**一个文件 = 一个概念单元**，避免过度拆分。

**禁止**：`createEvent()`、`onEventCreate()`、`handleCreate()` 等动词顺序不统一的命名。

### 2.3 前端分层与数据流

```
┌─────────────────────────────────────────────────────────────┐
│  Views（视图）        纯展示与交互，不直接 invoke              │
├─────────────────────────────────────────────────────────────┤
│  Features（领域/应用） Hooks、事件处理，通过 Services 调后端   │
├─────────────────────────────────────────────────────────────┤
│  Services（服务层）    封装 invoke，对接后端命令               │
├─────────────────────────────────────────────────────────────┤
│  DataStore（状态层）   Zustand Store，规范化存储               │
├─────────────────────────────────────────────────────────────┤
│  Backend（Rust）      命令实现、状态管理、类型推断、执行引擎    │
└─────────────────────────────────────────────────────────────┘
```

### 2.4 TypeScript 与类型系统

- **禁止 `any`**：所有类型必须显式声明，必要时使用 `unknown` 或泛型。
- **类型来源**：优先使用 `@/shared/types`，避免在参数位置使用 `import('@/...').Type`，改为文件顶部导入。

**类型目录与使用原则**：

| 目录        | 用途                        | 示例                                        |
| ----------- | --------------------------- | ------------------------------------------- |
| `domain/`   | 领域模型，与后端结构对应    | Graph, Node, Pin, Connection                |
| `dto/`      | 前后端传输对象（camelCase） | GraphInstanceDTO, NodeInstanceDTO           |
| `store/`    | Store 规范化格式            | NodeData, PinData, GraphData, GraphDataLike |
| `ui/`       | UI 专用类型                 | EditorGroup, EditorGesture, LayoutTab       |
| `state/`    | 状态与 Hook 相关            | -                                           |
| `settings/` | 应用设置                    | -                                           |

- API 调用使用 domain 或 dto 类型；Store 读写使用 store 类型。
- DTO → Store 可直接传入 `addGraphFromData`（支持 GraphInstanceDTO）；Canvas 视图从 Store 的 `pinConnections` 派生连接状态，不再在 domain `Pin` 上维护 `links`。

### 2.5 状态管理（Zustand）

- **必须使用选择器**：`useStore((s) => s.xxx)`，禁止无选择器订阅整个 Store，否则任意字段变化都会触发重渲染。
- **按领域划分**：graphDataStore、graphMetaStore、variableStore 等，避免过细或过粗。
- **数据更新**：命令成功 → 拉取最新数据 → `addGraphFromData`、`replaceGraphNodes` 等更新 Store，由 Store 驱动 UI。

### 2.6 Services 与 invoke

- 所有后端调用通过 `@/services` 下的 Service 类完成。
- **命名**：`XxxService.methodName`（如 `ConnectionService.connectPins`、`GraphService.getGraph`）。
- **流程**：invoke 命令 → 成功 → 拉取最新数据 → 更新 Store（如 `addGraphFromData`）。

### 2.7 命名约定

| 场景           | 约定                   | 示例                           |
| -------------- | ---------------------- | ------------------------------ |
| Domain / Store | snake_case             | node_type, ui_style, graph_id  |
| DTO（JSON）    | camelCase              | nodeType, uiStyle, graphId     |
| 组件 / Hook    | PascalCase / camelCase | PinInput, useCanvasInteraction |
| 文件           | camelCase              | graphDataStore.ts              |

### 2.8 代码风格

- **ESLint + Prettier** 自动化处理。
- 不争论单双引号、尾逗号、arrow body 等，由工具统一。

---

## 3. 后端设计

### 3.1 目录与模块边界

```
src-tauri/src/
├─ lib.rs                   # 应用入口，invoke_handler 注册所有命令
│
├─ commands/                # API 层：Tauri 命令入口
│   ├─ command_project.rs   # 项目 CRUD（new/load/save/get）
│   ├─ command_schema.rs    # Schema 查询（definitions/categories/pin_types）
│   ├─ command_settings.rs  # 设置读写
│   ├─ command_graph/       # 图相关命令（按子领域拆分）
│   │   ├─ command_graph.rs       # Graph CRUD（create/update/remove/get）
│   │   ├─ command_node.rs        # Node 操作（create/delete/update_positions）
│   │   ├─ command_connection.rs  # 连接操作（connect/disconnect/get）
│   │   └─ command_pin.rs         # Pin 操作（update_value/dynamic_pin）
│   ├─ command_variable/    # 变量 CRUD
│   ├─ command_dataframe/   # DataFrame 导入与管理
│   └─ command_log/         # 日志查询
│
├─ graph/                   # 核心领域：图运行时
│   ├─ core/                # 图实例与状态
│   │   ├─ graph_instance.rs      # GraphInstance（持有 data_state，提供 mutation API）
│   │   ├─ graph_data_state.rs    # GraphDataState（nodes、pins、connections 的容器）
│   │   ├─ graph_id.rs            # GraphId（UUID 包装）
│   │   ├─ graph_kind.rs          # GraphKind（Event/Function）
│   │   └─ graph_position.rs      # 画布位置 { x, y, scale }
│   ├─ node/                # 节点系统
│   │   ├─ node_definition.rs     # 节点定义模板（name、category、pins、metadata）
│   │   ├─ node_instance.rs       # 节点运行时实例
│   │   ├─ node_id.rs             # NodeId
│   │   └─ node_position.rs       # 节点画布坐标
│   ├─ pin/                 # Pin 系统
│   │   ├─ pin_definition.rs      # Pin 定义（name、direction、data_type、kind）
│   │   ├─ pin_instance.rs        # Pin 运行时实例
│   │   ├─ pin_id.rs              # PinId
│   │   ├─ pin_data_type.rs       # DataType 枚举（Bool/Int/Float/String/...）
│   │   └─ pin_role.rs            # PinRole（Exec/Data）
│   ├─ connection/          # 连接管理
│   │   └─ mod.rs                 # ConnectionManager（connect/disconnect/query）
│   ├─ infer/               # 类型推断引擎
│   │   ├─ type_inference_context.rs
│   │   ├─ type_inference_session.rs
│   │   ├─ type_var_*.rs          # 类型变量定义与推断
│   │   └─ type_constraint.rs     # 类型约束
│   ├─ register/            # 节点注册表
│   │   ├─ registry.rs            # NodeRegistry（注册/查询节点定义）
│   │   └─ catalog/               # 内置节点目录
│   │       ├─ value/             # 常量、变量、类型转换
│   │       ├─ math/              # 数学运算
│   │       ├─ logic/             # 逻辑运算
│   │       ├─ control/           # 控制流
│   │       ├─ debug/             # 调试
│   │       └─ dataframe/         # DataFrame 操作
│   └─ value/               # 运行时值系统
│       ├─ data_type.rs           # DataType 定义
│       └─ data_value.rs          # DataValue 运行时值
│
├─ schema/                  # DTO 层：前后端数据传输定义
│   ├─ graph.rs             # GraphInstanceDTO
│   ├─ node.rs              # NodeInstanceDTO
│   ├─ pin.rs               # PinInstanceDTO
│   ├─ connection.rs        # ConnectionDTO / ConnectionItemDTO
│   ├─ project.rs           # ProjectDataDTO / ProjectMetadataDTO
│   ├─ variables.rs         # VariableDefinitionDTO
│   ├─ database.rs          # DatabaseDeclDTO / DatabaseEngineDTO
│   ├─ pin_types.rs         # Pin 类型元数据（供前端 Schema 查询）
│   ├─ categories.rs        # 节点分类元数据
│   ├─ ui_styles.rs         # UI 样式元数据
│   └─ validation.rs        # 校验规则
│
├─ event/                   # 事件层：后端 → 前端通知
│   ├─ event_project.rs     # 项目事件
│   ├─ event_event.rs       # Event 子图事件
│   ├─ event_function.rs    # Function 子图事件
│   ├─ event_node.rs        # 节点事件
│   ├─ event_connection.rs  # 连接事件
│   ├─ event_variable.rs    # 变量事件
│   └─ event_dataframe.rs   # DataFrame 事件
│
├─ project/                 # 项目管理
│   ├─ project_state.rs     # ProjectState（内存中的项目状态管理器）
│   ├─ project_data.rs      # ProjectData（可序列化的项目数据）
│   ├─ project_metadata.rs  # 项目元数据（版本、导出时间）
│   ├─ project_store.rs     # 项目存储（NodeRegistry 持有）
│   └─ project_state_*.rs   # 子状态管理（graph、database）
│
├─ variable/                # 变量系统
│   ├─ variable_definition.rs
│   └─ variable_scope.rs
│
├─ database/                # 数据库系统
│   ├─ database_decl.rs     # 声明
│   ├─ database_engine.rs   # 引擎 trait
│   ├─ database_engine_sql.rs # SQL 实现
│   └─ database_*.rs        # 实例、视图、状态、访问、错误
│
├─ execution/               # 执行引擎
│   ├─ engine/              # 执行器（executor、stack、frame、effect）
│   └─ context/             # 节点执行上下文
│
├─ editor/                  # 编辑器设置
│   └─ settings/            # 各类设置（app、editor、theme、window）
│
└─ log/                     # 日志系统
    ├─ log_manager.rs
    ├─ log_type.rs
    └─ macros.rs
```

**各层依赖规则**：

| 层级           | 职责                          | 可依赖                                            | 禁止                                   |
| -------------- | ----------------------------- | ------------------------------------------------- | -------------------------------------- |
| **commands/**  | API 入口，参数解析与转发      | project, graph, schema, event, variable, database | ❌ 包含业务逻辑；❌ 直接操作内部数据结构 |
| **graph/**     | 核心领域，图运行时与 mutation | value, 标准库                                     | ❌ 依赖 commands/schema/event/project   |
| **schema/**    | DTO 定义与序列化转换          | graph（只读取类型定义）, variable, database       | ❌ 修改 graph 状态；❌ 依赖 commands     |
| **event/**     | 后端→前端事件 emit            | schema（DTO 类型）, tauri::AppHandle              | ❌ 包含业务逻辑；❌ 依赖 graph 内部状态  |
| **project/**   | 项目状态持有与管理            | graph, variable, database                         | ❌ 依赖 commands/schema/event           |
| **execution/** | 图执行引擎                    | graph, project                                    | ❌ 依赖 commands/schema/event           |

### 3.2 命名与领域约定

**文件命名**：

| 分类 | 文件名格式 | 示例 |
|------|-----------|------|
| 命令 | `command_<domain>.rs` | `command_node.rs`、`command_connection.rs` |
| 图核心 | `graph_<concept>.rs` | `graph_instance.rs`、`graph_data_state.rs` |
| 节点/Pin | `<entity>_<concept>.rs` | `node_definition.rs`、`pin_instance.rs` |
| Schema/DTO | `<entity>.rs` | `node.rs`、`pin.rs`、`connection.rs` |
| 事件 | `event_<domain>.rs` | `event_node.rs`、`event_project.rs` |
| 项目 | `project_<concept>.rs` | `project_state.rs`、`project_data.rs` |

**核心领域三层模型**（Definition → Instance → RuntimeState）：

```
Definition（静态模板）     ─ 不可变，描述"是什么"
   │  create from
   ▼
Instance（运行时实例）     ─ 持有 ID + Definition 引用，描述"存在哪"
   │  wrap with
   ▼
RuntimeState（执行态快照） ─ 持有 ID + 当前状态 + 运行时值，描述"当前怎样"
```

| 层级 | Node | Pin |
|------|------|-----|
| Definition | `NodeDefinition { name, category, flow_processor, data_evaluator, pin_generator, metadata }` | `PinDefinition { name, direction, kind, role, data_type, meta_data }` |
| Instance | `NodeInstance { id: NodeId, definition: Arc<NodeDefinition>, position, pin_ids }` | `PinInstance { id: PinId, node_id, definition: PinDefinition, order, user_value }` |
| RuntimeState | `NodeRuntimeState { id, state: NodeState }` | `PinRuntimeState { id, state: PinState, current_value }` |

- `GraphDataState` 持有所有 Instance：`nodes: HashMap<NodeId, NodeInstance>`、`pins: HashMap<PinId, PinInstance>`、`connections: ConnectionManager`。
- `GraphRuntime` 持有 RuntimeState 映射：`HashMap<NodeId, NodeRuntimeState>`、`HashMap<PinId, PinRuntimeState>`，供 execution 引擎消费。
- RuntimeState 带 `Serialize/Deserialize`，可序列化推送至前端，用于展示节点/Pin 的执行状态。

**Struct 命名模式**：

| 分类 | 后缀约定 | 示例 |
|------|---------|------|
| 定义模板 | `Definition` | `NodeDefinition`、`PinDefinition` |
| 运行时实例 | `Instance` | `NodeInstance`、`PinInstance`、`GraphInstance` |
| 执行态 | `RuntimeState` | `NodeRuntimeState`、`PinRuntimeState` |
| 状态容器 | `State` / `DataState` | `ProjectState`、`GraphDataState` |
| DTO | `DTO` 后缀 | `NodeInstanceDTO`、`GraphInstanceDTO`、`ConnectionItemDTO` |
| 管理器 | `Manager` / `Registry` | `ConnectionManager`、`NodeRegistry` |
| 元数据 | `MetaData` | `NodeMetaData`、`PinMetaData` |

**Enum 命名模式**：

| 分类 | 后缀约定 | 示例 |
|------|---------|------|
| 状态枚举 | `State` | `NodeState { Idle, Ready, Executing, Completed, Error }` |
| 分类枚举 | `Kind` | `GraphKind { Event, Function }`、`PinKind { Data, Exec }` |
| 方向枚举 | `Direction` | `PinDirection { Input, Output }` |
| 语义角色 | `Role` | `PinRole { Exec(ExecRole), Data(DataRole) }` |
| 事件枚举 | `Event<Domain>` | `EventNode { NodeCreated, NodeDeleted, ... }` |

**函数命名模式**：

| 分类 | 约定 | 示例 |
|------|------|------|
| CRUD | `create_` / `get_` / `update_` / `delete_` / `remove_` | `create_node()`、`delete_node()`、`get_graph()` |
| 按条件查询 | `get_<entity>_by_<qualifier>` | `get_node_instance_by_node_id()`、`get_pin_instance_by_pin_role()` |
| 批量查询 | `get_<entities>_by_<qualifier>` | `get_pin_instances_by_node_id()` |
| Builder | `with_<property>()` | `with_category()`、`with_flow_processor()` |
| 连接操作 | `connect` / `disconnect` | `connect(from_pin, to_pin)`、`disconnect_pin(pin_id)` |

**ID 类型**（newtype wrapper over UUID）：

- `GraphId(Uuid)`、`NodeId(Uuid)`、`PinId(Uuid)`、`TypeVarId(Uuid)`
- 统一提供 `new()`（随机）、`nil()`（空）、`from(Uuid)` 方法。

**模块文档**：每个 `mod.rs` 使用 `//!` 注释说明模块职责。

### 3.3 Tauri Command 规范

- 所有命令在 `lib.rs` 的 `invoke_handler` 中统一注册。
- **命名**：snake_case（如 `connect_pins`、`disconnect_pin`、`get_graph`）。
- **参数**：`#[tauri::command]` 接收，前端传入 camelCase 对象，serde 自动映射。
- **返回**：`Result<T, String>`，错误通过 `Err(String)` 传递。

### 3.4 DTO 与序列化

- DTO 定义在 `schema/` 中，使用 `#[serde(rename_all = "camelCase")]`。
- JSON 键统一 camelCase，与前端一致。
- 类型映射详见 [DTO_TYPE_MAPPING.md](./DTO_TYPE_MAPPING.md)。

### 3.5 规则层级

Command 返回 `Result`，Event 通过 EventEmitter 推送至前端。

### 3.6 代码风格

遵循 Rust 惯例，使用 `cargo fmt` 统一格式。

### 3.7 Schema 与 Pin 解析

数据处理节点的输出形状在**编辑期**即可由上游 schema 推导，不应依赖执行结果。Pin 结构与此信息层分离维护，执行引擎只消费已解析结果。

#### 3.7.1 三层模型

| 层 | 何时确定 | 存储位置 | 示例 |
| --- | --- | --- | --- |
| **结构（Structure）** | 节点定义时固定 | `NodeDefinition.pin_slots` | Exec In/Out、Add 的 A/B/C、OLS 的可重复输入 |
| **信息（Schema）** | 连线 / 断线 / 改参时链式传播 | `PinInstance.resolved_schema` | DataFrame 列名与类型、Model 特征列 |
| **数据（Value）** | 执行或预览时计算 | `PinRuntimeState.current_value` | 回归系数、残差序列、预览前几行 |

- **结构层**决定节点「有哪些槽位、能否增删 pin」。
- **信息层**决定「每个 DataFrame / Model pin 携带什么列结构」，沿连接图拓扑序传播。
- **数据层**仅在 run / preview 时填充，**不得**在 exec 过程中增删 pin 或改写 schema。

#### 3.7.2 Pin 槽位类型

`PinSlot` 表达结构层；schema 派生 pin 不是「运行时动态」，而是「编辑期由信息层物化」：

| 槽位 | 含义 | 初始 pin | 后续变更 |
| --- | --- | --- | --- |
| `Fixed` | 固定 pin | 创建节点时生成 | 仅用户改值 |
| `Repeatable` | 可重复 pin（如 Add、OLS） | 按 `min_count` 生成 | 用户 `+` / 移除，后端 reindex |
| `DerivedFromInput` | 由上游 schema 派生 | 空（无初始 pin） | `pin_resolver` 在 schema 变化时物化 |

- **可预测 ≠ 全部写死在 Definition 里**。Decompose 列数随上游 schema 变化，仍属可预测，应使用 `DerivedFromInput` + `pin_resolver`，而非 exec 时计算。
- 命名上避免「动态节点」；统一称为 **schema 派生 pin** 或 **信息层物化**。

#### 3.7.3 Schema 传播（信息层）

- 入口：`GraphInstance::propagate_schemas()`，按拓扑序填充各 DataFrame output pin 的 `resolved_schema`。
- 节点自算 output schema：在 `NodeDefinition` 上注册 `output_schema_resolver`（如 TS Align、Combine）。
- 默认 fallback：无 resolver 时透传上游 Input 的 `resolved_schema`。
- 按 `dataframe_id` 查列结构：通过 `OutputSchemaContext.schema_provider`（如 Get DataFrame、自配 Model）。

**触发时机**（必须做 schema 传播）：

- `connect_pins` / `disconnect_pin`
- 影响上游 schema 的节点参数变更
- 项目/图加载恢复连接后（恢复信息层）

**禁止**：在执行引擎、`flow_processor` / `data_evaluator` 内修改 pin 列表或 `resolved_schema`。

#### 3.7.4 Pin 物化（结构层增量更新）

- 有 `pin_resolver` 的节点：在 `PinResolverContext`（`instance_params` + 上游 `input_schemas`）下生成 **DerivedFromInput** 部分的 pin。
- **连线路径**（推荐模式）：`propagate_schemas` → 仅对受影响节点调用 `resolve_dynamic_pins`（含下游），见 `get_downstream_resolve_nodes`。
- **打开项目**：至少 `propagate_schemas`；全图 `resolve_all_dynamic_pins` 仅作兼容兜底，大项目应改为 **延迟物化**（打开 tab / 节点进入视口 / 后台分帧 + 事件推送），避免阻塞首屏。
- Pin 变更通过 `PinChangeSet` + `NodePinsUpdated` 事件同步前端；renames 须包含在 `updated_pins` 中。

#### 3.7.5 Predict 等双模式节点

| 模式 | Schema 来源 | Pin 更新时机 |
| --- | --- | --- |
| 连线 Model | 上游 output 的 `resolved_schema` | connect / 上游 schema 变化 |
| 自配 Model | `instance_params` + `schema_provider` | 改参 / 选择 model 时 |

两种模式共用同一机制：信息层在编辑期解析，不在 exec 时生成 pin。

#### 3.7.6 编译 / 预览（补充层）

借鉴 UE compile、ipynb 的「编译后可见结果」用于：

- 中间结果预览（前几行、摘要统计）
- 用户主动触发的「展开列 pin / 编译后才显示 preview 口」
- 缓存执行结果以加速重复预览

**不用于**：类型检查、schema 传播、默认 pin 列表。编辑期连线与校验必须不依赖是否已执行。

#### 3.7.7 宽表 UX（可选）

列数很大时，可不物化 N 个 output pin，而保留单一 DataFrame output + schema 驱动的列选择 UI。属产品选择，不改变信息层在 connect 时传播的原则。

#### 3.7.8 反模式

| 反模式 | 原因 |
| --- | --- |
| Exec 等待样式 + 执行时才生成 data pin | 未执行无法连线/校验；编辑与运行耦合 |
| 打开项目对整图同步 `resolve_all_dynamic_pins` | 大图首屏卡顿 |
| 为未发布格式保留旧 pin 解析兼容层 | 见 §Early Stage：直接改当前架构与测试 |
| 前端自行推断 schema 并改 pin 结构 | 违反 §1.1 后端为唯一真实来源 |

#### 3.7.9 相关实现位置

| 职责 | 模块 |
| --- | --- |
| Schema 传播 | `graph/core/graph_instance.rs` — `propagate_schemas`, `compute_output_schema_for_node` |
| Pin 物化 | `graph/core/graph_instance.rs` — `resolve_dynamic_pins`, `resolve_all_dynamic_pins` |
| 节点注册 | `graph/register/catalog/` — `with_output_schema_resolver`, `with_pin_resolver` |
| 槽位定义 | `graph/pin/pin_slot.rs` — `PinSlot` |
| 前端同步 | `features/core/sync/` — `NodePinsUpdated`, `batchUpdatePins` |

---

## 4. 参考文档

- [DTO_TYPE_MAPPING.md](./DTO_TYPE_MAPPING.md) - 前后端类型映射
- [ARCHITECTURE_ISSUES.md](./ARCHITECTURE_ISSUES.md) - 架构问题与优化路线
- [runtime-source-lifecycle.md](./runtime-source-lifecycle.md) - RuntimePin / Window 结果 source 生命周期与前后端投影规则
