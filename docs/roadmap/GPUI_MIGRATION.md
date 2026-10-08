# GPUI 迁移

> Status: Planned
> Scope: Tauri/React 界面向 GPUI 原生界面的分阶段迁移与验收
> Canonical owners: 原生实现由 yss-desktop-gpui/README.md 拥有；现有业务契约仍由各 Rust 模块拥有
> Update when: 迁移阶段、未完成能力或验收状态变化时

迁移在 `gpui` 分支进行。原生宿主复用现有 Rust 业务和项目契约；不迁移旧格式，不另建 Graph
文档、历史、解析或执行权威。默认入口已按当前开发方向切为 GPUI，旧宿主配置和适配退出；功能与人工验收仍分阶段完成。
当前先完善 GPUI 原生界面，再补齐功能；不要求复制原 React 样式。`react/` 只作为源码与契约样本参考。

- [x] 首阶段原生宿主实现与 Fedora 构建：独立启动入口、Project 只读快照、节点和 GPU 连线。
- [x] Fedora Event 图人工验收：用户确认节点与连线显示、右键平移、滚轮缩放及重置视图正常。
- [x] 原生视觉实现：统一主题与 SVG 图标、自绘标题栏、固定画布工具栏、分节属性和侧栏资源高亮；单侧端口固定贴对应边缘。
- [x] 端口位置与连线的 Fedora 人工验收：用户确认单侧与双侧端口分别贴 input 左、output 右，连线对齐正常。
- [ ] 其余视觉及属性交互的 Fedora 人工验收；图诊断中文模板与严重程度样式已接入，仍需确认实际显示。
- [ ] 首阶段补充验收：Function 图、空图、中键平移和关闭最后窗口退出。
- [x] Application 平台中立服务组装；初始化与工作台 binding 的 9 个后端回归通过，原生入口没有 Tauri/WebView 依赖。
- [x] 原生项目导航与生命周期：欢迎页、直接打开目录、可搜索最近项目、原生新建/另存为表单、
  关闭、保存后导航及部分提交恢复；复用原 Application 和登记存储，项目库选择页及顶栏按钮已移除。
- [x] 项目打开方式人工反馈：用户确认目录与最近项目打开方式符合预期。
- [x] 紧凑欢迎页人工验收：用户确认新版样式合适，并确认人工验收通过（含窄窗口）。
- [ ] 项目导航的剩余 Fedora 人工验收：未保存输入切换、新建/另存为、关闭及重启恢复。
- [ ] 迁移节点端口和布局、选择、框选、多选拖动、Escape 取消及节点配置输入。
- [ ] 复用现有图命令、历史、显式保存和发布回执；节点位置只在松键时提交。
- [ ] 迁移节点目录、连接兼容性反馈、连接替换、Reroute 和 Details。
- [x] Details 类型化参数及端口操作实现：开关、选项、常量引用、数列和列顺序、筛选、编码映射、默认值恢复、字面量与端口增删/重排；复用原投影和图事务。
- [ ] 类型化属性的 Fedora 人工验收；继续补齐创建前配置。
- [x] 原生新建 Event/Function 图、函数签名、图常量与引用拖动实现；复用现有 Project 事务、图历史及资源发布。
- [ ] 图属性的 Fedora 人工验收：函数参数及调用方、签名与正文保存关系、精确常量值、引用拖动及撤销/保存；继续完善复杂值的图形编辑器。
- [ ] 接入运行、Results 和 Output；Problems 已接入编辑器投影，仍需完整人工验收。
- [x] 原生运行、取消、快照恢复与 Output 接入现有 Application；结果目录、租约、标量/结构概览和每页 100 行虚拟表格完成实现与构建检查。
- [ ] 运行、失败定位、结果分页与租约关闭的 Fedora 人工验收；补齐完整报告、分析、图形与独立结果窗口。
- [ ] 中立日志运行时及原生 Logs：原生构建、两个持久化回归和宿主编译通过，仍需人工验收。
- [ ] 迁移项目浏览器、工作台布局、窗口和焦点；原生工作台建立单一拓扑 owner。
- [x] 基础控件复用 GPUI Component：原有 Dock/Table/输入/菜单基础上，设置导航使用 Sidebar，
  图/数据库/图表及供应商删除、放弃设置使用 AlertDialog；保留原 owner 的提交和失效校验。
- [ ] Sidebar 导航与 AlertDialog 的 Fedora 人工验收；继续统一剩余确认提示与通用界面控件。
- [x] 原生 Markdown 文档新建、目录打开、编辑/基础 GFM 预览、版本化保存、文件 Details、UI intent 和标签恢复；窗口与保存全部纳入文档输入保护。
- [x] 文件/视图菜单提供原生导航，菜单与 UI intent 复用面板入口。
- [x] 顶栏复用组件窗口图标 / AppMenuBar：统一 Menu 定义和类型化动作，资源子菜单、紧凑快捷按钮及原窗口关闭流程；
  原生菜单和系统菜单消费同一声明，窗口选项采用组件的拖动/双击配置。
- [ ] 项目侧共享窗口手势人工验收：主窗口与设置窗口的边缘/角落缩放、释放后的移动、拖动、双击、贴边恢复及窗口按钮。
- [x] Fedora 顶栏基础人工验收：布局、菜单悬停切换、方向键/Escape、新建资源子菜单、拖窗和双击最大化/还原正常；用户反馈比之前好。
- [ ] 顶栏编辑焦点保存、忙碌禁用、长标题/窄窗口与关闭保护人工验收；Windows/macOS 分别验收。
- [x] Markdown 基础 Fedora 人工验收：显示、编辑、预览、文本撤销、保存及图/文档切换正常。
- [ ] Markdown 保存全部、关闭保护、外部修改、重启恢复的 Fedora 人工验收；数学公式、引用预览和完整文件生命周期菜单仍待迁移。
- [x] 原生 Mind 基础实现：自动树布局与 Markdown 主题、选择/框选/折叠、平移缩放、Details 内容和结构操作、
  原类型化提交、目录/新建/UI intent/标签恢复及保存全部和窗口保护；Doc/Mind 共用原生保存提交。
- [ ] Mind 的 Fedora 编辑、导航、保存和恢复人工验收；继续补齐长文本测量、资源生命周期菜单和外部引用操作。
- [x] 原生数据库基础界面：有界分页虚拟表格、选择与复制、元数据和单元格预览、物理类型转换、
  语义设置、CSV/Parquet 导出、版本化保存、标签恢复与保存全部/关闭保护；复用原 Application 数据用例。
- [x] 数据库基础 Fedora 人工验收：真实大表显示、选中内容预览、滚动、分页、复制及类型/语义设置浏览正常。
- [x] 数据库当前页范围选择实现：拖选、Shift 点击/方向键及行列扩选、全选高亮、选区大小与复制；复用组件活动光标。
- [ ] 数据库范围选择，以及设置提交、checkpoint 保存、退出、导出与恢复的 Fedora 人工验收。
- [x] 原生导入与数据库资源菜单实现：CSV/Parquet/Excel/SQLite、PostgreSQL/MySQL/MariaDB 连接、
  工作表/表选择、示例目录读取、失败输入保留、重命名/复制/删除/路径复制；提交后复用权威索引安装与会话重连。
- [x] 恢复五个示例的固定来源、版本化 Parquet 与校验目录，使用原 Cargo 示例入口校验及准备可执行文件旁的资源。
- [ ] 导入、示例导入、资源菜单及会话重连的 Fedora 人工验收。
- [x] 原生插件管理实现：签名/平台检查、信任确认、安装、搜索、启停、卸载、存储维护、任务历史与诊断；复用原 PluginManager。
- [ ] 插件管理的 Fedora 人工验收，真实插件的任务历史及诊断验收。
- [x] Unix 插件进程组终止实现：保留未回收的组首身份，停机取走唯一句柄并终止同组子进程；两个 Linux 真实子进程后端回归通过。
- [ ] 将插件 HTML 自定义视图契约替换为原生视图协议并迁移插件实现；保留各目标平台的进程、任务与真实插件运行验收。
- [x] 独立图表原生实现：直方图/散点/折线、原数据查询与配置草稿、轴/悬浮信息、保存/保存全部、
  新建与目录资源操作、UI intent、关闭保护和标签恢复；复用 Project 文件与 Application 预览用例。
- [ ] 独立图表的 Fedora 人工验收，日期轴、外部变更和完整资源生命周期验收。
- [x] 原生 AI 模型设置：供应商/模型 CRUD、协议与认证、遮蔽密钥、生成参数/推理档位、默认模型、草稿发现与逐项添加；
  原服务保存、保存全部和退出保护，使用独立原生设置窗口。
- [x] 独立设置窗口实现与人工验收：统一入口、重复打开聚焦、复用原草稿与保存流程；2026-10-07 用户确认本轮改动通过。
- [x] 原生模型目录失效交付：供应商/默认模型回执通知会话，重新读取原模型服务。
- [ ] 供应商/模型操作及共享目录交付的 Fedora 人工验收、真实系统凭据库/远端发现；项目知识库及外观设置。
- [x] 原生 Assistant 基本界面：原会话目录与共用 Activity 列表、搜索/新建/重命名、会话打开及布局恢复，
  有序历史与流式投影、发送/取消、模型/模式/推理档位、资源引用、队列、Markdown、工具/任务/引用详情。
- [ ] Assistant 独立 AI 分组、草稿/待确认输入/队列跨重启持久化、缓存和订阅释放、完整工具/计划/用量卡片及链接/公式；
  Fedora 交互、恢复、来源失效、实际模型工具与 Worker 交付验收。
- [ ] 迁移完整结果报告/统计图；独立图表不替代图执行结果里的图形呈现。
- [ ] Fedora/Windows/macOS 验收输入法、剪贴板、拖放、文件对话框、缩放、无障碍与打包。
- [ ] 用同一真实图记录原生输入延迟和呈现表现，确认目标平台性能收益。
- [x] 根目录纯 Cargo workspace，以 GPUI 为默认入口；移除 Tauri 主程序、构建/权限配置、专用 IPC 适配及依赖，日志由中立 `yss-logging` 提供。
- [ ] 后续按原生功能接入情况清理 `react/` 参考源码；当前不以删除参考源码代替功能迁移。

当前实现能力与限制见 [GPUI host](../../crates/yss-desktop-gpui/README.md)。
首阶段预览不意味着编辑器、应用服务或所有平台迁移已经完成。

## 验收记录

项目打开流程改为简洁欢迎页、直接目录打开及组件 List 的最近项目搜索弹窗，顶栏和文件菜单的项目库选择入口已移除。
旧项目库视图、扫描/收藏/登记清理及原生回收站打开入口退出宿主；原 Rust 登记、路径、激活和生命周期服务未改动。
最近记录只读自 ProjectManagement，保留原排序和路径；查询合并刷新，捕获代次及完整记录拒绝过期点击，关闭弹窗释放列表输入。
新建/另存为和打开后的保存确认、部分提交恢复、关闭及布局恢复沿用原流程，近期列表读取不阻塞直接目录打开。
`cargo check -p yss-desktop-gpui --bin yss-desktop-gpui`、
`cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings`、
`cargo build -p yss-desktop-gpui --bin yss-desktop-gpui`、`cargo fmt -p yss-desktop-gpui -- --check`、
`node scripts/generate-crate-dependencies.mjs --check`、受影响 owner 链接/新文件空白和 `git diff --check` 通过。
移除宿主不再消费的 ProjectProgress 直接依赖，生成索引为 60 个 crate、250 条内部依赖声明；其原 Application 消费者仍保留。
临时验收工具经原 Application 激活并登记两个独立副本，实际重读到两条最近记录，窗口使用独立应用数据无项目启动。
未新增 UI 单元测试、重跑未变化的后端回归或执行完整 workspace CI。用户确认打开方式符合预期，要求欢迎页更简洁、更像 Zed。
欢迎页随后改为窄幅纵向分组、紧凑整行 ghost 入口与实际 action binding 的快捷键提示，最近项目名称/路径同排并提供完整路径悬停提示；
新版紧凑样式及欢迎页人工验收（含窄窗口）已获用户确认；未保存输入切换、新建/另存为和重启恢复专项仍开放。
用户反馈需进一步紧凑后，欢迎内容收至 440px、品牌图标 24px，入口和最近项目统一为 30px 行高，
并优先显示当前平台的实际快捷键。针对本轮展示代码，严格宿主 Clippy、宿主构建、修改文件 rustfmt
与 `git diff --check` 通过；两个副本仍通过原登记服务准备，用户人工确认这版样式合适。

顶栏已由两个独立下拉按钮改为组件 AppMenuBar，文件/新建资源/视图使用 GPUI Menu 与原入口动作，
忙碌和项目可用性变化重建菜单；命令提交再次检查当前工作台。设置焦点下的当前保存调用原设置入口。
窗口采用 TitleBar 提供的选项，品牌、菜单、项目名与快捷按钮统一排布；根 DockArea 及业务契约没有改变。
`cargo check -p yss-desktop-gpui --bin yss-desktop-gpui`、
`cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings`、
`cargo build -p yss-desktop-gpui --bin yss-desktop-gpui` 与 `cargo fmt -p yss-desktop-gpui -- --check` 通过。
生成模块索引、受影响 owner 链接和源文件空白检查、`git diff --check` 通过；没有新增 UI 单元测试，
未重跑未变化的后端回归或执行完整 workspace CI。新版使用项目副本和独立应用数据启动，日志记录 AMD Vulkan adapter，
无启动 panic 或资源加载错误。用户已确认布局和基础菜单/窗口交互正常，比之前好；
焦点保存、忙碌禁用、长标题/窄窗口与关闭保护仍需分别验收，Windows/macOS 尚未实际验证。

基础组件整理只改变 `yss-desktop-gpui` 的视图和确认入口，继续消费原业务 owner；未改变持久项目或共享契约。
设置侧栏改为 Sidebar / SidebarMenu，资源及供应商删除、放弃设置改为 AlertDialog，捕获的资源版本和页面代次仍由原提交路径校验。
`cargo check -p yss-desktop-gpui --bin yss-desktop-gpui`、
`cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings`、
`cargo build -p yss-desktop-gpui --bin yss-desktop-gpui` 与 `cargo fmt -p yss-desktop-gpui -- --check` 通过。
生成模块索引只读检查、受影响文档相对链接和源文件空白检查、`git diff --check` 通过。
新版 Fedora 窗口使用项目副本和独立应用数据启动，未新增 UI 单元测试、重跑未变化的后端测试或执行完整 workspace CI；
侧栏及弹窗的实际交互仍等待人工验收。

原生 Assistant 消费原 Application/HarnessHost 及现有公开 HarnessEventDto/ToolInspectionDto，未改变 Harness 或业务公共契约。
目录仍由 Rust Activity 文档生成，正文逐个会话接纳连续序列；重放在候选中合并有界交付，缺口最多自动恢复一次。
发送失败通过持久 TurnStarted 检查接纳，未确认原文保留；取消不声称回滚已提交操作。当前草稿和队列仅保留在本次工作台缓存。
会话同名 ID 复用原面板，项目替换释放旧缓存；根 DockArea 继续拥有拓扑。独立 AI 分组与跨重启草稿等完整生命周期仍开放。
本轮以下既有后端回归实际运行 1、1、1、1、5 个测试并通过，没有新增 UI 单元测试：

- `cargo test -p yss-application --lib harness::tests::conversations_restore_after_project_and_database_reopen_and_remain_isolated`
- `cargo test -p yss-harness-core --lib host::tests::session_turn_persists_one_gap_free_ordered_event_stream`
- `cargo test -p yss-harness-core --lib host::tests::conversation_rename_preserves_history_and_rejects_active_turns`
- `cargo test -p yss-harness-core --lib host::preparation_tests::cancelled_reference_lookup_cannot_start_a_model_or_block_the_next_turn`
- `cargo test -p yss-ipc-contract --lib harness::`

宿主编译、严格 Clippy 与构建通过；命令为 `cargo check -p yss-desktop-gpui --bin yss-desktop-gpui`、
`cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings` 和
`cargo build -p yss-desktop-gpui --bin yss-desktop-gpui`。本轮没有执行完整 workspace CI。
Native 直接依赖原 Core 入口与共享只读投影，生成模块索引已更新为 60 个 crate、251 条内部依赖声明。
`cargo fmt -p yss-desktop-gpui -- --check`、生成索引只读检查、受影响 owner/roadmap 链接与新文件空白、
`git diff --check` 通过。临时本地 SSE 服务经原 ApplicationServices/Rig/Harness 完成一轮 Ask，
实际保存 24 条连续事件、1 条目录记录与 67 字回复；项目使用副本，应用数据独立，不调用真实模型供应商。
Fedora 窗口启动日志确认 AMD Vulkan adapter，未记录启动 panic 或资源加载错误。
上述证据不代表原生交互、真实供应商、Worker 工具、统计计划或完整报告已通过验收；界面人工确认仍开放。

原生模型设置直接使用 LanguageModelService 与当前 provider/model 类型；未增加设置文件、凭据存储或模型推断规则。
发现与保存期间锁定表单，失败保留输入；未保存草稿纳入保存全部与窗口/项目导航确认。模型表单与配置草稿由宿主持有，
完整配置、密钥原子替换/清理和服务端发现仍由原 Rust 服务负责。供应商/模型等专项 UI 验收保持开放。
`cargo test -p yss-application --lib harness::models::tests::` 实际运行 6 个既有后端回归并全部通过，覆盖
声明的推理默认值、自定义名称、未保存连接发现、连接变化拒绝旧发现、临时密钥保留原连接，以及密钥保密和持久清理。
宿主检查使用 `cargo check -p yss-desktop-gpui --bin yss-desktop-gpui`、
`cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings`、
`cargo build -p yss-desktop-gpui --bin yss-desktop-gpui` 和 `cargo fmt -p yss-desktop-gpui -- --check`。
原生保存及关闭流程的消费者均在此宿主中，构建覆盖这些入口；交互继续按人工验收，不新增 UI 单元测试或执行完整 workspace CI。
临时验收工具通过原模型服务创建无需认证的本地供应商，保存默认模型后重新读取验证一致，并实际发现两个本地模型。
独立应用数据的 Fedora 窗口已启动，日志确认 AMD Vulkan adapter，未记录启动 panic 或资源加载错误。
真实系统凭据库、远端服务、设置/保存全部/退出交互及 Assistant 接入仍待分别确认。

独立图表已直接使用当前 ChartDocument/ChartType 契约，配置草稿留在宿主，分布与有界点由原用例生成。
新菜单、目录、Details、UI intent、布局恢复及保存全部均接入根工作台；未新建 Graph、布局或统计 authority。
同一来源保留可修复的列选项，较旧 publication 不覆盖较新读取；已删除文件的迟到读取不能恢复旧视图。
GUI 保存仍提交完整内容并使用 Rust 当前基线，外部变化保留本地配置且提示覆盖语义；不添加格式转换。
数值、日期/微秒日期时间的刻度与悬浮值使用原算术表示，折线复用 GPUI 路径缓存。
本轮以下既有后端聚焦回归实际运行 1、1、2 个测试并通过，没有新增 UI 单元测试：

- `cargo test -p yss-application --lib chart::projection::tests::chart_projection_filters_caps_and_preserves_formats`
- `cargo test -p yss-application --lib database::query::tests::reads_require_the_requested_project_and_runtime_revision`
- `cargo test -p yss-project --lib project_writers::charts::tests::`

宿主使用 `cargo check -p yss-desktop-gpui --bin yss-desktop-gpui`、
`cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings`、
`cargo build -p yss-desktop-gpui --bin yss-desktop-gpui` 和 `cargo fmt -p yss-desktop-gpui -- --check`，均通过。
生成模块索引及相对链接/新文件空白检查、`git diff --check` 通过，未执行完整 workspace CI。
新增两个原生依赖仍使用原图表文档与分布类型，模块索引为 60 个 crate、249 条内部依赖声明。
临时后端验收工具通过原 Application 在项目副本中创建/保存/按 publication 重读三种图表；
Diamonds 实际元数据为 53,940 行、11 列，carat/price 的类型为 Numeric；列对查询返回 8,990 个有限点，
原分布查询返回 11 列的结果。该验证覆盖真实数据及文件交接，不代表图形已通过交互人工验收。
Fedora 原生窗口已使用项目副本和隔离应用数据打开直方图。首次启动发现组件图标资源缺失，
已改用宿主自有 SVG 并重建；重新打开后进程存活，启动日志确认 AMD Vulkan adapter，
未记录图标加载错误或启动 panic。直方图/散点/折线的实际绘制、配置、悬浮、保存及资源操作仍待用户人工确认。

原生插件管理接入原 PluginManager，未新建登记、信任、授权或任务状态 authority。
安装检查绑定摘要与签名者确认；重复提交沿用原操作身份，管理失败后仍读取实际状态并保留失败反馈。
平台检查已识别 Fedora 的本机目标；Unix 提取只给验证后的入口设置执行权限，不继承 ZIP 的执行位。
`cargo test -p yss-plugin-runtime --test installation` 实际运行 5 个后端回归并通过，包括新增的异平台包拒绝、
原幂等安装/卸载、长期回执、签名者变更与存储清理；原安装回归同时检查 Unix 入口及普通资产的权限。
临时验收包通过原 manager 检查：有效签名通过，错误签名及其他架构包分别返回原稳定失败码。
插件自定义视图仍为 HTML 契约，尚未迁移，不能以插件管理或安装通过代表完整插件界面完成。

数据库范围选择复用组件 TableState 的单光标、虚拟滚动与导航，视图保存范围锚点和绘制/复制投影。
新页、刷新后的新读取页及 Escape 清除范围，全选只覆盖已读取的当前页；多个选中列不显示单列修改入口。
Shift 键复用 `gpui-base` 的公开组件动作，限定到数据库表格上下文，未修改其他表格导航。
界面交互仍需人工验收，没有新增 UI 单元测试；数据库业务契约、历史与持久化未因选择扩展而改变。
本阶段使用以下聚焦命令检查受影响的宿主、插件运行时及已有安装目标：

- `cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui -p yss-plugin-runtime --lib --test installation --no-deps -- -D warnings`
- `cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings`（范围选择完成后）
- `cargo build -p yss-desktop-gpui --bin yss-desktop-gpui`
- `cargo fmt -p yss-desktop-gpui -p yss-plugin-runtime -- --check`
- `node scripts/generate-crate-dependencies.mjs --check`（60 个 workspace crate、247 条内部依赖声明）
- `git diff --check`

上述检查通过；新源文件空白、受影响 owner/路线图的相对链接也已核对。
安装回归的 5 个用例实际运行，范围选择只进行了构建检查，尚未通过人工交互验收。
没有运行完整 workspace CI。原生插件管理验收进程使用独立临时应用数据，启动日志确认
AMD Vulkan adapter 且无启动 panic；自定义插件视图与目标平台执行不能据此算作验收通过。
数据库范围选择的验收窗口已使用项目副本与另一独立应用数据目录打开 Diamonds；
启动日志确认同一 Vulkan adapter，无启动 panic，拖选、扩选、高亮及复制仍等待用户确认。
插件审核正文支持有界滚动，较长的声明不会把确认按钮挤出正文容器。

此前项目库阶段的生命周期入口已接入原 Application。聚焦宿主 Clippy、构建和格式检查通过，
模块索引由根 Cargo metadata 更新并只读复核；新增三个原生依赖仍指向原登记和进度 owner。
新建/另存为的部分提交保留实际目标；激活已确认而索引未读取时退休旧项目界面，进入空工作台恢复。
身份未变的失败保留原编辑输入。窗口与项目导航复用保存接纳流程，保存失败不继续后续命令。
以下已有回归各实际运行 1 个测试并通过，未添加 UI 单元测试：

- `cargo test -p yss-application --lib project::lifecycle::tests::created_project_name_agrees_with_manifest_and_registry`
- `cargo test -p yss-application --lib project::lifecycle::tests::save_as_activation_failure_and_success_return_exact_direct_receipts`
- `cargo test -p yss-application --lib project::lifecycle::tests::closing_project_releases_session_before_registered_deletion`
- `cargo test -p yss-project-registry --lib tests::toggle_and_remove_use_the_canonical_store_record`
- `cargo test -p yss-project-registry --lib tests::scan_preserves_cancelled_and_failed_outcomes`
- `cargo test -p yss-application --lib project::registry::tests::picker_cancellation_targets_current_task_after_an_old_future_is_dropped`

宿主检查使用 `cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings`、
`cargo build -p yss-desktop-gpui --bin yss-desktop-gpui` 和 `cargo fmt -p yss-desktop-gpui -- --check`。
相对链接、新文件空白、生成模块索引及 `git diff --check` 已检查，没有执行完整 workspace CI。
Fedora 新项目库预览使用隔离应用数据启动，日志确认 AMD Vulkan adapter，进程正常且无启动 panic。
验收目录及项目副本位于临时目录，原项目未用于写入；外观、创建、关闭、保存确认和重新打开仍待人工确认。

示例资产恢复后，`cargo run -p yss-application --example build_samples` 生成五个实际数据集；
暂时移除 CSV 缓存后，`cargo run -p yss-application --example build_samples -- --check --stage target/debug/resources/samples`
仍校验并安装成功。所有来源固定到具体提交和 SHA-256，原 CSV 不重复纳入 Git；原生启动无需网络。
`cargo test -p yss-application --lib database::tests::bundled_samples_import_edit_and_reopen_as_independent_project_datasets`
实际运行 1 个已有回归并通过，覆盖五个示例导入和独立数据的编辑、保存及重开。
`cargo clippy -p yss-application --example build_samples --no-deps -- -D warnings`、
`cargo fmt -p yss-application -- --check`、生成模块索引及受影响文档链接/空白检查通过。
下载脚本的固定来源下载与错误 hash 拒绝也已实际检查；失败保留原缓存，不发布不匹配文件。
本次只修改资源准备和文档，沿用同一任务未变的宿主构建结果；没有新增 UI 单元测试或运行全量 workspace CI。
GPUI 示例目录显示、导入和重复导入的人工确认仍待用户回复。

原生导入与数据库资源菜单接入后，严格宿主 Clippy、构建及格式检查通过。新入口全部复用现有
Application；会话重连刷新数据库编辑状态，并以宿主交付标识拒绝旧 binding 的迟到事件。
直接安装索引使旧查询失效，避免新资源被先前查询覆盖。以上宿主异步行为仍需人工验收。

本轮复用 `cargo test -p yss-application --lib automation::resources::tests::datasets_keep_rows_types_semantics_and_history_through_copy_export_and_lifecycle`，
实际运行 1 个测试并通过，覆盖原复制、类型/语义、历史、重命名、删除与导出用例；未添加 UI 单元测试。
原生检查使用 `cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings`、
`cargo build -p yss-desktop-gpui --bin yss-desktop-gpui` 和 `cargo fmt -p yss-desktop-gpui -- --check`。
生成索引、受影响 owner/roadmap 相对链接与空白以及 `git diff --check` 通过；没有执行完整 workspace CI。

Fedora 新宿主已使用项目副本和隔离应用数据启动，准备了临时 CSV、Excel、SQLite 来源供人工验收。
日志确认 Vulkan adapter，未记录启动 panic。活动窗口截图捕获了其他窗口，已删除且不作为证据；
KDE 拒绝指定窗口截图接口，本轮没有取得有效界面截图。导入、资源操作、来源错误和会话重连的验收仍开放。
当时源码及可执行文件旁的示例目录缺失，无法进行示例导入验收。后续已恢复资产并接入原生资源准备入口；
示例导入的界面人工验收与各平台安装包仍保持开放。

数据库接入沿用 Application 的读取版本 gate、原转换历史和保存入口，未改变数据库公共契约或新增 UI 单元测试。
本轮复用以下已有后端回归，各实际运行 1 个测试并通过：

- `cargo test -p yss-application --lib database::tests::project_import_edit_cast_undo_save_and_reopen_use_committed_dataset_snapshots`
- `cargo test -p yss-application --lib database::tests::project_database_query_rejects_an_unpublished_declaration`

字段实际提交、保存、导出和关闭的交互验收仍开放，后端回归与宿主构建不能替代人工验收。
真实表格显示检查发现重复打开会再插入标签、部分组件仍沿用旧配色；导航已改为查询原 DockArea
并激活已有面板，主题通过组件库更新入口同步各层投影。这些共享变更的交互验收仍需覆盖图、
文档、Mind、结果与视图菜单，不能仅以数据库显示检查代替。
2026-10-07 用户确认 Diamonds 的显示与基础交互正常，覆盖单元格预览、滚动、分页、复制及列设置浏览。
验收使用原项目副本与隔离的应用数据，未修改原项目；该确认不覆盖实际字段提交、checkpoint、退出或导出。
数据库 mutation 已写入项目，checkpoint 清理运行时历史；原生属性、状态栏和退出提示已明确这一区别，
直接退出不声称回滚数据库设置。
本轮原生宿主使用 `cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings`、
`cargo build -p yss-desktop-gpui --bin yss-desktop-gpui` 和 `cargo fmt -p yss-desktop-gpui -- --check`；
同时检查生成模块索引、受影响 owner/roadmap 的相对链接与新文件空白以及 `git diff --check`。
原生导航和主题变更的消费者全部位于同一宿主，编译覆盖这些入口；图/文档/Mind/结果的导航交互
仍保持单独人工验收项。本轮未运行完整 workspace CI。

Mind 基础接入后，原生宿主的包级编译、严格 Clippy、构建与格式检查通过；
Fedora 项目副本的运行日志确认 Vulkan adapter，已检查十个主题的实际树布局和连线渲染。
交互人工验收仍待确认，渲染检查不代表编辑、关闭或恢复验收完成。

本轮复用以下已有树模型回归，各实际运行 1 个测试并通过；未新增 UI 单元测试：

- `cargo test -p yss-project-model --lib mind::tests::batches::topic_batches_copy_fresh_identities_and_validate_final_parent_relationships`
- `cargo test -p yss-project-model --lib mind::tests::batches::local_tree_queries_preserve_sibling_order_depth_and_ancestor_paths`

原生检查使用 `cargo check -p yss-desktop-gpui --bin yss-desktop-gpui`、
`cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings`、
`cargo build -p yss-desktop-gpui --bin yss-desktop-gpui` 和
`cargo fmt -p yss-desktop-gpui -- --check`，以及生成索引、相对链接、新文件空白与 `git diff --check`。
Project 的 authored-file 生命周期与外部修改检查沿用同一任务的新鲜结果，业务代码与输入未改变；
本轮没有执行完整 workspace CI。

此前 Fedora 已确认首阶段 Event 图显示与导航，以及 input/output 端口和连线对齐。
这些确认不覆盖 GPUI 组件升级后的完整界面、图属性、执行和所有平台行为。
原生编辑与运行验收使用项目副本和隔离应用数据；原项目不用于编辑验收。

2026-10-07 用户确认 Markdown 基础交互正常。验收使用项目副本中的《原生界面验收》，
未修改原项目。该确认不覆盖保存全部、外部修改、关闭保护、重启恢复或其他未实现的编辑器。

本轮文档与菜单接入后，原生包编译、构建、Clippy 与格式检查通过；Project 复用以下两项
聚焦回归，各实际运行 1 个测试并通过，没有新增 UI 单元测试：

- `cargo test -p yss-project --lib file_resources::tests::authored_document_lifecycle_preserves_unsaved_content_and_rejects_stale_edits`
- `cargo test -p yss-project --lib file_resources::tests::external_document_changes_refresh_clean_state_and_cannot_overwrite_dirty_state`

原生检查使用 `cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings`、
`cargo build -p yss-desktop-gpui --bin yss-desktop-gpui` 和
`cargo fmt -p yss-desktop-gpui -- --check`。同一轮的文档生成检查及 `git diff --check` 通过；
已删除指向旧 `src-tauri` 的重复模块索引生成器，模块清单只有根 Cargo metadata 一个入口。

此前 Application 的初始化、工作台 binding、执行和结果读取已有聚焦回归结果；
配置清理后重新执行以下检查，不将旧结果当作新构建证据：

- `cargo check -p yss-desktop-gpui --bin yss-desktop-gpui` 与 `cargo build -p yss-desktop-gpui --bin yss-desktop-gpui` 通过。
- `cargo clippy -p yss-desktop-gpui -p yss-application -p yss-logging --bins --lib --no-deps -- -D warnings` 通过。
- `cargo test -p yss-application --lib runtime::tests::`、`presentation::tests::`、`result_encoding::tests::` 分别运行 4、5、1 个回归，全部通过。
- `cargo test -p yss-logging --lib` 运行 23 个日志回归，全部通过。
- `cargo test -p yss-ipc-contract --lib` 运行 7 个共享契约回归；`cargo test -p yss-project-registry-contract --lib` 运行 3 个回归，全部通过。
- `cargo test -p yss-harness-contract --lib model_catalog_preserves_shared_provider_and_generation_configuration` 运行 1 个迁移路径后的模型契约回归，通过。
- 改动 Rust 包的 `cargo fmt ... -- --check`、生成索引/诊断只读检查和 `git diff --check` 通过。
- `cargo check -p yss-application --examples` 通过，示例命令与资源路径不再指向旧宿主目录。
- `cargo run -- --help` 确认默认入口；原生依赖树不含 Tauri runtime/build、Wry、WebKitGTK 或 GTK。
- Fedora 隔离数据目录的原生进程启动正常，日志确认 AMD Vulkan adapter。此项不替代界面显示和交互人工验收。

本阶段没有执行完整 workspace CI，剩余人工验收保持开放。
