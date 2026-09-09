# Workbench Dockview 当前架构

> Status: Current
> Scope: root/nested Dockview authority、panel identity、布局操作、生命周期和持久化
> Canonical owners: Workbench 源码与测试拥有布局默认值；本文拥有稳定的布局和 identity contract
> Update when: root topology、panel role、close/reset/replacement 或 persistence contract 改变时

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
│     │  ├─ Commands
│     │  ├─ Plugins
│     │  └─ 插件贡献的 sidebar views（包已安装且启用时）
│     ├─ grid groups：editor、Result 与 tool panels 可混排和分割
│     ├─ native right edge group：Details（fixed）、Assistant、Inspect、Result 的 home
│     └─ native bottom edge group
│        └─ content：Logs、Output 或 Problems
├─ StatusBar slot
│  ├─ 最左侧：Settings 图标，位于 Activity Bar 下方
│  ├─ 左侧：Problems、Output、Logs 图标入口，与 central grid 左对齐
│  ├─ 右侧：节点、连线、选择与视口信息，与 central grid 右对齐
│  └─ 最右侧：Details、Assistant 图标入口
└─ WorkbenchOverlayHost
```

Menu、StatusBar、dialogs 与 modal overlays 位于 root Dockview 外。工作台层只有一个 root `DockviewReact`；它直接承载四个受限 Activity panels、editor、Result、Details、Assistant、Inspect、Logs、Output 与 Problems。Activity panel tabs 使用 Dockview 原生 vertical header，只能在 `workbench-edge-left` 内重排；普通 panel 不能拖入该 group，Activity panel 不能拖出。

Project sidebar 按 Events → Functions → Charts → Data 展示同级资源分类，共用一棵虚拟化树和分类展开状态。Project 与 Nodes 面板直接展示分类树，不提供顶部搜索输入区；分类可独立展开和收起，画布节点选择器保留自己的搜索入口。Data 分类提供导入入口，数据行保留拖拽、详情、数据库编辑窗口与右键管理操作；数据来自既有数据库投影，不再注册独立 Data Activity panel。

应用内 Dialog 统一通过点击遮罩空白区域或按 Escape 关闭；遮罩不参与原生窗口拖动。
调用方处理 `onOpenChange(false)` 更新弹窗状态，关闭弹窗不取消已经开始的后台操作。

`src/app/windows/workbench/rootPanelRegistry.tsx` 是唯一同时组合多个业务 panel contribution 的位置，
`editorRendererRegistry.ts` 是唯一把 event/function/chart 映射到具体 editor 的位置。Workbench module
只接收 typed registries、tab renderer、activation/DnD capabilities 与 chrome slots，不导入具体业务模块。
当前 registry 分别从 `src/modules/logs/public.ts`、`src/modules/output/public.ts` 和
`src/modules/problems/public.ts` 组合三个独立 panel contribution；Workbench 只拥有它们的位置和
生命周期。Problems、Results 与 Run Output 的业务语义见 [Graph 与 Execution](GRAPH_AND_EXECUTION.md)，
Logs 的业务语义见 [Runtime Signals](RUNTIME_SIGNALS.md)。

root Dockview 是以下物理事实的唯一 authority：

- grid/edge topology 与 group membership；
- group 和 edge sizes；
- panel 顺序与 split 方向；
- active group 与 active panel；
- edge group 的位置、可见性、尺寸和 collapsed state。

实现按职责分开：`workbenchDockviewOperations.ts` 提供元数据与 live Dockview 操作，`workbenchDockviewTransaction.ts` 保存一次待提交事务的临时布局和命令，`workbenchDockviewInternal.ts` 负责绑定、串行执行、hydration 与事件观察。事务接口由 `workbenchTypes.ts` 定义，临时事务不构成独立的已提交布局 authority。

`useWorkbenchUiStore` 只保存 Settings/Dialog 等非 placement UI state。Zustand 不保存 panel placement、visibility、sizes、tab order、Activity active tab 或 edge collapse 的镜像。

直接 invariant：工作台不存在 `Gridview`、shell Dockview 或 editor nested Dockview compatibility model，也不存在第二套 application-owned topology。root 内的 native Dockview drag/drop 是 panel 移动、分组和排序的物理 authority；floating groups 与 browser popouts 禁用。

Activity 底部固定显示 Plugins 原生 tab，替代原 Julia 入口。仅通过 CSS 将该 tab 排在标签列最下方，样式、选中状态与点击逻辑均复用上方 Activity tabs：点击已展开的当前 tab 收起 left edge；折叠时点击展开对应面板，点击其他 tab 则切换面板。不另设入口按钮或选中状态订阅。

## 2. Root panel 角色与默认 home

root group 可以混合承载不同角色；唯一例外是 Activity group。角色决定内容和应用语义，Activity group 还受到固定成员和 drop policy 约束：

| 角色             | 内容                        | deterministic home                  |
| ---------------- | --------------------------- | ----------------------------------- |
| `editor`         | Graph/Function/Chart editor | 当前 central grid group             |
| `view:project`   | Project activity panel      | left Activity edge                  |
| `view:nodes`     | Nodes activity panel        | left Activity edge                  |
| `view:commands`  | Commands activity panel     | left Activity edge                  |
| `view:details`   | permanent fixed Details     | right edge index 0                  |
| `view:assistant` | movable/closable Assistant  | right edge index 1 on default/reset |
| `view:inspect`   | contextual Inspect          | right edge                          |
| `result`         | 一个可检查结果              | right edge                          |
| `view:logs`      | Logs workspace              | bottom edge                         |
| `view:output`    | Run Output                  | bottom edge                         |
| `view:problems`  | Graph Problems              | bottom edge                         |

默认空布局建立 central grid group，并放置：

- Project、Nodes、Commands、Plugins：同一个 left Activity edge group，使用 `WORKBENCH_EDGE_SIZES.left`，默认顺序为 Project → Nodes → Commands → Plugins；
- Logs、Output、Problems：bottom edge，使用 `WORKBENCH_EDGE_SIZES.bottom`，顺序为 Problems → Output → Logs；
- bottom edge 仅包含 Problems、Output、Logs 时隐藏原生 header，由 Status Bar 图标切换；混入 editor 或其他 panel 时恢复原生 header，保留混合 group 的完整操作入口。

right edge 使用 `WORKBENCH_EDGE_SIZES.right`。Details 始终由默认/恢复/reset 流程安装在 canonical right edge index 0，并且是唯一 permanent/fixed panel；Assistant 默认紧邻 Details，但作为普通 singleton 可移动、split、关闭。Inspect 仍按有效 editor/node context 延迟创建；Result 允许多个实例，但每个 `resultKey` 只对应一个 canonical panel。Activity panels 始终由默认布局安装，不能由 close coordinator 删除；Activity edge 的可见性通过 root edge 的 visible/collapsed state 控制。三个 edge 的具体当前像素默认值只由 `src/modules/workbench/internal/dockview/workbenchDockviewDefaults.ts` 维护。

Problems 只使用 `viewId: "problems"` 与 registry component `Problems`。Layout parser 只接受当前
exact envelope 与 canonical panel identity，不执行旧 ID 转换或 alternate read。

旧 `Diagnostics` identity 没有迁移路径；含非 canonical identity 的 root snapshot 会回退默认布局。这是当前 0.x 的直接替换行为，已有本地布局可能因此重置。默认与 reset 的顺序统一由 WORKBENCH_BOTTOM_DEFAULT_ORDER 定义；有效已保存布局保留用户排序。

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

- Project、Nodes、Commands、Plugins、Details、Assistant、Inspect、Logs、Output、Problems 由 `viewId` 保证 singleton；
- Project、Nodes、Commands、Plugins 随默认 Activity group 安装且保持存在；
- Details 是 permanent fixed singleton；
- Assistant 是普通 layout-persisted singleton；
- Inspect 只在上下文有效时按需创建；
- Result 按 `resultKey` upsert，同 key 更新 metadata 并 reveal，多个不同 `resultKey` 同时存在；
- `resultId` 可以在同一个 `resultKey` panel 上更新，而不改变其 `panelInstanceId`。

Result panel 通过 `source` 的 output address 订阅当前结果；重算时卸载旧内容并显示运行状态，成功后在原 panel 渲染新 ResultId，失败或取消只显示状态。菜单不提供历史值选择。独立展示窗口收到结果失效或 Project replacement 通知后释放本地 payload。

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

`getEdgeState` 只投影 edge identity、visibility 与 collapse，不为 UI 状态读取序列化布局。
尺寸由 Dockview 持有，通过 `configureEdge` 的结果和显式 layout serialization 读取；临时布局
事务从已捕获的 root snapshot 读取 edge size，避免为每个 edge 重复序列化。
Root runtime 按实际 panel/edge instance 保留监听器，只在实例新增、移除或替换时重新绑定；
普通布局变化仍同步发布 revision，事务的通知边界不变。

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

文件菜单提供事件图、函数和图表的新建入口；创建后打开对应编辑器。“保存”与 `Ctrl+S`
共用当前物理激活 editor 的保存命令，仅保存该文件；无项目或非文件 panel 激活时禁用。
“项目另存为”仍由项目状态决定是否可用。视图菜单提供 Activity、Assistant 的切换和
布局重置；Problems、Output、Logs 通过 Status Bar 图标访问。

## 7. Reveal、reset 与 project replacement

### 7.1 Reveal

Reveal 已存在的 panel 时保持其实际位置，不把它搬回 deterministic home；若位于 edge group，则显示并展开该 edge。缺失的 singleton 才在 home edge 创建。Details 由 permanent placement 规则固定；缺失 Assistant 通过 View 菜单在 Details 后创建并激活；Inspect 创建还要求有效 context；同 `resultKey` 的 Result 只更新并 reveal 既有 panel。

### 7.2 Reset

Reset 使用一个 `PendingWorkbenchTransaction` 临时布局事务，并保留既有 editor、Result 与 panel identities：

- Project、Nodes、Commands、Plugins 回到同一个 left Activity edge group，并恢复 Activity tab 顺序；
- editor panels 按 deterministic snapshot order 集中到 central grid group；
- Details 与 Assistant 始终确保存在并回到 right edge index 0/1；Inspect、Result 回到其后，reset 不凭空创建 Inspect/Result；
- Logs、Output、Problems 回到 bottom edge，恢复 Problems → Output → Logs 顺序；Status Bar 图标顺序跟随该 group；重置完成时不显示其中任何 panel，用户通过 Status Bar 再次打开；
- left/right/bottom 恢复 `WORKBENCH_EDGE_SIZES` 的当前默认值，left/right 展开，bottom 收起并隐藏，不保留折叠标签条；
- main Logs nested Dockview 恢复七 domain 默认布局；
- 优先恢复 reset 前 physically active editor，其次恢复仍有效的 focused editor，再次选择第一个 editor；无 editor 时激活 Project。

### 7.3 Project replacement

Project replacement 先使 pending root operations、hydration generation 与 resources-ready callback 失效，再在当前 FIFO 中移除 project-scoped panels：

- 所有 editor；
- 所有 Result；
- Inspect。

随后清理 editor pane/session 与 project-scoped detail state。Project、Nodes、Commands、Plugins、Details、Assistant、Logs、Output、Problems 及 Logs domain layout 保留；持久化 root 中的 editor、Result、Inspect 会被 scrub，避免新 project hydration 打开旧 project 内容。Problems panel 保留与否不影响 `GraphProjectionStore` 生命周期；Canvas、Details 和 Run Gate 仍从同一完整 projection 更新。

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

`view:data` 已移除；包含该旧 panel identity 的 root snapshot 按现有验证规则回退默认布局，不影响项目资源或有效的 Logs nested snapshot。

## 9. 视觉尺寸层级

root 水平标签使用蓝色圆角背景表示选中；Status Bar 面板图标使用强调色表示选中，不再增加背景。
Problems、Output、Logs
的图标位于窗口最底部的 Status Bar，提供悬停名称和无障碍标签；再次点击当前 bottom panel 图标
收起底部区域，折叠时不预留旧标签行。缺失的 panel 可由图标重新打开，移到其他 group 的 panel
则在其实际位置 reveal。

Status Bar 最右侧提供 Details、Assistant 图标，沿用选中高亮、悬停名称和点击 reveal；再次点击
当前 right panel 的图标可折叠右侧区域。右侧信息为这两个入口预留空间，避免侧栏折叠时重叠。
Settings 图标位于状态栏最左侧、Activity Bar 正下方，通过独立的 Workbench UI state 打开设置弹窗。
Plugins 通过 Activity Bar 最下方原生 tab 打开。`src/modules/plugins/` 仅渲染通用包投影：已安装行不可折叠，管理菜单提供打开、启用/禁用与卸载；本地 `.yssplugin` 安装入口仅在顶部工具栏提供。“已安装”分组直接展示全部已安装插件及总数，不提供搜索或过滤功能；无条目时不渲染空状态文案、安装引导或按钮，保留分组标题和计数。不伪造在线市场条目。

`PluginProvider` 从 Rust registry 获取投影，按清单中的 sidebar 贡献补入可选面板，不抢占焦点。面板使用 `{ role: "plugin", pluginId, viewId, title, location }` 元数据；`pluginId + viewId` 决定 singleton，而不是固定的 Julia 组件名。安装状态独立于 Julia 等外部运行时是否存在。

页面内容来自校验后的不可变包，通过只允许脚本的 sandbox iframe 和绑定安装代际的 MessagePort 与宿主交互，不导入宿主 React/Tauri。第一次可见时才激活插件页面。禁用、卸载、重启或项目切换使旧 context 失效；禁用或安装状态尚未确定时保留布局位置并显示占位。页面关闭不取消独立的后台任务。Julia 运行时页贡献 sidebar，贝叶斯编辑器贡献 editor，宿主不含对应业务页面。

插件 panel 的渲染策略固定为 `renderer: "always"`，新建与布局恢复都由 Workbench 归一化；其他 panel 保持原渲染策略。未激活的插件仍只保留占位组件，首次可见才创建 iframe 与会话。普通显隐或同窗口移动不移除 iframe 文档，不触发解绑和重新加载；可见性通过页面 context 单独通知。

每个已激活页面由 `PluginViewSession` 持有一个后端 lease。替换、重试与释放串行执行：等待旧 attach 返回并释放，确认旧 detach 成功后才允许新 attach。失败释放保留原 lease 身份供显式重试，不继续占用名额。非预期 iframe 导航立即撤销端口，显示明确错误并等待用户重新连接；不使用按切换频率计数的自动重载循环。错误投影保留阶段、稳定代码与 incidentId，不展示内部错误 prose 或输入内容。

后端确认卸载成功后，`PluginProvider` 立即撤销该插件的投影，即使后续列表刷新失败也不恢复已卸载项。`syncPluginWorkbenchViews` 在 root Dockview FIFO 事务中按 `pluginId` 移除全部 `role: "plugin"` 面板，包含 sidebar、editor 及用户移动过的位置；tab 随面板物理删除，随后由 layout controller 立即 flush 持久化布局。确认取消或后端失败不执行卸载清理，插件管理入口、其他插件、普通编辑器与项目结果保留。

完整 registry 查询成功后也会清理旧布局中确认未安装的插件贡献面板；查询失败不等于插件不存在，不据此删除布局。并发查询与排队的 open/register 操作都验证投影当前性，不能由迟到响应重新创建已卸载的 tab。

已有布局缺少 Plugins 时，hydration 补入插件浏览面板并保留已有面板 identity。插件的协议、信任边界与持久化职责见 [Plugin 契约](PLUGIN.md)。

Status Bar 通过 Workbench application hook 订阅 root Dockview 的 group 顺序、active panel、visibility
与 collapsed state，不保存独立的选中或布局状态。订阅投影只在图标顺序、选中或可操作状态改变时
触发 React 更新；编辑器的 active 判定同样只订阅自身布尔结果，不因无关的 layout revision 重绘画布。
图统计从当前 Graph projection 的节点数量和 connection 集合派生，仅 connection 集合替换时重新计数；
视口文字更新按动画帧合并，只写入变化后的显示文本，切换 editor 时取消旧帧并刷新文字。

图标入口随 main grid 的实时左边缘对齐，右侧信息随其右边缘对齐；Root host 通过 application layout
binding 测量 Dockview 的 middle column 来设置 chrome 偏移，侧栏缩放、折叠和恢复均会更新。
偏移 CSS variables 只写在 Status Bar footer 上，数值不变时不重复写入，避免向整个 editor 子树传播
继承样式失效。Bottom header 调整与几何测量分帧执行。Logs 内部的 domain tabs 继续使用自己的样式。

工作台 chrome 使用以下 token 层级。下表像素值只是 `src/app/App.css` 中的 current default，CSS token 才是调用方 contract：

| Token                    | Current default | 用途                                                 |
| ------------------------ | --------------: | ---------------------------------------------------- |
| `--titlebar-height`      |          `36px` | Menubar/titlebar chrome                              |
| `--workbench-tab-height` |          `32px` | root tabs；collapsed edge 同步由 Dockview theme 配置 |
| `--logs-tab-height`      |          `30px` | Logs nested domain tabs                              |
| `--panel-toolbar-height` |          `28px` | panel toolbar                                        |
| `--statusbar-height`     |          `26px` | Status Bar                                           |

## 10. Verification

验证命令以 [本地开发工作流](../development/LOCAL_WORKFLOW.md) 为准。工作台改动期间运行
受影响的 `pnpm test:ts <path>`；交付前按改动范围运行 TypeScript check、完整 Frontend
tests 与 Frontend architecture gate，不在本文维护易漂移的测试文件库存。
