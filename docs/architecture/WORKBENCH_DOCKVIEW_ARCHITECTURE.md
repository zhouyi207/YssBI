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

Project sidebar 按 Events → Functions → Charts → Data 展示同级资源分类，内容与分类来自 Rust 生成的 ActivityPanelDocument，展开状态由前端保存。Project 与 Nodes 面板直接展示分类树，不提供顶部搜索输入区；分类可独立展开和收起，画布节点选择器保留自己的搜索入口。Data 分类提供导入入口；单击数据项在顶部主编辑区（central grid）打开只读数据标签，按 DatabaseId 复用已打开标签。行尾按钮与右键“打开”使用同一入口，双击不再创建外部窗口；拖拽与右键管理保留。数据来自既有数据库投影，不注册独立 Data Activity panel。

Graph 分类与 Chart、Data 使用同一打开语义：单击即在当前 central grid 打开并固定资源标签，同一资源复用已有标签。Graph 不再区分侧栏预览与双击固定，也不再通过替换旧预览标签来控制标签数量。

Graph 条目的选中背景由现有编辑器资源上下文决定，不使用 Details 的查看对象。
实际活动的资源编辑器优先；工具面板获得焦点时，保留既有图会话所在组中仍活动的图标签高亮。
切换到另一张图时跟随切换，切换到非图编辑器或已无对应图标签时清除旧高亮。
画布内单选、多选、框选和清空节点选择只改变节点选择及 Details，不改变所属 Graph 条目的背景。

Event、Function、Chart、Data 打开后共用 `activateEditorPanelAndSyncSession`，保持资源编辑器为物理活动面板，并被动同步 Details 上下文。打开资源不额外激活 Details 或 Project sidebar，避免 `Ctrl+W` 的目标从编辑器转移到固定面板；Project 分类展开不改变活动面板。

数据标签使用 `editor` role 与 `resourceKind: "database"`，随标签激活更新 Details 上下文。`DatabaseEditorContent` 在工作台与独立数据库窗口间复用数据表格、分页、选择及导出；嵌入模式不执行窗口初始化或窗口控制，键盘选择仅处理表格容器内的事件。独立窗口仍由菜单入口打开。数据标签没有本地文档草稿，不参与图/图表编辑与保存命令；关闭标签只释放面板状态，不卸载图文档或清除共享数据库投影。

应用内 Dialog 统一通过点击遮罩空白区域或按 Escape 关闭；遮罩不参与原生窗口拖动。
调用方处理 `onOpenChange(false)` 更新弹窗状态，关闭弹窗不取消已经开始的后台操作。
`UIHost` 按 `UIStore` 的弹窗栈保持各层挂载，后打开的弹窗及其遮罩位于前一层之上；
只有最上层接受交互与关闭操作，关闭后恢复下层的焦点和本地输入状态。
导入外部数据时，选择数据源、取消文件选择或内层弹窗均保留导入弹窗及当前分类；
导入成功后由导入流程按弹窗 ID 关闭对应的导入弹窗。

`src/app/windows/workbench/rootPanelRegistry.tsx` 是唯一同时组合多个业务 panel contribution 的位置，
`editorRendererRegistry.ts` 是唯一把 event/function/chart/database 映射到具体 editor 的位置。Workbench module
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

### 1.1 Activity 文档与模板

四个内置 Activity panels 的文档均由 Rust 生成，共用 `ActivityPanelDocumentView`：

```text
Rust ProjectIndex ─┬→ 资源索引
                   ├→ Project ActivityPanelDocument
                   └→ Catalog → Nodes ActivityPanelDocument
                         ↓
get_project_index → { index, activityPanels: 游标增量 }
                         ↓
ProjectPublicationCoordinator → ResourceStore + sidebarStore

Rust Plugin Manager / Commands → get_activity_panel_document
                              → sidebarStore
                                      ↓
                     useActivityPanelDocument → 模板
```

Project 文档是传入 ProjectIndex 的纯投影，不独立扫描文件。Project/Nodes 使用同一份
已捕获索引生成，Catalog 复用该索引并通过版本校验重验，而不是再次读盘。
Global Plugins/Commands 与无活动项目时的空文档使用独立 Activity 查询，但共享相同的协议、缓存与渲染入口。
后端决定分类、工具、默认展开、空提示和固定条目类型；不存在前端 Project 文档生成分支。

文档只有 category、item、message 三种行；item 限于 graph、chart、database、node、
command 和 plugin 六种。文本使用本地化 key 或字面值；资源路径是 opaque identity。
Workbench 模板只接收文档、展开状态、格式化错误与有限操作回调，不订阅业务 store。
已有资源打开、拖拽、详情和右键操作继续由对应 module/application owner 执行。
数据行直接使用文档中必填的 resourcePath，不再反查前端索引补路径；可用的 loadFailed
状态直接从数据库投影读取。空分组的右键事件统一由文档模板处理。资源行只提供单击
打开入口，双击不再绑定额外打开操作。Nodes/Commands 直接贡献现有面板组件。
画布 NodePalette 保留自己的搜索目录和选择行；Activity 拖拽行由文档条目独立渲染。
资源行的选中/悬停样式与拖拽描述就绪状态分开：描述未就绪时只禁用拖拽，仍可点击打开，
不降低整行透明度或将整行标记为 aria-disabled。目录加载完成、取消选中或关闭标签
不会通过拖拽状态改变普通行的亮度。

Event、Function、Chart、Data 的 mutation 回执与 watcher 索引失效共用项目发布协调器的串行队列。
回执提供相关性校验、受影响路径与 move 信息，不再先写一套 delta Store、随后又用索引覆盖。
正常更新和恢复均由 `projectPublicationSnapshot` 安装同次响应中的权威 ProjectIndex 与 Activity 增量；Chart 文档和已加载的干净 Graph
会话在提交前准备，UI 不预写入临时 Chart。快照可以覆盖较晚才到达的命令/事件回执，同一版本只结算一次。
实际 Dockview 提交处再次检查项目身份和本地草稿，未保存内容不被磁盘快照覆盖。
CRUD 调用方不保留独立的 post-commit refresh；数据删除不显示额外全屏进度蒙层。
已应用的资源版本和已发布的索引版本共同约束后续查询；过期索引只重读一次，不得覆盖较新的资源状态。
导入及数据来源读取仍保留进度提示。异步删除完成时只清理仍指向被删资源的详情焦点，
保留用户在等待期间切换的新选择。

共享本地化节点目录由项目发布协调器在成功提交、恢复和初始化后提供 Rust publication revision；
索引内容变化才替换资源快照，`indexRevision` 保存安装时的 Rust publication revision。
重复 watcher 查询忽略 exportTime 等非资源字段，不重复安装。Project/Nodes 文档随同次响应更新，
不订阅 ResourceStore 对象或索引代际以触发另一轮 Activity 查询。共享节点目录以 Rust 水位判断新旧；只有索引内容变化而
水位未推进时才显式失效，不为普通发布额外维护一套代际表。项目生命周期重置清除旧请求，后挂载或切换语言的消费者
继承当前水位。落后的目录自动重查，已经足够新的目录复用；不依赖首次点击数据行来触发更新。
插件自带的 sandbox iframe 页面仍遵循 Plugin 契约，不转换成宿主 Activity 条目。

后端 Activity 首次查询返回 `{ kind: "snapshot", cursor, document }`。索引发布或插件变更驱动失效；
后续查询只携带 cursor，Rust 从权威查询生成当前投影，与该游标绑定的不可变基线比较，
仅返回 `{ kind: "patch", baseCursor, cursor, patch, operations }`。不会把旧树传回后端，
也不会在普通更新时跨 IPC 重新发送完整树；后端在已捕获的权威事实之上生成投影并计算差分。

`operations` 使用稳定行 id，支持 update、insert、remove、move。update 的 patch
只包含发生变化的顶层字段；嵌套字段值整体替换，null 是合法值而不是删除指令。例如：

```json
{ "op": "update", "id": "summary", "patch": { "label": { "text": "新的分析结果" } } }
```

insert/move 的 afterId 指定平面行顺序中的前一行，null 表示首行；remove 只删除指定行，
子行变化由同一批次内的其他操作描述。文档级 patch 仅更新标题、工具、空状态和 publicationRevision。
整批操作在隔离副本上校验后一次发布；普通更新保留可见文档和未修改行的引用，
不切换回 loading。无变化返回空操作并保持 cursor，前端保留原快照引用。

同一面板作用域内请求串行、失效信号合并。语言、项目/生命周期变化建立新作用域，
旧响应不能发布；组件卸载只停止订阅，运行期文档可供同作用域再次挂载复用。Rust 传输缓存按窗口、面板、项目及语言隔离，
以数量和序列化体积限制不可变基线，旧基线可以支持丢失回复后的增量请求。
缓存缺失/淘汰时返回 snapshot；前端遇到基线不匹配或非法增量时只尝试一次快照恢复。
合并响应中任何面板的基线或协议无效，整批重取一次快照；重复失败显示错误并保留上一份完整投影。
后端 Activity 不回退到前端重建。
Commands 的可用性来自本地草稿历史，数据库运行状态和选中状态仍从既有 UI/runtime
投影读取；这些变化不生成另一份资源文档。

标题栏使用 `--workbench-tab-height`。分类统一复用 `SidebarTreeCategoryRow`；
内容使用普通滚动列表，仅挂载展开分支，取消 Activity 的虚拟列表测量依赖。
后端文档上限由 transport/parser 约束，超限明确失败，不静默截断。折叠只改变本地可见行，
不会产生 IPC。四个面板的运行期文档、游标和错误由现有 `sidebarStore` 缓存，并按面板/项目/语言/生命周期隔离；
同作用域共享请求，过期作用域不能覆盖新文档。展开偏好按 panel/category 保存到
`yssbi-activity-panel-expansion`；文档不落盘，Dockview placement 不进入该 store。
旧 Project 分类偏好 key 不迁移，首次使用采用各文档定义的默认值。

外部文件变化先在 watcher 中合并为有界 rescan 信号，再由 Project 同步磁盘与驻留资源并触发索引刷新。
索引的文件成员关系以磁盘为准；驻留 revision 不得重新添加磁盘已删除的 chart。
缺失但已加载的资源记录可保留为 `exists: false`，用于保留本地文档意图；它不表示文件仍存在。
tab 清理检查 exists，并在 Dockview 提交前重验项目身份和资源仍然缺失，只释放面板状态，
不因异步删除清理用户切换后的项目或重新出现的文件。

## 2. Root panel 角色与默认 home

root group 可以混合承载不同角色；唯一例外是 Activity group。角色决定内容和应用语义，Activity group 还受到固定成员和 drop policy 约束：

| 角色             | 内容                                      | deterministic home                  |
| ---------------- | ----------------------------------------- | ----------------------------------- |
| `editor`         | Graph/Function/Chart editor、只读数据表格 | 当前 central grid group             |
| `view:project`   | Project activity panel                    | left Activity edge                  |
| `view:nodes`     | Nodes activity panel                      | left Activity edge                  |
| `view:commands`  | Commands activity panel                   | left Activity edge                  |
| `view:details`   | permanent fixed Details                   | right edge index 0                  |
| `view:assistant` | movable/closable Assistant                | right edge index 1 on default/reset |
| `view:inspect`   | contextual Inspect                        | right edge                          |
| `result`         | 一个可检查结果                            | right edge                          |
| `view:logs`      | Logs workspace                            | bottom edge                         |
| `view:output`    | Run Output                                | bottom edge                         |
| `view:problems`  | Graph Problems                            | bottom edge                         |

默认空布局建立 central grid group，并放置：

- Project、Nodes、Commands、Plugins：同一个 left Activity edge group，使用 `WORKBENCH_EDGE_SIZES.left`，默认顺序为 Project → Nodes → Commands → Plugins；
- Logs、Output、Problems：bottom edge，使用 `WORKBENCH_EDGE_SIZES.bottom`，顺序为 Problems → Output → Logs；
- bottom edge 仅包含 Problems、Output、Logs 时隐藏原生 header，由 Status Bar 图标切换；混入 editor 或其他 panel 时恢复原生 header，保留混合 group 的完整操作入口。

right edge 使用 `WORKBENCH_EDGE_SIZES.right`。Details 始终由默认/恢复/reset 流程安装在 canonical right edge index 0，并且是唯一 permanent/fixed panel；Assistant 默认紧邻 Details，但作为普通 singleton 可移动、split、关闭。Inspect 仍按有效 editor/node context 延迟创建；Result 允许多个实例，但每个结果引用只对应一个 canonical panel。Activity panels 始终由默认布局安装，不能由 close coordinator 删除；Activity edge 的可见性通过 root edge 的 visible/collapsed state 控制。三个 edge 的具体当前像素默认值只由 `src/modules/workbench/internal/dockview/workbenchDockviewDefaults.ts` 维护。

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
editor → { role, resourceRef, resourceKind, sticky? }
view   → { role, viewId }
result → { role, reference, leaseId, title, presentation }
```

以下 identity 永远分离：

| Identity          | 含义                                                                                                           |
| ----------------- | -------------------------------------------------------------------------------------------------------------- |
| `resourceRef`     | editor 打开的 opaque backend resource key：图/图表使用路径，数据使用 DatabaseId；同一资源可有多个 editor panel |
| `reference`       | `{ executionSessionId, resultId }`，标识 Rust ResultStore 中的不可变快照                                       |
| `leaseId`         | 当前面板持有的后端租约 token，不随移动、隐藏或重新渲染改变                                                     |
| `panelInstanceId` | 一个 root Dockview panel instance 的物理 identity                                                              |
| `groupId`         | Dockview 当前物理 group 的 identity；panel 移动后可改变                                                        |

不得从 `panelInstanceId` 或 `groupId` 推导 `resourceRef`、结果引用或 `leaseId`，也不得把这些 identity 合并为一个 tab id。

Singleton 与 multi-instance contract：

- Project、Nodes、Commands、Plugins、Details、Assistant、Inspect、Logs、Output、Problems 由 `viewId` 保证 singleton；
- Project、Nodes、Commands、Plugins 随默认 Activity group 安装且保持存在；
- Details 是 permanent fixed singleton；
- Assistant 是普通 layout-persisted singleton；
- Inspect 只在上下文有效时按需创建；
- Result 按完整结果引用复用并 reveal，不同运行产生的新引用创建独立面板；
- 重复打开同一快照保留既有面板和租约，调用方释放重复申请的临时租约。

Result panel 固定读取 `reference`，不订阅 pin 的当前结果。删除来源节点或重新运行不会清空已打开的报告。
Application 的结果租约控制器订阅完成 hydration 后的真实面板集合，按 `leaseId` 与后端对账；切换标签、移动、重置布局保留持有关系，真实关闭才释放。
跨窗口交接和后端窗口销毁负责独立报告的租约生命周期，详见 [Graph 与 Execution](GRAPH_AND_EXECUTION.md#6-results)。
结果引用的来源信息留在不可变 provenance 中，不随图重命名改写；会话结束后关闭对应独立窗口并移除项目面板。

## 5. Module seams 与布局 mutation

### 5.1 Public seam

`src/modules/workbench/public.ts` 将能力拆成独立的 `workbenchDockviewRead`、
`workbenchDockviewControl` 和 `workbenchDockviewRootBinding`。它们提供 role-aware semantic
operations：

- `openEditor`；
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

Reveal 已存在的 panel 时保持其实际位置，不把它搬回 deterministic home；若位于 edge group，则显示并展开该 edge。缺失的 singleton 才在 home edge 创建。Details 由 permanent placement 规则固定；缺失 Assistant 通过 View 菜单在 Details 后创建并激活；Inspect 创建还要求有效 context；同一结果引用的 Result 只 reveal 既有 panel。

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

`view:data` 已移除；包含该旧 panel identity 的 root snapshot 按现有验证规则回退默认布局，不影响项目资源或有效的 Logs nested snapshot。包含旧 `params.metadata.pinned` 的 editor snapshot 同样不再是 canonical 格式，会回退默认布局。项目不再在 editor metadata 中镜像 Dockview 的 pinned 状态；Root Dockview 启用原生 `pinnedTabs`，新打开的 editor 通过 `panel.api.setPinned(true)` 设置原生状态，布局序列化保留 Dockview 自己的 `panels[id].pinned` 字段。

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
节点、连接、选中数量与 X/Y/缩放仅在当前激活的 editor 为 Event 或 Function 时显示，
切换到 Chart、Data、其他 panel 或没有激活 editor 时整组隐藏，不显示默认零值占位。
显示条件复用状态项注册的 `visible`，不保存另一份激活 tab 状态。
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
