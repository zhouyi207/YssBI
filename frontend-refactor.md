# Frontend Refactor

> 实施状态：已完成（2026-09-01）。当前维护中的总体边界与 Dockview 细节分别以
> [`ARCHITECTURE.md`](docs/architecture/ARCHITECTURE.md) 和
> [`WORKBENCH_DOCKVIEW_ARCHITECTURE.md`](docs/architecture/WORKBENCH_DOCKVIEW_ARCHITECTURE.md) 为准。

## 1. 目标

前端最终采用以下架构：

> 一个 Workbench Shell、一个 root Dockview authority、多个互相隔离的业务模块，以及一个应用组合层。

其中：

- `Editor` 不再表示整个主窗口，只表示具体的资源编辑器内容。
- `Dockview` 是 panel、tab、group 和物理布局的唯一 authority。
- `Workbench` 是 Dockview 之上的应用语义层，只负责规则、生命周期和持久化协议。
- Graph、Chart、Project、Data、Result 等业务组件彼此不可见，只通过显式 public API 和应用组合层协作。
- Rust 继续拥有项目、资源、图、Chart document、数据库和执行结果等权威业务状态。

## 2. 最终运行时拓扑

```text
WorkbenchWindow
├─ WorkbenchMenuBar
├─ RootDockviewHost                    # 唯一 root DockviewReact
│  ├─ left Activity edge group
│  │  ├─ ProjectActivityPanel
│  │  ├─ NodeCatalogActivityPanel
│  │  ├─ DataActivityPanel
│  │  └─ CommandsActivityPanel
│  │
│  ├─ central grid groups              # Dockview 可动态创建多个
│  │  ├─ EditorResourcePanel
│  │  │  ├─ GraphDocumentEditor
│  │  │  └─ ChartEditor
│  │  ├─ ResultPanel
│  │  └─ 其他允许进入 grid 的 panel
│  │
│  ├─ right edge group
│  │  ├─ DetailsPanel
│  │  ├─ AssistantPanel
│  │  ├─ InspectPanel
│  │  └─ ResultPanel
│  │
│  └─ bottom edge group
│     ├─ LogsPanel
│     │  └─ LogDomainDockviewHost      # 唯一 nested Dockview
│     ├─ OutputPanel
│     └─ DiagnosticsPanel
│
├─ StatusBar
└─ WorkbenchOverlayHost
   ├─ SettingsDialog
   └─ NodeDocumentationDialog
```

不得再建立以下静态 React 布局容器：

```text
LeftSidebarContainer
CenterContainer
RightSidebarContainer
BottomContainer
```

这些区域本身就是 Dockview 的 group/edge topology。再通过 React 容器表达一次，会形成第二套布局系统。

只有 `LogsPanel` 内部的七个日志域 tab 使用 nested Dockview。central grid 与 left/right/bottom edge 都属于同一个 root Dockview。

## 3. Authority 边界

### 3.1 Dockview 唯一拥有物理布局事实

Dockview live instance 唯一拥有：

```text
panelInstanceId
groupId
panel order
group membership
active panel/group
split direction
edge position/size/collapse
serialized layout
```

这些事实不得：

- 写入 Zustand。
- 转换成第二份长期存活的 tab/group tree。
- 通过 `LayoutTab[]`、active-page flag 或自定义 layout store 镜像。
- 从 `resourceRef`、`resultId` 或其他业务 identity 反向推导。

### 3.2 Workbench 只拥有应用语义

Workbench 负责：

```text
合法 panel role
singleton/fixed policy
default home
dirty-close
project replacement
layout restore/reset
panel metadata
```

Workbench 不保存 tab 列表，只通过 Dockview read/command port 操作物理布局。

### 3.3 Panel Adapter 只连接 Dockview 与业务组件

```ts
interface DockPanelScope {
  panelInstanceId: string;
  groupId: string;
  metadata: WorkbenchPanelMetadata;
}
```

示例：

```tsx
function EditorResourceDockPanel({ scope }: { scope: DockPanelScope }) {
  const metadata = requireEditorMetadata(scope.metadata);
  const Editor = editorRendererRegistry[metadata.resourceKind];

  return (
    <Editor
      panelInstanceId={scope.panelInstanceId}
      groupId={scope.groupId}
      resourceRef={metadata.resourceRef}
    />
  );
}
```

`GraphDocumentEditor` 和 `ChartEditor` 不得直接导入 Dockview。

### 3.4 业务状态 owner

| 状态                                      | Owner                                                 |
| ----------------------------------------- | ----------------------------------------------------- |
| panel/group/tab 物理位置与激活态          | Dockview                                              |
| panel role、默认位置和生命周期规则        | Workbench                                             |
| panel-local selection、viewport、临时交互 | 以 `panelInstanceId` 为 key 的 UI runtime state       |
| Graph document、节点、连接、revision      | Rust authority；React 只保存 projection               |
| Chart document、revision                  | Rust authority；React 只保存 draft/projection         |
| Database、Variable、Result                | Rust authority；React 只保存 projection/runtime state |

## 4. 最终目录结构

```text
src/
├─ app/
│  ├─ main.tsx
│  ├─ providers/
│  └─ windows/
│     └─ workbench/
│        ├─ WorkbenchComposition.tsx
│        ├─ rootPanelRegistry.ts
│        ├─ editorRendererRegistry.ts
│        ├─ detailContributionRegistry.ts
│        ├─ menuContributionRegistry.ts
│        └─ integrations/
│           ├─ panelActivationCoordinator.ts
│           ├─ activityEditorDndCoordinator.ts
│           ├─ projectReplacementCoordinator.ts
│           └─ editorResultCoordinator.ts
│
├─ modules/
│  ├─ workbench/
│  │  ├─ public.ts
│  │  └─ internal/
│  │     ├─ domain/
│  │     │  ├─ panelMetadata.ts
│  │     │  ├─ panelIdentity.ts
│  │     │  ├─ panelPolicies.ts
│  │     │  └─ defaultPlacement.ts
│  │     ├─ application/
│  │     │  ├─ panelCommands.ts
│  │     │  ├─ panelCloseCoordinator.ts
│  │     │  ├─ layoutLifecycle.ts
│  │     │  └─ layoutReset.ts
│  │     ├─ dockview/
│  │     │  ├─ RootDockviewHost.tsx
│  │     │  ├─ RootPanelTabRenderer.tsx
│  │     │  ├─ rootDockviewRuntime.ts
│  │     │  ├─ rootDockviewRead.ts
│  │     │  ├─ rootDockviewCommands.ts
│  │     │  ├─ rootDockviewBinding.ts
│  │     │  ├─ rootLayoutTransaction.ts
│  │     │  └─ rootLayoutPersistence.ts
│  │     └─ ui/
│  │        ├─ WorkbenchShell.tsx
│  │        ├─ WorkbenchMenuBar.tsx
│  │        ├─ StatusBar.tsx
│  │        └─ WorkbenchOverlayHost.tsx
│  │
│  ├─ graph-editor/
│  │  ├─ public.ts
│  │  └─ internal/
│  │     ├─ domain/
│  │     │  ├─ graphSelection.ts
│  │     │  ├─ editorProjection.ts
│  │     │  └─ connectionRules.ts
│  │     ├─ application/
│  │     │  ├─ graphSessionCoordinator.ts
│  │     │  ├─ graphMutationCoordinator.ts
│  │     │  ├─ graphClipboardCommands.ts
│  │     │  └─ nodeCreationCommands.ts
│  │     ├─ state/
│  │     │  ├─ graphProjectionStore.ts
│  │     │  ├─ graphInteractionStore.ts
│  │     │  └─ editorPaneState.ts
│  │     └─ ui/
│  │        ├─ GraphDocumentEditor.tsx
│  │        ├─ GraphCanvas.tsx
│  │        ├─ nodes/
│  │        │  ├─ GraphNodeController.tsx
│  │        │  └─ GraphNodeView.tsx
│  │        ├─ pins/
│  │        │  ├─ GraphPinController.tsx
│  │        │  └─ GraphPinView.tsx
│  │        ├─ edges/
│  │        ├─ contextMenus/
│  │        └─ overlays/
│  │
│  ├─ chart/
│  │  ├─ public.ts
│  │  └─ internal/
│  │     ├─ domain/
│  │     │  ├─ ChartDocument.ts
│  │     │  ├─ ChartEncoding.ts
│  │     │  └─ ChartType.ts
│  │     ├─ application/
│  │     │  ├─ chartDocumentCommands.ts
│  │     │  ├─ chartPreviewCoordinator.ts
│  │     │  └─ chartSaveCoordinator.ts
│  │     ├─ state/
│  │     │  └─ chartDocumentStore.ts
│  │     └─ ui/
│  │        ├─ ChartEditor.tsx
│  │        ├─ ChartPreview.tsx
│  │        └─ ChartConfigurationPanel.tsx
│  │
│  ├─ project-explorer/
│  ├─ node-catalog/
│  ├─ data-explorer/
│  ├─ database-editor/
│  ├─ details/
│  ├─ results/
│  ├─ logs/
│  ├─ assistant/
│  ├─ settings/
│  └─ commands/
│
├─ services/
│  ├─ ipc/
│  ├─ project/
│  ├─ graph/
│  ├─ chart/
│  ├─ database/
│  └─ results/
│
├─ components/
│  └─ ui/                            # shadcn primitives
│
└─ shared/
   ├─ types/
   │  └─ dto/                        # IPC wire contract
   ├─ charts/
   │  ├─ ChartModel.ts
   │  ├─ ChartRenderer.tsx
   │  ├─ cartesian/
   │  └─ statistical/
   ├─ ui/
   ├─ kernel/                        # 真正跨模块的纯函数/值对象
   ├─ theme/
   └─ utils/
```

## 5. App Composition 是唯一跨模块可见点

`WorkbenchComposition` 是唯一允许同时看见多个业务模块 UI 的位置。

```ts
const rootPanelRegistry = createRootPanelRegistry({
  project: projectActivityPanelContribution,
  nodes: nodeCatalogActivityPanelContribution,
  data: dataActivityPanelContribution,
  commands: commandsActivityPanelContribution,
  details: detailsPanelContribution,
  assistant: assistantPanelContribution,
  results: resultPanelContribution,
  logs: logsPanelContribution,
});

const editorRendererRegistry = createEditorRendererRegistry({
  event: graphDocumentEditorContribution,
  function: graphDocumentEditorContribution,
  chart: chartEditorContribution,
});
```

Workbench module 不导入 Graph、Chart、Project、Data 等业务模块。它只接收注册表和 typed contribution。

## 6. 统一 Editor Resource Panel

Dockview 不再注册两个业务 component：

```text
GraphEditor
WorksheetEditor
```

最终只注册一个通用 component：

```text
EditorResource
```

metadata 决定实际内容：

```ts
type EditorResourceKind = "event" | "function" | "chart";

type EditorPanelMetadata = {
  role: "editor";
  resourceRef: string;
  resourceKind: EditorResourceKind;
  pinned?: boolean;
  sticky?: boolean;
};
```

渲染注册表：

```ts
const editorRendererRegistry = {
  event: GraphDocumentEditor,
  function: GraphDocumentEditor,
  chart: ChartEditor,
} satisfies EditorRendererRegistry;
```

这样 Dockview 不需要知道具体业务组件，`LayoutTab.component` 可以删除。

## 7. 删除平行 Tab 模型

最终删除：

```text
LayoutTab
LayoutTabType
LayoutTabComponent
EditorGroupSnapshot
layoutTabModel.ts
layoutTabQueries.ts
dockviewTabProjection.ts
useTabManagement.ts
```

统一使用：

```text
WorkbenchPanelInfo
EditorPanelMetadata
EditorResourceTarget
```

尚未打开的资源使用请求模型，而不是 tab 模型：

```ts
interface EditorResourceTarget {
  resourceRef: string;
  resourceKind: EditorResourceKind;
  pinned?: boolean;
  sticky?: boolean;
}
```

统一查询：

```ts
getActiveEditorPanel();
getActiveEditorPanelInGroup(groupId);
listEditorPanelsInGroup(groupId);
findEditorPanelsByResource(resourceRef);
```

命令名称统一使用 panel 语义：

```text
openEditorPanel
activateEditorPanelAndSyncSession
requestCloseEditorPanel
requestCloseEditorPanels
splitEditorPanel
buildEditorPanelTabMenu
```

Dockview 管理 tab；Application 只执行打开、关闭前确认和激活后的业务同步。

## 8. Chart 资源模型

当前 Worksheet contract 实际只保存单张图表，因此全栈收敛为 Chart：

```text
WorksheetDocument       → ChartDocument
WorksheetResourcePath   → ChartResourcePath
WorksheetEditor         → ChartEditor
WorksheetPreviewPayload → ChartPreviewPayload
WorksheetService        → ChartService
useWorksheetStore       → useChartDocumentStore
worksheet resource kind → chart
worksheets/              → charts/
.yssbi-worksheet         → .yssbi-chart
```

必须保持三个概念分离：

```text
ChartDocument       用户持久化的图表配置
ChartPreviewPayload 后端计算后的预览数据
ChartModel          与数据来源无关的通用渲染模型
```

`ChartRenderer` 只消费 `ChartModel`，不依赖 Chart document、IPC、store、Graph 或 Result workflow。

按照当前 0.x 策略，该更名直接替换旧路径，不保留 Worksheet/Chart 双协议或兼容 reader。

## 9. 模块可见性

每个业务模块只能通过根 `public.ts` 暴露能力：

```ts
// modules/chart/public.ts
export { ChartEditor } from "./internal/ui/ChartEditor";
export type { ChartDocument } from "./internal/domain/ChartDocument";
export { chartPanelContribution } from "./internal/chartPanelContribution";
```

模块外禁止 deep import：

```ts
// 禁止
import { ChartPreview } from "@/modules/chart/internal/ui/ChartPreview";

// 允许
import { chartPanelContribution } from "@/modules/chart/public";
```

依赖方向：

```text
app composition
    ↓
module public APIs
    ↓
module internal ui/controller
    ↓
module application/state/domain
    ↓
services/shared
```

不同业务模块之间禁止直接导入：

```text
graph-editor ✕ chart
chart ✕ results
project-explorer ✕ graph-editor
details ✕ graph-editor internals
```

跨模块交互统一放入：

```text
app/windows/workbench/integrations/
```

## 10. UI Component 边界

### 10.1 Controller 与 View 分离

```text
GraphNodeController
  ├─ 读取 projection/runtime state
  ├─ 生成 GraphNodeViewModel
  └─ 绑定 GraphNodeActions
       ↓
GraphNodeView
  ├─ 只接收 props
  ├─ 不读取 Store
  ├─ 不调用 Service
  └─ 不依赖 Dockview
```

### 10.2 组件通信规则

- 父组件可以组合子组件。
- 子组件不能反向导入父组件。
- 兄弟组件通过父级 ViewModel/Actions 通信。
- 业务 View 不读取其他模块 Store。
- 叶子 View 不调用 Application workflow。
- 跨 panel 状态只通过 app integration 或 typed contribution 协调。

### 10.3 Panel scope 必须显式传递

所有 editor content 统一接收：

```ts
interface EditorPanelScope {
  panelInstanceId: string;
  groupId: string;
  resourceRef: string;
  resourceKind: "event" | "function" | "chart";
}
```

Graph 和 Chart 都只使用自己的 `resourceRef`，禁止通过 `GroupContext + activeTabId` 反推资源身份。

## 11. 命名规范

| 类型              | 命名规则                     | 示例                          |
| ----------------- | ---------------------------- | ----------------------------- |
| 完整窗口          | `*Window`                    | `WorkbenchWindow`             |
| Dockview 宿主     | `*DockviewHost`              | `RootDockviewHost`            |
| Dockview adapter  | `*DockPanel`                 | `EditorResourceDockPanel`     |
| 实际 panel 内容   | `*Panel` / `*Editor`         | `DetailsPanel`, `ChartEditor` |
| 纯展示组件        | `*View`                      | `GraphNodeView`               |
| 状态连接组件      | `*Controller`                | `GraphPinController`          |
| 应用流程          | `*Coordinator` / `*Commands` | `ChartPreviewCoordinator`     |
| 后端投影状态      | `*ProjectionStore`           | `GraphProjectionStore`        |
| 临时 UI 状态      | `*UiState`                   | `ShellOverlayUiState`         |
| Dockview tab 渲染 | `*TabRenderer`               | `RootPanelTabRenderer`        |
| 注册表            | `*Registry`                  | `RootPanelRegistry`           |

当前名称建议替换：

| 当前名称                      | 目标名称                                 |
| ----------------------------- | ---------------------------------------- |
| `EditorWindow`                | `WorkbenchWindow`                        |
| `EditorView`                  | `WorkbenchWindow` 或拆成独立 panel views |
| `Workspace`                   | `RootDockviewHost`                       |
| `WorkbenchDockviewPanels`     | `RootPanelRegistry`                      |
| `WorkbenchDockviewTab`        | `RootPanelTabRenderer`                   |
| `WorkbenchEditorPanel`        | `EditorResourceDockPanel`                |
| `GraphEditor`                 | `GraphDocumentEditor`                    |
| `LogWorkspaceDockview`        | `LogDomainDockviewHost`                  |
| `BottomBar`                   | `StatusBar`                              |
| `features/application/layout` | `modules/workbench/internal/application` |
| `features/core/workbench`     | `modules/workbench/internal/ui`          |
| `openEditorTab`               | `openEditorPanel`                        |
| `switchEditorTab`             | `activateEditorPanelAndSyncSession`      |
| `tabCommands`                 | `editorPanelCloseCommands`               |
| `tabContextMenu`              | `editorPanelTabMenu`                     |
| `useTabManagement`            | 删除或改成窄 `useEditorPanelCommands`    |
| `LayoutTab`                   | 删除                                     |

禁止继续引入无法表达边界的名称：

```text
EditorView
Workspace
Layout
BottomBar
TabManagement
WorkbenchStore
Node
Pin
Canvas
```

## 12. Workbench 与 Dockview 的保留逻辑

以下不是重复 authority，应保留：

- Dockview runtime 中的临时 shadow transaction。
- dirty-close coordinator。
- Dockview snapshot persistence。
- panel-local selection/viewport state。
- graph session 与 backend residency state。
- 自定义 Dockview tab renderer。

需要删除或收敛的重复逻辑：

- `LayoutTab` 与 `EditorPanelMetadata` 的一一映射。
- 多套 Dockview→Tab projection。
- 重复的 group workspace hooks。
- 未实际执行 bind 的重复 `useWorkbenchLayout()` 调用。
- 同时组合 tab/project/graph/database/node/worksheet 的全局 `EditorSessionCommands`。
- 同时控制 Project/Nodes/Data/Commands 的宽 `WorkbenchActivityPanelsProvider`。

## 13. 分阶段迁移顺序

### Phase 1：Tab authority 收敛

1. 删除 `LayoutTab` 平行模型。
2. 所有 tab 查询直接返回 Dockview panel。
3. 合并 Dockview tab/group projection。
4. 将 tab workflow 重命名为 panel workflow。
5. 删除无效的第二次 `useWorkbenchLayout()` 调用。

验收条件：

```text
LayoutTab = 0
第二 topology store = 0
Dockview tab projection implementation = 1
```

### Phase 2：Workbench Shell 收敛

1. `EditorWindow` 改为 `WorkbenchWindow`。
2. `Workspace` 改为 `RootDockviewHost`。
3. 建立 `RootPanelRegistry`。
4. 建立统一 `EditorResourceDockPanel`。
5. 将 Settings/Node Documentation 移入 `WorkbenchOverlayHost`。

### Phase 3：Worksheet → Chart 全栈切换

1. Rust document/resource/path/layout owner 改名。
2. Project model/history/lifecycle/watcher 改名。
3. Tauri commands/events/DTO/error codes 改名。
4. 前端 service/store/application/ui 改名。
5. Dockview metadata resource kind 改为 `chart`。
6. 更新目录、扩展名、i18n、测试和文档。

不得保留 Worksheet/Chart 双路径。

### Phase 4：Activity Panels 解耦

1. 拆分 Project、Nodes、Data、Commands controller。
2. 删除宽 `WorkbenchActivityPanelsProvider`。
3. sidebar-to-editor DnD 移到 app integration。
4. 每个 Activity panel 只暴露一个 public contribution。

### Phase 5：EditorSession 拆分

1. 删除全局命令合集。
2. Graph、Chart、Menubar、Project、Details 使用各自窄 command capability。
3. 将跨模块 workflow 上移到 app integration。

### Phase 6：Graph UI 组件化

1. `Pin` 拆成 `GraphPinController + GraphPinView`。
2. `Node` 拆成 `GraphNodeController + GraphNodeView`。
3. `Canvas` 拆成 `GraphCanvasController + GraphCanvasView`。
4. ContextMenu、overlay、execution badge 改为显式 props/slot。

### Phase 7：Shared 清理

1. 单模块类型迁回所属模块。
2. `shared/types/dto` 只保留 IPC wire。
3. `shared/charts` 只保留 source-independent ChartModel/renderer。
4. `shared/ui` 只保留无业务语义的展示组件。

## 14. 架构门禁

最终必须由 production architecture audit 保证：

```text
跨模块 internal deep import = 0
模块级循环依赖 = 0
业务组件直接导入 Dockview = 0
View 叶子组件直接导入 Store/Service = 0
第二套 tab/group topology = 0
LayoutTab 等平行 tab model = 0
非 app composition 的跨业务 panel import = 0
```

还应增加以下规则：

1. 模块外只能导入 `modules/<name>/public.ts`。
2. Workbench module 不得导入具体业务 panel。
3. 只有 `RootPanelRegistry` 可以组合多个 panel contribution。
4. 只有 `editorRendererRegistry` 可以映射 editor resource kind 到具体 editor。
5. Dockview raw API 只能出现在 Workbench Dockview adapter 内。
6. React store write 只能由所属模块 application/publication owner 执行。
7. `ChartRenderer` 不得依赖 Chart document、Result、Graph、Store 或 IPC。

## 15. 最终核心关系

```text
WorkbenchWindow
    ↓ app composition
Root Dockview
    ↓ panel metadata
DockPanel Adapter
    ↓ explicit props
Business Editor / Panel
```

最终原则：

```text
Dockview 管理 panel/tab/group
Workbench 管理规则和生命周期
Panel Adapter 连接 Dockview 与业务组件
业务组件只管理自己的内容
跨模块交互只发生在 app composition/integration
```
