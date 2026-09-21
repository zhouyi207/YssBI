# Workbench FlexLayout 当前架构

> Status: Current
> Scope: FlexLayout Model authority、panel identity、布局操作、生命周期和持久化
> Canonical owners: Workbench 源码与测试拥有布局默认值；本文拥有稳定的布局和 identity contract
> Update when: root topology、panel role、close/reset/replacement 或 persistence contract 改变时

工作台使用一个原生 FlexLayout Model 保存已提交的物理布局。React 的 Layout 和组件注册表呈现该模型，Application 通过语义操作协调面板生命周期。工作台布局、Graph 文档和统计结果具有独立的 owner。

Rust 可以通过受控界面意图请求打开图、定位节点、打开结果或显示固定目录中的面板。工作台先认领请求，再调用现有编辑器、结果租约与面板入口，并回传实际执行状态。页面内容的 JSON 与增量由 Rust Application session 拥有，不能直接 patch FlexLayout；具体协议见 [JSON 页面与界面意图](../../../src-tauri/crates/yss-ui-contract/README.md)。

Assistant 面板的挂载只拥有事件订阅和界面投影；对话列表与历史由 Harness 持久化，关闭或移动面板不结束对话。项目归属、恢复与多对话切换见 [Harness 会话契约](../../../src-tauri/crates/yss-harness-core/README.md#5-session-turn-and-events)。

## 1. 渲染层级与 authority

```text
WorkbenchWindow
├─ WorkbenchMenuBar
├─ RootLayoutHost：一个 FlexLayout Layout + Model
│  ├─ left border：Project、Nodes、Commands、Plugins 和插件 sidebar
│  ├─ central column
│  │  ├─ central tabsets：资源编辑器、Result 和可移动工具面板
│  │  └─ bottom content：Problems、Output、Logs 的内容面板
│  ├─ right border：Details、Assistant 和 Result 的默认位置
│  └─ bottom bar：贯穿窗口，最左侧设置、对齐中央区左边的原生 tab、对齐右边的状态信息
└─ WorkbenchOverlayHost
```

Model 是拓扑、分组、顺序、选择、尺寸与边栏折叠的唯一可写 authority。border 的 selected 为 -1 表示折叠；折叠后仍显示 FlexLayout 原生边栏标签。Activity 只在 left border 内排序，Details 固定在 right border，普通面板可在允许的区域移动和分屏。Settings 支持窗口内 float；其余面板的 float、浏览器 popout 和标签分组禁用。

FlexLayout 的 selected tab 与 active tabset 分别拥有组内选择和顶部活动分组；各 border 独立保存选择，不覆盖顶部活动 tab。`getActivePanel` 只读取顶部活动 tabset 的 selected tab。非空顶部分组始终有选中 tab，活动分组被删除后通过原生 Action 选定剩余分组。输入焦点由 DOM 和事件路径决定，不写入布局配置或另建状态库。

中央 group 的最后一个 tab 关闭或移走后，由 FlexLayout 删除空组并回收分屏空间；空 border 自动隐藏，底部仅保留承载状态信息的条带，空的内容面板仍消失。只有中央工作区没有非空 group 时才显示一份 watermark，侧栏和底部工具面板不影响该判断。FlexLayout 内部保留的最后一个空投放容器用于接收新 tab，不计入 Workbench 的 group 查询，也不显示分组标签栏。恢复旧布局时移除阻止空组删除的节点配置，由原生模型清理空组；不逐帧扫描或重建布局。

Project sidebar 按 Events → Functions → Charts → Data 展示同级资源分类，内容与分类来自 Rust 生成的 ActivityPanelDocument，展开状态由前端保存。Project 与 Nodes 面板直接展示分类树，不提供顶部搜索输入区；分类可独立展开和收起，画布节点选择器保留自己的搜索入口。Data 分类提供导入入口；单击数据项在顶部主编辑区（central grid）打开只读数据标签，按 DatabaseId 复用已打开标签。行尾按钮与右键“打开”使用同一入口，双击不再创建外部窗口；拖拽与右键管理保留。数据来自既有数据库投影，不注册独立 Data Activity panel。

Graph 分类与 Chart、Data 使用同一打开语义：单击即在当前 central grid 打开并固定资源标签，同一资源复用已有标签。Graph 不再区分侧栏预览与双击固定，也不再通过替换旧预览标签来控制标签数量。

Graph 条目的选中背景由现有编辑器资源上下文决定，不使用 Details 的查看对象。
高亮直接来自顶部活动资源编辑器；侧栏获得输入焦点不改变顶部选择，顶部选中工具或非 Graph 标签时清除图高亮。
切换到另一张图时跟随切换，切换到非图编辑器或已无对应图标签时清除旧高亮。
画布内单选、多选、框选和清空节点选择只改变节点选择及 Details，不改变所属 Graph 条目的背景。

Event、Function、Chart、Data 打开后共用 `activateEditorPanelAndSyncSession`，保持资源编辑器为物理活动面板，并被动同步 Details 上下文。打开资源不额外激活 Details 或 Project sidebar，避免 `Ctrl+W` 的目标从编辑器转移到固定面板；Project 分类展开不改变活动面板。

数据标签使用 `editor` role 与 `resourceKind: "database"`，随标签激活更新 Details 上下文。`DatabaseEditorContent` 在工作台与独立数据库窗口间复用数据表格、分页、选择及导出；嵌入模式不执行窗口初始化或窗口控制，键盘选择仅处理表格容器内的事件。独立窗口仍由菜单入口打开。数据标签没有本地文档草稿，不参与图/图表编辑与保存命令；关闭标签只释放面板状态，不卸载图文档或清除共享数据库投影。

帮助菜单的架构弹窗由现有 HashRouter 的 `architecture` 查询参数控制：
`/editor?architecture=overview` 展示总览，`/editor?architecture=frontend` 展示前端子系统、状态归属与代表实现，
`/editor?architecture=backend` 展示后端子系统，
`/editor?architecture=communication` 展示 Tauri IPC 的 Command、Event、Channel 与 DTO / 错误契约。
React、Rust、Tauri IPC 节点、面包屑和工具栏视图下拉框通过路由切换视图，浏览器前进、后退同步更新内容；关闭弹窗移除该参数并保留其他查询参数。
架构导航保持工作台路由与 FlexLayout 挂载，不保存第二份弹窗打开状态或视图历史。

`architecture=dependencies` 在同一张图上展示全部 Cargo workspace crates 及其直接引用关系，按依赖层级从左向右排列，箭头从引用方指向被依赖方。不设置单独的 crate 下拉框；点击节点仅高亮引用关系，`crate` 参数同步高亮状态，始终保留全部节点和连线。条件、可选和构建依赖以虚线标示，不显示 dev 或第三方依赖，也不宣称这些声明在同一次构建中全部启用。数据为静态快照，Cargo 清单变化后运行 `pnpm docs:crate-dependencies` 更新，`pnpm docs:crate-dependencies:check` 校验快照。

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
生命周期。Problems、Results 与运行失败的业务语义见 [Graph 与 Execution](../../../src-tauri/crates/yss-application/src/graph/README.md)，
Logs 的业务语义见 [Runtime Signals](../../features/application/observability/README.md)。

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
实际 FlexLayout 提交处再次检查项目身份和本地草稿，未保存内容不被磁盘快照覆盖。
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
`yssbi-activity-panel-expansion`；文档不落盘，FlexLayout placement 不进入该 store。
旧 Project 分类偏好 key 不迁移，首次使用采用各文档定义的默认值。

外部文件变化先在 watcher 中合并为有界 rescan 信号，再由 Project 同步磁盘与驻留资源并触发索引刷新。
索引的文件成员关系以磁盘为准；驻留 revision 不得重新添加磁盘已删除的 chart。
缺失但已加载的资源记录可保留为 `exists: false`，用于保留本地文档意图；它不表示文件仍存在。
tab 清理检查 exists，并在 FlexLayout 提交前重验项目身份和资源仍然缺失，只释放面板状态，
不因异步删除清理用户切换后的项目或重新出现的文件。

## 2. Root panel 角色与默认 home

root group 可以混合承载不同角色；Activity group 和 bottom edge 具有成员限制。角色决定内容和应用语义，Activity group 还受到固定成员和 drop policy 约束：

bottom edge 只接受 Problems、Output、Logs 三种 singleton tab，允许标签排序；助手、编辑器、Result 和插件面板不能加入底部组。原生拖放、程序化移动和持久化布局校验共用同一条放置规则。

| 角色             | 内容                                      | deterministic home                  |
| ---------------- | ----------------------------------------- | ----------------------------------- |
| `editor`         | Graph/Function/Chart editor、只读数据表格 | 当前 central grid group             |
| `view:project`   | Project activity panel                    | left Activity edge                  |
| `view:nodes`     | Nodes activity panel                      | left Activity edge                  |
| `view:commands`  | Commands activity panel                   | left Activity edge                  |
| `view:details`   | permanent fixed Details                   | right edge index 0                  |
| `view:assistant` | movable/closable Assistant                | right edge index 1 on default/reset |
| `view:settings`  | application Settings                      | 独立 float 子布局                   |
| `result`         | 一个可检查结果                            | right edge                          |
| `view:logs`      | Logs workspace                            | bottom edge                         |
| `view:output`    | Graph 运行失败摘要                        | bottom edge                         |
| `view:problems`  | Graph Problems                            | bottom edge                         |

默认空布局保留原生中央投放容器，并放置：

- Project、Nodes、Commands、Plugins：同一个 left Activity edge group，使用 `WORKBENCH_EDGE_SIZES.left`，默认顺序为 Project → Nodes → Commands → Plugins；
- Logs、Output、Problems：bottom edge，使用 `WORKBENCH_EDGE_SIZES.bottom`，顺序为 Problems → Output → Logs；
- bottom bar 使用原生 tab 切换、折叠和关闭面板，右侧承载状态信息；不保留另一套图标切换入口。

right edge 使用 `WORKBENCH_EDGE_SIZES.right`。Details 始终由默认/恢复/reset 流程安装在 canonical right edge index 0，并且是唯一 permanent/fixed panel；Assistant 默认紧邻 Details，但作为普通 singleton 可移动、split、关闭。节点参数、配置、端口、诊断与文档统一由 Details 展示，节点选择只同步 Details 上下文；Result 允许多个实例，但每个结果引用只对应一个 canonical panel。Activity panels 始终由默认布局安装，不能由 close coordinator 删除；Activity edge 的可见性通过 root edge 的 visible/collapsed state 控制。三个 edge 的具体当前像素默认值只由 `src/modules/workbench/internal/layout/workbenchLayoutDefaults.ts` 维护。

Problems 只使用 `viewId: "problems"` 与 registry component `Problems`。Layout parser 只接受当前
exact envelope 与 canonical panel identity，不执行旧 ID 转换或 alternate read。

旧 `Diagnostics` identity 没有迁移路径；含非 canonical identity 的 root snapshot 会回退默认布局。这是当前 0.x 的直接替换行为，已有本地布局可能因此重置。默认与 reset 的顺序统一由 WORKBENCH_BOTTOM_DEFAULT_ORDER 定义；有效已保存布局保留用户排序。

## 3. Logs 内部布局

Logs 内部使用一个受限的 FlexLayout Model，承载七个 domain tabs。标签可以在同一 tabset 内排序，不能关闭、跨工作台拖放、分屏、浮动或 popout。

工作台内的 Logs 复用外层面板边框，移除内层日志 tab 内容的主题边框与圆角，避免内容比 Output、Problems 多缩进一层。分类 tab、工具栏和日志行自身的内距仍由对应组件负责；独立 Logs 窗口保留原生主题边框。

main Logs 的布局由 logsRuntime 在挂载时交给真实 Model，卸载时保存只读快照，作为 nested.logs 随工作台持久化。独立 Logs 窗口使用自己的临时 Model。日志内容、过滤、选择和运行观测继续属于 Logs owner。

## 4. Canonical metadata 与 identity

`WorkbenchPanelMetadata` 是 root panel 的 canonical metadata：

```text
editor → { role, resourceRef, resourceKind }
view   → { role, viewId }
result → { role, reference, leaseId, title, presentation }
```

以下 identity 永远分离：

| Identity          | 含义                                                                                                           |
| ----------------- | -------------------------------------------------------------------------------------------------------------- |
| `resourceRef`     | editor 打开的 opaque backend resource key：图/图表使用路径，数据使用 DatabaseId；同一资源可有多个 editor panel |
| `reference`       | `{ executionSessionId, resultId }`，标识 Rust ResultStore 中的不可变快照                                       |
| `leaseId`         | 当前面板持有的后端租约 token，不随移动、隐藏或重新渲染改变                                                     |
| `panelInstanceId` | 一个 root FlexLayout Model panel instance 的物理 identity                                                      |
| `groupId`         | FlexLayout 当前物理 group 的 identity；panel 移动后可改变                                                      |

不得从 `panelInstanceId` 或 `groupId` 推导 `resourceRef`、结果引用或 `leaseId`，也不得把这些 identity 合并为一个 tab id。

Singleton 与 multi-instance contract：

- Project、Nodes、Commands、Plugins、Details、Assistant、Settings、Logs、Output、Problems 由 `viewId` 保证 singleton；
- Project、Nodes、Commands、Plugins 随默认 Activity group 安装且保持存在；
- Details 是 permanent fixed singleton；
- Assistant 是普通 layout-persisted singleton；
- Settings 按需打开，是普通 layout-persisted singleton；
- Result 按完整结果引用复用并 reveal，不同运行产生的新引用创建独立面板；
- 重复打开同一快照保留既有面板和租约，调用方释放重复申请的临时租约。

Result panel 固定读取 `reference`，不订阅 pin 的当前结果。删除来源节点或重新运行不会清空已打开的报告。
Application 的结果租约控制器订阅完成 hydration 后的真实面板集合，按 `leaseId` 与后端对账；切换标签、移动、重置布局保留持有关系，真实关闭才释放。
跨窗口交接和后端窗口销毁负责独立报告的租约生命周期，详见 [Graph 与 Execution](../../features/application/results/README.md#results)。
结果引用的来源信息留在不可变 provenance 中，不随图重命名改写；会话结束后关闭对应独立窗口并移除项目面板。

## 5. Module seams 与布局 mutation

布局实现位于 src/modules/workbench/internal/layout/：

- workbenchLayoutOperations.ts 在原生 Model 上执行 openEditor、ensureView、upsertResult、activate、reveal、move、split、边栏调整和资源重映射。
- workbenchRead.ts 与 workbenchControl.ts 提供业务侧查询和语义命令；调用方不持有可写 Model。
- workbenchRootBinding.ts 只为 RootLayoutHost 创建渲染绑定，并接收原生布局动作与面板激活请求。
- workbenchLayoutInternal.ts 拥有 FIFO、hydration gate、操作代际、关闭提交和复合事务。
- workbenchLayoutController.ts 协调窗口绑定、恢复、项目资源就绪、持久化防抖和关闭时 flush。

普通操作使用原生 Actions。复合操作通过 PendingWorkbenchTransaction 创建独立候选 Model，在候选上使用同一套语义操作；验证结构、metadata 和基线 revision 后，以 Model.fromJson(candidate, previousModel) 发布并移交既有标签的视图状态。候选准备期间不移交活动视图。没有另一份自制 panel/group/edge 模型，也不回放 Dockview 风格命令。

publication transaction 可以在准备期间等待业务查询。提交前重验 binding、operation generation 和布局 revision；项目切换或用户布局操作会使过期候选失败。Actions.group 只提供原生动作的分组通知，不替代候选验证和业务提交边界。

### 5.1 拖动与订阅

Layout 通过 `subscribeModel` 只订阅 Model 实例替换；Model 内部动作由 FlexLayout 自己处理。所有带 `isAdjusting()` 标记的中间动作统一延迟应用通知，全由中间动作组成的嵌套 GroupAction 同样处理，不再维护手势动作名称清单。原生几何路径继续实时更新，结束动作再提交通知；无论是否通知，每次动作都推进绑定 revision，避免拖动期间的旧候选覆盖新布局。活动分组补全也跳过中间动作，避免空中央区时逐帧扫描布局。

提交后从原生 Model 生成轻量面板、分组和边栏只读投影，复用未变化的记录与嵌套字段引用，按实际状态变化通知消费者。没有 JSON 签名、全量字符串序列化或面板排序，也不根据动作名称猜测影响：

- `subscribePersistence` 接收提交及生命周期通知，用于布局保存调度；纯几何变化不会广播到业务订阅者。
- `subscribe` / `getSnapshot` 反映面板、分组和边栏语义变化，菜单使用此入口，不因尺寸或浮窗坐标变化重算。
- `subscribeActivePanel` / `getActiveSnapshot` 只反映中央活动面板及就绪状态，供活动 Graph 上下文和激活协调者使用。
- `subscribePanelSet` 只反映面板身份和 metadata 集合变化，供结果租约对账使用；排序、选择、可见性和尺寸变化不触发它。
- `subscribePanel(id, listener)` 只反映该面板及所在边栏的状态变化，供内容、标签菜单和浮动设置的呈现使用。

绑定、解除绑定及 hydration 状态变化会通知相关订阅者。公共查询读取同一份冻结的提交投影，保证记录引用稳定；中间手势只改变原生 Model 与 mutation revision，结束后发布读投影。投影没有写入口，不是第二份可写面板布局。

异步命令的过期检查使用 `getMutationRevision()`，它包含绑定/操作代际、hydration 及原生动作 revision，中间动作也会改变此 token。`getSnapshot().revision` 与 `getActiveSnapshot().revision` 仅表示对应通知投影的变化，不用于并发提交校验；图问题定位和内部布局事务分别使用 mutation token 和 binding revision。

当前 FlexLayout 依赖补丁跳过 adjusting `MOVE_FLOAT` 引起的整棵 Layout 重绘：FloatWindow 自己更新矩形，尺寸变化继续由原生 ResizeObserver 处理，结束动作恢复正常布局通知。补丁与缺失 CSS source map 的修正同由 `patches/flexlayout-react@0.11.0.patch` 管理，升级依赖时应核对上游实现。

PanelContent 按自身 group、title、metadata 和 visible 订阅，未变化时返回相同快照。父级回调和 drag overlay 保持稳定，减少画布、标签和无关面板的重渲染。查询面板集合时只计算一次活动面板。

这些边界减少宿主附加开销，不保证大型 Graph、统计图表或表格的实际帧率；真实数据和桌面 WebView 下的交互需要单独验收。

## 6. Close、命令目标与输入路由

### 6.1 Close coordinator

所有 root tab 关闭入口都进入 `requestCloseWorkbenchPanel(s)`：close button、中键、context menu、`Ctrl+W`、view toggle 和 Close Group 不直接调用 FlexLayout close。

Problems、Output、Logs 的原生 tab 使用 `enableClose: false` 隐藏关闭按钮；新建和恢复布局应用同一规则。Details、Problems、Output、Logs 均提供 tab 右键菜单，其中“关闭”复用关闭按钮规则保持禁用，隐藏/展开内容仍可用。点击底部 tab 仍可切换、展开和折叠内容，中键与应用关闭命令继续由上述 coordinator 处理。

工具与 Result tab 的右键菜单不再提供 Close Group；位于边栏时，订阅 root edge 的真实折叠状态，展开时显示本地化的“隐藏内容”，折叠时显示“展开内容”。隐藏只收起所在边栏内容，保留标签、面板实例和内容状态；展开会显示右键目标面板，点击标签也可恢复。中央 tabset 中不显示该切换项。共用 ActionMenu 阻止菜单点击和键盘事件沿 React Portal 冒泡到宿主 tab，避免一次菜单操作又触发原生 tab 切换。

Coordinator 按顺序执行：

1. 捕获 `panelInstanceId + groupId + metadata` commit tokens；project-scoped panel 同时捕获 project identity。
2. 计算哪些 editor document 将失去最后一个 panel。
3. 对 dirty document 执行 save/discard/cancel confirmation。
4. 在 FIFO 内重新校验 token 与 project identity。
5. 通过 internal `commitRemove` 执行物理 close。
6. 仅对已经物理移除的 panel 释放 pane、viewport、graph session 或 chart document state。

并发 close workflow 串行化；取消、stale token 或 project replacement 都不会提前释放 domain state。

### 6.2 物理命令

命令以 root FlexLayout Model 的实时 group 为准：

- `Ctrl+Tab` 在键盘事件所属面板的原生 group 中循环，未指向面板时使用顶部活动 group；
- Close Group 关闭该 physical group 中 editor、Result 和 tool panels 的完整集合；若 group 同时包含 fixed Details，现有 close coordinator 拒绝整批关闭，Assistant 只能单独关闭；Assistant 移到不含 fixed panel 的普通 group 后沿用 Close Group；
- editor tab 的 Close Others、Close All、Close Saved 只筛选该 group 中的 `editor` role；
- split 作用于 active canonical editor，native FlexLayout drag/drop 继续拥有后续物理移动与顺序。

### 6.3 命令目标与输入焦点

`editorCommandFocus` 从两个明确入口捕获目标：菜单和保存命令读取顶部活动编辑器；画布按钮、手势和编辑快捷键读取自身可见面板。执行时重验项目身份、面板、分组、资源和可见性；顶部菜单目标还要重验活动 tab。切到非编辑器 tab 后，不回退到旧 Graph 会话。

输入框、菜单、弹窗等先消费自己的快捷键。Delete、复制粘贴、节点选择和画布导航根据键盘事件路径或 DOM 当前焦点定位面板，不能从侧栏误操作顶部 Graph。`Ctrl+W` 按实际键盘面板关闭；保存和分屏使用顶部活动编辑器。

可见 Graph 的交互由可见性、文档可用性和保存状态决定。各画布工具栏始终绑定自身 graphPath；侧栏或另一分组获得焦点不卸载工具栏。隐藏面板、保存锁定和项目替换仍取消对应手势。

文件菜单提供事件图、函数和图表的新建入口；创建后打开对应编辑器。“保存”与 `Ctrl+S`
共用顶部活动 editor 的保存命令，仅保存该文件；无项目或顶部选中非文件 panel 时禁用。
“项目另存为”仍由项目状态决定是否可用。视图菜单提供 Activity、Assistant 的切换和
布局重置；Problems、Output、Logs 由底部原生 tab 切换，不在视图菜单中提供入口。

## 7. Reveal、reset 与 project replacement

### 7.1 Reveal

Reveal 已存在的 panel 时保持其实际位置，不把它搬回 deterministic home；若位于 edge group，则显示并展开该 edge。缺失的 singleton 才在 home edge 创建。Details 由 permanent placement 规则固定；缺失 Assistant 通过 View 菜单在 Details 后创建并激活；同一结果引用的 Result 只 reveal 既有 panel。

设置菜单、底栏齿轮和 Ctrl+, 共用 `revealWorkbenchView("settings")`。缺失时通过原生 `Actions.createSubLayout` 直接创建 singleton float，不经过中央标签；已存在时置前同一浮窗。Settings 不能停靠、分屏或拖入中央与边栏区域，中央区不提供 float 图标或转换入口。其打开状态、位置和尺寸只来自 FlexLayout，不在 UI store 保留第二份状态。布局恢复与项目切换保留浮窗，关闭后恢复不会重新创建。

Settings 模块继续拥有分类、搜索、表单和标题内容；设置值由现有 Settings store 持久化，不进入布局 JSON。页面根据浮窗宽度适配。工作台不再为设置挂载 Dialog；项目管理页的简化设置弹窗仍属于独立页面。恢复布局不承诺恢复 Settings 的搜索和分类选择。

浮动页使用同一根 Model 的 `subLayouts`，位置、尺寸和层级由 FlexLayout 管理，并限制在工作台窗口内。中央分组最大化不隐藏它，重置主布局保留已打开设置的浮动位置。恢复校验拒绝停靠的 Settings、其他浮动内容和浏览器窗口子布局；不存在旧格式迁移。浮动设置不改变顶部活动编辑器的命令目标。

浮动 Settings 的齿轮、标题和关闭按钮通过依赖补丁提供的 `renderFloatHeader` 扩展点渲染在原生 float header 内；标题区域由库直接处理拖动，按钮隔离 pointer-down，关闭经过统一协调者。内部 tabset 通过原生 `enableTabStrip: false` 隐藏标签。没有透明覆盖层、固定按钮避让宽度或自建拖动几何状态；加载期间也保留标题与关闭操作。

### 7.2 Reset

Reset 在独立的原生 FlexLayout Model 上准备布局，校验后一次提交，并保留既有 editor、Result 与 panel identities：

- Project、Nodes、Commands、Plugins 回到同一个 left Activity edge group，并恢复 Activity tab 顺序；
- editor panels 按 deterministic snapshot order 集中到第一个 central tabset；Settings 保留浮窗，reset 不创建已关闭的 Settings；
- Details 与 Assistant 始终确保存在并回到 right edge index 0/1；Result 回到其后，reset 不凭空创建 Result；
- Logs、Output、Problems 回到 bottom edge，恢复 Problems → Output → Logs 的原生 tab 顺序；重置完成时收起内容面板，用户点击底部 tab 再次展开；
- left/right/bottom 恢复 `WORKBENCH_EDGE_SIZES` 的当前默认值，left/right 展开，bottom 收起，保留原生 border 标签条；
- main Logs nested FlexLayout 恢复七 domain 默认布局；
- 优先恢复 reset 前顶部活动 editor，否则选择第一个 editor；没有 editor 时显示 Project。

### 7.3 Project replacement

Project replacement 先使 pending root operations、hydration generation 与 resources-ready callback 失效，再在当前 FIFO 中移除 project-scoped panels：

- 所有 editor；
- 所有 Result。

随后清理 editor pane/session 与 project-scoped detail state。Project、Nodes、Commands、Plugins、Details、Assistant、Logs、Output、Problems 及 Logs domain layout 保留；持久化 root 中的 editor、Result 会被 scrub，避免新 project hydration 打开旧 project 内容。Problems panel 保留与否不影响 `GraphProjectionStore` 生命周期；Canvas、Details 和 Run Gate 仍从同一完整 projection 更新。

## 8. Persistence contract

每个窗口使用新的存储键：

```text
yssbi-workbench-flexlayout:<window-label>
```

tab config 只包含 metadata，不持久化输入焦点；含其他 config 字段的布局按现有无效快照规则回退默认布局。

value 为：

```text
{
  root: IJsonModel,
  nested: { logs: IJsonModel }
}
```

root 与 nested.logs 独立验证和恢复。解析在原生 Model 标准化前检查树结构、稳定 ID、深度/数量限制、面板 metadata、组件匹配、singleton 和受限位置。root 的 Settings float 与主树共用 ID、singleton 和数量校验，浮动矩形必须包含有限坐标及正尺寸；nested.logs 不接受浮动子布局。恢复时重新施加宿主的浮动、关闭和拖放约束。底部包含 Problems、Output、Logs 之外面板的已保存 root 判为无效，沿用默认布局回退。

窗口关闭在当前 hydration 和 FIFO idle 后 flush。Result 从持久化快照移除；Project replacement 另外清理 editor。用户关闭 Assistant 后，恢复不自动重建；显式重置会重新安装它。插件缺失状态仍由插件注册协调者处理。

旧 Dockview 存储不读取、不转换；第一次使用新键加载 FlexLayout 默认布局。此变化只影响工作台偏好，不迁移或修改项目资源。布局快照不保存后端结果本体和 Graph 撤销历史。

### 8.1 原生窗口几何与关闭

主窗口的项目管理页（`/`、`/projects`）与编辑页（`/editor`）分别记忆位置、尺寸和最大化状态。
[mainWindowGeometry](../../services/platform/mainWindowGeometry.ts) 在 localStorage 的
`yssbi-main-window:projects` 与 `yssbi-main-window:editor` 中保存物理几何，监听移动和缩放，
路由切换时先保存离开页面再恢复目标页面；最大化保留普通窗口尺寸，最小化不覆盖记录。
首次进入项目管理页使用 1100×720、编辑页使用 1600×900 逻辑像素并居中，已保存的几何优先。
Tauri 主窗口创建尺寸与项目管理页默认尺寸一致；保存位置不在可用显示器上时重新居中。
两页共用同一个原生窗口和项目会话，所有项目共用编辑页窗口偏好。

主窗口排除在 `tauri-plugin-window-state` 之外。次级窗口的位置、尺寸和最大化状态仍由桌面根包装配的插件维护，
持久化到应用配置目录的 `.window-state.json`。子窗口逻辑像素默认尺寸来自
[createPersistedWindow](../../features/application/window/createPersistedWindow.ts)；保存后的物理几何由插件恢复。
次级窗口以 label 第一个 `-` 前的部分作为状态键，按种类共享状态；实例 label 仍用于窗口及结果租约身份。

插件在次级窗口创建时自动恢复几何；主窗口由 Rust setup 显示，前端路由初始化时恢复页面几何。
插件不管理可见性、装饰或全屏：子窗口隐藏创建，由内容准备流程显示；
装饰继续采用当前应用设置。创建 Promise 等待原生 `tauri://created` 或 `tauri://error`，结果租约据此判断打开是否成功。

工作台只有 `useWorkbenchWindowCloseGuard` 决定是否关闭，先处理未保存内容和布局 flush。
原生几何监听不参与这个决策。窗口销毁并从 Tauri manager 移除后，根包调用插件落盘，失败记录诊断；
应用退出也沿用插件保存。几何属于可恢复偏好，采用插件的缓存和直接文件写入语义，不具有业务事务提交保证。
同种窗口并存时共享插件缓存；保存时仍打开的同组窗口可能刷新缓存，不保证最后关闭实例的状态获胜。
旧 `window_state.json` 不再读取，没有兼容读写或自动迁移；首次使用新存储按默认尺寸打开。

## 9. 样式与标签

直接加载 flexlayout-react/style/combined.css，使用 alpha_light / alpha_dark 原生主题，随应用主题切换。标签选中、关闭按钮、边栏、分隔条、拖放指示和最大化按钮均使用库的设计。

工作台横向标签栏移除首部 spacer 和叠加的左侧 padding，使首个 tab button 与 tab content 的外边缘对齐；按钮自身内距和标签之间的间隔保留原生样式。该规则不作用于侧栏或底部 border 的标签，底栏与中央区的列对齐保持独立。

src/app/workbench-layout.css 设置宿主尺寸、字体尺度、标题图标/dirty 标记与边框区域的排列。根布局将原生 border 元素排入 CSS Grid：左右侧栏延伸到底部栏上方，top/bottom border 的内容和分隔条与中央 tabsets 共用一列，bottom bar 横跨整个窗口。保留原生 Model、尺寸、测量与拖放能力，不增加布局模型或挪动组件 DOM。RootPanelTabRenderer 只贡献标题内容和业务右键菜单。关闭按钮、中键和原生关闭动作进入既有关闭协调者。

底部直接使用 FlexLayout 原生 tab，取消独立 footer 和重复的面板 icon。StatusBar 只呈现状态条目，通过 onRenderTabSet 放进 bottom border 的原生 toolbar；Settings 使用同一底栏的 leading 插槽。整条栏贯穿窗口，通过 CSS subgrid 共用工作台列宽：设置位于窗口最左侧，三个 tab 对齐中央区左边，节点、连接、选中等状态对齐中央区右边。侧栏缩放和折叠时由 CSS 自动保持对齐，不测量边界、不维护偏移 CSS 变量。日志等内容仍只位于中央工作区下方。底部 tab 全部关闭后，可用 Ctrl+反引号重新打开 Logs，或重置布局恢复三个工具面板。底部所有 tab 都关闭时不显示内容面板，条带仍承载状态信息。Details 和 Assistant 使用右侧原生 tab。

## 10. Verification

检查命令见[前端 README](../../README.md)，范围遵循[根规则](../../../.rules)。布局库替换需验证所有公共查询和操作的消费者、架构依赖策略、类型和前端构建。保留应用层关闭、焦点、项目切换和 Result 生命周期的行为检查；移除依赖旧 Dockview 实例与 JSON 结构的专属测试夹具。

遵守仓库规则，UI 不新增单元测试。交互验收覆盖原生拖动/分屏、折叠、主题切换、面板状态保留、取消关闭、结果租约和项目切换。浏览器中使用模拟平台边界的检查不能替代真实 Tauri 数据与窗口生命周期验收。

## 相关模块

[画布交互](../graph-editor/README.md) · [Results 生命周期](../../features/application/results/README.md) · [JSON 页面](../../../src-tauri/crates/yss-ui-contract/README.md)
