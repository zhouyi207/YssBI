# GPUI 组件逐项迁移审查

> Status: Planned
> Scope: 所有 Tauri/React 参考组件的必要性、原生覆盖、架构优化与验收
> Canonical owners: React 源码定义参考行为；GPUI 和业务模块 README 定义当前契约；本表只记录逐项审查与开放工作
> Update when: 每次完成组件审查、迁移、优化或人工验收时

用户目标是逐个检查全部参考组件：必要的能力迁入 GPUI，已迁移部分复核效率与职责边界，每批完成后独立提交。
不以名称相似、存在原生文件、编译通过或完成部分功能代表该组件已完整迁移。

当前清单以 `react/src/**/*.tsx` 的生产文件为入口，共 265 项；85 个测试 TSX 文件作为行为证据，不迁成 UI 单元测试。
每个文件的导出组件、内部子组件及其调用的 hooks/服务均需在实际审查时核对；非 TSX 注册、样式、平台适配也随对应组件检查。
仓库已无 `src-tauri` 宿主源码；平台功能按 Application/GPUI 当前入口及 React 平台调用核实。

审查结论使用：**待查**、**迁移**、**优化**、**复用原生组件**、**无需迁移**。无需迁移必须说明当前产品行为或原生替代依据。
实现与验收分别记录：已有源码不自动算作完整覆盖；需要人工验收的行为保持待验收，只有实际证据才更新为通过。
框架组件优先复用 `gpui-component`，业务用例继续调用现有 Rust owner，根 DockArea 持有唯一工作台拓扑。

验证按受影响模块选择契约测试、原生编译/Clippy 和局部格式检查；UI 使用人工验收，不增加 UI 单元测试。
历史迁移和平台验收见 [GPUI 迁移](GPUI_MIGRATION.md)，当前原生契约见 [GPUI host](../../crates/yss-desktop-gpui/README.md)。

## 已审查批次

### 项目知识库与共用设置结构

- 项目知识库设置已接入 `ProjectKnowledgeService` 与既有项目索引，覆盖选择文档、来源状态、添加/重建/移除和打开原文。
- 索引内容和来源状态仍归 Application/Harness；原生视图只持有当前查询与选择。文档索引变化合并刷新，项目切换清除选择并拒绝迟到回复。
- 共用字段按窄窗口改为纵向排列；设置页结构与面包屑已逐项对照，其他设置缺口继续保留。
- 聚焦验证：`cargo test -p yss-application --lib harness::knowledge::tests::` 两项通过，覆盖正文变化/删除/移除与重开/跨项目隔离。
- `cargo check -p yss-desktop-gpui --bin yss-desktop-gpui` 和 `cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings` 通过。
- 本批界面人工验收保持开放，操作路径见 GPUI README；编译和业务测试不作为界面验收证据。

### 模型与供应商设置

- 已逐项阅读四个模型设置组件及关联目录/服务：供应商列表和默认模型、供应商编辑、可搜索选择器、模型编辑。
- 原生供应商选择复用 GPUI Combobox，直接使用服务预设；新增配置直接进入编辑，更换预设保留账户及自定义名称并清空模型和临时密钥。
- 已补齐独立协议选择、模型显示名回退与发现结果按 ID 自动合并；删除旧发现列表，保留已有参数和正在编辑的输入。
- 模型表单继续只挂载当前编辑项，复用共享类型校验；不复制 React 的每行输入状态或为未展开模型创建控件。
- 列表补齐实际供应商名称；列表和默认模型菜单借用目录，编辑按钮按稳定账户 ID 读取当前配置，去掉每次渲染的整份配置复制。
- 凭据归属由 Contract 定义，Application 在保存及发现时强制校验，避免更换供应商后隐式复用旧密钥；新加两项业务回归分别保护这两个入口。
- `cargo test -p yss-application --lib harness::models::tests::` 八项通过，覆盖发现草稿/过期回执、账户身份、密钥替换与配置持久化。
- `cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings` 与 `cargo clippy -p yss-application -p yss-harness-contract --lib --tests --no-deps -- -D warnings` 通过；UI 人工验收保持开放，具体路径见 GPUI README。

### 数据导入

- 已逐项阅读六个导入组件、导入步骤 hook、示例目录 hook 与 Application 调用编排，核对失败输入保留、重复提交、目录读取、表选择和完成后的资源打开。
- 原生 `ImportDialog` 使用统一步骤状态承接各子弹窗，复用原文件选择器、URL 输入和 Application 导入/发现服务；表选择继续使用虚拟列表。
- 补齐选择页来源标题：本地文件名可悬浮查看路径，远端只呈现引擎与服务器地址；不移植包含认证信息的原始连接串提示框。
- 示例页补齐本地化名称、用途说明、KB/MB 大小、加载/空目录/读取失败与重试、当前项进度。回调仅捕获 ID/版本，读取与导入错误不再统一丢弃分类。
- 连接草稿返回后保留自定义端口，切换引擎只更新原默认端口；脏输入判断使用当前引擎默认值。表单 Enter 复用当前步骤的提交入口与忙碌保护，窄窗口分类改为横向排列。
- 原生继续在显式确认或选择后提交，保留 CSV 参数与可选项目名称，不移植单表自动提交；GPUI 路径选择器不提供扩展名过滤，由所选入口的原 Reader 校验文件内容。
- React 的“其他 / REST API”只有禁用占位，没有导入服务，因此无需注册原生操作；实际支持来源继续保留 Parquet。
- `cargo test -p yss-application --lib database::tests::bundled_samples_import_edit_and_reopen_as_independent_project_datasets -- --exact` 一项通过，实际覆盖五个示例的独立导入、重复导入、编辑、保存和重开。
- `cargo check -p yss-desktop-gpui --bin yss-desktop-gpui` 与 `cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings` 通过；这批界面人工验收仍开放，具体操作见 GPUI README。

### 数据库分页、选择与内容预览

- 已阅读四个数据库编辑器组件和选中内容预览，连同分页、导出、键盘、选择适配、剪贴板和主要单元格投影实现一起核对。
- 数据读取、资源版本、历史和保存继续使用原 Application；grid 与 Details 共享不可变元数据，GPUI TableState 继续拥有活动光标、焦点、滚动和导航。
- 补齐固定页面行号、原列类型提示、布尔勾选/数字对齐/空值样式；行号列只属于展示适配，键盘跳过该列，数据与复制不增加虚构列。React 的补位空列无需迁移，空白区域交给原生表格。
- 选区扩展支持 Ctrl/Cmd 增选矩形、切换离散行列和 Ctrl/Cmd+Shift 扩选；高亮、统计、复制与 Details 消费同一范围。重叠统计合并区间，常见单矩形直接计数。
- 复制多矩形中的当前矩形，离散行列按页面顺序输出；修复空值被复制为字符串 `null`，保留宽整数与 TSV 转义。详情预览复用默认收起的 Collapsible 与只读 Textarea，区分未选择、NULL 和空字符串，主要单元格不再局限于单元格选择模式。
- 工具栏补齐页面行号范围与随页面交付的读取耗时，图标按钮自然换行；导出选择器防止重复进入，取消后保留原页面并处理排队刷新。界面保持参考实现的只读表格，不凭未调用的旧文案添加编辑功能。
- `cargo test -p yss-application --lib database::tests::rows::row_batches_return_committed_ids_and_keep_one_history_unit_and_revision_gate -- --exact` 一项通过，覆盖稳定行身份、批次历史和过期版本拒绝。
- 原生 `cargo check -p yss-desktop-gpui --bin yss-desktop-gpui` 与 `cargo build -p yss-desktop-gpui --bin yss-desktop-gpui` 通过，独立提交内容通过 `cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings`。
- 使用临时 `YSSBI_APP_DATA_DIR` 启动 Linux/X11 窗口并确认主窗口渲染；当前显示环境的模拟输入未能驱动控件，未据此宣称交互验收通过。短表补位行越界与取消当前多选项后的组件光标已经按组件调用契约修复，完整操作路径仍保留在 GPUI README。


### 数据列详情与语义表单

- 已逐项阅读 DataDetailPanel、DataColumnSettings、DataColumnSemanticDialog、DataColumnSemanticFields，以及语义草稿读取、修改入口和已有参考行为测试。
- Details 使用原生 Collapsible 提供信息与分页列目录，空表和超过 100 列仍可访问全部字段；只展开当前页需要的设置。列设置使用明确列名，表格选区继续独立提供内容预览。
- 物理类型菜单保留当前自定义类型与确认步骤；语义选项直接消费后端支持列表，补齐编辑映射/约束入口。菜单按资源版本校验并在点击时读取列，去掉逐帧与菜单项间的整份映射复制。
- 语义草稿切换类别类型保留原声明域、未出现类别、标签和顺序，再合并整列非空取值。只为当前 50 项创建输入，补齐值和标签编辑、增删、跨页排序与二元正值清除/联动。
- 读取失败可重试，提交校验重复值、二元数量和共享映射上限；其余兼容性和精度校验沿用原业务 owner。未修改时直接关闭，失败及过期保留输入；物理确认期间阻止重复操作并继续处理排队刷新。
- `cargo test -p yss-database-arrow --lib tests::column_semantics_enforce_explicit_domains_and_numeric_constraints -- --exact` 一项通过；`cargo test -p yss-application --lib database::tests::project_import_edit_cast_undo_save_and_reopen_use_committed_dataset_snapshots -- --exact` 一项通过，分别保护显式域/精度约束和应用修改/历史/持久化流程。
- `cargo check -p yss-desktop-gpui --bin yss-desktop-gpui` 已通过；独立提交内容通过 `cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings`。集成工作区后续检查遇到并行插件视图与 Runtime 接口未对齐，未据此修改其他会话内容。人工验收操作见 GPUI README，本批不添加 UI 单元测试，UI 验收仍开放。

### 独立图表编辑与配置

- 已逐项阅读 ChartEditor、ChartPreview（含错误与内容子组件）、ChartEmptyState 和 ChartDetailPanel（含配置子组件），连同文件读取、预览缓存、保存及图形适配入口核对。共享结果图形的额外模型/参考线等能力仍单独待审查。
- 配置草稿仍属于当前 ChartEditor，资源版本与保存仍由原 Project/Application 管理。初次打开使用工作台加载/错误/资源重开入口，原生面板只接纳完整文件读取；接纳后使用更新的已知目录启动预览，不重复读取相同配置。
- 数据源/列菜单改为打开时借用共享投影构造，去掉每次 Details 渲染复制完整选项列表；列清单分页。重复选择当前数据源或图表类型保留编码，无变化不触发读取。
- 预览复用匹配数据库身份与资源版本的元数据；缺列、数据源删除、空数值对与读取失败分别呈现，错误显示稳定码与重试，不展示内部错误对象或虚构 incidentId。重试刷新基线且保留未保存配置。
- 直方图分布沿用既有 Application/Runtime 读取入口，新增显式列选择并在 Engine 聚合前投影；保留原分箱与类别顺序、预算、会话及版本 gate，移除全列聚合开销。
- `cargo test -p yss-application --lib database::query::tests::chart_distributions_only_read_explicit_columns_and_reject_invalid_selection -- --exact`、`cargo test -p yss-application --lib database::query::tests::reads_require_the_requested_project_and_runtime_revision -- --exact` 和 `cargo test -p yss-application --lib chart::projection::tests::chart_projection_filters_caps_and_preserves_formats -- --exact` 各一项通过，分别覆盖选列/无效选择、版本 gate、有限坐标/截断/格式。
- `cargo clippy -p yss-application -p yss-database-runtime --lib --tests --no-deps -- -D warnings` 与 `cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings` 均通过，独立提交版本的 GPUI Clippy 同样通过。人工验收路径见 GPUI README，交互验收仍开放，未添加 UI 单元测试。


### 基础结果图形与共享绘制

- 已逐项阅读 ChartThemeProvider、图形 theme、ChartRenderer、Line/Scatter/Ecdf/Kde/HistogramChart、LinePlotControls 和 PlotResultView，核对当前图形映射与 RuntimeValue/SCI 载荷；不把其他统计 renderer 记为完成。
- 已接入散点、折线、ECDF、KDE、直方图、气泡、象限、P-P/Q-Q、ROC 九类完整结果。Application 按原结果类别与会话读取，不经 JSON 往返；原结果页预算在分配前检查，不以第一页或本地抽样代替整张图。
- 共享 `plots` 由原 `charts/plot` 移入，独立图表与结果共用轴、缓存曲线、BarChart 和悬浮格式。ECDF/KDE 复用组件 StepAfter/Area，参考线参与默认范围；概率图固定坐标、漏斗下降轴和气泡平方根半径保留原语义。
- 原生 ActiveTheme 承接 React Context 的颜色职责；布局直接消费实际 GPUI bounds，不迁移 DOM 尺寸 Hook。普通折线的点显示开关和工具栏属于结果实体，重绘、主题/语言变化不重新读取或解析结果。
- 使用临时 `cargo build -p yss-desktop-gpui --example review_result_plots` 窗口目视核对九类图形首帧，发现并修复组件 StepAfter 省略末端跳变的问题，复看确认 ECDF 到达最后概率值。预览只使用样例数据，源文件已移除；这不替代结果打开/关闭、悬浮、切换及窗口生命周期的完整人工验收。
- Scatter 的报告高亮/对称残差轴、KDE 的报告 xMin、其他统计图与系数分页见后续统计图批次；帕累托分页与独立图形窗口仍开放。Histogram 的 compact 分支目前无生产调用；不为无消费者参数复制另一套原生布局。
- `cargo test -p yss-application --lib graph::results::plot::tests -- --nocapture` 两项通过，覆盖完整 5000 点/历史租约/执行会话，以及当前生产载荷与无效几何；`cargo test -p yss-node-kernel --lib builtins::visualization::tests::every_visualization_kernel_executes_its_declared_input_layout_and_plot_carrier -- --exact` 一项通过。
- `cargo clippy -p yss-application --lib --tests --no-deps -- -D warnings` 和 `cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings` 均通过，独立提交版本的 GPUI Clippy 同样通过；变更 Rust 格式、依赖图与文档链接检查通过。人工验收路径见 GPUI README，未新增 UI 单元测试。


### 统计图形

- 已逐项阅读 CorrelationMatrixChart、CorrelogramChart、DistributionChart、IntervalChart、HeatmapChart、NomogramChart 和对应当前 SCI/Kernel 载荷；接入八类结果，图形覆盖达到 17/20。原计算/结果引用/租约保持同一 owner。
- Application 直接构造原 SCI 中立类型，读取时验证矩阵形状、可空推断、分位数/区间和刻度；不复制类型或经 JSON 重新解析。205 项系数测试保留完整行，显示分页只由结果实体管理。
- 原生矩阵共用轴/图例/悬浮，分布曲线复用 PathCaches；系数页范围、矩阵范围和密度轮廓在 worker 准备，普通重绘不重新计算。图例使用主题颜色，标签截断使用组件的文本测量。
- 系数每页 100 项，页变化重建悬浮身份；列线图保留全部后端轴/刻度并提供滚动，布局不推算新的预测值。原数值读取继续承接帕累托/组合/词云。
- `cargo test -p yss-application --lib graph::results::plot::tests -- --nocapture` 四项通过（其中两项为本批新增），验证旧九类及新八类生产类型、空推断、矩阵形状、完整系数行与结果身份；`cargo test -p yss-node-kernel --lib builtins::visualization::tests::every_visualization_kernel_executes_its_declared_input_layout_and_plot_carrier -- --exact` 一项通过。
- `cargo clippy -p yss-application --lib --tests --no-deps -- -D warnings` 与 `cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings` 通过，独立提交版本的 GPUI Clippy 同样通过。16 个变更 Rust 文件格式、依赖图与变更文档的本地链接检查通过；无新增依赖和 UI 单元测试。
- 临时 `cargo build -p yss-desktop-gpui --example review_statistical_plots` 使用生产 PlotView/plots 目视核对八类样例，修复混色参数权重方向，复看矩阵空格/端点、置信带、分布和区间正常。当前桌面未接收合成鼠标输入，因此悬浮、翻页和滚动未记为验收通过；临时源文件在提交前移除。
- 真实项目的结果打开/关闭、主题/语言、重挂载与租约生命周期验收仍开放。报告消费者的接入另行审查，不把共享绘制完成等同于完整报告完成。

### 帕累托、组合图与词云

- 已逐项阅读 CompositeChart、WordCloudChart、PlotResultView 与对应 SCI 生产函数和解析约束，接入最后三类 PlotData，结果类别覆盖达到 20/20。Application 直接复用原 SCI 类型，并改为穷尽匹配全部类别。
- 帕累托保留完整分类和全样本累计比例，每页 100 项；组合图共用 Frame 与缓存曲线，保留负柱、重复标签、两组值及双轴选择。词频计数不经浮点转换后再展示，读取边界检查计数溢出与列对齐。
- 词云使用 GPUI 原生字体测量，在数据、尺寸或字体变化时才排布；每个词复用同一组螺旋候选位置，主题和悬浮不触发重新排布。长词按宽度缩小，信息栏显示实际排下词数与后端总体数量。
- `cargo test -p yss-application --lib graph::results::plot::tests -- --nocapture` 六项通过，本批两项检查完整 205 类、精确宽整数、累计比例、组合图原值及无效形状/计数；Clippy 简化表达式后单独复跑 `graph::results::plot::tests::categorical` 两项通过。
- `cargo test -p yss-sci --lib visualization::tests::categorical_plots_count_terms_and_cumulative_shares_from_full_samples -- --exact` 一项通过；Application 与 GPUI 的聚焦严格 Clippy 通过，独立提交检出的 GPUI Clippy 同样通过。13 个 Rust 文件格式、模块索引与变更文档相对链接检查通过。未新增依赖或 UI 单元测试。
- 临时 `cargo build -p yss-desktop-gpui --example review_categorical_plots` 复用生产 PlotView/plots，目视核对帕累托首/末页、双轴/共轴、负柱、重复标签、中文/长词/换行和拥挤词云；修正百分比刻度显示并复看。临时预览源文件在提交前移除。
- Fedora 预览中的 `📈` 有测量和排布位置，但字形不可见；同窗口普通 GPUI 文本也复现。该字体渲染问题、真实结果打开/关闭、悬浮、分页按钮、窗口缩放和主题/字体切换验收继续开放。报告专用呈现及独立图形窗口另行审查。

### 结构化统计报告

- 已阅读 ReportView、StructuredResult、StructuredReportTable、StructuredData、Section、DataTable、KeyValue，以及声明解析、分页 hook 和数值/报告容器；线性报告与追加内容流程已阅读，仍单独待迁移。
- Application 在原 Results owner 增加有界声明与页展示校验，直接消费 RuntimeValue；原 `query_result_table`、结果身份、租约和数组引用继续复用，没有 IPC、JSON 往返或新依赖。
- GPUI 增加数值/报告切换，报告实体保留章节与页状态。章节首次展开才加载；原方程以纯文本和复制入口呈现；标量记录共用虚拟表格，嵌套数组保留原绝对路径和 100 行页。
- 稳定性图使用原根、等比例坐标与单位圆，悬浮只展示原模值；分页超过一页有明确提示，不据当前页推断整体稳定性。错误声明保留原值，页失败保留上次成功页和重试入口。
- `cargo test -p yss-application --lib graph::results::report::structured::tests -- --nocapture` 两项通过，保护声明绑定/预算、宽整数/空值/完整页/无效行；`cargo test -p yss-application --lib graph::results::structured::tests -- --nocapture` 三项通过，保护真实 Poisson 无声明报告、嵌套路径、结果租约与完整数组分页。
- 临时 `cargo build -p yss-desktop-gpui --example review_structured_reports` 使用样例页核对原生表格、方程、空值、完整整数、单位圆、范围与无效声明布局；样例页不作为真实服务或交互验收证据。当前 X11 截图在调整窗口尺寸后才显示后续帧，自动重绘及完整交互保留待验收，不加入无效的整窗刷新绕行。
- 独立提交内容通过 `cargo clippy -p yss-application --lib --tests --no-deps -- -D warnings` 和 `cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings`；12 个变更 Rust 文件格式、翻译键、模块索引、文档本地链接与 `git diff --check` 通过。
- 本批未添加 UI 单元测试；具体交互步骤和生命周期验收见 GPUI README。线性回归专项章节、追加报告内容与独立结果窗口仍开放。

### 线性报告基础章节

- 已逐项核对 Equation、FormulaMappingTable、CoefficientTable、CoefficientChart、KeyValue、StatCard、DataTable、TableFrame 与受控切换；完整 LinearRegressionReport 的分析/追加流程保持待迁移。
- 原生保留类型化报告，方程/表/图共用 200 项系数页，观测页复用已有 100 行懒加载和重试；直接读取原统计值及原显著性，不新增依赖。
- Application 共用内容许可、分页范围、会话/租约重验；截距由模型标记及首项位置决定，名称为 `const` 的普通变量不会被误认。
- 方程用原生文本呈现符号式/展开式、映射与复制；仅显示完整且不超过 200 项的系数集。模型摘要、ANOVA 和条件数复用原展示投影，微小非零数使用科学记数法。
- `cargo test -p yss-application --lib graph::results::report::tests::` 在工作区和隔离提交中各运行 7 项通过；新增用例覆盖原值、页边界、失效会话与租约，既有用例补充内容许可、201 项尾页及截距身份。
- 隔离提交的 `cargo clippy -p yss-application --lib --tests --no-deps -- -D warnings` 和 `cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings` 通过；13 个改动 Rust 文件格式、文案键/参数、文档链接和模块索引检查通过。
- 样例窗口核对了含/不含截距的方程、映射、摘要、ANOVA、系数表与正负条形布局；完整真实结果交互与自动重绘验收仍开放。诊断、残差图、ACF/PACF、序列/假设检验及追加内容是后续独立批次。


### 线性报告诊断与分析

- 已逐项核对 LinearRegressionReport 的诊断/残差分支、ACFPACFBlock、SerialTestsBlock、HypothesisTestBlock 及报告使用的 Scatter/KDE/Correlogram 选项；AddReportContents 仍单独待迁移。
- 原生章节直接读取已有 Application 分析接口，统计结果类型复用原 SCI 合约；诊断数组共用 100 行分页与虚拟表格，展开才准备页面，不新建统计或租约 owner。
- 残差明确应用有限横轴范围和 0–100% 高亮比例，模式切换重置范围，相同选择复用已读结果；最多 2000 点、原观测编号、全样本排名与抽样计数由 Application 保留。
- 共享绘制补齐零线、对称残差轴、高亮点和杠杆值密度的零起点；ACF/PACF 保留原置信带与滞后编号。检验卡片展示原值，假设文本保留原约束并可复制。
- 隔离提交的 `cargo test -p yss-application --lib graph::results::report::tests::` 运行 8 项通过；本批新增一项保护范围筛选/抽样后的原观测编号、全样本高亮排名、匹配数量与空范围。
- `cargo clippy -p yss-application --lib --tests --no-deps -- -D warnings` 与 `cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings` 在隔离提交均通过。16 个改动 Rust 文件格式、64 个引用文案键、31 项新增中英文参数、本批三份文档的 287 条本地链接、模块索引及 `git diff --check` 通过。
- 临时 `cargo build -p yss-desktop-gpui --example linear_analysis_review` 样例窗口目视核对高亮残差、三项序列检验、假设、ACF/PACF，以及 205 行诊断首页、空值、不可用原因与密度曲线。样例不代表真实服务或按钮交互验收；临时源文件提交前移除。
- 完整真实结果交互、自动重绘、悬浮、筛选及生命周期验收继续开放，步骤见 GPUI README；未添加 UI 单元测试或新依赖。


### 显式追加线性报告内容

- 已逐项核对 AddReportContents 及其编辑、执行、保留租约和面板替换流程；原生表单只拥有未提交选择，业务编排归 Application 的 report/addition。
- 只合并勾选内容及其关联参数，按捕获版本执行单个 Summary 输出并复用已有输入；新回执必须匹配当前输出。图编辑沿用原撤销/脏状态，不隐式保存。
- 完整新报告持有原 Results 自动租约，安装前再次检查会话与图版本；失败、取消、关闭及迟到交付保留旧报告并释放临时租约。结果面板成功后同时更新值、报告和工作台引用索引。
- 原生普通结果读取也复用 Application 的自动租约，删除桌面侧重复释放包装；面板常规回收继续交给 worker。原显式 lease API 与 ResultStore 保持原职责。
- 独立提交内容的 L2 验证：`cargo test -p yss-application --lib graph::results::report::` 的 12 个报告测试及 `cargo test -p yss-application --lib presentation::tests::open_result_intents_follow_result_retention -- --exact` 的 1 个消费者测试通过；`cargo clippy -p yss-application --lib --tests --no-deps -- -D warnings` 与 `cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings` 通过。
- 18 个变更 Rust 文件的局部格式、3 份变更文档的元信息与 287 个相对链接、25 个复用文案键及占位符、模块索引生成器与 `git diff --check` 通过。临时样例窗口已目视核对正常、计算中、失败状态及 205 个中文参数名的有界布局；样例已移出仓库，此检查不等同于真实交互验收。
- 真实项目中的按钮、面板替换、拖动、语言、关闭和自动重绘验收仍开放，不新增 UI 单元测试或依赖。


## app

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [app/App.tsx](../../react/src/app/App.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [app/main.tsx](../../react/src/app/main.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## app/providers

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [app/providers/ChartThemeProvider.tsx](../../react/src/app/providers/ChartThemeProvider.tsx) | 复用原生主题：无需独立 React Provider | 全部原生图形直接读取 ActiveTheme，主题变更不重建结果投影 | 代码已覆盖；人工验收待完成 |
| [app/providers/SettingsEffectsProvider.tsx](../../react/src/app/providers/SettingsEffectsProvider.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## app/ui

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [app/ui/UIHost.tsx](../../react/src/app/ui/UIHost.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## app/windows

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [app/windows/workbench/WorkbenchComposition.tsx](../../react/src/app/windows/workbench/WorkbenchComposition.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [app/windows/workbench/integrations/PluginProvider.tsx](../../react/src/app/windows/workbench/integrations/PluginProvider.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [app/windows/workbench/integrations/activityEditorDndOverlay.tsx](../../react/src/app/windows/workbench/integrations/activityEditorDndOverlay.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [app/windows/workbench/menuContributionRegistry.tsx](../../react/src/app/windows/workbench/menuContributionRegistry.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [app/windows/workbench/rootPanelRegistry.tsx](../../react/src/app/windows/workbench/rootPanelRegistry.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [app/windows/workbench/rootPanelTabRenderer.tsx](../../react/src/app/windows/workbench/rootPanelTabRenderer.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [app/windows/workbench/statusBarContributionRegistry.tsx](../../react/src/app/windows/workbench/statusBarContributionRegistry.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## components/ui

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [components/ui/alert.tsx](../../react/src/components/ui/alert.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/badge.tsx](../../react/src/components/ui/badge.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/button.tsx](../../react/src/components/ui/button.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/card.tsx](../../react/src/components/ui/card.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/checkbox.tsx](../../react/src/components/ui/checkbox.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/collapsible.tsx](../../react/src/components/ui/collapsible.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/combobox.tsx](../../react/src/components/ui/combobox.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/context-menu.tsx](../../react/src/components/ui/context-menu.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/dialog.tsx](../../react/src/components/ui/dialog.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/dropdown-menu.tsx](../../react/src/components/ui/dropdown-menu.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/empty.tsx](../../react/src/components/ui/empty.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/input-group.tsx](../../react/src/components/ui/input-group.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/input.tsx](../../react/src/components/ui/input.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/label.tsx](../../react/src/components/ui/label.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/menubar.tsx](../../react/src/components/ui/menubar.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/popover.tsx](../../react/src/components/ui/popover.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/progress.tsx](../../react/src/components/ui/progress.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/scroll-area.tsx](../../react/src/components/ui/scroll-area.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/select.tsx](../../react/src/components/ui/select.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/separator.tsx](../../react/src/components/ui/separator.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/switch.tsx](../../react/src/components/ui/switch.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/table.tsx](../../react/src/components/ui/table.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/textarea.tsx](../../react/src/components/ui/textarea.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/toggle-group.tsx](../../react/src/components/ui/toggle-group.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/toggle.tsx](../../react/src/components/ui/toggle.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui/tooltip.tsx](../../react/src/components/ui/tooltip.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## components/ui-presentation

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [components/ui-presentation/Chart.tsx](../../react/src/components/ui-presentation/Chart.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [components/ui-presentation/CoefficientChart.tsx](../../react/src/components/ui-presentation/CoefficientChart.tsx) | 迁移：有界原生系数条形图 | 共享类型化 200 项系数页；保留正负方向、相对大小、原显著性透明度和完整标签提示 | 代码已覆盖；人工验收待完成 |
| [components/ui-presentation/CoefficientTable.tsx](../../react/src/components/ui-presentation/CoefficientTable.tsx) | 复用原生组件：虚拟 Table | 共享系数页显示原统计量、置信区间、原显著性及 p 值星号；极小非零值不格式化为零 | 代码已覆盖；人工验收待完成 |
| [components/ui-presentation/Controls.tsx](../../react/src/components/ui-presentation/Controls.tsx) | 复用原生组件：受控 Button 切换 | 方程符号/展开模式由报告持有；不另建通用 toggle 状态 | 方程已覆盖；其余消费者随迁移复核 |
| [components/ui-presentation/DataTable.tsx](../../react/src/components/ui-presentation/DataTable.tsx) | 复用原生组件：虚拟 Table | 结构化表与线性 ANOVA 使用 results/table 和类型化展示格式 | 代码已覆盖；其余分析消费者待迁移 |
| [components/ui-presentation/Equation.tsx](../../react/src/components/ui-presentation/Equation.tsx) | 迁移：原生线性方程展示 | 符号式/展开式、变量映射、水平滚动与复制；模型标记决定截距，只显示有界完整系数的方程 | 代码已覆盖；人工验收待完成 |
| [components/ui-presentation/FormulaMappingTable.tsx](../../react/src/components/ui-presentation/FormulaMappingTable.tsx) | 复用原生组件：虚拟 Table | 映射从共享完整系数页生成，文字标签不解析为 Markdown/LaTeX | 代码已覆盖；人工验收待完成 |
| [components/ui-presentation/KeyValue.tsx](../../react/src/components/ui-presentation/KeyValue.tsx) | 迁移：原生字段与指标展示 | 结构化字段与线性模型摘要已覆盖，原统计值与展示格式直接读取 Application | 代码已覆盖；人工验收待完成 |
| [components/ui-presentation/Section.tsx](../../react/src/components/ui-presentation/Section.tsx) | 复用原生组件：Collapsible | `results/report/section` 首次展开创建内容，保留折叠与页状态；不迁移 DOM details | 代码已覆盖；人工验收待完成 |
| [components/ui-presentation/StatCard.tsx](../../react/src/components/ui-presentation/StatCard.tsx) | 迁移：类型化统计卡片 | 条件数消费原 presentation::DisplayData，不复制统计计算 | 代码已覆盖；人工验收待完成 |
| [components/ui-presentation/StructuredData.tsx](../../react/src/components/ui-presentation/StructuredData.tsx) | 迁移：通用结构化值 | 原生标量/记录表格、字段与嵌套数组；分页 100 行，宽整数和空值保持原值 | 代码已覆盖；人工验收待完成 |
| [components/ui-presentation/TableFrame.tsx](../../react/src/components/ui-presentation/TableFrame.tsx) | 复用原生组件：Table 及报告边框 | 条纹、滚动、表头和表格生命周期交给组件；不移植 DOM/CSS 包装 | 代码已覆盖；人工验收待完成 |

## features/application

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [features/application/assistant/AssistantRuntimeProvider.tsx](../../react/src/features/application/assistant/AssistantRuntimeProvider.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [features/application/presentation/LinePlotControls.tsx](../../react/src/features/application/presentation/LinePlotControls.tsx) | 迁移：实体只拥有工具栏和点可见性，复用 Button/Switch | results/plot 普通折线局部切换，保留曲线/参考线和租约，ROC 无此开关 | 代码已覆盖；人工验收待完成 |
| [features/application/presentation/PlotResultView.tsx](../../react/src/features/application/presentation/PlotResultView.tsx) | 迁移：完整结果与统计值归 Application，几何与开关归原生视图 | results/plot 已支持 20/20 类；系数/帕累托每页 100 项，保留完整数据与原置信水平/累计比例，翻页不重新读取 | 图形已覆盖；真实结果交互和生命周期验收开放 |
| [features/application/results/components/ReadOnlyDataGrid.tsx](../../react/src/features/application/results/components/ReadOnlyDataGrid.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [features/application/results/components/ResultPageToolbar.tsx](../../react/src/features/application/results/components/ResultPageToolbar.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [features/application/results/components/ResultReadError.tsx](../../react/src/features/application/results/components/ResultReadError.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [features/application/results/components/ResultViewShell.tsx](../../react/src/features/application/results/components/ResultViewShell.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [features/application/results/components/UnifiedResultView.tsx](../../react/src/features/application/results/components/UnifiedResultView.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [features/application/results/components/renderers/ResultRenderers.tsx](../../react/src/features/application/results/components/renderers/ResultRenderers.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [features/application/results/resultViewPresentation.tsx](../../react/src/features/application/results/resultViewPresentation.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [features/application/statusBar/useStatusBarItems.tsx](../../react/src/features/application/statusBar/useStatusBarItems.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [features/application/window/PresentationWindowShell.tsx](../../react/src/features/application/window/PresentationWindowShell.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## features/core

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [features/core/statusBar/builtInStatusBarItems.tsx](../../react/src/features/core/statusBar/builtInStatusBarItems.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## modules/assistant

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/assistant/internal/ui/AssistantComposer.tsx](../../react/src/modules/assistant/internal/ui/AssistantComposer.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantConversationHeader.tsx](../../react/src/modules/assistant/internal/ui/AssistantConversationHeader.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantConversationPanel.tsx](../../react/src/modules/assistant/internal/ui/AssistantConversationPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantConversationToggle.tsx](../../react/src/modules/assistant/internal/ui/AssistantConversationToggle.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantConversations.tsx](../../react/src/modules/assistant/internal/ui/AssistantConversations.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantExecution.tsx](../../react/src/modules/assistant/internal/ui/AssistantExecution.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantMarkdown.tsx](../../react/src/modules/assistant/internal/ui/AssistantMarkdown.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantModelPicker.tsx](../../react/src/modules/assistant/internal/ui/AssistantModelPicker.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantPanel.tsx](../../react/src/modules/assistant/internal/ui/AssistantPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantReferences.tsx](../../react/src/modules/assistant/internal/ui/AssistantReferences.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantResources.tsx](../../react/src/modules/assistant/internal/ui/AssistantResources.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantRunOptions.tsx](../../react/src/modules/assistant/internal/ui/AssistantRunOptions.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantTasks.tsx](../../react/src/modules/assistant/internal/ui/AssistantTasks.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantThread.tsx](../../react/src/modules/assistant/internal/ui/AssistantThread.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantTokenUsage.tsx](../../react/src/modules/assistant/internal/ui/AssistantTokenUsage.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/assistant/internal/ui/AssistantToolCalls.tsx](../../react/src/modules/assistant/internal/ui/AssistantToolCalls.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## modules/chart

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/chart/internal/ui/ChartEditor.tsx](../../react/src/modules/chart/internal/ui/ChartEditor.tsx) | 优化：复用工作台资源打开和原生面板生命周期，配置/保存归原 owner | 首次接纳读取后使用更新目录启动预览；保存、外部修改保护和资源重开沿用原流程 | 代码已覆盖；人工验收待完成 |
| [modules/chart/internal/ui/ChartEmptyState.tsx](../../react/src/modules/chart/internal/ui/ChartEmptyState.tsx) | 复用原生组件：无需复制 React Empty 的 DOM 层级 | `appearance::empty_state` 与原图标库复用参考标题/配置提示，支持无编码与加载状态 | 代码已覆盖；人工验收待完成 |
| [modules/chart/internal/ui/ChartPreview.tsx](../../react/src/modules/chart/internal/ui/ChartPreview.tsx) | 优化：预览数据、错误与读取身份在 charts 模块收口，统计继续由 Application 提供 | `query/preview` 单列分布、匹配元数据复用、缺列与空数值对错误、稳定码和重试；原 GPUI 图形承担三类独立预览 | 代码已覆盖；人工验收待完成 |

## modules/commands

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/commands/internal/ui/activity/SidebarCommandsTab.tsx](../../react/src/modules/commands/internal/ui/activity/SidebarCommandsTab.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## modules/data-explorer

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/data-explorer/internal/ui/import/ExcelSheetSelectModal.tsx](../../react/src/modules/data-explorer/internal/ui/import/ExcelSheetSelectModal.tsx) | 优化：工作表选择复用原生统一步骤和虚拟列表，无需另建弹窗状态 | `imports/selection` 补齐文件名与完整路径提示；失败保留工作表列表与输入，返回继续原流程 | 代码已覆盖；人工验收待完成 |
| [modules/data-explorer/internal/ui/import/ImportModal.tsx](../../react/src/modules/data-explorer/internal/ui/import/ImportModal.tsx) | 迁移/优化：原生分类、步骤和窗口复用原服务；禁用的 REST API 占位无需迁移 | 三类来源、取消/忙碌保护与失败保留已接入；补齐窄窗口分类；保留显式确认及原生路径选择器，完成后按实际回执打开数据 | 代码已覆盖；人工验收待完成 |
| [modules/data-explorer/internal/ui/import/SampleDatasetList.tsx](../../react/src/modules/data-explorer/internal/ui/import/SampleDatasetList.tsx) | 迁移：目录与导入继续由 Application 持有，视图仅持有读投影和当前任务 | `imports/samples` 补齐名称/说明、尺寸单位、空/加载/失败/重试和当前项进度；`feedback` 保留类型化错误分类 | 示例业务回归通过；人工验收待完成 |
| [modules/data-explorer/internal/ui/import/SqlConnectionModal.tsx](../../react/src/modules/data-explorer/internal/ui/import/SqlConnectionModal.tsx) | 优化：复用原生输入和 URL 编码，连接草稿留在统一流程，不新增连接存储 | `imports/inputs` 保留字段/遮蔽连接串模式与失败草稿；修复返回后的端口重置和默认端口脏判断 | 代码已覆盖；人工验收待完成 |
| [modules/data-explorer/internal/ui/import/SqlRemoteTableSelectModal.tsx](../../react/src/modules/data-explorer/internal/ui/import/SqlRemoteTableSelectModal.tsx) | 优化：与 Excel/SQLite 共用虚拟选择列表，连接读取沿用原 Application | `imports/selection` 补齐服务器标识；提示框不包含认证或查询参数；失败后保留表选择与连接草稿 | 代码已覆盖；人工验收待完成 |
| [modules/data-explorer/internal/ui/import/SqliteTableSelectModal.tsx](../../react/src/modules/data-explorer/internal/ui/import/SqliteTableSelectModal.tsx) | 优化：复用同一来源选择器和原 SQLite 发现/导入入口 | `imports/selection` 补齐数据库文件名/路径提示，保留显式选择、返回和失败输入 | 代码已覆盖；人工验收待完成 |

## modules/database-editor

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/database-editor/internal/ui/DatabaseEditorContent.tsx](../../react/src/modules/database-editor/internal/ui/DatabaseEditorContent.tsx) | 优化：原生每页查询沿用版本化 Application，面板拥有读投影和当前页交互，布局由 DockArea 持有 | `databases/query` 与 `install_read` 绑定元数据、编辑状态、页数据与耗时；新页清选区，语义刷新保留匹配数据页 | 业务回归通过；人工验收待完成 |
| [modules/database-editor/internal/ui/Layout/Toolbar.tsx](../../react/src/modules/database-editor/internal/ui/Layout/Toolbar.tsx) | 迁移/优化：复用 GPUI 按钮与原分页/导出入口，统计由已接纳页面派生 | `databases/toolbar` 补齐行号范围、读取耗时、图标提示和窄面板换行；导出选择期间防止重复操作并保留刷新队列 | 代码已覆盖；人工验收待完成 |
| [modules/database-editor/internal/ui/Table/DataTable.tsx](../../react/src/modules/database-editor/internal/ui/Table/DataTable.tsx) | 优化：复用原生虚拟表格、键盘和光标；补位空列无需迁移 | `grid` 固定行号与 `selection` 多范围/离散行列选择；高亮、复制、Details 共用选区；只读、分页、版本失效与主题沿用原 owner | 代码已覆盖；人工验收待完成 |
| [modules/database-editor/internal/ui/Table/DatabaseGridRenderers.tsx](../../react/src/modules/database-editor/internal/ui/Table/DatabaseGridRenderers.tsx) | 迁移：列头、只读单元格和行号由同一个 GPUI TableDelegate 提供 | 补齐原列类型提示、布尔勾选、数字右对齐、弱化空值和全局行号；行号不进入数据索引及剪贴板 | 代码已覆盖；人工验收待完成 |

## modules/details

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/details/internal/ui/DetailEmptyState.tsx](../../react/src/modules/details/internal/ui/DetailEmptyState.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/DetailsPane.tsx](../../react/src/modules/details/internal/ui/DetailsPane.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/constantValue/ConstantValueEditorModal.tsx](../../react/src/modules/details/internal/ui/constantValue/ConstantValueEditorModal.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/node/DescriptionResultSection.tsx](../../react/src/modules/details/internal/ui/node/DescriptionResultSection.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/node/NodeCreationForm.tsx](../../react/src/modules/details/internal/ui/node/NodeCreationForm.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/node/NodeDocumentationPanel.tsx](../../react/src/modules/details/internal/ui/node/NodeDocumentationPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/node/NodePinConnectionField.tsx](../../react/src/modules/details/internal/ui/node/NodePinConnectionField.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/node/NodePinInterfacePanel.tsx](../../react/src/modules/details/internal/ui/node/NodePinInterfacePanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/node/NodePortInstanceControls.tsx](../../react/src/modules/details/internal/ui/node/NodePortInstanceControls.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/node/parameterEditors/NodeParameterEditor.tsx](../../react/src/modules/details/internal/ui/node/parameterEditors/NodeParameterEditor.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/node/parameterEditors/RelationalParameterEditors.tsx](../../react/src/modules/details/internal/ui/node/parameterEditors/RelationalParameterEditors.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/node/parameterEditors/SemanticDomainEditor.tsx](../../react/src/modules/details/internal/ui/node/parameterEditors/SemanticDomainEditor.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/panels/ChartDetailPanel.tsx](../../react/src/modules/details/internal/ui/panels/ChartDetailPanel.tsx) | 优化：共享当前草稿及元数据，菜单按需生成，列清单有界呈现 | `charts/details` 数据源/类型/编码和清除入口；重复选择保留配置，旧菜单版本校验，列清单分页 | 代码已覆盖；人工验收待完成 |
| [modules/details/internal/ui/panels/ConstantValueFields.tsx](../../react/src/modules/details/internal/ui/panels/ConstantValueFields.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/panels/DataColumnSemanticDialog.tsx](../../react/src/modules/details/internal/ui/panels/DataColumnSemanticDialog.tsx) | 迁移/优化：复用当前对话框宿主与类型化提交，视图只拥有未提交草稿 | `semantic` 补齐类别切换保留映射、失败重试、重复/数量校验、未修改直接关闭与过期保护 | 代码已覆盖；人工验收待完成 |
| [modules/details/internal/ui/panels/DataColumnSemanticFields.tsx](../../react/src/modules/details/internal/ui/panels/DataColumnSemanticFields.tsx) | 迁移：复用原生 Input、Checkbox 与图标按钮；控件数量限定在当前页 | `semantic/fields` 与 `semantic/inputs` 补齐值/标签编辑、增删、跨页排序、正值清除和精确数值输入 | 代码已覆盖；人工验收待完成 |
| [modules/details/internal/ui/panels/DataColumnSettings.tsx](../../react/src/modules/details/internal/ui/panels/DataColumnSettings.tsx) | 优化：只挂载当前页展开项，设置目标来自列名和捕获版本 | `details/columns` 复用原类型转换/语义用例；补齐直接编辑映射入口，菜单点击时借用权威元数据 | 代码已覆盖；人工验收待完成 |
| [modules/details/internal/ui/panels/DataDetailPanel.tsx](../../react/src/modules/details/internal/ui/panels/DataDetailPanel.tsx) | 迁移/优化：元数据沿用编辑器同一投影，局部折叠与分页只属于 Details | `details` 信息与 `details/columns` 分页目录；移除 100 列截断，空表可独立配置列，失败可重试 | 代码已覆盖；人工验收待完成 |
| [modules/details/internal/ui/panels/DataSelectionPreview.tsx](../../react/src/modules/details/internal/ui/panels/DataSelectionPreview.tsx) | 迁移/优化：从当前数据库面板的同一选区派生主要单元格，不新增全局面板状态 | `databases/details` 补齐默认收起的预览、行号、列名与可选择的只读内容，区分 NULL/空字符串/未选择；覆盖行列与全页选择 | 代码已覆盖；人工验收待完成 |
| [modules/details/internal/ui/panels/EventDetailPanel.tsx](../../react/src/modules/details/internal/ui/panels/EventDetailPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/panels/FileDetailPanel.tsx](../../react/src/modules/details/internal/ui/panels/FileDetailPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/panels/FunctionDetailPanel.tsx](../../react/src/modules/details/internal/ui/panels/FunctionDetailPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/panels/GraphConstantsPanel.tsx](../../react/src/modules/details/internal/ui/panels/GraphConstantsPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/panels/LogDetailPanel.tsx](../../react/src/modules/details/internal/ui/panels/LogDetailPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/panels/MindDetailPanel.tsx](../../react/src/modules/details/internal/ui/panels/MindDetailPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/panels/NodeDefinitionDetailPanel.tsx](../../react/src/modules/details/internal/ui/panels/NodeDefinitionDetailPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/panels/NodeDetailPanel.tsx](../../react/src/modules/details/internal/ui/panels/NodeDetailPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/shared/DetailCollapsibleSection.tsx](../../react/src/modules/details/internal/ui/shared/DetailCollapsibleSection.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/shared/DetailColumnList.tsx](../../react/src/modules/details/internal/ui/shared/DetailColumnList.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/shared/DetailFieldRow.tsx](../../react/src/modules/details/internal/ui/shared/DetailFieldRow.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/shared/DetailForm.tsx](../../react/src/modules/details/internal/ui/shared/DetailForm.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/shared/DetailPanelShell.tsx](../../react/src/modules/details/internal/ui/shared/DetailPanelShell.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/shared/DetailText.tsx](../../react/src/modules/details/internal/ui/shared/DetailText.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/shared/PinEditor.tsx](../../react/src/modules/details/internal/ui/shared/PinEditor.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## modules/document-editor

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/document-editor/internal/DocEditor.tsx](../../react/src/modules/document-editor/internal/DocEditor.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/document-editor/internal/FileEditor.tsx](../../react/src/modules/document-editor/internal/FileEditor.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/document-editor/internal/MindEditor.tsx](../../react/src/modules/document-editor/internal/MindEditor.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/document-editor/internal/ReferencePanel.tsx](../../react/src/modules/document-editor/internal/ReferencePanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## modules/graph-editor

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/graph-editor/internal/ui/Canvas/core/CanvasDropZone.tsx](../../react/src/modules/graph-editor/internal/ui/Canvas/core/CanvasDropZone.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Canvas/core/Edge.tsx](../../react/src/modules/graph-editor/internal/ui/Canvas/core/Edge.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Canvas/core/GraphCanvasController.tsx](../../react/src/modules/graph-editor/internal/ui/Canvas/core/GraphCanvasController.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Canvas/core/GraphCanvasView.tsx](../../react/src/modules/graph-editor/internal/ui/Canvas/core/GraphCanvasView.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Canvas/core/GraphDocumentEditor.tsx](../../react/src/modules/graph-editor/internal/ui/Canvas/core/GraphDocumentEditor.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Canvas/core/GraphFlowCanvas.tsx](../../react/src/modules/graph-editor/internal/ui/Canvas/core/GraphFlowCanvas.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Canvas/core/GraphFlowConnection.tsx](../../react/src/modules/graph-editor/internal/ui/Canvas/core/GraphFlowConnection.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Canvas/core/GraphFlowEdge.tsx](../../react/src/modules/graph-editor/internal/ui/Canvas/core/GraphFlowEdge.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Canvas/core/GraphFlowNode.tsx](../../react/src/modules/graph-editor/internal/ui/Canvas/core/GraphFlowNode.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Canvas/core/ViewportGrid.tsx](../../react/src/modules/graph-editor/internal/ui/Canvas/core/ViewportGrid.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Canvas/overlays/CanvasExecutionToolbar.tsx](../../react/src/modules/graph-editor/internal/ui/Canvas/overlays/CanvasExecutionToolbar.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Canvas/overlays/CanvasOverlays.tsx](../../react/src/modules/graph-editor/internal/ui/Canvas/overlays/CanvasOverlays.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Canvas/overlays/PinResultSearchPalette.tsx](../../react/src/modules/graph-editor/internal/ui/Canvas/overlays/PinResultSearchPalette.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Canvas/overlays/WatermarkView.tsx](../../react/src/modules/graph-editor/internal/ui/Canvas/overlays/WatermarkView.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/ContextMenu/ConnectionContextMenu.tsx](../../react/src/modules/graph-editor/internal/ui/ContextMenu/ConnectionContextMenu.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/ContextMenu/NodeContextMenu.tsx](../../react/src/modules/graph-editor/internal/ui/ContextMenu/NodeContextMenu.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/ContextMenu/PinContextMenu.tsx](../../react/src/modules/graph-editor/internal/ui/ContextMenu/PinContextMenu.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/NodePalette.tsx](../../react/src/modules/graph-editor/internal/ui/NodePalette.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Nodes/DefaultNodeLayout.tsx](../../react/src/modules/graph-editor/internal/ui/Nodes/DefaultNodeLayout.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Nodes/GraphNodeController.tsx](../../react/src/modules/graph-editor/internal/ui/Nodes/GraphNodeController.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Nodes/GraphNodeView.tsx](../../react/src/modules/graph-editor/internal/ui/Nodes/GraphNodeView.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Nodes/RerouteNodeLayout.tsx](../../react/src/modules/graph-editor/internal/ui/Nodes/RerouteNodeLayout.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Pins/GraphPinController.tsx](../../react/src/modules/graph-editor/internal/ui/Pins/GraphPinController.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Pins/GraphPinView.tsx](../../react/src/modules/graph-editor/internal/ui/Pins/GraphPinView.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/graph-editor/internal/ui/Pins/PinInput.tsx](../../react/src/modules/graph-editor/internal/ui/Pins/PinInput.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## modules/logs

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/logs/internal/ui/LogDomainLayoutHost.tsx](../../react/src/modules/logs/internal/ui/LogDomainLayoutHost.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/logs/internal/ui/LogDomainPanel.tsx](../../react/src/modules/logs/internal/ui/LogDomainPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/logs/internal/ui/LogItemRow.tsx](../../react/src/modules/logs/internal/ui/LogItemRow.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/logs/internal/ui/LogPanelList.tsx](../../react/src/modules/logs/internal/ui/LogPanelList.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/logs/internal/ui/LogPanelStatus.tsx](../../react/src/modules/logs/internal/ui/LogPanelStatus.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/logs/internal/ui/LogPanelToolbar.tsx](../../react/src/modules/logs/internal/ui/LogPanelToolbar.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/logs/internal/ui/LogPanelVirtualList.tsx](../../react/src/modules/logs/internal/ui/LogPanelVirtualList.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/logs/internal/ui/LogWindow.tsx](../../react/src/modules/logs/internal/ui/LogWindow.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/logs/internal/ui/LogWorkspaceActions.tsx](../../react/src/modules/logs/internal/ui/LogWorkspaceActions.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/logs/internal/ui/logWorkspaceContext.tsx](../../react/src/modules/logs/internal/ui/logWorkspaceContext.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## modules/node-catalog

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/node-catalog/internal/ui/activity/LocalizedCatalogTreeRow.tsx](../../react/src/modules/node-catalog/internal/ui/activity/LocalizedCatalogTreeRow.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/node-catalog/internal/ui/activity/SidebarNodesTab.tsx](../../react/src/modules/node-catalog/internal/ui/activity/SidebarNodesTab.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## modules/output

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/output/internal/ui/RunFailurePanel.tsx](../../react/src/modules/output/internal/ui/RunFailurePanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## modules/plugins

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/plugins/internal/ui/PluginMaintenanceDialog.tsx](../../react/src/modules/plugins/internal/ui/PluginMaintenanceDialog.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/plugins/internal/ui/PluginViewFrame.tsx](../../react/src/modules/plugins/internal/ui/PluginViewFrame.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/plugins/internal/ui/PluginsPanel.tsx](../../react/src/modules/plugins/internal/ui/PluginsPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## modules/problems

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/problems/internal/ui/GraphProblemsPanel.tsx](../../react/src/modules/problems/internal/ui/GraphProblemsPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## modules/project-explorer

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/project-explorer/internal/ui/activity/ProjectActivityPanelController.tsx](../../react/src/modules/project-explorer/internal/ui/activity/ProjectActivityPanelController.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/project-explorer/internal/ui/activity/SidebarDataRow.tsx](../../react/src/modules/project-explorer/internal/ui/activity/SidebarDataRow.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/project-explorer/internal/ui/activity/SidebarFileRow.tsx](../../react/src/modules/project-explorer/internal/ui/activity/SidebarFileRow.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/project-explorer/internal/ui/activity/SidebarProjectTab.tsx](../../react/src/modules/project-explorer/internal/ui/activity/SidebarProjectTab.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/project-explorer/internal/ui/activity/SidebarProjectTreeRow.tsx](../../react/src/modules/project-explorer/internal/ui/activity/SidebarProjectTreeRow.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/project-explorer/internal/ui/activity/buildProjectSidebarContextMenuSections.tsx](../../react/src/modules/project-explorer/internal/ui/activity/buildProjectSidebarContextMenuSections.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/project-explorer/internal/ui/picker/DeleteProjectConfirmDialog.tsx](../../react/src/modules/project-explorer/internal/ui/picker/DeleteProjectConfirmDialog.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/project-explorer/internal/ui/picker/NewProjectModal.tsx](../../react/src/modules/project-explorer/internal/ui/picker/NewProjectModal.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/project-explorer/internal/ui/picker/ProjectLibrary.tsx](../../react/src/modules/project-explorer/internal/ui/picker/ProjectLibrary.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/project-explorer/internal/ui/picker/ProjectPickerActionPanel.tsx](../../react/src/modules/project-explorer/internal/ui/picker/ProjectPickerActionPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/project-explorer/internal/ui/picker/ProjectPickerChrome.tsx](../../react/src/modules/project-explorer/internal/ui/picker/ProjectPickerChrome.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/project-explorer/internal/ui/picker/ProjectPickerFeedbackDetails.tsx](../../react/src/modules/project-explorer/internal/ui/picker/ProjectPickerFeedbackDetails.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/project-explorer/internal/ui/picker/ProjectPickerPageIssueAlert.tsx](../../react/src/modules/project-explorer/internal/ui/picker/ProjectPickerPageIssueAlert.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/project-explorer/internal/ui/picker/ProjectPickerScreen.tsx](../../react/src/modules/project-explorer/internal/ui/picker/ProjectPickerScreen.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/project-explorer/internal/ui/picker/projectPickerContextMenu/buildProjectPickerContextMenuSections.tsx](../../react/src/modules/project-explorer/internal/ui/picker/projectPickerContextMenu/buildProjectPickerContextMenuSections.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## modules/results

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/results/internal/ui/info/AddReportContents.tsx](../../react/src/modules/results/internal/ui/info/AddReportContents.tsx) | 迁移：原生选择表单与 Application 追加用例 | 只合并勾选项，复用原输入执行并核对本次输出；失败保留旧报告及输入，成功更新面板引用 | 代码已覆盖；真实交互验收待完成 |
| [modules/results/internal/ui/info/LinearRegressionReport.tsx](../../react/src/modules/results/internal/ui/info/LinearRegressionReport.tsx) | 迁移：线性报告专项章节与显式追加 | 方程、摘要、ANOVA、系数/观测页、选中分析和追加流程均已接入；结果与版本仍由 Rust owner 管理 | 代码已覆盖；真实交互及生命周期验收待完成 |
| [modules/results/internal/ui/info/ReportView.tsx](../../react/src/modules/results/internal/ui/info/ReportView.tsx) | 迁移：类型化报告入口 | 结构化与线性报告沿用同一面板租约；数值/报告切换保留局部状态 | 代码已覆盖；人工验收待完成 |
| [modules/results/internal/ui/info/StructuredReportTable.tsx](../../react/src/modules/results/internal/ui/info/StructuredReportTable.tsx) | 迁移：声明表格与稳定性图 | Application 校验全部声明和行；原生共享表格、单位圆、当前页提示、分页与重试 | 代码已覆盖；人工验收待完成 |
| [modules/results/internal/ui/info/StructuredResult.tsx](../../react/src/modules/results/internal/ui/info/StructuredResult.tsx) | 迁移：结构化报告 | 原方程文本、声明章节和结构化原值；嵌套数组按原路径读取，不复制统计逻辑 | 代码已覆盖；人工验收待完成 |
| [modules/results/internal/ui/info/shared/ACFPACFBlock.tsx](../../react/src/modules/results/internal/ui/info/shared/ACFPACFBlock.tsx) | 迁移/复用：报告与结果共享 correlogram | 首次展开读取原 ACF/PACF；分别从滞后 0/1 开始，保留置信带与观测数 | 代码已覆盖；真实结果验收待完成 |
| [modules/results/internal/ui/info/shared/HypothesisTestBlock.tsx](../../react/src/modules/results/internal/ui/info/shared/HypothesisTestBlock.tsx) | 迁移：类型化检验与原约束文本 | 原 H₀/H₁ 支持复制，t/F、自由度及微小 p 值沿用原结果；不按变量名重写公式 | 代码已覆盖；真实结果验收待完成 |
| [modules/results/internal/ui/info/shared/SerialTestsBlock.tsx](../../react/src/modules/results/internal/ui/info/shared/SerialTestsBlock.tsx) | 迁移：原生统计卡片 | BG、Ljung–Box 与 DW 直接显示原统计量、滞后和概率，章节共享懒加载及重试 | 代码已覆盖；真实结果验收待完成 |
| [modules/results/internal/ui/panel/ResultContent.tsx](../../react/src/modules/results/internal/ui/panel/ResultContent.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/results/internal/ui/panel/ResultInspector.tsx](../../react/src/modules/results/internal/ui/panel/ResultInspector.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/results/internal/ui/panel/ResultPanel.tsx](../../react/src/modules/results/internal/ui/panel/ResultPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/results/internal/ui/plot/PlotWindow.tsx](../../react/src/modules/results/internal/ui/plot/PlotWindow.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/results/internal/ui/source-inspector/SourceInspectorWindow.tsx](../../react/src/modules/results/internal/ui/source-inspector/SourceInspectorWindow.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## modules/settings

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/settings/internal/ui/KnowledgeSettings.tsx](../../react/src/modules/settings/internal/ui/KnowledgeSettings.tsx) | 迁移：保留显式项目文档索引管理；原生直接调用 Application，移除 Web IPC 适配需求 | `settings/knowledge` 状态/命令/渲染；`workbench/settings` 注入项目与打开原文；来源刷新按文档变化合并 | 已实现并通过聚焦业务测试/编译/Clippy；人工验收待完成 |
| [modules/settings/internal/ui/LanguageModelEditor.tsx](../../react/src/modules/settings/internal/ui/LanguageModelEditor.tsx) | 优化：只挂载当前编辑模型，数值/JSON/推理配置继续复用共享校验 | `settings/models` 已有单模型输入及应用/取消，补齐空显示名使用模型 ID；未编辑项只保留原配置 | 代码已覆盖；人工验收待完成 |
| [modules/settings/internal/ui/LanguageModelProviderEditor.tsx](../../react/src/modules/settings/internal/ui/LanguageModelProviderEditor.tsx) | 迁移/优化：连接编辑和模型草稿留在 UI，保存/发现及凭据归属由 Application 负责 | `settings/models/provider` 补齐预设切换、独立协议/认证与换密钥提示；`commands` 合并发现结果并保留原参数 | 业务回归通过；人工验收待完成 |
| [modules/settings/internal/ui/LanguageModelProviderSelect.tsx](../../react/src/modules/settings/internal/ui/LanguageModelProviderSelect.tsx) | 复用原生组件：GPUI Combobox 替代 shadcn/Base UI，搜索与键盘由组件处理 | 直接消费 Application 预设，重选同项不重置，切换保留稳定账户 ID 与自定义名称 | 代码已覆盖；人工验收待完成 |
| [modules/settings/internal/ui/LanguageModelSettings.tsx](../../react/src/modules/settings/internal/ui/LanguageModelSettings.tsx) | 优化：目录仍由 Application 持有，原生仅缓存读投影和未提交草稿 | 默认模型、列表、删除与存储已接入；新增直接打开编辑器，面包屑统一使用配置显示名 | 代码已覆盖；人工验收待完成 |
| [modules/settings/internal/ui/SettingsField.tsx](../../react/src/modules/settings/internal/ui/SettingsField.tsx) | 优化：共用名称/说明/控件行，不迁移 DOM Label 包装 | `settings/fields` 按窗口宽度排列，模型与知识库共用；键盘/焦点与标签关系须人工检查 | 实现已复核；人工验收待完成 |
| [modules/settings/internal/ui/SettingsPage.tsx](../../react/src/modules/settings/internal/ui/SettingsPage.tsx) | 复用原生组件：标题/动作/通知固定，正文独立滚动 | `settings/render` 统一页面结构；知识库失败/加载提示在正文滚动区之外 | 已有原生实现；人工验收待完成 |
| [modules/settings/internal/ui/SettingsPageHeader.tsx](../../react/src/modules/settings/internal/ui/SettingsPageHeader.tsx) | 复用原生组件：原生按钮/图标展示面包屑与页面操作 | `settings/render::header`；模型草稿导航保留放弃确认，知识库使用独立标题；长标题提示和焦点待检查 | 已有原生实现；人工验收待完成 |
| [modules/settings/internal/ui/SettingsView.tsx](../../react/src/modules/settings/internal/ui/SettingsView.tsx) | 迁移/复核：原生独立设置窗口替代 React Dialog，保留分类、搜索与外观设置 | 知识库已接入；语言、搜索、主题、标题栏、平滑滚动及重置仍需逐项复核/补齐 | 审查中，不以知识库完成代表整个设置完成 |

## modules/workbench

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [modules/workbench/internal/layout/EditorResourcePanel.tsx](../../react/src/modules/workbench/internal/layout/EditorResourcePanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/layout/RootLayoutHost.tsx](../../react/src/modules/workbench/internal/layout/RootLayoutHost.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/layout/RootPanelTabRenderer.tsx](../../react/src/modules/workbench/internal/layout/RootPanelTabRenderer.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/WorkbenchWindow.tsx](../../react/src/modules/workbench/internal/ui/WorkbenchWindow.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/WorkbenchWindowEntry.tsx](../../react/src/modules/workbench/internal/ui/WorkbenchWindowEntry.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/activity/ActivityPanelDocumentView.tsx](../../react/src/modules/workbench/internal/ui/activity/ActivityPanelDocumentView.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/activity/ActivityPanelShell.tsx](../../react/src/modules/workbench/internal/ui/activity/ActivityPanelShell.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/dnd/SidebarDragOverlay.tsx](../../react/src/modules/workbench/internal/ui/dnd/SidebarDragOverlay.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/menu/AboutModal.tsx](../../react/src/modules/workbench/internal/ui/menu/AboutModal.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/menu/ArchitectureModal.tsx](../../react/src/modules/workbench/internal/ui/menu/ArchitectureModal.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/menu/BackendArchitecture.tsx](../../react/src/modules/workbench/internal/ui/menu/BackendArchitecture.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/menu/CommunicationArchitecture.tsx](../../react/src/modules/workbench/internal/ui/menu/CommunicationArchitecture.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/menu/CrateDependencies.tsx](../../react/src/modules/workbench/internal/ui/menu/CrateDependencies.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/menu/FrontendArchitecture.tsx](../../react/src/modules/workbench/internal/ui/menu/FrontendArchitecture.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/menu/WorkbenchMenuBar.tsx](../../react/src/modules/workbench/internal/ui/menu/WorkbenchMenuBar.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/sidebar/SidebarEmptyState.tsx](../../react/src/modules/workbench/internal/ui/sidebar/SidebarEmptyState.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/sidebar/SidebarRenameDialog.tsx](../../react/src/modules/workbench/internal/ui/sidebar/SidebarRenameDialog.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/sidebar/SidebarSectionEmptyState.tsx](../../react/src/modules/workbench/internal/ui/sidebar/SidebarSectionEmptyState.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/sidebar/primitives/SidebarChevron.tsx](../../react/src/modules/workbench/internal/ui/sidebar/primitives/SidebarChevron.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/sidebar/primitives/SidebarDraggableItem.tsx](../../react/src/modules/workbench/internal/ui/sidebar/primitives/SidebarDraggableItem.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/sidebar/primitives/SidebarListItem.tsx](../../react/src/modules/workbench/internal/ui/sidebar/primitives/SidebarListItem.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/sidebar/primitives/SidebarRowActionButton.tsx](../../react/src/modules/workbench/internal/ui/sidebar/primitives/SidebarRowActionButton.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/sidebar/primitives/SidebarTreeCategoryRow.tsx](../../react/src/modules/workbench/internal/ui/sidebar/primitives/SidebarTreeCategoryRow.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/sidebar/primitives/SidebarTreeSearchInput.tsx](../../react/src/modules/workbench/internal/ui/sidebar/primitives/SidebarTreeSearchInput.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/status/StatusBar.tsx](../../react/src/modules/workbench/internal/ui/status/StatusBar.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/workbench/internal/ui/status/StatusBarItem.tsx](../../react/src/modules/workbench/internal/ui/status/StatusBarItem.tsx) | 待查 | 待逐项阅读源码 | 待审查 |

## shared/charts

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [shared/charts/ChartRenderer.tsx](../../react/src/shared/charts/ChartRenderer.tsx) | 迁移：共享 plots 按已验证类别分流 | 二十类 PlotData 与报告专用高亮、对称残差轴、零线及密度横轴已接入 | 代码已覆盖；真实结果验收开放 |
| [shared/charts/cartesian/CompositeChart.tsx](../../react/src/shared/charts/cartesian/CompositeChart.tsx) | 迁移/优化：帕累托与组合图共用原数据、Frame 轴和缓存曲线 | plots/composite 保留双轴/共轴、负柱、重复标签；帕累托每页 100 项，0–100% 轴及全样本累计不随页重算 | 代码已覆盖；真实结果与翻页验收待完成 |
| [shared/charts/cartesian/EcdfChart.tsx](../../react/src/shared/charts/cartesian/EcdfChart.tsx) | 迁移：复用 StepAfter 曲线，累计值直接来自 Rust | plots/cartesian 从零基线绘制，Y 轴固定 [0,1]，保留点序和重复 X | 代码已覆盖；人工验收待完成 |
| [shared/charts/cartesian/HistogramChart.tsx](../../react/src/shared/charts/cartesian/HistogramChart.tsx) | 迁移/优化：复用 BarChart；当前无生产调用的 compact 分支不单独迁移 | plots/histogram 共用原分箱次序与计数，重复标签按序号区分，悬浮计数保留精确整数 | 代码已覆盖；人工验收待完成 |
| [shared/charts/cartesian/KdeChart.tsx](../../react/src/shared/charts/cartesian/KdeChart.tsx) | 迁移/复用：Area/Line，不在 GUI 估计密度 | 节点与报告共用零基线面积、曲线缓存；杠杆值横轴从零开始 | 代码已覆盖；真实结果验收待完成 |
| [shared/charts/cartesian/LineChart.tsx](../../react/src/shared/charts/cartesian/LineChart.tsx) | 迁移/优化：复用组件 Line 与路径缓存，不重算后端点 | plots/cartesian 支持参考线、显式坐标、日期格式和点开关；独立图表与结果共用 | 代码已覆盖；人工验收待完成 |
| [shared/charts/cartesian/ScatterChart.tsx](../../react/src/shared/charts/cartesian/ScatterChart.tsx) | 迁移/复用：共享坐标、主题与原观测信息 | 结果散点/气泡/象限/概率图与报告残差共用绘制；保留高亮、对称轴、零线和观测编号 | 代码已覆盖；真实结果验收待完成 |
| [shared/charts/categorical/WordCloudChart.tsx](../../react/src/shared/charts/categorical/WordCloudChart.tsx) | 迁移/优化：使用 GPUI 原生字体测量，复用 Plot 的悬浮与布局状态 | plots/wordcloud 按数据/尺寸/字体缓存螺旋排布，保留原词序、频次与中文；主题变化重新着色，实际排下词数单独呈现 | 代码已覆盖；真实结果、缩放及悬浮验收待完成 |
| [shared/charts/core/theme.tsx](../../react/src/shared/charts/core/theme.tsx) | 复用原生主题：颜色随原窗口主题读取，不另存 Context 状态 | plots 与原生组件共享主题、网格、标签和曲线颜色 | 代码已覆盖；人工验收待完成 |
| [shared/charts/statistical/CorrelationMatrixChart.tsx](../../react/src/shared/charts/statistical/CorrelationMatrixChart.tsx) | 迁移：复用 PlotAxis/PlotLabel 与现有混色，行列身份用原位置 | plots/matrix 保留方阵、空系数/空 p、重复标签和固定 [-1,1] 色阶；悬浮给出完整标签/原值 | 代码已覆盖；真实结果验收待完成 |
| [shared/charts/statistical/CorrelogramChart.tsx](../../react/src/shared/charts/statistical/CorrelogramChart.tsx) | 迁移/复用：只绘制原 ACF/PACF | 报告与结果面板共享正负柱、零线和原置信区；Q/p 悬浮仍按原结果提供 | 代码已覆盖；真实结果验收待完成 |
| [shared/charts/statistical/DistributionChart.tsx](../../react/src/shared/charts/statistical/DistributionChart.tsx) | 迁移：箱线/小提琴共用原分位数与像素轴，轮廓复用 PathCaches | plots/distribution 保留须线/中位数、原异常点/总数、分组标签与后端密度；渲染不排序样本或计算密度 | 代码已覆盖；真实结果验收待完成 |
| [shared/charts/statistical/HeatmapChart.tsx](../../react/src/shared/charts/statistical/HeatmapChart.tsx) | 迁移/优化：矩阵与相关图共用绘制和悬浮，范围只准备一次 | plots/matrix 保留采样行号、重复列名、原值及最小/最大色阶；标签按空间稀疏展示，单元格不截断 | 代码已覆盖；真实结果验收待完成 |
| [shared/charts/statistical/IntervalChart.tsx](../../react/src/shared/charts/statistical/IntervalChart.tsx) | 迁移/优化：误差线和系数复用区间绘制，系数范围按页提前准备 | plots/interval 显示原估计/上下界与系数零线；results/plot 每页 100 项，翻页更换悬浮身份 | 代码已覆盖；真实结果验收待完成 |
| [shared/charts/statistical/NomogramChart.tsx](../../react/src/shared/charts/statistical/NomogramChart.tsx) | 迁移：只排列已有刻度，不在 UI 计算风险或概率 | plots/nomogram 排序原刻度位置、按像素抑制标签重叠，悬浮显示完整值；滚动保留全部轴/刻度 | 代码已覆盖；真实结果验收待完成 |

## shared/ui

| 参考文件（含其子组件） | 必要性/架构结论 | 原生对应与缺口 | 实现/验收 |
| --- | --- | --- | --- |
| [shared/ui/BrandMark.tsx](../../react/src/shared/ui/BrandMark.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/MarkdownLink.tsx](../../react/src/shared/ui/MarkdownLink.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/MarkdownRenderer.tsx](../../react/src/shared/ui/MarkdownRenderer.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/MessageDialog.tsx](../../react/src/shared/ui/MessageDialog.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/Modal.tsx](../../react/src/shared/ui/Modal.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/PageAlert.tsx](../../react/src/shared/ui/PageAlert.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/ProgressOverlay.tsx](../../react/src/shared/ui/ProgressOverlay.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/Select.tsx](../../react/src/shared/ui/Select.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/ToolbarIconButton.tsx](../../react/src/shared/ui/ToolbarIconButton.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/WindowChrome.tsx](../../react/src/shared/ui/WindowChrome.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/WindowChromeControls.tsx](../../react/src/shared/ui/WindowChromeControls.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/WindowTitleBar.tsx](../../react/src/shared/ui/WindowTitleBar.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/actionMenu/ActionMenu.tsx](../../react/src/shared/ui/actionMenu/ActionMenu.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/markdownRendering.tsx](../../react/src/shared/ui/markdownRendering.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
