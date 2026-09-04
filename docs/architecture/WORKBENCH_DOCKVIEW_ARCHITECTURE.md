# Workbench Dockview 当前架构

本文描述主编辑窗口当前的布局 authority、panel identity、应用 seam、生命周期与持久化 contract。Dockview live instance 保存物理布局事实；React application modules 只通过语义 interface 协调用例。

## 1. 渲染层级与 authority

`WorkbenchComposition` 组装的 chrome 层级固定为：

```text
WorkbenchWindow
├─ WorkbenchMenuBar slot
├─ body
│  └─ RootDockviewHost                 # 唯一 root DockviewReact
│     ├─ native left Activity edge group
│     │  ├─ Project
│     │  ├─ Nodes
│     │  ├─ Data
│     │  └─ Commands
│     ├─ grid groups：editor、Result 与 tool panels 可混排和分割
│     ├─ native right edge group：Details（fixed）、Assistant、Inspect、Result 的 home
│     └─ native bottom edge group
│        ├─ 上方 content：Logs、Output 或 Problems
│        └─ 下方 tabs：Logs、Output、Problems
├─ StatusBar slot
└─ WorkbenchOverlayHost
```

Menu、StatusBar、dialogs 与 modal overlays 位于 root Dockview 外。工作台层只有一个 root `DockviewReact`；它直接承载四个受限 Activity panels、editor、Result、Details、Assistant、Inspect、Logs、Output 与 Problems。Activity panel tabs 使用 Dockview 原生 vertical header，只能在 `workbench-edge-left` 内重排；普通 panel 不能拖入该 group，Activity panel 不能拖出。

`src/app/windows/workbench/rootPanelRegistry.tsx` 是唯一同时组合多个业务 panel contribution 的位置，
`editorRendererRegistry.ts` 是唯一把 event/function/chart 映射到具体 editor 的位置。Workbench module
只接收 typed registries、tab renderer、activation/DnD capabilities 与 chrome slots，不导入具体业务模块。
当前 registry 分别从 `src/modules/logs/public.ts`、`src/modules/output/public.ts` 和
`src/modules/problems/public.ts` 组合 `LogDomainDockviewHost`、`RunOutputPanel` 与
`GraphProblemsPanel`。Logs 只拥有 operational logs；Output 只拥有 execution output；Problems 只读取
完整 Graph Projection，三个 panel 不通过 `modules/logs` 聚合。

root Dockview 是以下物理事实的唯一 authority：

- grid/edge topology 与 group membership；
- group 和 edge sizes；
- panel 顺序与 split 方向；
- active group 与 active panel；
- edge group 的位置、可见性、尺寸和 collapsed state。

`useWorkbenchUiStore` 只保存 Settings/Dialog 等非 placement UI state。Zustand 不保存 panel placement、visibility、sizes、tab order、Activity active tab 或 edge collapse 的镜像。

直接 invariant：工作台不存在 `Gridview`、shell Dockview 或 editor nested Dockview compatibility model，也不存在第二套 application-owned topology。root 内的 native Dockview drag/drop 是 panel 移动、分组和排序的物理 authority；floating groups 与 browser popouts 禁用。

## 2. Root panel 角色与默认 home

root group 可以混合承载不同角色；唯一例外是 Activity group。角色决定内容和应用语义，Activity group 还受到固定成员和 drop policy 约束：

| 角色             | 内容                        | deterministic home                  |
| ---------------- | --------------------------- | ----------------------------------- |
| `editor`         | Graph/Function/Chart editor | 当前 central grid group             |
| `view:project`   | Project activity panel      | left Activity edge                  |
| `view:nodes`     | Nodes activity panel        | left Activity edge                  |
| `view:data`      | Data activity panel         | left Activity edge                  |
| `view:commands`  | Commands activity panel     | left Activity edge                  |
| `view:details`   | permanent fixed Details     | right edge index 0                  |
| `view:assistant` | movable/closable Assistant  | right edge index 1 on default/reset |
| `view:inspect`   | contextual Inspect          | right edge                          |
| `result`         | 一个可检查结果              | right edge                          |
| `view:logs`      | Logs workspace              | bottom edge                         |
| `view:output`    | Run Output                  | bottom edge                         |
| `view:problems`  | Graph Problems              | bottom edge                         |

默认空布局建立 central grid group，并放置：

- Project、Nodes、Data、Commands：同一个 left Activity edge group，宽度 `292`，默认顺序为 Project → Nodes → Data → Commands；
- Logs、Output、Problems：bottom edge，高度 `200`，顺序为 Logs → Output → Problems；
- bottom edge header 位于底部，因此 content 在上、tabs 在下。

right edge 默认宽度 `320`。Details 始终由默认/恢复/reset 流程安装在 canonical right edge index 0，并且是唯一 permanent/fixed panel；Assistant 默认紧邻 Details，但作为普通 singleton 可移动、split、关闭。Inspect 仍按有效 editor/node context 延迟创建；Result 允许多个实例，但每个 `resultKey` 只对应一个 canonical panel。Activity panels 始终由默认布局安装，不能由 close coordinator 删除；Activity edge 的可见性通过 root edge 的 visible/collapsed state 控制。

Problems 只使用 `viewId: "problems"` 与 registry component `Problems`。Layout parser 只接受当前
exact envelope 与 canonical panel identity，不执行旧 ID 转换或 alternate read。

## 3. 唯一有界 nested Dockview：Logs

Logs panel 内包含工作台唯一的 bounded nested Dockview。它只拥有七个 operational log domain panels：

1. `all`
2. `application`
3. `execution`
4. `system`
5. `graph`
6. `data`
7. `ui`

该 nested Dockview 不拥有 root editor、Result 或 tool panels，也不参与 root topology。它不桥接任何 drag/drop：root panel 不能进入 Logs nested Dockview，domain panel 也不能进入 root；domain panel 的分组、顺序和 split 始终限制在 Logs host 内。

Logs layout 有两种明确生命周期：

- **main**：主窗口 Logs 通过 `logsDockviewRootBinding` 绑定；Application
  `workbenchLayoutController` 负责 hydration、project replacement 与 persistence，最新 nested
  snapshot 作为 `nested.logs` 随工作台 payload 持久化；
- **ephemeral**：独立 `LogWindow` 每次挂载都从七 domain 默认布局开始，不绑定 main controller，也不读写工作台 layout persistence。

## 4. Canonical metadata 与 identity

`WorkbenchPanelMetadata` 是 root panel 的 canonical metadata：

```text
editor → { role, resourceRef, resourceKind, pinned?, sticky? }
view   → { role, viewId }
result → { role, resultKey, resultId, title, presentation, source }
```

以下 identity 永远分离：

| Identity          | 含义                                                                      |
| ----------------- | ------------------------------------------------------------------------- |
| `resourceRef`     | editor 打开的 opaque backend resource path；同一资源可有多个 editor panel |
| `resultKey`       | logical Result panel key；同 key 执行 upsert，不同 key 可并存             |
| `resultId`        | Rust `ResultStore` 中当前 payload 的 opaque identity                      |
| `panelInstanceId` | 一个 root Dockview panel instance 的物理 identity                         |
| `groupId`         | Dockview 当前物理 group 的 identity；panel 移动后可改变                   |

不得从 `panelInstanceId` 或 `groupId` 推导 `resourceRef`、`resultKey` 或 `resultId`，也不得把这些 identity 合并为一个 tab id。

Singleton 与 multi-instance contract：

- Project、Nodes、Data、Commands、Details、Assistant、Inspect、Logs、Output、Problems 由 `viewId` 保证 singleton；
- Project、Nodes、Data、Commands 随默认 Activity group 安装且保持存在；
- Details 是 permanent fixed singleton；
- Assistant 是普通 layout-persisted singleton；
- Inspect 只在上下文有效时按需创建；
- Result 按 `resultKey` upsert，同 key 更新 metadata 并 reveal，多个不同 `resultKey` 同时存在；
- `resultId` 可以在同一个 `resultKey` panel 上更新，而不改变其 `panelInstanceId`。

## 5. Module seams 与布局 mutation

### 5.1 Public seam

`src/modules/workbench/public.ts` 将能力拆成独立的 `workbenchDockviewRead`、
`workbenchDockviewControl` 和 `workbenchDockviewRootBinding`。它们提供 role-aware semantic
operations：

- `openEditor`、`setEditorPinned`；
- `ensureView`、`upsertResult`；
- `activate`、`reveal`、`move`、`split`；
- `configureEdge`、`setEdgeCollapsed`、`setEdgeSize`；
- canonical panel/group queries、resource remap 与 serialization。

Application 负责组合这些能力；调用方不持有 raw root `DockviewApi`，也不自行实现
singleton、Result upsert、home edge 或 reveal 规则。

### 5.2 Internal seam

`src/modules/workbench/internal/dockview/workbenchDockviewInternal.ts` 保存 hydration、committed
removal、layout transaction 与 publication transaction。它不从 Workbench root `public.ts`
导出；这些能力不属于普通 module 或 application caller 的 public interface。
`modules/workbench/internal/application/workbenchLayoutController.ts` 负责 window-scoped bind、
startup hydration、persistence flush 与 project-generation invalidation。

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
6. 仅对已经物理移除的 panel 释放 pane、viewport、graph session 或 chart document state。

并发 close workflow 串行化；取消、stale token 或 project replacement 都不会提前释放 domain state。

### 6.2 物理命令

命令以 root Dockview 的实时 group 为准：

- `Ctrl+Tab` 在 active physical group 的全部 canonical panels 间循环；
- Close Group 关闭该 physical group 中 editor、Result 和 tool panels 的完整集合；若 group 同时包含 fixed Details，现有 close coordinator 拒绝整批关闭，Assistant 只能单独关闭；Assistant 移到不含 fixed panel 的普通 group 后沿用 Close Group；
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

Reveal 已存在的 panel 时保持其实际位置，不把它搬回 deterministic home；若位于 edge group，则显示并展开该 edge。缺失的 singleton 才在 home edge 创建。Details 由 permanent placement 规则固定；缺失 Assistant 通过 View 菜单在 Details 后创建并激活；Inspect 创建还要求有效 context；同 `resultKey` 的 Result 只更新并 reveal 既有 panel。

### 7.2 Reset

Reset 使用一个 runtime shadow layout transaction，并保留既有 editor、Result 与 panel identities：

- Project、Nodes、Data、Commands 回到同一个 left Activity edge group，并恢复 Activity tab 顺序；
- editor panels 按 deterministic snapshot order 集中到 central grid group；
- Details 与 Assistant 始终确保存在并回到 right edge index 0/1；Inspect、Result 回到其后，reset 不凭空创建 Inspect/Result；
- Logs、Output、Problems 回到 bottom edge，恢复 Logs → Output → Problems 顺序与 bottom tabs；
- left/right/bottom 恢复 `292/320/200`，相关 edge 展开；
- main Logs nested Dockview 恢复七 domain 默认布局；
- 优先恢复 reset 前 physically active editor，其次恢复仍有效的 focused editor，再次选择第一个 editor；无 editor 时激活 Project。

### 7.3 Project replacement

Project replacement 先使 pending root operations、hydration generation 与 resources-ready callback 失效，再在当前 FIFO 中移除 project-scoped panels：

- 所有 editor；
- 所有 Result；
- Inspect。

随后清理 editor pane/session 与 project-scoped detail state。Project、Nodes、Data、Commands、Details、Assistant、Logs、Output、Problems 及 Logs domain layout 保留；持久化 root 中的 editor、Result、Inspect 会被 scrub，避免新 project hydration 打开旧 project 内容。Problems panel 保留与否不影响 `GraphProjectionStore` 生命周期；Canvas、Details 和 Run Gate 仍从同一完整 projection 更新。

## 8. Persistence contract

每个窗口只使用以下 key：

```text
yssbi-workbench-layout:<window-label>
```

value 是不含版本字段的 exact envelope：

```text
{
  root: SerializedDockview,
  nested: {
    logs: SerializedDockview
  }
}
```

Persistence invariant：

- payload 不包含 `version` field，storage key 保持 window-scoped semantic key；
- top-level 只接受 `root` 与 `nested`，`nested` 只接受 `logs`；
- root 与 `nested.logs` 独立验证；某一 snapshot 非法时只把该部分恢复为默认布局；
- 任一 root 或 main Logs 变化都会调度完整 payload 写入；window close 在当前 hydration 与 FIFO idle 后直接 flush；
- Result 与 Inspect 是 transient/project-scoped panels，写入前从 `root` snapshot 及其空 topology 中剔除；editor 也随 project-scoped scrub 移除；Details 与 Assistant 是持久化 root topology，Activity panels 作为固定 left edge 成员保留；用户关闭 Assistant 后，缺失状态会随 snapshot 保留，startup restore 不会自动重建；
- main Logs nested snapshot 持久化，ephemeral standalone Logs 不持久化。

非 canonical envelope 会被拒绝并回退默认布局；parser 不提供 alternate reader 或迁移路径。若未来需要 breaking persistence format，直接使用新的 semantic storage key。

## 9. 视觉尺寸层级

工作台 chrome 使用以下紧凑层级：

|   高度 | 用途                             | Token                                                             |
| -----: | -------------------------------- | ----------------------------------------------------------------- |
| `36px` | Menubar/titlebar chrome          | `--titlebar-height`                                               |
| `32px` | root tabs 与 collapsed root edge | `--workbench-tab-height` / `DockviewTheme.edgeGroupCollapsedSize` |
| `30px` | Logs nested domain tabs          | `--logs-tab-height`                                               |
| `28px` | panel toolbar                    | `--panel-toolbar-height`                                          |
| `26px` | Status Bar                       | `--statusbar-height`                                              |

## 10. Verification

验证命令以 [本地开发工作流](../development/LOCAL_WORKFLOW.md) 为准。工作台改动期间运行
受影响的 `pnpm test:ts <path>`；交付前按改动范围运行 TypeScript check、完整 Frontend
tests 与 Frontend architecture gate，不在本文维护易漂移的测试文件库存。
