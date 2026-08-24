# Workbench Dockview 当前架构

本文描述主编辑窗口当前的布局 authority、panel identity、应用 seam、生命周期与持久化 contract。Dockview live instance 保存物理布局事实；React application modules 只通过语义 interface 协调用例。

## 1. 渲染层级与 authority

`EditorWindow` 的 chrome 层级固定为：

```text
EditorWindow
├─ Menubar
├─ body
│  └─ root Dockview
│     ├─ native left Activity edge group
│     │  ├─ Project
│     │  ├─ Nodes
│     │  ├─ Data
│     │  └─ Commands
│     ├─ grid groups：editor、Result 与 tool panels 可混排和分割
│     ├─ native right edge group：Details、Inspect、Result 的 contextual home
│     └─ native bottom edge group
│        ├─ 上方 content：Logs 或 Output
│        └─ 下方 tabs：Logs、Output
└─ Status Bar
```

Menubar、Status Bar、dialogs 与 modal overlays 位于 root Dockview 外。工作台层只有一个 root `DockviewReact`；它直接承载四个受限 Activity panels、editor、Result、Details、Inspect、Logs 与 Output。Activity panel tabs 使用 Dockview 原生 vertical header，只能在 `workbench-edge-left` 内重排；普通 panel 不能拖入该 group，Activity panel 不能拖出。

root Dockview 是以下物理事实的唯一 authority：

- grid/edge topology 与 group membership；
- group 和 edge sizes；
- panel 顺序与 split 方向；
- active group 与 active panel；
- edge group 的位置、可见性、尺寸和 collapsed state。

`useWorkbenchStore` 只保存 Settings/Dialog 等非 placement UI state。Zustand 不保存 panel placement、visibility、sizes、tab order、Activity active tab 或 edge collapse 的镜像。

直接 invariant：工作台不存在 `Gridview`、shell Dockview 或 editor nested Dockview compatibility model，也不存在第二套 application-owned topology。root 内的 native Dockview drag/drop 是 panel 移动、分组和排序的物理 authority；floating groups 与 browser popouts 禁用。

## 2. Root panel 角色与默认 home

root group 可以混合承载不同角色；唯一例外是 Activity group。角色决定内容和应用语义，Activity group 还受到固定成员和 drop policy 约束：

| 角色 | 内容 | deterministic home |
|---|---|---|
| `editor` | Graph/Function/Worksheet editor | 当前 central grid group |
| `view:project` | Project activity panel | left Activity edge |
| `view:nodes` | Nodes activity panel | left Activity edge |
| `view:data` | Data activity panel | left Activity edge |
| `view:commands` | Commands activity panel | left Activity edge |
| `view:details` | contextual Details | right edge |
| `view:inspect` | contextual Inspect | right edge |
| `result` | 一个可检查结果 | right edge |
| `view:logs` | Logs workspace | bottom edge |
| `view:output` | Run Output | bottom edge |

默认空布局建立 central grid group，并放置：

- Project、Nodes、Data、Commands：同一个 left Activity edge group，宽度 `292`，默认顺序为 Project → Nodes → Data → Commands；
- Logs、Output：bottom edge，高度 `200`，顺序为 Logs → Output；
- bottom edge header 位于底部，因此 content 在上、tabs 在下。

right edge 在首个 contextual panel 出现时建立，默认宽度 `320`。Details 与 Inspect 是按上下文延迟创建的 singleton；无有效 Details/Inspect context 时不会创建。Result 允许多个实例，但每个 `resultKey` 只对应一个 canonical panel。Activity panels 始终由默认布局安装，不能由 close coordinator 删除；Activity edge 的可见性通过 root edge 的 visible/collapsed state 控制。

## 3. 唯一有界 nested Dockview：Logs

Logs panel 内包含工作台唯一的 bounded nested Dockview。它只拥有七个 diagnostic domain panels：

1. `all`
2. `application`
3. `execution`
4. `system`
5. `graph`
6. `data`
7. `ui`

该 nested Dockview 不拥有 root editor、Result 或 tool panels，也不参与 root topology。它不桥接任何 drag/drop：root panel 不能进入 Logs nested Dockview，domain panel 也不能进入 root；domain panel 的分组、顺序和 split 始终限制在 Logs host 内。

Logs layout 有两种明确生命周期：

- **main**：主窗口 Logs 绑定 `logsDockviewLayoutController`，最新 nested snapshot 作为 `nested.logs` 随工作台 payload 持久化；
- **ephemeral**：独立 `LogWindow` 每次挂载都从七 domain 默认布局开始，不绑定 main controller，也不读写工作台 layout persistence。

## 4. Canonical metadata 与 identity

`WorkbenchPanelMetadata` 是 root panel 的 canonical metadata：

```text
editor → { role, resourceRef, resourceKind, pinned?, sticky? }
view   → { role, viewId }
result → { role, resultKey, resultId, title, presentation, source }
```

以下 identity 永远分离：

| Identity | 含义 |
|---|---|
| `resourceRef` | editor 打开的 opaque backend resource path；同一资源可有多个 editor panel |
| `resultKey` | logical Result panel key；同 key 执行 upsert，不同 key 可并存 |
| `resultId` | Rust `ResultStore` 中当前 payload 的 opaque identity |
| `panelInstanceId` | 一个 root Dockview panel instance 的物理 identity |
| `groupId` | Dockview 当前物理 group 的 identity；panel 移动后可改变 |

不得从 `panelInstanceId` 或 `groupId` 推导 `resourceRef`、`resultKey` 或 `resultId`，也不得把这些 identity 合并为一个 tab id。

Singleton 与 multi-instance contract：

- Project、Nodes、Data、Commands、Details、Inspect、Logs、Output 由 `viewId` 保证 singleton；
- Project、Nodes、Data、Commands 随默认 Activity group 安装且保持存在；
- Details、Inspect 只在上下文有效时 lazy ensure；
- Result 按 `resultKey` upsert，同 key 更新 metadata 并 reveal，多个不同 `resultKey` 同时存在；
- `resultId` 可以在同一个 `resultKey` panel 上更新，而不改变其 `panelInstanceId`。

## 5. Module seams 与布局 mutation

### 5.1 Public seam

`src/features/core/dockview/workbenchDockviewPort.ts` 定义 public `workbenchDockviewPort`。它提供 role-aware semantic operations：

- `openEditor`、`setEditorPinned`；
- `ensureView`、`upsertResult`；
- `activate`、`reveal`、`move`、`split`；
- `configureEdge`、`setEdgeCollapsed`、`setEdgeSize`；
- canonical panel/group queries、resource remap 与 serialization。

调用方不持有 raw root `DockviewApi`，也不自行实现 singleton、Result upsert、home edge 或 reveal 规则。

### 5.2 Internal seam

`src/features/core/dockview/workbenchDockviewInternal.ts` 保存 bind/hydration、committed removal、layout transaction 与 publication transaction。它不从 `src/features/core/dockview/index.ts` barrel export；这些能力不属于普通 view 或 application caller 的 public interface。

root `fromJSON` 只由 startup `workbenchLayoutController` 在空 root 上执行。运行时 reset、project cleanup、publication 和复合布局修改都使用 FIFO 中的 `ShadowWorkbenchModel` transaction：先从 live snapshot 构造 shadow、执行同步语义命令并验证 identity/topology/currentness，再把 buffered commands 应用到 live Dockview。运行时不使用 root `fromJSON` 重建布局。

`workbenchLayoutController` 负责 window-scoped bind、hydration gate、project-resources readiness、debounced persistence 与 close-time flush。普通 semantic operations 可以在 ready 前进入 FIFO，但只会在当前 binding 完成 hydration 后执行。

## 6. Close、物理命令与 editor focus gate

### 6.1 Close coordinator

所有 root tab 关闭入口都进入 `requestCloseWorkbenchPanel(s)`：close button、中键、context menu、`Ctrl+W`、view toggle 和 Close Group 不直接调用 Dockview close。

Coordinator 按顺序执行：

1. 捕获 `panelInstanceId + groupId + metadata` commit tokens；project-scoped panel 同时捕获 project identity。
2. 计算哪些 editor document 将失去最后一个 panel。
3. 对 dirty document 执行 save/discard/cancel confirmation。
4. 在 FIFO 内重新校验 token 与 project identity。
5. 通过 internal `commitRemove` 执行物理 close。
6. 仅对已经物理移除的 panel 释放 pane、viewport、graph session 或 worksheet document state。

并发 close workflow 串行化；取消、stale token 或 project replacement 都不会提前释放 domain state。

### 6.2 物理命令

命令以 root Dockview 的实时 group 为准：

- `Ctrl+Tab` 在 active physical group 的全部 canonical panels 间循环；
- Close Group 关闭该 physical group 中 editor、Result 和 tool panels 的完整集合；
- editor tab 的 Close Others、Close All、Close Saved 只筛选该 group 中的 `editor` role；
- split 作用于 active canonical editor，native Dockview drag/drop 继续拥有后续物理移动与顺序。

### 6.3 Editor focus gate

Editor mutation/selection/save shortcuts 必须先通过 `editorCommandFocus`：

- 目标必须是 root Dockview 当前 physically active 的 `editor` panel；
- 捕获并在执行前重验 `panelInstanceId`、`groupId`、`resourceRef`、`resourceKind` 与 project identity；
- tool 或 Result 激活、panel/group 改变、project replacement 都使旧 target 失效；
- application modal、dialog、menu、input、contenteditable、popover 等 shortcut consumer 会阻止 editor command。

因此 focused session 投影不能替代 physical active panel 判定。

## 7. Reveal、reset 与 project replacement

### 7.1 Reveal

Reveal 已存在的 panel 时保持其实际位置，不把它搬回 deterministic home；若位于 edge group，则显示并展开该 edge。缺失的 singleton 才在 home edge 创建。Details/Inspect 创建还要求有效 context；同 `resultKey` 的 Result 只更新并 reveal 既有 panel。

### 7.2 Reset

Reset 使用一个 runtime shadow layout transaction，并保留既有 editor、Result 与 panel identities：

- Project、Nodes、Data、Commands 回到同一个 left Activity edge group，并恢复 Activity tab 顺序；
- editor panels 按 deterministic snapshot order 集中到 central grid group；
- 已存在的 Details、Inspect、Result 回到 right edge；reset 不凭空创建 Details/Inspect；
- Logs、Output 回到 bottom edge，恢复 Logs → Output 顺序与 bottom tabs；
- left/right/bottom 恢复 `292/320/200`，相关 edge 展开；
- main Logs nested Dockview 恢复七 domain 默认布局；
- 优先恢复 reset 前 physically active editor，其次恢复仍有效的 focused editor，再次选择第一个 editor；无 editor 时激活 Project。

### 7.3 Project replacement

Project replacement 先使 pending root operations、hydration generation 与 resources-ready callback 失效，再在当前 FIFO 中移除 project-scoped panels：

- 所有 editor；
- 所有 Result；
- Details 与 Inspect。

随后清理 editor pane/session 与 project-scoped detail state。Project、Nodes、Data、Commands、Logs、Output 及 Logs domain layout 保留；持久化 root 中的 editor、Result、Details、Inspect 会被 scrub，避免新 project hydration 打开旧 project 内容。

## 8. Persistence contract

每个窗口只使用以下 key：

```text
yssbi-workbench-layout:<window-label>
```

value 是 exact envelope；不包含版本字段：

```text
{
  root: SerializedDockview,
  nested: {
    logs: SerializedDockview
  }
}
```

Persistence invariant：

- payload 没有 `version` field，也不使用 versioned key；当前格式不做迁移、不兼容旧 envelope，也不保留旧 reader；
- top-level 只接受 `root` 与 `nested`，`nested` 只接受 `logs`；
- root 与 `nested.logs` 独立验证；某一 snapshot 非法时只把该部分恢复为默认布局；
- 任一 root 或 main Logs 变化都会调度完整 payload 写入；window close 在当前 hydration 与 FIFO idle 后直接 flush；
- Details、Inspect、Result 是 session-only panels，写入前从 `root` snapshot 及其空 topology 中剔除；Activity panels 作为持久化 root topology 的固定 left edge 成员保留；
- main Logs nested snapshot 持久化，ephemeral standalone Logs 不持久化。

若未来需要 breaking persistence format，必须直接使用新的 semantic storage key；当前 key 下不增加版本字段、迁移逻辑或 alternate reader。

## 9. 视觉尺寸层级

工作台 chrome 使用以下紧凑层级：

| 高度 | 用途 | Token |
|---:|---|---|
| `36px` | Menubar/titlebar chrome | `--titlebar-height` |
| `32px` | root tabs 与 collapsed root edge | `--workbench-tab-height` / `DockviewTheme.edgeGroupCollapsedSize` |
| `30px` | Logs nested domain tabs | `--logs-tab-height` |
| `28px` | panel toolbar | `--panel-toolbar-height` |
| `26px` | Status Bar | `--statusbar-height` |

## 10. Focused verification

当前 contract 由以下 focused tests 覆盖：

- `src/views/EditorView/Layout/Workspace.editorPanel.test.tsx`：唯一 root Dockview、activation gates、root 与 Logs nested keyboard boundary；
- `src/features/core/dockview/workbenchDockviewPort.test.ts`：role-aware operations、identity、edge state、FIFO、committed removal、shadow transactions 与 public/internal interface；
- `src/features/core/dockview/workbenchPanelModel.test.ts`：canonical metadata validation 与 component mapping；
- `src/features/core/dockview/workbenchLayoutPersistence.test.ts`：semantic key、exact envelope、session-only stripping、project scrub 与七-domain Logs validation；
- `src/features/core/dockview/logsDockviewLayoutController.test.ts`：main Logs restore、snapshot 与 reset lifecycle；
- `src/views/LogView/LogWorkspaceDockview.test.tsx`：七个 bounded domain panels、main binding 与 ephemeral default；
- `src/features/application/layout/workbenchLayoutController.test.ts`：startup hydration、root `fromJSON` 限制、persistence flush 与 project generations；
- `src/features/application/layout/workbenchLayoutActions.test.ts`：home、reveal、bottom collapse 与 deterministic reset；
- `src/features/application/editor/workbenchPanelClose.test.ts`：batch close、dirty confirmation、commit currentness 与 finalization；
- `src/features/application/editor/editorCommandFocus.test.ts`：physical editor focus 与 shortcut-consumer gate；
- `src/features/application/project/projectWorkbenchLifecycle.test.ts`：project-scoped panel cleanup 与 lifecycle currentness。
