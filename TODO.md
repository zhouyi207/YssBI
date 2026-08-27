# 在正式打包分发前，可以等渲染完毕再显示窗口
# 在目前的开发环境中，不要这样做，可以取消 debug
# 项目未发布 不做任何迁移处理

每次更新版本都需要

由于历史代码重构原因，目前项目中存在许多的历史遗留代码，或多余或逻辑重复或实现低效；请检查整体项目，寻找出项目中的重复逻辑和未使用的逻辑，分析必要性，如果有更高效的更干净的架构请添加到 todo 的 v1.0 待办中，如果单纯的逻辑重复或者多余，也请添加到 v1.0 待办中

请分析这个问题有没有必要修复，如果有必要，则使用高效且干净的架构来执行这个逻辑，同时清除掉无效逻辑代码和重复逻辑代码

重复逻辑问题？无效逻辑问题？代码漂移问题？多事实源问题？代码冲突问题？无效函数问题？deprecated 兼容问题？

[@improve-codebase-architecture](zed:///agent/skill?name=improve-codebase-architecture&source=global&path=C%3A%5CUsers%5Czhou.yi31%5C.agents%5Cskills%5Cimprove-codebase-architecture%5CSKILL.md) [@grill-me](zed:///agent/skill?name=grill-me&source=global&path=C%3A%5CUsers%5Czhou.yi31%5C.agents%5Cskills%5Cgrill-me%5CSKILL.md) [@vercel-react-best-practices](zed:///agent/skill?name=vercel-react-best-practices&source=global&path=C%3A%5CUsers%5Czhou.yi31%5C.agents%5Cskills%5Cvercel-react-best-practices%5CSKILL.md) [@vercel-composition-patterns](zed:///agent/skill?name=vercel-composition-patterns&source=global&path=C%3A%5CUsers%5Czhou.yi31%5C.agents%5Cskills%5Cvercel-composition-patterns%5CSKILL.md) 请检查前端 react 架构，是否有 重复逻辑问题？无效逻辑问题？代码漂移问题？多事实源问题？代码冲突问题？无效函数问题？deprecated 兼容问题？如果有请修复，并优化代码架构，同时删除边缘测试，在这里有很多的测试是低效的完全没必要的

[@improve-codebase-architecture](zed:///agent/skill?name=improve-codebase-architecture&source=global&path=C%3A%5CUsers%5Czhou.yi31%5C.agents%5Cskills%5Cimprove-codebase-architecture%5CSKILL.md) [@grill-me](zed:///agent/skill?name=grill-me&source=global&path=C%3A%5CUsers%5Czhou.yi31%5C.agents%5Cskills%5Cgrill-me%5CSKILL.md) 请先检查 rust 后端架构，是否有重复逻辑问题？无效逻辑问题？代码漂移问题？多事实源问题？代码冲突问题？无效函数问题？deprecated 兼容问题？如果有请修复，并优化代码架构，同时删除边缘测试，在这里有很多的测试是低效的完全没必要的

测试应该保护未来仍然成立的行为或架构约束，而不是永久证明某次历史重构确实做过。

```
同步改四处版本（例如 0.1.1）：

src-tauri/Cargo.toml
src-tauri/tauri.conf.json
package.json
src/app/appConfig/appLinks.ts


提交并 push，或手动再跑 publish.yml
```

# DOLIST

## 2026.08.26

- [ ] 将根 Dockview 的固定 view tab 标题接入现有 activityBar/panel 翻译 key，覆盖底部、右侧和 Activity tab。
- [ ] 保持编辑器资源名、结果标题和 Logs workspace 内部 domain tab 的既有动态标题逻辑，不把本地化文本写入布局持久化数据。
- [ ] 增加固定 workbench view 标题的统一回归测试，并通过相关测试、TypeScript 检查和 i18n key 校验。
- [ ] 将 Workbench graph tab 的中键与右键监听从会被 Dockview 重建的 `.dv-tab` 外壳替换到复用的 React header host。
- [ ] 增加 graph tab 拆分到新 group 后仍可打开文档右键菜单的行为回归测试，并校验菜单使用新的 groupId。
- [ ] 通过 WorkbenchDockviewTab 聚焦测试、相关 Dockview 测试、TypeScript 检查和差异校验。
- [ ] 审计 Dockview tab/group 相关 DOM 监听，确认除稳定 React header host 外未发现第二处挂载到临时 `.dv-tab` 的生产逻辑。
- [ ] 发现 Workbench tab 的 edge collapsed 状态未订阅面板 group 迁移，central/edge 移动后折叠视觉状态可能滞后，需单独修复。
- [ ] 确认 Workspace 的 Delete 键事件过滤、Workbench group 订阅和 Logs nested Dockview 不属于本次 tab 监听失效问题。
- [ ] 修复 Workbench tab 的 edge collapsed 状态订阅，使面板跨 group 移动后重新绑定当前 group 的折叠事件。
- [ ] 增加 edge tab 从收缩 group 移回中央 group 后清除视觉收缩标记的回归测试。
- [ ] 修复 Logs nested Dockview 自定义 tab 未订阅 `onDidTitleChange` 导致 i18n 标题不刷新的问题，并覆盖 title event 回归测试。
- [ ] 复查 Dockview 的 title、group、visibility、collapse 和 DOM 监听生命周期，未发现其它同类生产 bug。

- [ ] 分析 Output/Diagnostics 顶部当前图路径仅为展示信息，确认可移除而不改变图级数据语义。
- [ ] 将 Output/Diagnostics 顶部栏统一为 Logs 风格的紧凑横向 header，保留标题、清理操作和诊断数量。
- [ ] 增加 Output/Diagnostics header 回归测试，验证隐藏图路径且保留 focused graph 的输出与诊断行为。

## 2026.08.25

- [ ] 将 Logs 内嵌 Dockview 子 tab 固定为完整日志域集合，移除 tab 关闭 X 和中键关闭行为。
- [ ] 删除 Logs 右侧“+”新增日志域菜单及相关新增逻辑，缺少固定域的持久化布局自动回退到完整默认布局。
- [ ] 限制 Logs tab 仅可在同一组内交换顺序，并优化激活态、悬停态和右侧工具栏样式。
- [ ] 增加 Rust production-module architecture audit，强制生产 Project 不依赖 Application 或 Commands，并对 raw identifier、conditional path 与代码 include 采取可验证的保守处理。
- [ ] 将 Project-relative DuckDB runtime binding/physical removal 下移 Database，将现有 ColumnInfoDTO conversion 下移 Schema，保持事务、快照、错误与 IPC 行为不变。
- [ ] 更新权威架构图与 Database module 文档，明确 Application 编排、Project authority、Database primitives 和 Schema wire conversion 的单向边界。
- [ ] 执行 strict architecture policy：用 Rust/TypeScript canonical-origin 审计、exact debt 与 semantic guards 强制单向依赖，并把现有债务逐项清零。
- [ ] 执行 Rust backend adapter boundaries：让 SCI、Database、watcher/progress 和 scientific/relational/resource ports 脱离 Graph、Project、Tauri 与具体后端。
- [ ] 执行 Project–Graph ownership decoupling：Graph 只拥有 document/schema/catalog/compiler contract，Project 保持唯一持久化与 history authority，Application 负责 capture/plan/commit。
- [ ] 执行 Execution runtime extraction：建立原子 Application session、Execution-owned plan/runtime/settings、RunRegistry 与两阶段 finalization，删除 Project 执行 owner。
- [ ] 执行 Presentation/Event/Command boundaries：把 editor/result presentation 与跨域事件策略归 Application，Schema/Event 只做 wire/delivery，Tauri commands 保持薄层。
- [ ] 执行 Frontend Application boundaries：后端状态只作不可变 projection，Application hooks/coordinators 统一 reconciliation、optimistic echo 与 use-case，UI/store 不再直连 Services/Tauri。
- [x] 将 Canvas 编辑器交互按 `panelInstanceId` 隔离，修复同一 group 中多个 tab 共享 active tab 导致的右键创建节点、选择、连线、拖放和快捷键操作失效问题。
- [x] 清理 Canvas 的 group 级 active tab、重复命令路由、失效拖放处理和旧的自定义 tab 移动逻辑，统一使用 Dockview 的默认移动行为。
- [x] 删除与上述历史实现绑定的冗余测试和失效测试；保留必要的 pane 快照稳定性约束。
- [x] 修复空 panel 选择状态返回新对象导致 React `getSnapshot` 无限更新的问题，使用稳定的空选择快照。
- [x] 修复首次进入项目时左侧 Activity sidebar 默认激活 `commands` 的问题，默认改为激活 `project`。
- [ ] 将现有双语节点 Markdown 以编译期 catalog 文档 registry 接入 Detail 的 documentation projection。
- [ ] 让静态节点与资源绑定节点按当前 locale 读取 Markdown，并在语言缺失时回退英文。
- [ ] 对没有显式 Markdown 映射的节点停止使用 i18n documentation 作为隐式 fallback，保持文档单一来源。
- [ ] 将 `PortSpec.title` 固定为各节点结构化 pin 定义直接提供的非本地化标题。
- [ ] 删除协议层按 `nodeType`/`key` 推导 pin title 的全局映射、特例分支和未知 key fallback。
- [ ] 保持 Markdown 中的 pin title 与 Rust 结构化 pin 定义人工同步，不在运行时或编译期解析 Markdown title。
- [ ] 按 ponytail 审核 pin title/Markdown 相关测试，移除 catalog 职责之外的重复文档断言并保留稳定的 projection 契约。
- [ ] 更新 node-system golden fixtures 以反映 Markdown documentation 与结构化 `PortSpec.title`，移除已失效的 `label_key` 快照契约。
- [ ] 合并 Markdown documentation 的正向验证到 catalog 公共 projection 测试，覆盖英文、中文与 locale 差异。
- [ ] 移除 DataFrame 参数化节点测试中与节点参数契约无关的 documentation 断言。
- [ ] 重新生成 node-system golden fixtures，使 Markdown documentation 与 `PortSpec.title` 成为当前契约。
- [x] 使用的 ag-grid ~~我想将 @glideapps/glide-data-grid 切换为 shadcn 中的 data table，主要是因为风格和组件和目前的 shadcn 组件不搭，同时在构建的时候还有一些其他的错误，如下。需要考虑替换的可行性；或许使用 Handsontable 替代（商用收费）~~
- [x] tolerance 和 num_traits
- [x] 在前端中的 graph 中的 data pin 的类别都是 unknown，导致节点没有颜色，同时在 pin 的时候不会筛选节点，更不会自动连接节点，这个是需要修复的，可能需要完整的从后端发送类型过来避免字符串解析？这样会更加完整？这里需要仔细考虑
- [x] 将过去的操作尽可能实现后归纳到 v0_0.md 文档
- [ ] node 的 tooltip 功能，可以查看节点的信息
- [ ] 在根 Dockview 的 Output 右侧新增 Diagnostics tab，集中展示当前图的节点诊断信息。
- [ ] 按当前聚焦图的节点顺序汇总所有节点 diagnostics，并显示严重级别、节点、诊断 code 与消息。
- [ ] 点击 Diagnostics 条目后定位对应节点并切换 Details 上下文；旧布局恢复时自动补齐缺失的 Diagnostics tab。
- [ ] 移除 editor 顶部主题切换按钮右侧的 Details 切换按钮及其 menubar/View 菜单逻辑。
- [ ] 将 Details 固定为根 Dockview 右侧常驻 sidebar，初始化、旧布局恢复和重置布局时自动创建并默认展开，同时保留用户调整的宽度。
- [ ] 保留 Details Dockview tab 原有图标与文本标题样式，仅移除关闭入口，不改变 Activity bar 的纯图标样式。
- [ ] 禁止 Details 通过关闭、上下文菜单或拖拽布局离开右侧 sidebar，并保留 Details context 更新功能。

架构，不要你中有我我中有你，最好组件化？是这个意思吧？即下面的分析

- [ ] snapshot 有必要吗？？？？ 还有 run id，以及每次允许之后会在 details 中出现的 developer trace 中记录的历史数据，打开会很卡。
- [ ] 在更改 graph 的时候 tabbar 中的样式并没有其他变化，如果在更改后不保存关闭，那么下次打开打开的时候还是更改前的状态，这里明显是不符合逻辑的，除此之外还有其他的需要检查；同时磁盘上以及更新的符号和标签我感觉可以去掉，可以学习 vscode 的 tabbar 处理
- [ ] 在 sidebar 中创建 item 的时候首先会出现在最下方然后根据 name 移动位置，能不能直接根据 name 出现在某个位置，忽略出现在下方的过程，这样不美观
- [ ] 目前后端节点的定义好像也不太清晰明了，需要讨论怎么处理
- [ ] 在 graph 中的右键菜单我希望根据 section subsection 等等分类，包括 activitybar 为节点的 sidebar 中的节点也是一样，这样如果不记得名称找起来非常方便，需要讨论
- [ ] 后续我会加入 mcp 功能方便 llm 直接调用统计方法获得数值结果，同时我会加入智能分析功能利用 llm 分析数值报告获得分析结果，在这里我初步的构想是在 activitybar 中添加一个报告的 icon，其对应的 sidebar 中显示各种数值报告的 item，然后 llm 可以对这些报告进行执行分析输出得到结果编写论文；你怎么看，目前怎么预留接口，前端应该如何设计等等需要仔细讨论和实现（在这里 data 中每一列我希望可以添加一个描述统计，意味着我们可以在导入数据的时候对 data column 添加一个文本标注，方便模型知道 data column 并进行描述统计分析一些有意义的结果并输出一些合理的假设）
- [ ] 关于可视化层面，目前可视化的图表还不够，我希望更加的丰富；并将这些图表组件化放置在一起，哪里需要就调用避免重复实现，差异较大可以分为两个组件
- [ ] 思考是否有必要多窗口进行跨窗同步，这样就不需要什么多进程的 token 了吧
- [ ] graph 分为两种，一种是纯计算 graph，一种是目前这种；纯计算 graph 使用 notebook 这种形式，修改节点会污染依赖该节点的下游节点，递归污染；运行到此节点可以做到将上游阶段全部干净，
- [ ] 我认为下面的版本信息完全没有必要


            GlobalVariableMutation::Delete {
                id,
                expected_revision,
            },


## v1.0 待办

`v2` 原本用于区分旧持久化格式，但既然明确要求**不迁移、不兼容、直接删除旧路径**，继续维护版本号没有价值，反而暗示未来会做 schema migration。

### 窗口跨窗同步

- [ ] assistant-ui
- [ ] 将 worksheet 重命名为 charts

```
"/*#__PURE__*/"

in "node_modules/.pnpm/@glideapps+glide-data-grid@6.0.3_lodash@4.18.1_marked@4.3.0_react-dom@19.2.7_react@19.2_c19a5bde3a2383671a6324b7c97614b7/node_modules/@glideapps/glide-data-grid/dist/esm/internal/data-editor-container/data-grid-container.js" contains an annotation that Rollup cannot interpret due to the position of the comment. The comment will be removed to avoid issues.
node_modules/.pnpm/@glideapps+glide-data-grid@6.0.3_lodash@4.18.1_marked@4.3.0_react-dom@19.2.7_react@19.2_c19a5bde3a2383671a6324b7c97614b7/node_modules/@glideapps/glide-data-grid/dist/esm/internal/data-grid-overlay-editor/private/markdown-overlay-editor-style.js (2:13): A comment
```
### 口语化表达

- [ ] **多数据库 DataView 直接编辑行定位抽象**：当前项目内 DuckDB 持久化表用 DuckDB `rowid` 做分页/编辑定位；后续若支持 SQLite / MySQL 等外部数据库直接编辑，需要新增 `RowLocator` / `BackendRowKey` 类能力抽象，各 backend 明确自己的稳定行键策略（DuckDB `rowid`、SQLite `rowid` 或主键、MySQL 必须主键/唯一键）；无稳定行键的外部表默认只读或先导入项目 DuckDB，避免把 DuckDB `rowid` 语义错误泛化到所有数据库
- [ ] **Worksheet 图表切 tab 性能优化（坚持 ChartViewModel 路线）**：不要全局把所有 tab 内容 hidden 保活；继续沿用当前 preview/data 缓存方向，把昂贵工作从 React mount 生命周期中移出。后续将 `WorksheetPreviewPayload` 细化为更完整的 `ChartViewModel`（缓存数据列、聚合结果、domain、ticks、legend/tooltip 元信息等），组件重挂载时直接复用模型；绘制层避免 `svg.selectAll('*').remove()` 全量重建，尺寸变化只重算 scale/位置，大数据 scatter/line 考虑采样或 canvas 渲染；缓存使用 LRU，并在 DataView 编辑、数据版本变化或 worksheet spec 变化时精确失效
- [ ] **变量类型切换时的值迁移 / 智能转换（暂缓）**：当前策略——切换类型且未显式提交新值时，重置为 `DataType::default_value()`；Array / Object / DataFrame / DataSeries 已用 JSON 列式编辑 + `tabular/` 存储（见 ## 2026.07.03 已勾项）。**暂不实现**跨类型自动保留或 coerce（如 Int→String、DataFrame↔Array、Object 字段映射等）；后续可考虑接入 `DataValue::coerce_to`、切换前「将丢失当前值」提示。变量类型不可选 Any。
- [ ] **View：大 Array 分页 tabular（暂缓，没有合适的前端表现）**：一维、同质、较长 Array 走后端 `getPage`（2 列 `#` / `value`，与 DataSeries 同 API 形状）；短数组 / 嵌套 / 异构仍走 `json` + `JsonTreeView`；需 `ResultSource` 或虚拟 tabular 存储与 builder 分支。
- [ ] **Struct handle → View JSON 架构重设计（暂缓实现）**：当前 `ExecutionDataStore` 仅存 `Arc<dyn Any>`，`DataValue::Struct` 的 `typeKey` 与 handle 分离；View / `build_struct_source` 只能事后按 `typeKey` downcast，临时用 `execution/struct_json.rs` 中央 match 表（OLSModel、OLSResult 等逐个注册）——每增 Struct 要改表，且 `typeKey` 双写，不可持续。**待选方向（均未定稿，先不实现）**：① **入库 JSON 快照**：`put_struct(type_key, T: Serialize)` 写入 handle 时同步 `view_json`，View 只读快照、Predict 仍 downcast；② **`dyn ViewPayload` trait**：handle 自带 `view_json()` + `as_any()`；③ **TypeId 注册表 + macro**：注册点贴近类型定义，替代 central match；④ **View 永不碰 handle**：所有 output 注册 source 时必须带 JSON（仍要解决无 upstream source 时的首次序列化）。实施前需统一：handle 层 vs `ResultSourceStore` 谁为 JSON 真源、不可 `Serialize` 的类型（如 `StandardizeTransform1D`）策略、与现有 `source_id` 复用链如何衔接。完成后删除 `struct_json.rs` 式 per-type 注册。
- [ ] **View 节点展示（续）**：核心 renderer / source 统一 / 子窗口 layout 已完成，见 ## 2026.07.03 未完成项（Array 分页 tabular、子窗口 chrome、runtime source 生命周期、Struct handle JSON 架构重设计）。
- [ ] 复制粘贴撤回逻辑的快捷键效果有问题
- [ ] 值类型处理
- [ ] 还有 7 个组件属于「壳统一了、内部还没拆干净」，优先级建议：VEC → Panel → DID → VARSoc → DFADFSummaryList → VecRank → DFADF。
- [ ] 优点：window_* 是「当时那一刻」的不可变快照，重跑不会误改已打开窗口里的内容。代价：不关窗时会累积（For 循环多次 View 会留下多个 window_*），直到关窗或 clear_all。文档里提过 Window LRU/TTL，尚未实现。
- [ ] **On Error / 错误传播（待设计）**：MaxIterations + loop_counters + 执行前清空已落地。错误模型仍停在「节点失败 → 记日志 + 发事件 + 整图 has_error」，没有可连线的错误传播；要做 On Error 需先定：错误是否中断下游、是否进专用 exec pin、与 Loop/Sequence 如何交互等，再扩 `ExecutionEffect` 和 executor。
- [ ] 节点样式问题
- [ ] **CI 扩展：`cargo clippy` + integration tests 矩阵**：在 `cargo test` 之外增加 `cargo clippy --all-targets`（先 `yss-sci` 修 error 再全 workspace）；与前端 `typecheck` 并列，形成全栈静态门禁。
- [ ] **CI 门禁 `tsc --noEmit`**：`package.json` 增加 `typecheck` script，CI 与 pre-push 跑 `pnpm typecheck`（`noUnusedLocals` 已开，需防止类型债再次累积）。
- [ ] **CI 门禁：`typecheck` + vitest + `cargo test` 并列**：`tsc` 无法捕获仅运行时才暴露的 API 形参错误（如 `batchCreateNodes` 三参数旧调用）；`package.json` scripts 与 CI workflow 至少跑 `tsc --noEmit`、核心 vitest 套件、Rust integration tests。
- [ ] **OLS 取数「逐边」vs「批量」语义文档化**：当前执行器按边 `emit_data_pull` → 求值 → `emit_data_flow`；确认是否故意取代旧 NodeStart 批量高亮，并在 `TODO`/执行器注释中写清 UX 预期，避免后续误改回批量形式。
- [ ] uistyle 可能需要根据节点类型来进行重构
- [ ] 在 editor group 多个的情况下，刷新后回到了单个 watermake 界面，但是同时会出现警告：当前编辑器图未能加载，请重新点击标签页或画布
- [ ] 函数图层中 **递归 Call 编辑器提示**：`CallDepthGuard`（64）仅 runtime 报错；编辑器内对自递归/深链 Call 做静态提示（非阻断），与超限单测（见 Rust 复盘）配套。
- [ ] sidebar 内容中的 scrollbar 以及日志及其他组件内容的拖动逻辑有问题
- [ ] 剩余唯一标记是 Rust 执行上下文中的 get_bound_type TODO。它依赖尚未提供类型绑定状态的 GraphRuntime，当前直接返回 None 是明确的未实现能力，不适合通过猜测补丁，否则可能引入错误类型推断。
- [ ] **ACF/PACF 命令与 Plot 节点 DTO 对齐**：`plot/correlogram.rs` 输出 `CorrelogramDatum { lag, value, q_stat, p_value }`；`command_sci::compute_acf_pacf` + InfoView `ACFPACFBlock` 仅 `Vec<f64>` + `n`——复用 `cumulative_ljung_box`，扩展 `AcfPacfResponse` 或共用 `CorrelogramPlotData`，避免 Summary 图 tooltip 缺 Q/p-value（前端 `CorrelogramChart` 已按可选字段防御）。
- [ ] **Julia 第二个迁移目标选择**：ACF/PACF 已经有 `src/sci` API、Julia worker 和 Rust/Julia golden fixture 测试；下一步不要直接上 VEC/RE MLE/DID。优先在「serial tests / Ljung-Box / DW」和「描述性统计」里选一个做第二个 PoC：输入输出简单、能复用 Arrow IPC、容易与 golden result 对齐。简化 OLS 可以排第三步，先只做 `y: Float64` + `x: Float64 matrix` + `hasIntercept`，暂不碰公式、分类变量、robust/cluster/HAC。

- [ ] bayes 中的有很多的 errors.push(error("PREDICTOR_REQUIRED", "预测表达式尚未解析或绑定。", "boundPredictor")); 后期都是要修复的
- [ ] bayes 中的 ast 感觉可以和 src 下的 ast 放置在一起，在这里好像有 latex -> json ast，json -> julia ast，normal formula -> json ast 等等 ast
- [ ] bayes 长任务的通知最好是作为复用模块
- [ ] Failed to install Juliaup: 找不到与输入条件匹配的程序包。安装不了 julia
- [ ] 在这里似乎日志类的测试感觉没有必要，可以直接删掉
- [ ] clippy::too_many_arguments 这些感觉需要清理，不符合 rust 代码标准

函数和事件保持一致性的 API 重复层面：不影响编辑一致性，但维护成本高：

useGraphManagement 里 addEvent / addFunction、deleteEvent / deleteFunction 几乎镜像，底层已是 createGraphResource(kind) / deleteGraphWithConfirm(kind)
GraphResourceKind vs GraphResourceType 两处 type alias（sidebar / editor）
快捷键 Ctrl+N 仅新建 Event（产品选择，非 bug）
Menubar / Watermark 仍分「新建 Event / 新建 Function」两项（入口文案差异，合理）
若要进一步收敛，可以把 Session 对外 API 收成 addGraph(kind) / deleteGraph(kind)，Sidebar/Menubar 只传 kind，不再暴露四套函数名。

# TODOLIST

绘图组件库需要重构

xt align 进行对齐，在这里是不是可以于 ts align 可以共用呢

感觉 faer 计算得很慢呢，是不是没有开启并行的原因，4500*17 维度需要耗时 1600ms 有点儿久

然后就是真的需要 polars-dtype 这个 crate 吗？我感觉 polar 应该包含了吧， ai 是不是弄错了

polars 的 csv 有意思，with_try_parse_dates 可以检索日期，没有检索 categories 的

现在节点分类特别混乱

gls 的 data input pin 中我认为可以设置为 matrix，这就意味着需要在值系统中添加并定义 matrix 类型，目前是 dataframe

wls 和 gls 的 predict 节点有问题，目前 wls 报错：Node 6b7c0693-8d92-4c76-a253-6d49333221ab failed: Predict: Model input is not connected or invalid


HAC 已实现（Bartlett、Parzen、Quadratic Spectral kernel，lag 参数）；hac-panel、hac-groupsum 尚未实现。

目前使用 fixed scale 并没有提供 scale pin ，同时在 ols configure vce 可接受的结构体中删除掉 hac-panel 和 hac-groupsum

对了 schema 中关于 vce 的要删除，暂时没有用到

OneOf 还是使用 Restriction + TypeVar 来做类型推断之类的东西？

oneof 类型连接其中一个类型的 pin 的时候，会变成该类型；（这个效果是好还是不好？很难知道）

我觉得 plot 可以学习 seaborn 来进行参数选择和绘制

catelog 中的 plot（尤其） 和 distribution 中的内容需要大量重构

同时 plot 可以使用数据节点，然后数据节点需要使用 plot 的 show 节点才可以展示，然后 show 节点可以结合多张图的数据节点，在新的窗口中配置如何结合的信息。

复制粘贴节点以及项目的保存不够完善

直方图有问题，其显示将 null 值当作了 0 在图中处理，正常应该忽略

打开 dataviewer 后，在 editorviewer 中导入数据，dataviewer 并没有更新；

类型推断系统不够完善

type infer 使用 dirty 的形式推测，不要每次都推测全量类型

function, macro 功能添加（感觉没必要区分 function 和 macro ）

ols 节点的 evaltor 逻辑有巨大的优化空间

不如纯 data 节点在连接的时候就进行计算就好了

执行动画有一点bug，节点在获取数据执行其他节点的时候本身状态并没有执行完毕但是ui上显示执行完毕了

为了解决 dummy 问题，例如个体效应和时间效应的哑变量问题，可能需要在 dataseries 中添加额外信息进而去提出多出来的哑变量：计划做一个 add dummy info 节点，下方有个下拉列表或者文本框选择需要设置 dummy 的信息，仅对 dataseries 为 string 的信息管用。

面板数据，可能需要对 dataframe 数据类型也加上 info，来表示个体信息和时间信息 dataseries

未来可视化需要加上地图，地图应该使用下载的方式下载到数据库中，下载完毕后才能使用；

settingview.tsx 中，要注意区分，首先 dataframe 是必须要设置形状和颜色的，然后 array, dataseries 等复合类型应该是控制形状，基础类型控制颜色，同时基础类型应该是相同的形状，any 和 受限制的 any 应该是颜色，应该区分 all any, base any, part any

工具变量法：2sls

dataframe 抽样方法，是在 ols 配置还是 dataframe 层面配置呢

参数网格 -》 数据变换（异常值检验，对数变换等等操作） -》 检验结果 -》 存储

ols_summary 打开 ols_result_viewer 并返回 ols_result, 这里面存储了一些统计模型信息可以使用节点进行提取；这些节点应该是使用类似于函数或者其他功能的注册的方式而不是定义，不然后续结构体太多了这里会爆炸

ols model 可以引申出一个新的节点 predict，这个节点可以使用 endog, exog 两个玩意获得拟合值，然后真实值 - 拟合值可以得到残差。这是基本操作不应该删除

deserializeGraph 这个玩意是干嘛的，好多地方都没必要用他，感觉好卡

下面这玩意在软件退出时保存了两次

[12:43:12.734][BE][INFO] [APP] Settings loaded successfully via backend
[12:43:12.734][BE][INFO] [APP] Settings loaded successfully via backend
[12:43:12.735][BE][INFO] [APP] Settings loaded successfully via backend
[12:43:12.736][BE][INFO] [APP] Settings loaded successfully via backend
[12:43:12.756][BE][DEBUG] [APP] Settings saved successfully via backend
[12:43:12.757][BE][DEBUG] [APP] Settings saved successfully via backend
[12:43:12.757][BE][DEBUG] [APP] Settings saved successfully via backend
[12:43:12.758][BE][DEBUG] [APP] Settings saved successfully via backend


需要你确认的执行语义

我建议选择：

### A. Demand-driven（推荐）

- 普通运行只计算终端结果、effect 和跨 island 依赖；
- 中间 Pin 不自动物化；
- 点击 Pin 预览时单独请求该输出；
- compiler 根据 requested outputs 决定 roots。

### B. 所有 Pin 每次都可立即查询

- 每次运行自动计算并保存所有节点输出；
- 行为接近旧架构；
- relational pushdown 和大图优化空间明显受限。

### C. 用户配置

- 默认 demand-driven；
- 可将特定 Pin 标记为“始终保留”；
- 灵活，但第一阶段复杂度更高。

请选择 **A、B 或 C**。我推荐 **A**。


## 项目分析

我看了你这个仓库当前的 `shadcn` 分支，包括 README、前后端目录、Cargo 依赖、services、features、commands、execution、TODO 和 CI。整体上我认为 **方向是对的，而且已经明显超过普通 Tauri CRUD 项目**：你实际上在做一个「节点式统计分析 IDE / 桌面 BI 工作台」。README 里已经覆盖数据管理、节点工作流、计量经济学、时间序列、因果推断和可视化。([GitHub][1])

但现在确实到了一个很关键的阶段：**接下来最应该做的是收敛架构，而不是继续堆功能。**

我会把优化优先级排成下面这样。

### P0：先把后端真正拆成“宿主层”和“计算层”

你现在已经有：

```text
src-tauri/
├── src/
│   ├── application
│   ├── commands
│   ├── database
│   ├── execution
│   ├── graph
│   ├── node_system
│   ├── project
│   ├── sci
│   └── ...
│
└── sci/
    └── 独立 yss-sci crate
```

这说明你已经开始做正确的拆分。([GitHub][2])

但是主 `yssbi` crate 依然直接依赖：

```text
ndarray
polars
polars-arrow
polars-dtype
statrs
sqlx
duckdb
calamine
rand
...
```

而 `yss-sci` 又已经存在。([GitHub][3])

我建议继续收敛成：

```text
Tauri / Host
        │
        ▼
Application
        │
        ▼
Execution Engine
        │
        ├───────────────┐
        ▼               ▼
Data Engine         Compute Engine
Polars/DuckDB       Rust / Julia
                        │
                        ▼
                     yss-sci
```

也就是说：

```text
yssbi
负责：
Tauri
窗口
IPC
项目
Graph
Executor
数据生命周期
任务调度

yss-sci
负责：
OLS
GLS
WLS
IV
Panel
VAR
VECM
DID
Hypothesis Test
统计量
线性代数
```

尤其应该避免：

```rust
commands -> 统计实现
```

而变成：

```text
command
   ↓
application/use-case
   ↓
execution
   ↓
compute backend
   ↓
yss-sci / Julia
```

你自己的 TODO 已经提到“Rust 保留宿主、数据层和必要 fallback；科学计算逐步迁移 Julia”，这个总体思想是合理的。([GitHub][4])

---

## P0：把 Execution Engine 做成整个 YssBI 的核心

我认为 **YssBI 最值钱的代码将来可能不是 OLS，也不是 UI，而是 execution engine。**

README 已经显示你的核心交互是：

```text
Node
 ↓
Pin
 ↓
Connection
 ↓
Graph
 ↓
Execution
```

并且你已经有独立：

```text
execution/
graph/
node_system/
```

目录。([GitHub][2])

建议进一步明确一个非常重要的边界：

```text
Graph ≠ Execution
```

Graph 只描述：

```text
节点是什么
连接是什么
参数是什么
依赖是什么
```

Execution 才负责：

```text
拓扑排序
dependency resolution
dirty propagation
cache
task scheduling
cancellation
progress
error propagation
parallel execution
```

最终可以形成：

```rust
ExecutionPlan
    ↓
TaskGraph
    ↓
Scheduler
    ↓
Executor
    ↓
Backend
```

类似：

```rust
trait ExecutionBackend {
    async fn execute(
        &self,
        task: &ExecutionTask,
        ctx: &ExecutionContext,
    ) -> Result<TaskOutput, ExecutionError>;
}
```

backend 可以有：

```text
PolarsBackend
DuckDbBackend
RustSciBackend
JuliaBackend
AIBackend
```

这样以后加 Python、R、GPU 都不会重新设计节点系统。

---

# P0：解决“计算导致 UI 卡死”

你 TODO 里自己已经发现了：

> 按下按钮涉及大量计算的时候，页面会卡死。([GitHub][5])

这个千万不要简单理解成：

> `spawn 一个 thread`

真正应该做的是 **Task System**。

例如：

```text
ExecutionTask
├── id
├── node_id
├── state
│   ├── queued
│   ├── running
│   ├── completed
│   ├── failed
│   └── cancelled
├── progress
├── cancellation_token
└── result
```

前端：

```text
Run Node
   ↓
invoke start_execution
   ↓
立即返回 task_id
   ↓
Rust 后台执行
   ↓
event:
execution:started
execution:progress
execution:completed
execution:failed
```

而不是：

```text
React
 ↓
invoke()
 ↓
等 20 秒
 ↓
Result
```

这对：

```text
VAR
VECM
Bayes
大数据聚合
数据库 import
未来 AI
```

全部有用。

以后还可以自然支持：

```text
Cancel
Retry
Pause
Parallel
Queue
Execution history
```

---

# P1：你现在的前端目录有一点“重复架构”

目前前端同时存在：

```text
src/
├── app
├── components
├── features
├── lib
├── services
├── shared
├── utils
├── views
```

而 `features` 内部又已经定义：

```text
core
domain
application
```

并且明确规定依赖关系。([GitHub][4])

这个思想本身很好。

问题是：

```text
services/
features/
views/
components/
shared/
lib/
utils/
```

长期非常容易产生归属不明确：

> “这个函数到底放 services、shared、utils 还是 feature？”

你现在 `services` 已经包含：

```text
bayes
clipboard
database
graph
ipc
julia
log
nodeSystem
project
result
stats
variable
window
worksheet
```

([GitHub][6])

这里已经有一点明显的“横向 service 大目录”趋势。

我更推荐：

```text
src/
├── app/
│
├── features/
│   ├── graph/
│   ├── project/
│   ├── dataframe/
│   ├── statistics/
│   ├── worksheet/
│   ├── visualization/
│   └── workbench/
│
├── platform/
│   ├── tauri/
│   ├── ipc/
│   └── window/
│
└── shared/
    ├── ui/
    ├── hooks/
    ├── types/
    └── utils/
```

然后例如：

```text
features/project
├── api
├── model
├── store
├── ui
└── lib
```

这样：

```text
ProjectService
ProjectStore
ProjectView
ProjectDTO
```

全部围绕 `project` 放置。

这比现在：

```text
services/project
features/...
views/...
```

更适合一个越来越大的应用。

---

# P1：你现在正在做的 AppError 非常值得完成

TODO 里这一条，我非常赞同：

> 绝大多数 `#[tauri::command]` 仍然是 `Result<_, String>`，准备统一成结构化 `AppError`。([GitHub][5])

应该尽快做完。

不要：

```rust
Result<T, String>
```

而应该：

```rust
struct AppError {
    code: ErrorCode,
    message: String,
    details: Option<Value>,
}
```

例如：

```json
{
  "code": "DATAFRAME_COLUMN_NOT_FOUND",
  "message": "Column `age` does not exist",
  "details": {
    "column": "age"
  }
}
```

前端：

```ts
switch (error.code) {
  case "PROJECT_NOT_FOUND":
  case "NODE_EXECUTION_FAILED":
  case "DATABASE_CONNECTION_FAILED":
}
```

这件事情收益非常高。

因为以后 AI Agent 调用 YssBI 工具时，也可以直接理解：

```text
code
message
details
```

而不是解析字符串。

---

# P1：DTO 自动生成，我建议直接做

TODO 里你也已经意识到了：

> Rust DTO 和 TypeScript 手写 types.ts 容易漂移，考虑 typeshare / ts-rs。([GitHub][5])

我的答案是：

**做。**

你这个项目非常适合。

因为数据类型本身已经很多：

```text
GraphInstanceDTO
DatabaseDecl
DatabaseEngine
DataType
Pin
Node
Variable
ExecutionResult
RegressionResult
```

手动：

```text
Rust struct
+
TypeScript interface
```

迟早出错。

可以变成：

```rust
#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RegressionResult {
    ...
}
```

然后：

```text
cargo xtask bindings
```

自动产生：

```text
src/generated/api/
```

甚至再进一步：

```text
Rust Command
       ↓
generated TS binding
       ↓
typed invoke()
```

最终让：

```ts
invoke<any>()
```

基本消失。

---

# P1：数据库层需要选一个明确主战略

你现在同时有：

```text
Polars
DuckDB
SQLx
Arrow
Excel
```

Cargo 已经明确体现出来。([GitHub][3])

这不是坏事。

但必须定义谁干什么。

我推荐非常明确地规定：

```text
DuckDB
= 项目内持久化 + SQL analytics

Polars
= DataFrame / LazyFrame transformation

Arrow
= 数据交换格式

SQLx
= 外部数据库连接

yss-sci / Julia
= Statistics
```

不要让：

```text
DuckDB
Polars
Julia DataFrame
Rust Vec
JSON
```

互相随意转换。

理想数据通道应该是：

```text
External DB
    ↓
Arrow
    ↓
Polars / DuckDB
    ↓
Arrow
    ↓
Statistics
```

也就是把 **Arrow 当成 YssBI 的数据 ABI**。

这会让你以后接：

```text
Julia
Python
GPU
Remote Executor
```

都简单很多。

---

# P1：不要把 DataFrame 本体放 Zustand

你已经使用 Zustand。package.json 里目前是 Zustand 5。([GitHub][7])

Zustand 很适合：

```text
activeProjectId
selectedNodeId
openedTabs
layout
theme
selection
panel state
```

但是不应该存：

```text
1,000,000 rows DataFrame
```

或者：

```text
大型 RegressionResult 原始数据
```

应该遵循：

```text
Frontend = references
Backend = actual data
```

例如：

```ts
{
  dataframeId: "df_123",
  rowCount: 12_000_000,
  schema: [...]
}
```

真正的数据：

```text
Rust / DuckDB / Polars
```

前端 DataGrid 请求：

```text
rows 1000..1100
```

你 README 已经明确说 DataView 面向大数据量优化并用了虚拟化，这个方向是正确的。([GitHub][1])

---

# P1：测试现在有点过度“Architecture Contract 化”

我注意到已经存在：

```text
architecture.test.ts
observabilityArchitectureContract.test.ts
userFeedbackArchitectureContract.test.ts
```

同时 Rust commands 目录里面还有：

```text
command_*_tests.rs
command_blueprint_graph_phase1_tests.rs
command_node_system_reroute_tests.rs
...
```

([GitHub][8])

这里我建议稍微控制。

特别是 AI 辅助开发很容易生成：

```text
ArchitectureContractTest
RegressionContractTest
ModuleBoundaryTest
...
```

最后导致：

> 改文件路径 → 一堆测试挂了

而不是：

> 行为错了 → 测试挂了

我建议测试比例更接近：

```text
60% domain / calculation tests
25% integration tests
10% regression tests
5% architecture tests
```

对于统计软件尤其应该重视：

```text
Golden Tests
```

例如：

```text
OLS:
YssBI
vs
Stata
vs
R
```

验证：

```text
coef
std error
t
p
R²
F
CI
```

而不是大量测试：

```text
某文件必须 import 某路径
```

你已经计划 Rust/Julia golden fixture，这其实是非常好的方向。([GitHub][4])

---

# P1：CI 需要比现在再强一点

当前 `.github/workflows` 里只有：

```text
publish.yml
```

([GitHub][9])

而 package.json 已经有很完整的：

```text
verify
verify:frontend
verify:rust
verify:full
```

([GitHub][7])

所以非常适合增加：

```text
ci.yml
```

PR 至少跑：

```text
pnpm install --frozen-lockfile
pnpm typecheck
pnpm test

cargo fmt --check
cargo check
cargo clippy
cargo test -p yss-sci
```

其中建议加上的关键一项是：

```text
cargo clippy
```

目前你的 `verify:rust` 只有：

```text
fmt
check
```

没有 clippy。([GitHub][7])

---

# P2：Cargo 编译时间可以进一步优化

你后端现在依赖很重：

```text
Polars
DuckDB bundled
SQLx
ndarray
faer
statrs
Tauri
```

([GitHub][3])

这个组合在 Windows 上编译会非常重。

而你现在：

```text
cargo test --jobs 1
```

已经说明你可能是在主动控制内存。([GitHub][7])

建议逐渐拆 workspace：

```text
crates/
├── yss-core
├── yss-graph
├── yss-execution
├── yss-data
├── yss-sci
└── yss-app
```

不一定现在一次性拆完。

第一步我只建议：

```text
yss-sci
yss-data
yss-execution
```

三个真正稳定的边界。

好处是：

```text
改 React/Tauri
不会重新编整个 science stack

改 OLS
不用影响 graph

改数据库
不用重新编统计模型
```

---

# P2：README 对外展示还有非常大的提升空间

这个项目技术内容其实比 README 表现出来的成熟很多。

目前 README 相对短，而且还有：

```text
数据采集？
报告输出？
```

这样的问号。([GitHub][1])

如果准备开放给别人用，我建议 README 首页直接变成：

```text
YssBI
Visual Statistical Computing Environment

[Screenshot]

Why YssBI
• Visual workflow
• Econometrics
• Large datasets
• Native desktop
• Reproducible analysis

Architecture

Node → Graph → Execution Engine → Data/Compute Backend

Features

Screenshots

Roadmap

Contributing
```

尤其是你已经有非常多：

```text
OLS
WLS
GLS
2SLS
LIML
FE
RE
VAR
VEC
DID
Diagnostics
```

([GitHub][1])

对一个 2-star 项目来说，**代码能力明显比项目包装强。**

---

# 我认为目前架构最大的潜在风险

不是性能。

也不是 React。

也不是 Rust。

而是：

```text
Node System
Graph
Execution
Data
Statistics
Project
UI State
```

逐渐互相知道太多东西。

你需要强制变成：

```text
                  ┌──────────────┐
                  │      UI      │
                  └──────┬───────┘
                         │
                  Application
                         │
           ┌─────────────┴────────────┐
           │                          │
        Graph                     Project
           │
           ▼
    Execution Engine
           │
      ┌────┼────┬─────────┐
      ▼    ▼    ▼         ▼
   Polars DuckDB RustSci Julia
```

其中非常关键：

```text
Graph 不知道 UI
Graph 不知道 Tauri
Sci 不知道 Graph
Sci 不知道 Tauri
DuckDB 不知道 React
Execution 不知道具体 UI
```

这就是我认为 YssBI 接下来最值得守住的东西。

---

## 如果让我给当前项目打一个“下一阶段优先级”

| 优先级    | 工作                            | 收益    |
| ------ | ----------------------------- | ----- |
| **P0** | Execution Task System         | ★★★★★ |
| **P0** | 锁定 Graph / Execution / Sci 边界 | ★★★★★ |
| **P0** | 长计算后台化 + cancel/progress      | ★★★★★ |
| **P1** | 统一 AppError                   | ★★★★★ |
| **P1** | Rust → TS DTO 自动生成            | ★★★★★ |
| **P1** | Arrow 作为数据交换 ABI              | ★★★★★ |
| **P1** | 收敛前端 features/services 结构     | ★★★★☆ |
| **P1** | Golden statistical tests      | ★★★★★ |
| **P1** | CI + clippy                   | ★★★★☆ |
| **P2** | workspace 进一步拆 crate          | ★★★☆☆ |
| **P2** | README / docs 重构              | ★★★★☆ |
| **P3** | 再增加新的统计模型                     | ★★☆☆☆ |

所以我反而建议你 **暂时少加几个统计节点**。

你现在已经有足够多功能证明项目方向了。([GitHub][1])

接下来应该花一轮开发周期，把：

**Graph → ExecutionPlan → Task → Backend → Result**

这一条链做成整个 YssBI 的“脊柱”。

如果这一层设计得好，之后无论你加 **LLM Agent、Julia、Python、GPU、远程计算、多核并行**，都只是增加新的 Backend，而不用重新推翻 YssBI。

[1]: https://github.com/zhouyi207/YssBI "GitHub - zhouyi207/YssBI: YssBI 是一个基于 Blueprint 的桌面数据分析与可视化应用 · GitHub"
[2]: https://github.com/zhouyi207/YssBI/tree/shadcn/src-tauri/src "YssBI/src-tauri/src at shadcn · zhouyi207/YssBI · GitHub"
[3]: https://github.com/zhouyi207/YssBI/blob/shadcn/src-tauri/Cargo.toml "YssBI/src-tauri/Cargo.toml at shadcn · zhouyi207/YssBI · GitHub"
[4]: https://github.com/zhouyi207/YssBI/tree/shadcn/src "YssBI/src at shadcn · zhouyi207/YssBI · GitHub"
[5]: https://github.com/zhouyi207/YssBI/blob/shadcn/TODO.md "YssBI/TODO.md at shadcn · zhouyi207/YssBI · GitHub"
[6]: https://github.com/zhouyi207/YssBI/tree/shadcn/src/services "YssBI/src/services at shadcn · zhouyi207/YssBI · GitHub"
[7]: https://github.com/zhouyi207/YssBI/blob/shadcn/package.json "YssBI/package.json at shadcn · zhouyi207/YssBI · GitHub"
[8]: https://github.com/zhouyi207/YssBI/tree/shadcn/src/features "YssBI/src/features at shadcn · zhouyi207/YssBI · GitHub"
[9]: https://github.com/zhouyi207/YssBI/tree/shadcn/.github/workflows "YssBI/.github/workflows at shadcn · zhouyi207/YssBI · GitHub"


这种代码是不是没有什么必要


+pub fn canonical_port_title(key: &str) -> Box<str> {
    132 +    let title = match key {
    133 +        "value" => "Value",
    134 +        "left" => "Left",
    135 +        "right" => "Right",
    136 +        "result" => "Result",
    137 +        "input" => "Input",
    138 +        "output" => "Output",
    139 +        "enter" => "Enter",
    140 +        "then" => "Then",
    141 +        "true" => "True",
    142 +        "false" => "False",
    143 +        "condition" => "Condition",
    144 +        "operands" => "Operands",
    145 +        "source" => "Source",
    146 +        "dataframe" => "DataFrame",
    147 +        "series" => "Data Series",
    148 +        "values" => "Values",
    149 +        "samples" => "Samples",
    150 +        "sample_count" => "Sample Count",
    151 +        "maximum_lag" => "Maximum Lag",
    152 +        "standard_deviation" => "Standard Deviation",
    153 +        "lower_bound" => "Lower Bound",
    154 +        "upper_bound" => "Upper Bound",
    155 +        "then_source" => "Then Source",
    156 +        "else_source" => "Else Source",
    157 +        "initial_source" => "Initial Source",
    158 +        "next_source" => "Next Source",
    169 +    };
    170 +    title.into()
    171 +}

## 2026.08.26

- [ ] 在隔离 worktree 中为 Rust architecture audit 增加 Cargo production root discovery，覆盖 library、binary、example 与 custom-build。
- [ ] 保留 workspace member crate alias、runtime/build/development dependency scope 与 target condition，禁止 workspace 依赖降级为 external fallback。
- [ ] 为 metadata fixture 与真实 workspace discovery 增加 focused Rust regression tests，并保持现有 Project/Application 依赖审计通过。
- [ ] 将 Rust production source traversal 拆出 raw dependency facts，覆盖 use、pub use、路径、宏、include、#[path]、inline 与 cfg 可达性。
- [ ] 为 raw facts 记录 owning package、repository-relative source file、fully-qualified owner、Runtime/Build mode 与稳定 source location，并对动态 include fail-closed。
- [ ] 增加 workspace-first canonical origin resolver，保留 language builtin、external declaration、workspace member re-export chain 与 development-only/unknown alias 的 typed failures。
- [ ] 运行 architecture_tests 全组回归，确认新的 discovery/resolver 不改变既有 Project→Application 依赖审计。
- [ ] 将 repository dependency resolver 补齐 lexical/private import 与 exported facade 的可见性区分，并忽略 lint-level attribute token 中的伪依赖。
- [ ] 为 15 个 Rust production layers 建立 total/exclusive classifier、单向 layer matrix 与 exact file/owner/symbol capability 校验。
- [ ] 建立 Cargo external declaration/use policy，逐 scope、target condition、source layer 与 package fail-closed 审计 production 依赖。
- [ ] 用 rule、file、owner、dependency kind、canonical target 和 occurrence count 冻结双向 exact architecture debt，新增与过期条目均使门禁失败。
- [ ] 按 Backend Adapter、Project–Graph、Execution Runtime、Presentation–Command 四个迁移 owner 拆分 debt manifests，避免单一巨型清单混合职责。
- [ ] 将四份边界目标迁入 docs/architecture 长期文档，移除 architecture audit 对 docs/superpowers 草稿的引用，并验证 owning spec 文件真实存在。
- [ ] 增加并运行 production root、raw fact、canonical resolver、layer/capability、external policy、exact debt 与真实仓库 focused Rust regressions。
- [ ] 运行 rustfmt、architecture_tests、rust:check、verify:rust 与 git diff --check，记录隔离 worktree 的最终验证结果。
- [ ] 将 strict Rust architecture gate rebase 到最新 `shadcn` 已提交基线，同时保留主工作区全部未提交改动。
- [ ] 将新增 `trash` runtime dependency 纳入 external declaration 审计，并把 Project 直接调用 `trash::delete` 记录为 Backend Adapter exact debt，而不扩大允许层。
- [ ] 在同步基线上重新运行 15 项 architecture focused tests、rustfmt 与 `cargo check`，确认 exact debt 双向清单和 production policy 一致。
- [ ] 建立完整 frontend production TypeScript inventory，统一排除测试、fixture、声明与 generated source，并覆盖所有生产目录。
- [ ] 增加 canonical module dependency resolver，区分 runtime/type-only、repository/external/stylesheet origin，并保留 declaration symbol identity。
- [ ] 增加递归 stylesheet dependency lexer 与 typed failure，覆盖 package、repository asset、url、parse failure、missing target 和 cycle。
- [ ] 抽取 raw Tauri invoke semantic audit helper，并让 project filesystem contract 复用 canonical production inventory。
- [ ] 运行 frontend architecture focused tests、project filesystem contract、typecheck 与 diff check，记录参数转发导致伪 focused 执行的诊断结论。
- [ ] 修复外部 stylesheet package 未解析时的 fail-closed typed error，并为 runtime/type-only 空 named import/export 保留 declaration-level dependency facts。
- [ ] 让 quoted CSS 反斜杠目标保留原始 payload 并产出 unsupported typed error，逐个坏样式输入断言 exact error 且零 dependency。
- [ ] 导出限制在真实 repository `src` 根内的只读 filesystem reader，拒绝 absolute、non-src、parent traversal 与 realpath escape。
- [ ] 通过真实 `App.css` 与 `workbench-dockview.css` 构建 stylesheet graph，并回归验证 Task 4 architecture、project filesystem contract 与 TypeScript typecheck。
- [ ] 将 `../parent.css` 纳入坏 stylesheet 逐输入 exact-payload 表，冻结 unsupported error 且零 dependency 的既有行为。
- [ ] 将 encoded package separator `react/%2fsecret` 纳入同一 table-driven contract，并记录本轮为 reviewer-requested assertion completeness、无伪造 RED。
- [ ] 将 TypeScript declaration 路径解析绑定到实际 tsconfig 项目根，仅显式映射隔离 `run-N/src/**`，拒绝仓库外非 node_modules 声明伪装成内部层。
- [ ] 在 canonical module resolver 回归中覆盖含 `/src/` 的 sibling declaration，冻结 `unresolved-module-dependency` typed failure。
- [ ] 在逐输入 stylesheet 表中加入真实存在目标的 `./../parent.css`，规范化前拒绝任意相对父段并保持 exact error 与零 dependency。
- [ ] 为 TypeScript audit context 记录当前精确 source root，使 production 与每次 isolated run 只从各自根目录 canonicalize `src/**`。
- [ ] 删除 declaration 与 production inventory 对任意 `run-N/src/**` 的名称猜测，并回归拒绝伪造 source root 外的 sibling 与顶层 run 目录。
- [ ] 让 collector 无条件保留已识别的 type-only/runtime dependency，统一由 resolver 对非字面量 import type 与缺参动态 import 返回 typed failure。
- [ ] 复验 Task 4 全部 architecture contracts、project filesystem contract、TypeScript typecheck 与 diff check，并继续隔离 Task 5 classifier RED 草稿。
- [ ] 为嵌套 ImportTypeNode 与 dynamic import options 增加 focused regression，逐项冻结 outer/inner dependency 的 syntax kind 与 canonical origin。
- [ ] 已识别 import type 与 module call 在记录当前 edge 后继续遍历子节点，同时保留 import/export/import-equals 的声明级去重语义。
- [ ] 使用临时未跟踪 Task 5 stub 复验 focused、Task 4 architecture、project filesystem contract 与 TypeScript typecheck，并在 staging 前删除 stub。
- [ ] 扩展 nested dependency regression，冻结 `export default import()` 外层 export-assignment 与 options 内层 dynamic-import 的逐项 canonical origin。
- [ ] 让 ExportAssignment 记录外层 module edge 后遍历 call children，同时避免重访 call 自身导致外层 dependency 重复。
- [ ] 复验 round 6 focused、Task 4 全组、project filesystem contract、带临时 Task 5 stub 的 typecheck 与 diff check，并继续隔离 classifier RED 草稿。
- [ ] 为十层 frontend production classifier 建立闭合集合审计，返回分类结果与结构化 zero/multiple membership 错误。
- [ ] 将 shared stateful、platform 与 presentation exceptions 作为 literal membership 从 base owner 集合移除后再校验 union/intersection。
- [ ] 冻结 frontend 单向 layer matrix、canonical Core capability 与现有 `WorkbenchDockviewPort` 的 read-member manifest，不新增 production alias。

## 2026.08.27

- [ ] 建立 frontend external declaration policy，双向固定 32 个 runtime dependencies 与唯一 build-only `tailwindcss` scope。
- [ ] 按 source layer、runtime/type-only/build-style mode、resource kind、canonical subpath 与 stylesheet consumer 审计 exact package rows。
- [ ] 让 repository asset audit 直接消费 Task 4 `ResolvedStylesheetGraph`，保留 resolver errors 且仅从 exact authorized parent edge 继承 stylesheet layer。
- [ ] 固定两个 production repository stylesheet consumer/path rows，并将 dev-only、unknown、invalid policy 与 asset resolution failures 排除在 debt 之外。
- [ ] 以 rule、source、owner、dependency kind、canonical origin 五字段建立 frontend debt identity，明确排除 line/column。
- [ ] 双向比较 actual 与 declared exact occurrences，让新增/增加和过期/减少均使 ratchet 失败。
- [ ] 对 duplicate debt key、零或非法 count、未批准 maintained migration spec 返回 typed declaration errors，并保持 import-type 与 dynamic-import 独立。
- [ ] 用单一 production audit pipeline 复用 Task 4 inventory、canonical module resolver 与完整 stylesheet graph，并分别报告 fatal error families。
- [ ] 为 Application→Wire validated result/type declarations、View→DnD exact symbols 与现有 WorkbenchDockviewPort read capability 建立 literal manifests。
- [ ] 审核 934 个 production sources 与 7110 个 dependency facts，将 610 个唯一 frontend debt keys、624 次 occurrences 固化为静态 exact manifest。
- [ ] 新增 maintained `FRONTEND_APPLICATION_BOUNDARIES.md` 记录最终 owner、单向依赖与原子 debt removal 判据，不引用 docs/superpowers。
- [ ] 收紧 external policy row validation，拒绝 duplicate subpath、unsupported mode/resource、prototype-inherited package names 与不存在的 build-style consumer。
- [ ] 分离 TypeScript runtime asset consumer 与 stylesheet build consumer，并回归 stylesheet layer conflict 不产生确定 layer。
- [ ] 将本地 named re-export 到 exact bare package 的 AST 可证明链纳入现有 canonical resolver，其他 node_modules origin 继续 typed fail-closed。
- [ ] 补齐 repository dependency target classifier closure，并将 WorkbenchDockviewRead 作为 App/View 指向现有 WorkbenchDockviewPort 的 policy-only manifest。
- [ ] 将 frontend classifier 改为十个独立 base membership sets，以 base-overlap regression 冻结 zero/multiple 检测不依赖 rule ordering。
- [ ] 在合并 literal policy membership 前从每个 base set 移除 overrides，显式排除 pure-shared 的其他 shared owners，并保持现有 exact capability/debt contracts。
- [ ] 先对 stylesheet 与 source layer provenance 求稳定 fixed point，再使冲突 parent 的 descendants 失效，最后仅用 singleton layers 生成 asset findings。
- [ ] 将 base-overlap fixture 改为注入两条同时命中的 FrontendBaseRule predicates，确保回退 first-match 会产生 focused failure。
- [ ] 让 production 与 classifier fixture 共用同一十层 rule-list builder，对每条命中独立 add 后再统一检查 zero/multiple membership。
- [ ] 删除预制 base membership sets 与 injectedBaseSets bypass，继续先从全部 base sets 移除 literal overrides 并保留 pure-shared 显式排除。
- [ ] 新增 compiler-backed frontend semantic audit，以 resolved import symbols、call/property access 与 canonical origins 守卫 raw invoke/dialog、View/Core capability、projection write、Application raw wire 和 Dockview constructor。
- [ ] 用唯一 table-driven fixture 冻结九个 stable semantic rule IDs，并覆盖 canonical IPC adapter、path-dialog service、approved read member 与 root/Logs Dockview exact paths。
- [ ] 将 236 个 reviewed semantic exact keys 合并进既有五字段 frontend debt manifest，与原有 610 个 dependency keys 通过同一 comparator 双向匹配。
- [ ] 在 unified production architecture assertion 通过后退役重复 regex/architecture blocks，同时保留 project command identity、stale-result、editor projection behavior 与 node/observability/user-feedback contracts。
- [ ] 补齐 policy-approved read interface 的 typed receiver 追踪，通过 checker type 与 canonical declaration symbol 拒绝局部参数调用未批准 authority member。
- [ ] 让 raw invoke call audit 识别经 repository barrel 解析到 `@tauri-apps/api/core::invoke` 的 canonical symbol，并与直接 binding helper 按完整 occurrence identity 去重。
- [ ] 支持 namespace `DockviewReact` JSX constructor，移除 semantic audit 的生产层过滤，并使 zero/multiple frontend classification errors 在扫描前 fail-closed。
- [ ] 将 persisted `DataType`、`DataValue`、`DataSeriesValue` 与分类/时间序列 metadata 原子迁入顶层 Pure Leaf `data_contract`，保持现有 serde tags、camelCase 字段和 DataSeries string/full wire。
- [ ] 将类型兼容、继承、默认值、转换、查询与运行时值算术保留在 Graph `value::type_system`，以原生数值逻辑替换旧 `num_traits` One/Zero 使用。
- [ ] 全量切换 Variable、Project、Schema、Commands、Graph、Node System、Runtime、Database、Tabular、SCI 与对应测试到 canonical `data_contract` 路径，并删除 Graph 旧声明与 re-export。
- [ ] 为 typed `DataTypeParseError`、persisted value wire、单一 Pure Leaf owner 与 Graph 无 compatibility re-export 增加 focused regression。
- [ ] 将 `data_contract` 三个 exact files 设为 literal Pure Leaf、将 Graph type-system behavior 设为 exact Graph classification，移除 contract namespace fallback，并精确删除已消失的三条 `num_traits` architecture debt occurrence。

## 2026.08.27

- [ ] 将 persisted value canonical-owner architecture guard 改为 typed required/allowed exact-origin 策略，保持五个 contract symbol 的单一 data-contract owner。
- [ ] 允许独立 SCI `CategoricalRole` 仅来自 `src-tauri/src/sci/api/computation.rs`，同时拒绝任意 SCI owner 与 SCI 对 persisted role 的 re-export/alias。
- [ ] 增加 focused regression，覆盖批准的双 owner 集合与未批准来源拒绝路径，并运行 Rust architecture、格式、编译及 diff 校验。
- [ ] 为 canonical-owner production guard 增加基于 `syn::Item::Type` 的窄扫描，拒绝 Graph 对六类 persisted data-contract symbol 的 type alias。
- [ ] 拒绝 SCI 对 persisted `CategoricalRole` 的 type alias，同时保留 `sci/api/computation.rs` 独立声明作为唯一批准 SCI owner。
- [ ] 增加真实 Rust source fixture 回归，覆盖 Graph 六类 alias、SCI alias、独立 SCI enum 与 test-only alias 排除，并执行 focused RED/GREEN 验证。
- [ ] 将 SCI 统计设置、分类角色、统计标量输入与九字段 observation metadata 原子迁入 `sci/api/computation.rs`，保持既有 metadata 字段名及 `project`/`node` 序列化值。
- [ ] 新增 Application-owned Project→SCI/Execution settings 与 persisted value/role→SCI input 穷尽映射，使用 closed typed errors 拒绝非有限数值和不支持的持久化值。
- [ ] 建立 production-unreachable 的独立 `execution/settings.rs` contract，并保持现有 Project run-parameter 与 node runtime settings 路径不变，留待 Execution Task 8 切换。
- [ ] 将 public SCI statistics、ACF/PACF、serial tests 与 hypothesis adapter 的字符串错误迁为 operation/violation typed `SciError`，禁止 raw algorithm text 决定稳定错误码。
- [ ] 为 Execution owner 增加 fail-closed layer classification，精确删除 SCI→Project missing-value debt并迁移其余 canonical debt keys，新增 SCI 隔离与 canonical-owner production guard。
- [ ] 按 TDD 运行 settings、statistical input、metadata wire 与 typed SCI error 的 RED/GREEN focused tests，并执行 Rust 编译、架构 policy、格式及 diff 验证。
- [ ] 为 T/Wald SCI adapter 增加显式的约束数量、自由度、矩阵维度与有限数值前置验证，分别映射 closed parameter、shape 与 non-finite violations。
- [ ] 将通过结构验证后的下游 T/Wald 数值失败统一映射为 operation-specific `ComputationFailed`，禁止解析 `yss_sci` 原始字符串选择公开错误语义。
- [ ] 增加窄 focused regressions，分别覆盖 typed input validation 与合法形状下零/奇异协方差的 computation-failure 映射。
- [ ] 新增 Frontend 与 Rust focused architecture scripts，并将 Rust architecture suite 纳入日常 `verify:rust`，保持 frontend/full Rust 验证无重复执行。
- [ ] 更新工程规范与本地工作流，明确 focused architecture 命令、daily/full 验证边界及 `package.json` 的 exact command composition。
- [ ] 在当前架构文档记录 Cargo/Frontend production-root discovery、15/10 层 total-exclusive classification 与 canonical dependency origins。
- [ ] 记录 exact Cargo/package declaration-use policy、双向 occurrence debt ratchet 与两端 semantic fitness checks，不复制具体 debt entries。
- [ ] 完成 focused scripts 的缺入口 RED 与 GREEN 验证，Frontend 66 项、Rust architecture 20 项测试通过。
- [ ] 补全当前架构文档中的 Rust canonical origin 列表，明确 repository asset 是独立于 repository declaration、language builtin 与 external dependency 的分支。
- [ ] 对照 `CanonicalOrigin::RepositoryAsset` 与 Include/Attribute resolver，记录 exact `repository-asset:<repository-relative-path>` target 规范。
- [ ] 将 serialized GraphDocument、GraphResourcePath、GraphRevision 与 document identities 原子迁入 Pure Leaf graph_document，删除 node_system/project 旧声明及 re-export。
- [ ] 将 OperationId、HistoryEntryId、ResourceRevision、ProjectRevision 与 ProjectTransactionRevision 拆为 Project-owned 独立 newtype，移除 ResourceRevision 到 GraphRevision 的 ownership alias。
- [ ] 建立 Graph-owned schema、compile settings、immutable resource catalog 与 21 项 closed mutation/compile typed error contract，并保留 TypedValue untagged JSON wire。
- [ ] 增加 resource catalog、graph-document wire、Pure Leaf JSON purpose 与 Project→Graph production edge focused guards，按 canonical origins 精确更新 architecture debt。
- [ ] 修正 GraphResourcePath 测试 fixture 的 canonical `.yssbi-event`/`.yssbi-function` 扩展名及关联 lookup/JSON 断言，不放宽 opaque path validator。
- [ ] 将 NFC、Unicode L/N、保留名、空格与长度规则下沉到 Pure Leaf graph-document name contract，并由 Project ResourceName 穷尽映射既有错误语义。
- [ ] 删除 GraphRevision 与 Project ResourceRevision 的 From/跨类型 PartialEq 隐式桥，将 mutation constructor 和全部 caller 改为显式 named conversion。
- [ ] 将 Pure Leaf serde_json guard 改为基于真实 production module/dependency facts 的 typed structured 审计，并覆盖 test module 后的 production source negative fixture。
- [ ] 将 legacy tabular snapshot 的三项既有 serde_json finding 静态归入 Backend Task 5 双向 exact debt，不提前重分类或实现 mixed-owner 拆分。
- [ ] 新增 SCI-owned cancellation source/token、显式 monotonic absolute deadline、run control 与独立 cancel-delivery control，禁止 wall clock、sentinel、global token 和 hidden default。
- [ ] 定义 validated opaque Bayes task/artifact IDs、非零 generation task handle 与 task-bound artifact handle，并用 closed typed errors 拒绝空值、超长、分隔符、NUL 和保留序列。
- [ ] 建立 `ValidatedBayesTask::try_new` 唯一构造路径，在 neutral `StatisticalInput` 上重验 model、binding、sampler、响应表达式和 indexed input invariants。
- [ ] 定义 production-unreachable `BayesWorkerPort`、typed terminal/phase/cancel errors、full-handle validated task result 与 immutable artifact bytes，保持旧 BayesBackend/Application/Julia production route 原样唯一。
- [ ] 增加 barrier/channel-free-sleep bounded worker fake 与 semantic authority guard，覆盖 cancel/completion linearization、retry、generation ownership、artifact deadline、private constructors/fields 及 broad import 拒绝。
- [ ] 将 Bayes summaries、diagnostics 与 warning DTO 迁入单一 neutral contract owner，并以 full-handle `BayesInferenceSnapshot` 替换 worker result 对旧 `InferenceResult` 的依赖。
- [ ] 收紧旧 `InferenceResult`、artifact manifest 与 artifact path fields 为 private getters，保持现有 serde wire、Julia owner 生命周期和唯一 production route 行为不变。
- [ ] 将 `BayesModelSpec` 八个 fields 全部私有化，仅公开 predictor、likelihood、parameters、data variables 与 sampler 五项 final-adapter capability。
- [ ] 将旧 dataset/response/display 访问限制为 crate-private canonical getters，并将测试 fixture 改走现有 serde wire，禁止 setter、compatibility view 与 old/new converter。
- [ ] 将 Bayes authority guard 改为 exact owner+method allowlist，覆盖 function-item alias、wrong-owner 同名函数、伪造 associated constructor 与 neutral result path/source 泄漏。
- [ ] 扩展 Bayes authority semantic guard，为 worker module 的显式 import rename、glob import 与多级 type alias 建立 per-source canonical owner map。
- [ ] 在扫描每个 production `ItemImpl` 时保存 canonical impl owner，将 `Self::...` authority reference 解析回真实 worker owner，并拒绝外部 inherent builder 声明。
- [ ] 增加独立恶意 fixtures，分别覆盖 `Handle`/type-alias function reference 与外部 `impl BayesTaskHandle` public forge/`Self::issue_for_worker` 绕过。
- [ ] 将 Bayes authority resolver 从 file-global alias map 改为递归 module-scoped symbol tables，对每个 inline scope 独立执行 use/type alias fixed-point。
- [ ] 规范化 `crate`、`self`、多级 `super` 与 relative/module-alias 路径，同时保持 `other::Handle` 等非 worker origin 不产生误报。
- [ ] 将所有显式 associated-function visibility 纳入 exact allowance，仅允许 worker boundary 的 public `ValidatedBayesTask::try_new` 与 exact `pub(crate)` authority builders。
- [ ] 增加 module alias、relative worker import、nested forward alias chain、`pub(super)`、`pub(in ...)` 与 syntactic `pub(in crate)` 恶意回归 fixtures。
- [ ] 修正 grouped use tree 的 terminal `self` 语义，使 `{self}` 与 `{self as alias}` 保留当前 module prefix 而不生成伪 `worker::self` origin。
- [ ] 增加 `worker::{self as w}` → `w::BayesTaskHandle as Handle` → authority call 的 focused semantic regression，锁住 module self-alias canonicalization。
- [ ] 新增 production-unreachable `JuliaBayesWorkerAdapter`，仅实现 final SCI `BayesWorkerPort`，constructor 只接收 app-data directory 与 `JuliaWorkerManager`，保持旧 Bayes route 唯一。
- [ ] 直接从 `ValidatedBayesTask`、五项 model projection 与 neutral inference DTO 生成 Julia task/source/result，使用 full task handle 封存 task directory 与 artifact ownership，不引入旧新转换。
- [ ] 为 accepted/cancel/deadline/stale/ownership/unknown-extension 与 JSON/CSV/PNG/Binary artifact mapping 增加 barrier/fake-runtime focused regressions，并保留原 Julia cancellation characterization owner。
- [ ] 将三个 final Julia adapter files 精确分类为 Backend Adapter，登记 literal SCI capability manifest，并将零 production caller activation debt 明确归属 Execution Task 8。
- [ ] 缩短 Julia worker cancel/restart 的 active-task mutex scope，以 barrier regression 证明 send/terminate 边界不持有该状态锁，同时保留原 non-active-task characterization。
- [ ] 新增 Execution-owned scientific request/result/error/control contracts 与单一 dynamic `ScientificBackend`，以三个 typed methods 固定 statistics、KDE、ACF/PACF 结果族。
- [ ] 新增 production-unreachable `SciApiScientificBackend`，穷尽映射 Execution settings、control、operations、results 与 closed SCI errors，不引入旧新 converter 或第二条 production route。
- [ ] 将 context-free KDE canonical owner 迁入 `sci/api/density.rs`，删除旧 `sci/kde.rs` 并原子更新 Application Bayes 与 Plot callers，保持数值算法和输出形状。
- [ ] 从 public ACF/PACF API 移除未使用的 `SciContext`，同步 command 与 Plot direct-SCI callers，保留当前唯一 production 行为。
- [ ] 为 final scientific port/adapter 增加 RED/GREEN fake fixtures、exact capability/debt 与 zero-production-caller semantic guard，并将 staged activation debt 保留给 Execution Task 8。
- [ ] 修复 ACF/PACF golden integration target 对已移除 `SciContext` 参数的遗留引用，改为调用唯一的单参数 SCI API。
- [ ] 将 scientific port fake 收紧为 recording fixture，逐方法验证 typed request/result 与 cancellation/deadline control 透传。
- [ ] 明确 synchronous scientific adapter 的 control 仅为 admission preflight，保留 Task 8 的真实 cooperative checkpoint activation debt。
- [ ] 将 DatabaseDecl、DatabaseEngine 与 DatabaseEngineSql 原子迁入顶层 database_contract Pure Leaf owner，保持既有 serde wire、InMemory 与 DuckDB table 语义。
- [ ] 全量切换 Application、Project、Schema、Commands、Database runtime 与测试 caller 到 crate::database_contract，删除 database 旧 declaration module/re-export。
- [ ] 移除已消失 database declaration origins 对应的 exact capability/debt 条目，保留 DatabaseInstance、DuckDB storage 与 schema conversion 的现有职责和债务。
- [ ] 增加 database contract 单一 owner 与 wire focused regressions，并完成 Rust architecture、数据库测试编译、fmt、check 与 diff 校验。
- [ ] 将 `TabularSnapshot` 的持久化 JSON 与 shape 事实迁入 `tabular/contract.rs` Pure Leaf，删除混合 snapshot owner。
- [ ] 将 snapshot→Polars materialization、dtype inference 与严格 JSON→AnyValue 转换集中到唯一 tabular Polars adapter，并切换 Database edit callers。
- [ ] 删除旧 tabular snapshot 的 Polars/Database imports 与 exact debt，补齐 contract wire/shape、adapter conversion/edit regression 及 Rust architecture 验证。
- [ ] 完成 Backend Task 5b 的 Pure Tabular ordered contract、manual serde 与 duplicate/ragged shape 校验，保持既有 wire shape。
- [ ] 将变量 JSON/handle normalization、Polars materialization、DataFrame I/O 分别归入 Project、Backend adapter、Database owner，并删除旧 mixed tabular owners。
- [ ] 补充 typed tabular/materialization/I/O/DTO mapping errors、atomic normalization 与 architecture/debt guard，避免 raw backend prose 和 lossy unsigned conversion。
- [ ] 通过 tabular 聚焦回归、数据库编辑 integration 回归、Rust 编译/格式/debt 验证及独立 review；保留当前 worktree 未提交状态等待集成授权。
- [ ] 将 DatabaseRuntimeSession 的 admission 状态与幂等 close_admission 收敛为 session-owned 生命周期，禁止 registry-wide close 影响其它 session。
- [ ] 分离 DatabaseOutstandingWork 的 Copy 计数投影与私有 DatabaseOperationLease RAII 所有权，保持未来 prepare/recovery 计数为私有字段。
- [ ] 让 DatabaseSessionDrainControl 携带显式单调 deadline，使用带 outstanding 计数的 closed drain/timeout outcome，并保持 timeout 后 lease 不脱离、Drop 不阻塞等待。
- [ ] 完成 Database foundation 的 Pure Leaf contract、typed error、显式 declaration caller migration 与 focused RED/GREEN、fmt/check/diff 交付记录。
- [ ] 修正 DatabaseError 公共 Debug/source 边界，保留私有 driver source 供 Database 内部使用且不向公共错误视图泄漏。
- [ ] 增加 database error module 的 focused regression，覆盖 Display、Debug 与 Error::source() 的 driver secret redaction。
- [ ] 完成 Database foundation fix round 的 RED/GREEN、Rust fmt/check 与 diff 校验，并提交仅含代码/测试的修复 commit。

## 2026.08.28

- [ ] 完成 Backend Task 5 的 Database neutral schema-facts owner：私有 revision/column/schema facts、Polars/DuckDB 类型归一化与 canonical column-name typed failure。
- [ ] 将现有 Loaded/DuckDB schema projection 与 Project runtime caller 切换到 neutral facts，再由 Transport 保持 `ColumnInfoDTO` 的 `name`/`type` wire shape 映射。
- [ ] 精确移除 Transport→Polars/dtype helper architecture debt，登记 neutral Database fact capability 与当前 Project runtime 的新增 exact occurrences。
- [ ] 完成 schema-facts RED/GREEN、Rust fmt/check、focused architecture audit 与 diff 校验；session API、catalog/data snapshot、mutations、Execution、Presentation 和 frontend 继续留待后续任务。
- [ ] 将项目文件变更收敛为 neutral Project contract，并保持 watcher 相关路径过滤与 burst coalescing 语义。
- [ ] 让 Application watcher session 以 epoch 和可重试 drain/join owner 管理替换与关闭生命周期，禁止 stale/closed worker 回写。
- [ ] 将 notify/filesystem 具体实现限制在 Platform adapter，保留 typed source/sink errors 与 watcher architecture guard。
- [ ] 建立 production-unreachable Execution identity、runtime generation 与 canonical commit receipt 合约，保持其与 Project/Graph/SCI 独立。
- [ ] 建立最终 `execution::plan` 的 opaque provenance/basis、resource requirement、parameter tree、observation intent 与 immutable package owner。
- [ ] 增加 duplicate parameter handle、空/空白 identity 拒绝的 focused regression，并完成 Rust check、focused plan tests、fmt 与 diff 校验。
- [ ] 建立 Database-owned session API 的 neutral catalog/data snapshot、ordered column selection 与 private query-basis seams，保持旧 Project database route 不变。
- [ ] 增加 whole-catalog session/generation/declaration/runtime/schema revalidation 与 typed mutation prepare/commit evidence，所有基础验证保持锁外。
- [ ] 完成 session API focused tests、Rust check、fmt 与 diff 校验；plot query、relational/resource adapters 与生产 session cutover继续由后续任务负责。
- [ ] 建立 Project-owned typed `ProjectRegistryStore` 与 bounded `ProjectProgress` contracts，隔离 registry persistence/progress 的消费者接口。
- [ ] 建立 Commands-owned FIFO progress publisher/worker、shared close state 与 retryable shutdown outcome，保留 Tauri Channel 仅在 delivery adapter。
- [ ] 完成 registry/progress staged contracts 的 Rust check、fmt 与 diff 校验；旧 Project SQLx/Channel route 保留至后续原子 caller cutover。
- [ ] 建立 production-unreachable Execution relational/resource ports 与 canonical plan-version preparation，使用 fields-private sealed grants 和 typed failures。
- [ ] 补齐 Graph value semantics、Project variable defaults 与 Execution RuntimeValue final owners，保持当前 node-system runtime caller 尚未切换。
- [ ] 完成 relational/resource/value owner 的 Rust check、fmt 与 diff 校验，保留旧 Project resource/provider route 至 Execution Task 8 原子切换。
- [ ] 建立 Application-owned pure Project/Database snapshot mapper，将完整函数、变量、数据库声明与 neutral schema facts 映射为 Graph ResourceCatalogSnapshot。
- [ ] 增加数据库 schema ID 完整性校验与独立 GraphCompileSettings 映射，保持 Database basis/revalidation 与旧 Project compiler route 隔离。
- [ ] 完成 Project–Graph Task 2 mapper focused test、Rust check、fmt 与 diff 校验；graph-open/catalog production routing继续留待后续任务。
- [ ] 建立 Project-free Graph analysis/compiler staged entry，只消费 GraphDocument、ResourceCatalog、GraphCompileSettings 与完整 Execution plan basis。
- [ ] 建立 GraphRuntimeState 的 epoch-bound component contract，保留旧 Project-owned registry/catalog/compiler production route 不变。
- [ ] 建立 Project-owned graph history before/after residency snapshots与可逆 change contract，避免将 Graph patch 穿透到 Project history。
- [ ] 完成 Project–Graph Tasks 3–6 的 focused owner checks、Rust check、fmt 与 diff 校验，graph mutation/open/catalog activation继续留待后续原子切换。
- [ ] 建立 production-unreachable `ExecutionRuntimeState` 的 session/generation/admission owner，保持旧 Project/node-system runtime path 唯一活跃。
- [ ] 建立 `ApplicationSessionSlot` 的 Inactive/Replacing/Recovering/Active capture/revalidation contract，禁止暴露混合 session tuple 或第二 production owner。
- [ ] 完成 Execution Tasks 2–3 的 staged state focused compile/test gate，session replacement recovery workflow与生产安装继续留待后续 cutover。
- [ ] 建立 Graph-owned linear `PlannedGraphMutation` candidate handoff 与 Project-owned graph operation capture/receipt capability。
- [ ] 建立 Application-only captured graph mutation planner，保持 Commands 旧 mutation route 不变，避免第二条 production mutation path。
- [ ] 将 Project registry scan/cleanup 的 progress 入参切换为借用 `ProjectProgressSink`，移除 Project 对 Tauri Channel 的直接依赖。
- [ ] 让 Commands registry 创建 bounded progress publisher/worker，在每个返回路径关闭 admission 并保留 timeout drain owner。
- [ ] 完成 Backend Task 7 progress seam 的 Rust check、focused publisher test、fmt 与 diff 校验；SQLx persistence owner 仍待同一任务的后续切换。
- [ ] 将 ProjectRegistry 的 SQLx pool/row/query ownership 移出 Project，改由 `Arc<dyn ProjectRegistryStore>` 驱动域验证、排序与 registry authority。
- [ ] 让 `backend_adapters/project_registry_sqlite.rs` 成为唯一 SQLite schema/query/row mapping owner，并在组合根一次性构造/擦除 concrete store。
- [ ] 完成 registry persistence 与 lifecycle focused 回归、Rust check、fmt 与 diff 校验，保留错误与项目生命周期语义。
- [ ] 建立正常编译但不路由的 Application editor projection model/mapper，消费 Graph analysis/document/catalog 的 neutral facts。
- [ ] 保持现有 Graph-owned editor projection 与 wire DTO 生产路径唯一，新增 Application projection 仅作为后续 Project–Graph/Presentation cutover handoff。
- [ ] 完成 Presentation Task 2A editor projection focused regression、Rust check、fmt 与 diff 校验。
- [ ] 建立 Project-owned execution authority prepare/effect-commit contracts，暴露只读快照与 typed cancellation/deadline control，不构造 runtime/adapters。
- [ ] 建立 Execution package preparation 与 immutable generation-pinned handle，验证 package basis/provenance 后再 mint prepared plan。
- [ ] 完成 Execution Tasks 4–5 focused package/authority checks、Rust check、fmt 与 diff 校验，旧执行 workflow 保留至 Task 8。
- [ ] 建立 Transport-owned `schema::graph_mutation` PortAddressDto typed mapper，保持现有 graph mutation command wire shape 与旧 production route。
- [ ] 建立 staged Application catalog query result/transport-parts seam，统一 localized/compatible query 的 session capture/revalidation 入口。
- [ ] 保持 catalog commands 与旧 Graph/Project snapshot route 不变，待 Project–Graph Task 8 / Execution Task 8 一次性切换并删除旧 owner。
- [ ] 建立 Frontend state-authority manifest 与 fail-closed audit，区分 backend base、optimistic overlay、local draft 与 frontend UI ownership。
- [ ] 增加缺失成员、View writer、delegated dirty writer、action cycle/unresolved delegate 的 focused TypeScript fixtures。
- [ ] 完成 Frontend Task 1 的 typecheck 与 diff 校验；Vitest focused runner 在当前 Windows 环境启动异常，未扩大到全量 suite。
- [ ] 建立 Frontend Application 的 project hydration/event ingress/reconciliation、database metadata、worksheet、result query、execution projection 与 window-close coordinator seams。
- [ ] 建立 Core 只读 read/publication/UI capability 类型，保持 Zustand stores、Views 与旧生产 publication route 尚未切换。
- [ ] 完成 Frontend Application staged capability 的 typecheck 与 diff 校验；Vitest runner 的环境阻塞继续记录，不引入重复测试路径。
- [ ] 建立 Services-owned platform outcome/failure contracts 与 path/window/webview/opener/clipboard/settings event seams，隔离原始平台 API。
- [ ] 建立 Application-owned settings synchronization coordinator 与 Core settings UI capability，保持跨窗口 echo suppression 与 UI state ownership边界。
- [ ] 完成 Frontend platform capability staged typecheck 与 diff 校验，未改动现有 production listener/invoke routes。
- [ ] 让 ExecutionRuntimeState session-local 持有 run registry 与 result store typed owners，支持后续 finalization/result query cutover。
- [ ] 完成 run/result owner 的 Rust check、fmt 与 diff 校验，继续保留旧 node-system runtime production route。
