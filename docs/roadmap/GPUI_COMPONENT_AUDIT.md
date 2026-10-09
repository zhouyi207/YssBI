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


### 结果容器与只读数据页

- 已逐项阅读结果 Application 的 7 个呈现组件、ResultContent/Inspector/Panel 以及 3 个独立窗口组件，核对查询与租约契约。数值、分页、报告和图形沿用原生类型化分流；React Context 与包装组件合并到原生容器。
- ResultPanel 的读取、容器渲染及工具栏分开组织；失败保留原页和精确的表/偏移重试目标，返回概览撤销旧分页交付资格。语言切换只刷新本地显示，不发起结果查询或清空展开状态。
- 普通只读表格补齐固定行号、后端原始列类型、数值对齐、布尔与空值样式以及完整单元格提示；嵌套列表/记录保持单个单元格。格式化文本按页准备并共享，重绘不重复展开或复制完整字符串；紧凑报告表保持自身列定义。
- 页范围、页码与下一页可用性采用已接纳回执；未知总数和偏移大于零的空页分别显示实际页码与 0 行。没有新增结果缓存、领域状态、依赖或 UI 单元测试。
- L2 验证：工作区及独立提交内容的 `cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings` 通过；临时样例用 `cargo build -p yss-desktop-gpui --example results_container_review` 构建，目视检查正常、读取中、失败和未知总数空页的静态布局，样例随后移出仓库。
- 7 个变更 Rust 文件的局部格式、2 份变更文档的元信息与 269 个相对链接、16 个文案键及占位符、模块索引生成器与 `git diff --check` 通过。此次只改变原生呈现及读取控制，不重复运行未改动的后端统计测试；没有把静态样例算作真实交互验收。
- 独立 Plot/Inspector 窗口的原生创建、租约交付和会话关闭已由后续独立窗口批次接入；真实翻页、失败重试、语言、滚动、提示与关闭验收继续开放。


### 基础控件与框架包装

- 已完整阅读 `components/ui` 的 26 个文件及其子组件，核对实际消费路径，并补读唯一进度条消费者 ProgressOverlay。按钮、输入、搜索选择、菜单、折叠、表格与提示复用当前 gpui-component 和既有宿主实现；不创建一套同名 Rust 包装层。
- Card、Badge、Alert、Empty、Label 与分隔线的样式由原生主题及已有字段/分节/反馈函数承担；业务数据、草稿、失败和提交仍归原使用方。React Context、Radix Portal、DOM 特例和 CSS 变体不构成新的应用职责。
- 特别记录 Dialog 的恢复焦点与堆叠、Tooltip 的拖窗关闭、InputGroup 的聚焦、单选/多选语义及 Progress 的比例单位；原生组件存在不代表所有消费者已经迁移或完成人工验收。项目进度遮罩的当前原生流程结论见下一批次。
- 本批只更新审查结论，不改变运行代码、依赖或 UI 状态，也不新增基础控件测试。文档元信息、267 个相对链接、265 个无重复组件行及 26 项基础控件结论检查通过；`git diff --check` 通过，并复用当前任务已通过的模块索引检查（59 crates / 239 条依赖声明）。


### 项目入口与进度

- 已完整阅读项目选择器的 9 个 TSX 文件及其内部子组件，并核对 `useProjectPicker` 的打开/创建与 `projectPickerProgress`、原生表单、最近列表和实际项目操作。当前产品以欢迎页、系统目录选择、最近项目和原生新建/另存为表单为入口；不恢复已移除的独立项目库、收藏、扫描、登记清理或回收站 UI。
- 新增 `projects/progress` 只负责显示，复用 gpui-component Progress 和 Tokio watch 的最新值交付。原操作 worker 在真实步骤边界发送阶段，工作台与表单共享无输入控件的显示实体；目标路径保留原值，状态按当前语言渲染。退出旧视图不会取消已提交的领域操作。
- 打开、新建、另存为和关闭没有底层取消契约与工作总量，使用未知进度且不提供取消；React 的 10%/50%/90% 等阶段估算无需移植。可取消扫描/清理已无原生入口，不添加新的任务登记层。保存前置步骤也显示进度，表单保留输入，失败恢复表单及原已写入目标。
- 通用忙碌遮罩补充原生鼠标遮挡，防止事件命中下层编辑区。完成与失败继续由现有宿主 lifecycle/回执处理，进度不判断成功，也不成为另一份忙碌或项目事实。
- 最近弹窗读取失败重试及类型化失败展示已在后续两批补齐；不以已存在通用提示视为完整覆盖。语言设置、窗口焦点及主题完整能力由各自组件批次继续核对。
- L2 验证：工作区及独立提交内容的 `cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings` 通过；8 个变更 Rust 文件的局部格式、工作区两份文档元信息与 275 个相对链接、8 个中英文进度文案、模块索引（59 crates / 239 条声明）及 `git diff --check` 通过。
- 临时 `cargo build -p yss-desktop-gpui --example project_progress_review` 构建后，目视核对长路径、省略、无目标状态，以及同一实体在两个原生窗口接收 watch 阶段更新；预览不调用 Application，源码随后移出仓库。真实项目提交、失败恢复、键盘/鼠标遮挡、语言和关闭仍待人工验收；本批没有后端契约改动，不重复后端测试，也不添加 UI 单元测试。


### 项目表单输入与最近列表恢复

- 默认父目录只在未编辑字段上安装；输入后清空、系统选择器的明确选择均阻止迟到回填。默认目录读取失败给出可操作提示；选择器取消不会丢弃正常返回的默认位置。
- 最近列表读取改为绑定原 RecentProjects 实体。关闭发起重试的弹窗后，结果仍完成同一读取、解除 loading 并通知欢迎页；没有新建任务或列表 owner。
- 最近弹窗在失败时隐藏不可用旧行并呈现原位重试，保留搜索输入与已有记录，仍以原 generation 和完整记录校验打开。错误持有翻译键，欢迎页与弹窗共用原查询；不从旧记录伪造成功或自动重新激活项目。
- L2 验证：独立提交内容与工作区的 `cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings` 通过；工作区首次被并行 SCI 修改的类型错误阻断，该源码修复后重跑通过（依赖中仍有一个未使用导入警告）。没有修改该批后端工作。
- 7 个变更 Rust 文件的局部格式、两份文档的元信息与 275 个相对链接、4 个中英文文案键及 `git diff --check` 通过；依赖和模块声明未变，复用本轮已通过的模块索引检查。没有后端契约变化，不重复后端测试；输入/清空、取消选择器、真实失败重试及关闭弹窗期间的读取仍待人工验收，不添加 UI 单元测试。


### 项目错误详情与恢复位置

- 复核 ProjectPickerFeedbackDetails、PageIssueAlert 并完整阅读共用 PageAlert；原生反馈沿用现有通知、表单和重试按钮，不增加平行告警 owner。
- `projects/feedback` 直接读取 Application 的类型化生命周期错误及 Project 的稳定错误码、恢复标记；未知故障提供安全通用提示，不解析或显示内部诊断正文，也不制造事件编号。
- 原 worker 保留失败类型到展示边界，在接纳回执时翻译文案。部分提交回执优先于泛化失败；已提交后打开失败保留写入指导，状态刷新失败明确提示核对实际结果。原表单新增真实恢复路径的完整提示和复制，打开入口和提交语义保持不变。
- L2 验证：工作区及对齐最新 HEAD 的独立提交内容均通过 `cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings`；4 个变更 Rust 文件的局部格式、两份文档元信息与 275 个相对链接、20 个中英文反馈键及占位符、`git diff --check` 通过。依赖未变，复用本轮模块索引检查；没有后端契约改动，不重复后端测试。
- `cargo build -p yss-desktop-gpui --example project_feedback_review` 的临时样例目视核对无效项目、忙碌、恢复所需、会话刷新失败、部分登记和写入后打开失败；原错误码与字面量路径正常，内部诊断未出现在界面。样例随后移出仓库。真实失败、复制、恢复打开及关闭仍待人工验收，不把类型化样例或编译算作真实事务验收，不添加 UI 单元测试。


### 独立图形与结果检查窗口

- 复核 PlotWindow、SourceInspectorWindow、PresentationWindowShell 及原窗口租约/会话逻辑；必要行为归并到一个原生窗口容器，内容继续由既有 ResultPanel 分流。
- 结果标签工具栏打开窗口；延后创建/激活并重新检查宿主和执行会话，重复请求按窗口当前引用聚焦。每个窗口有独立输入、分页和折叠，原始结果继续归 Application。
- 临时共享原自动租约保护交接，worker 取得新窗口自己的租约后释放临时持有；关闭来源、首次读取期间关闭窗口、迟到交付均沿用自动释放。窗口与标签共用幂等关闭入口。
- 报告追加直接改变原窗口面板的引用，窗口列表只保留句柄与弱引用；不再维护一份引用到窗口的可变索引。执行会话变化、项目实际替换和主工作台退出关闭窗口，普通重跑保留原快照。
- L2 验证：工作区及独立提交内容的 `cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings` 通过；7 个变更 Rust 文件的局部格式、两份文档元信息与 275 个相对链接、5 个中英文文案键、模块索引（59 crates / 239 条声明）及 `git diff --check` 通过。
- 临时 `cargo build -p yss-desktop-gpui --example result_windows_review` 构建后，隔离应用目录中通过原 Application 新建临时项目、生成 235 行结果并打开真实独立窗口；目视确认首次页面、点击下一页后的 101–200 行和窗口关闭。临时样例的模块路径与回调访问修正后构建通过，仅留下两处样例未使用变量警告；样例随后移出仓库。
- 预览没有验证全部跨窗口操作：重复聚焦、来源关闭后的持续读取、读取中关闭、报告追加、图形、会话替换和主窗口退出仍待人工验收。截图通过调整窗口尺寸促使 X11 重绘，不能用来证明平滑刷新或其他平台表现。没有新增依赖、后端契约或 UI 单元测试，未重复运行未改动的后端统计测试。

### 日志列表控制与订阅恢复

- 逐项阅读 Logs 的十个 TSX 入口及 LogDetailPanel，核对原订阅、buffer、筛选和滚动契约。必要的领域/级别筛选、搜索、跟随、清空及状态提示接入现有 LogsPanel；React Context 和虚拟化 wrapper 无需另建原生对应层。
- `entry` 按需缓存搜索文本，同流快照复用原记录；`filter` 维护唯一显示索引，新增批次只检查新增记录。列表和计数共用结果，保留 1,000 条上限，前缀裁剪调整现有滚动句柄。
- `stream` 保留原有界队列和订阅释放，缺口/积压连续恢复最多三次，新批次或手动刷新重置预算；存储失败停止交付。清空只清显示并保留 watermark，刷新重新读取原 recent snapshot。
- 工具栏复用原生 Input/Button/PopupMenu，语言变化保留输入。固定高度行缓存单行摘要并保留全文提示；跟随沿用参考的 80px 底部阈值，浏览旧记录不会被新增记录拉回。
- 日志详情选择与文本复制见下一批；多领域并列布局及独立窗口生命周期优化继续分别跟踪，不将列表控制完成等同于整个 Logs 模块迁移完毕。
- L2 验证：工作区及独立提交内容的 `cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings` 通过；6 个变更 Rust 文件格式、2 份文档元信息及独立提交的 269 条相对链接（当前工作区 275 条）、24 个中英文文案键与参数、模块索引（59 crates / 239 条依赖声明）和 `git diff --check` 通过。没有后端契约变化，不重复未改动的后端测试。
- 临时 `cargo build -p yss-desktop-gpui --example logs_controls_review` 在隔离应用目录使用真实 LogRuntime 提交样例。Linux/X11 窗口目视核对结构化字段搜索、领域/级别组合、无匹配、清空、刷新恢复，当前工作区中英文切换保留输入；新增、裁剪和连续两批 1,200 条记录的底部跟随/手动浏览均已核对。
- 预览发现并修复多行消息挤出固定行高、严格底部判定在缩放后失效、前缀裁剪与尚未完成的底部滚动相互影响；同流搜索缓存与原生滚动句柄继续复用。截图需调整窗口尺寸触发 X11 重绘，不能用来证明自动重绘、性能或其他平台表现。临时样例已移出仓库；断流、存储失败、关闭订阅、真实业务日志与跨窗口验收仍开放，不添加 UI 单元测试。

### 日志选择与只读详情

- 复核 LogDetailPanel/LogItemRow 的选择、清空及原文本行为，并逐项阅读 DetailPanelShell、DetailForm、DetailFieldRow、DetailCollapsibleSection、DetailText。这些包装复用原生 Details/Input/Textarea/Collapsible；可编辑字段的提交语义仍由后续参数批次核对。
- LogsPanel 只持有一个选中记录实体，Details 弱引用该实体；同一 stream/sequence 重选复用输入与折叠，筛选、刷新和缓冲裁剪保留不可变检查快照。清空/Escape 只撤销仍指向该实体的检查，不影响后来显示的资源。
- 显式检查保留原图/资源绑定和未提交表单，旧表单菜单失效；后台图投影不抢占日志展示，显式节点选择恢复属性。原窗口与 DockArea 仍拥有布局，未创建日志业务 store 或 IPC 接口。
- 列表提供高亮、上下键/Home/End/Enter、右键复制；详情提供原始元信息、可选择的消息与字段、完整 JSON 复制。只为选中记录创建控件，不把日志正文当 Markdown，也不为全部虚拟行分配输入实体。
- 多领域并列布局与独立窗口详情/生命周期仍待处理，故障和跨窗口专项验收保持开放。
- L2 验证：当前工作区与独立提交内容的 `cargo clippy -p yss-desktop-gpui --bin yss-desktop-gpui --no-deps -- -D warnings` 均通过；8 个 Rust 文件局部格式、两份文档元信息与 269 条相对链接（当前工作区 275 条）、8 个新增双语键及 22 个相关文案引用、模块索引（59 crates / 239 条依赖声明）和 `git diff --check` 通过。未变更后端契约，不重复后端测试；本批审查清单累计 131/265 项。
- 临时 `cargo build -p yss-desktop-gpui --example log_details_review` 使用隔离项目、真实 LogRuntime 和原生工作台。Linux/X11 窗口目视核对点击/方向键选中、折叠后重选和当前工作区的中英文切换、筛选无匹配时保留详情、搜索焦点不触发行切换，以及 Escape/清空/回到画布恢复图属性。
- 原消息及完整 JSON 复制通过只识别样例内容的预览检查核对，完整记录保留 `9007199254740993`；日志详情仍保留图绑定。消息里的 Markdown 符号和换行按原文显示，只读输入不能改写正文。右键菜单显示和复制已核对；X11 截图需要调整尺寸触发刷新，不代表自动重绘、真实项目完整交互或其他平台验收通过。临时样例不提交，不添加 UI 单元测试。

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
| [components/ui/alert.tsx](../../react/src/components/ui/alert.tsx) | 复用原生反馈区域或 Alert | 仅封装等级、标题、描述和布局；结果、模型与项目反馈保留各自失败 owner，不建立公共错误状态 | 无需独立移植；随业务反馈验收 |
| [components/ui/badge.tsx](../../react/src/components/ui/badge.tsx) | 复用原生 Badge 或状态文字 | 仅样式变体；现有状态卡片按主题渲染，等级由原业务投影决定 | 无需独立移植 |
| [components/ui/button.tsx](../../react/src/components/ui/button.tsx) | 复用 gpui_component::Button | 既有按钮提供尺寸、图标、禁用及选中展示；回调仍调用所属面板，Radix Slot/asChild 无须映射为 Rust 类型 | 已采用；键盘与焦点随使用方验收 |
| [components/ui/card.tsx](../../react/src/components/ui/card.tsx) | 合并到原生分节布局 | Header/Content/Footer 都是无状态样式包装；settings 与 results/report/display 已按原生主题组织卡片 | 无需独立移植 |
| [components/ui/checkbox.tsx](../../react/src/components/ui/checkbox.tsx) | 复用 gpui_component::Checkbox | 报告补选、模型能力和节点参数已有受控原生选项；选择仍是表单暂态，提交归 Application | 已采用；随表单验收 |
| [components/ui/collapsible.tsx](../../react/src/components/ui/collapsible.tsx) | 复用原生 Collapsible 与章节实体 | 报告保留章节和已读数据，展开状态归对应实体；不引入全局折叠 store | 已采用；业务消费者继续逐项审查 |
| [components/ui/combobox.tsx](../../react/src/components/ui/combobox.tsx) | 复用 ComboboxState/SearchableVec | 供应商预设已有可搜索原生列表、空态及受控选择；Assistant 模型选择随 Assistant 批次核对，不复制 Base UI Chips/Portal 包装 | 供应商已采用；Assistant 消费者待审查 |
| [components/ui/context-menu.tsx](../../react/src/components/ui/context-menu.tsx) | 复用 ContextMenuExt/PopupMenu | 既有 Activity 右键菜单提供项目动作和禁用状态；Portal、CSS 定位与快捷键文字由原生菜单承担 | 基础能力已采用；动作集合随使用方审查 |
| [components/ui/dialog.tsx](../../react/src/components/ui/dialog.tsx) | 复用原生弹窗与关闭流程 | 已核对堆叠层和恢复焦点逻辑；现有原生 Dialog 路径与宿主窗口负责焦点、遮罩和关闭，草稿确认仍归所属表单 | 无需复制 DialogStackContext；弹窗交互待验收 |
| [components/ui/dropdown-menu.tsx](../../react/src/components/ui/dropdown-menu.tsx) | 复用 DropdownMenu/PopupMenuItem | 仅 Portal 与项目样式包装；原生工具栏和模型配置已有菜单，选中及禁用来自原状态 | 已采用；随使用方验收 |
| [components/ui/empty.tsx](../../react/src/components/ui/empty.tsx) | 复用 appearance::empty_state 和局部空态 | 标题、说明、图标和可选动作都是展示；已有原生空态使用统一主题和图标 | 无需独立移植 |
| [components/ui/input-group.tsx](../../react/src/components/ui/input-group.tsx) | 复用原生 Input 的 prefix/suffix 与布局 | 参考实现只被组合框内部使用；装饰、清除按钮和点击聚焦由输入组件与调用方组合，不保存第二份输入值 | 无需独立移植；聚焦随组合框验收 |
| [components/ui/input.tsx](../../react/src/components/ui/input.tsx) | 复用 InputState/Input | 原生输入实体保留未提交文字、选择与撤销；placeholder、禁用和校验提示由当前表单提供 | 已采用；输入法与键盘随表单验收 |
| [components/ui/label.tsx](../../react/src/components/ui/label.tsx) | 复用字段标题、说明与原生标签 | Settings render_field 和 Details 字段已有展示；可访问名称与点击聚焦跟随对应输入绑定，不移植 HTML for 属性模型 | 展示已采用；焦点与可访问性待验收 |
| [components/ui/menubar.tsx](../../react/src/components/ui/menubar.tsx) | 复用原生菜单模型与 AppMenuBar | 原生 workbench/menus 提供菜单与命令，勾选/禁用从工作台派生；本文件只有 Radix 外壳，菜单项目另行审查 | 基础能力已采用；菜单项目审查仍开放 |
| [components/ui/popover.tsx](../../react/src/components/ui/popover.tsx) | 复用 gpui_component::Popover 或所属菜单 | 参考内容只是锚点与 Portal 布局；筛选草稿、选择和详情继续由所属面板管理，不建立通用弹出层状态库 | 无需独立移植；具体消费者待审查 |
| [components/ui/progress.tsx](../../react/src/components/ui/progress.tsx) | 复用原生 Progress | 项目生命周期没有工作总量，使用 loading(true)；原生值单位为 0–100，当前流程不移植估算百分比 | 原生项目进度已接入；真实交互待验收 |
| [components/ui/scroll-area.tsx](../../react/src/components/ui/scroll-area.tsx) | 复用原生滚动与虚拟列表/Table | 方向和滚动句柄归原生容器；结果和日志已有有界视口，不搬迁 Radix DOM wrapper 修补或额外滚动位置镜像 | 已采用；滚动及拖动随使用方验收 |
| [components/ui/select.tsx](../../react/src/components/ui/select.tsx) | 复用原生选项菜单/Combobox | workbench/controls::choice 与设置菜单消费原选项和当前值；需要搜索时复用 ComboboxState，不复制 Radix 选择状态 | 已采用；选项业务语义随使用方审查 |
| [components/ui/separator.tsx](../../react/src/components/ui/separator.tsx) | 复用原生 Separator 或边框 | 仅横/竖分隔样式，唯一直接消费者为架构介绍；不引入应用状态或新封装 | 无需独立移植 |
| [components/ui/switch.tsx](../../react/src/components/ui/switch.tsx) | 复用原生布尔控件 | 节点参数已有 Checkbox，图形控制使用原受控选项；开关外观可用原生 Switch，布尔值与提交入口保持原 owner | 基础能力已采用；各业务选项待验收 |
| [components/ui/table.tsx](../../react/src/components/ui/table.tsx) | 复用虚拟 Table 与已有报告表 | HTML 表头/行/单元格都是样式包装；results/table、系数及数据库网格已有有界行数据与原列定义 | 已采用；不复制 HTML 表格组件族 |
| [components/ui/textarea.tsx](../../react/src/components/ui/textarea.tsx) | 复用 InputState/Textarea | 模型参数表单已使用 Textarea，多行草稿保留在同一输入实体；不增加正文或撤销状态 | 已采用；多行编辑随模型表单验收 |
| [components/ui/toggle-group.tsx](../../react/src/components/ui/toggle-group.tsx) | 复用受控 Button 选中状态 | 报告数值/报告及图形模式已有原生按钮；单选/多选由所属显示状态决定，不复制仅传递样式的 Context | 已采用；随报告与图形验收 |
| [components/ui/toggle.tsx](../../react/src/components/ui/toggle.tsx) | 无需迁移样式常量 | 文件只导出 toggleVariants，供 ToggleGroup 使用，没有独立组件或业务行为；原生按钮主题覆盖其职责 | 无需移植 |
| [components/ui/tooltip.tsx](../../react/src/components/ui/tooltip.tsx) | 复用原生 Tooltip 与控件提示接口 | 已核对受控/非受控状态和自定义拖窗广播；原生提示归 Window/元素，不搬迁浏览器全局事件协调器 | 已采用；拖窗、焦点和遮挡仍需验收 |

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
| [features/application/results/components/ReadOnlyDataGrid.tsx](../../react/src/features/application/results/components/ReadOnlyDataGrid.tsx) | 复用原生虚拟 Table，迁移只读单元格语义 | results/table 持有一页格式化数据；固定行号、原始列类型、布尔、空值、宽整数及嵌套值提示；紧凑报告表不补列或行号 | 代码已覆盖；真实滚动与提示验收待完成 |
| [features/application/results/components/ResultPageToolbar.tsx](../../react/src/features/application/results/components/ResultPageToolbar.tsx) | 迁移分页控制，采用后端回执 | results/toolbar 显示已接纳页的范围与页码；未知总数采用 has_more，空页不显示错误的 0–offset 范围 | 代码已覆盖；真实翻页验收待完成 |
| [features/application/results/components/ResultReadError.tsx](../../react/src/features/application/results/components/ResultReadError.tsx) | 迁移原生失败反馈与原请求重试 | results/reading 保留失败读取的 part/offset；toolbar 在当前语言显示失败并重试，原页继续可读；不搬迁 IPC ErrorReference 展示层 | 代码已覆盖；失败与重试验收待完成 |
| [features/application/results/components/ResultViewShell.tsx](../../react/src/features/application/results/components/ResultViewShell.tsx) | 合并到原生结果容器 | results/render 和 toolbar 统一边框、工具栏和有界内容；没有独立业务状态，无须复制 React 包装层 | 原生面板已覆盖；独立窗口另行迁移 |
| [features/application/results/components/UnifiedResultView.tsx](../../react/src/features/application/results/components/UnifiedResultView.tsx) | 复用类型化读取分流 | results/query::open 按原 Result 类型选择分页、数值、报告和完整图形；不新增第二个路由器或预读取 | 代码已覆盖；人工验收待完成 |
| [features/application/results/components/renderers/ResultRenderers.tsx](../../react/src/features/application/results/components/renderers/ResultRenderers.tsx) | 迁移数列与标量呈现 | 数列复用有界表格，标量与对象沿用可展开值树；保留宽整数、空值和布尔，长标量提供完整提示 | 代码已覆盖；人工验收待完成 |
| [features/application/results/resultViewPresentation.tsx](../../react/src/features/application/results/resultViewPresentation.tsx) | 无需迁移 React Context | 原生 Panel 与窗口容器负责呈现边界；读取和租约不依赖 embedded/standalone Context | 面板与独立窗口已复用；交互待验收 |
| [features/application/statusBar/useStatusBarItems.tsx](../../react/src/features/application/statusBar/useStatusBarItems.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [features/application/window/PresentationWindowShell.tsx](../../react/src/features/application/window/PresentationWindowShell.tsx) | 迁移窗口外壳与加载失败呈现 | 复用 Root、window_chrome 与 ResultPanel；标题、窗口动作、读取及重试共用原生 owner，无 WebView 路由或结果存储副本 | 已接入，人工验收待完成 |

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
| [modules/details/internal/ui/panels/LogDetailPanel.tsx](../../react/src/modules/details/internal/ui/panels/LogDetailPanel.tsx) | 复用原 Details 容器与原生只读控件 | 当前日志实体唯一归 LogsPanel，Details 弱引用展示时间/流/序列/领域/来源及原消息/字段；默认展开、文本选择与复制不解析 Markdown | 代码已接入；完整交互验收待完成 |
| [modules/details/internal/ui/panels/MindDetailPanel.tsx](../../react/src/modules/details/internal/ui/panels/MindDetailPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/panels/NodeDefinitionDetailPanel.tsx](../../react/src/modules/details/internal/ui/panels/NodeDefinitionDetailPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/panels/NodeDetailPanel.tsx](../../react/src/modules/details/internal/ui/panels/NodeDetailPanel.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/shared/DetailCollapsibleSection.tsx](../../react/src/modules/details/internal/ui/shared/DetailCollapsibleSection.tsx) | 复用原生 Collapsible/Button 与既有局部折叠状态 | 日志消息/字段默认展开，同条记录重选保留状态；其余 Details 消费者按各自初始展开策略核对 | 日志已接入；其他消费者随功能验收 |
| [modules/details/internal/ui/shared/DetailColumnList.tsx](../../react/src/modules/details/internal/ui/shared/DetailColumnList.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [modules/details/internal/ui/shared/DetailFieldRow.tsx](../../react/src/modules/details/internal/ui/shared/DetailFieldRow.tsx) | 复用原生 flex 布局与 Input 标签 | 日志元信息使用有界标签/值列与只读输入，长内容可选择和水平查看；无需另建 CSS wrapper 层 | 日志已接入；其他消费者随功能验收 |
| [modules/details/internal/ui/shared/DetailForm.tsx](../../react/src/modules/details/internal/ui/shared/DetailForm.tsx) | 只读字段复用 Input，长文本复用 Textarea | 日志原值可选择复制；可编辑 DetailCommitInput 的 Enter/失焦提交与 Escape 恢复须由各参数 owner 继续核对 | 只读日志已接入；编辑提交语义待参数批次 |
| [modules/details/internal/ui/shared/DetailPanelShell.tsx](../../react/src/modules/details/internal/ui/shared/DetailPanelShell.tsx) | 复用既有 Details 滚动容器 | 日志详情在原面板展示，沿用根 DockArea 的位置和尺寸，不加入第二套布局或 ScrollArea 包装 | 已采用；跨面板交互待验收 |
| [modules/details/internal/ui/shared/DetailText.tsx](../../react/src/modules/details/internal/ui/shared/DetailText.tsx) | 复用 ActiveTheme、字体与只读原生文本 | 日志级别/领域、元信息和正文来自原记录；原生主题替代 CSS tone，全文按纯文本显示 | 日志已接入；其他消费者随功能验收 |
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
| [modules/logs/internal/ui/LogDomainLayoutHost.tsx](../../react/src/modules/logs/internal/ui/LogDomainLayoutHost.tsx) | 复用根 DockArea；不迁入第二个 FlexLayout 拓扑 | 当前用原生领域选择器访问全部及六个领域；原多领域并列对照的承载仍待处理 | 领域筛选已接入；并列布局待迁移 |
| [modules/logs/internal/ui/LogDomainPanel.tsx](../../react/src/modules/logs/internal/ui/LogDomainPanel.tsx) | 迁移领域过滤、选择与跟随到已有 LogsPanel | 同一有界集合和筛选索引；显式选择显示既有 Details，清空仅撤销对应日志检查 | 代码已接入；多领域并列承载另行处理 |
| [modules/logs/internal/ui/LogItemRow.tsx](../../react/src/modules/logs/internal/ui/LogItemRow.tsx) | 复用固定高度原生行，按需检查与复制 | 列表高亮单选并支持键盘导航；右键复制消息/完整记录，部分文本选择在原 Details 只读控件完成，不为每条行创建输入实体 | 代码已接入；完整交互验收待完成 |
| [modules/logs/internal/ui/LogPanelList.tsx](../../react/src/modules/logs/internal/ui/LogPanelList.tsx) | 复用 UniformList 与已有空状态 | 计数和列表共享筛选索引，区分加载、无记录、无匹配及失败 | 代码已接入；真实交互验收待完成 |
| [modules/logs/internal/ui/LogPanelStatus.tsx](../../react/src/modules/logs/internal/ui/LogPanelStatus.tsx) | 迁移连接/计数/截断提示 | 同一 LogsPanel 展示当前过滤/总数、连接状态和截断警告；存储失败停止交付 | 代码已接入；故障验收待完成 |
| [modules/logs/internal/ui/LogPanelToolbar.tsx](../../react/src/modules/logs/internal/ui/LogPanelToolbar.tsx) | 复用 Input、Button、PopupMenu | 刷新、自动跟随、领域、五种级别、搜索与清空显示；保留领域所有者和序列水位 | 代码已接入；真实交互验收待完成 |
| [modules/logs/internal/ui/LogPanelVirtualList.tsx](../../react/src/modules/logs/internal/ui/LogPanelVirtualList.tsx) | 复用原生 UniformList 与滚动句柄 | 只绘制可见行；按参考 80px 阈值跟随，浏览旧记录和前缀裁剪保持视口，不增加 DOM 虚拟化层 | 代码已接入；滚动验收待完成 |
| [modules/logs/internal/ui/LogWindow.tsx](../../react/src/modules/logs/internal/ui/LogWindow.tsx) | 复用既有原生日志窗口与 LogsPanel | 当前工作区已有单例句柄和独立订阅；共享 LogsPanel 获得新工具栏，窗口接入提交、延后创建/激活与跨窗口详情仍待核对 | 工作区已有实现；窗口生命周期优化待处理 |
| [modules/logs/internal/ui/LogWorkspaceActions.tsx](../../react/src/modules/logs/internal/ui/LogWorkspaceActions.tsx) | 并入已有面板工具栏和状态条 | 同一实体协调原生控件，日志批次只增量更新有界显示索引，无额外业务控制器 | 代码已接入；人工验收待完成 |
| [modules/logs/internal/ui/logWorkspaceContext.tsx](../../react/src/modules/logs/internal/ui/logWorkspaceContext.tsx) | React Context 无需迁移 | 操作与暂态状态归 LogsPanel；记录、持久化及序列归 LogRuntime，跨窗口各有独立输入实体 | 已归并；无需独立适配层 |

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
| [modules/project-explorer/internal/ui/picker/DeleteProjectConfirmDialog.tsx](../../react/src/modules/project-explorer/internal/ui/picker/DeleteProjectConfirmDialog.tsx) | 无需迁移已移除的项目回收站入口 | 当前项目入口只提供打开/新建/另存为/关闭；没有项目文件删除动作，不新增确认状态或删除授权 | 已审查；无需对应原生组件 |
| [modules/project-explorer/internal/ui/picker/NewProjectModal.tsx](../../react/src/modules/project-explorer/internal/ui/picker/NewProjectModal.tsx) | 复用原生项目表单并接入进度 | projects/form 使用路径组件校验、系统目录选择、原生命周期与部分提交恢复；提交保留输入，失败返回同一表单；默认目录只回填未编辑父目录 | 代码已覆盖；真实交互待验收 |
| [modules/project-explorer/internal/ui/picker/ProjectLibrary.tsx](../../react/src/modules/project-explorer/internal/ui/picker/ProjectLibrary.tsx) | 以欢迎页和最近项目替代独立项目库 | projects/recent 直接消费 registry 记录，ListState 拥有搜索/键盘/虚拟列表；选择校验原记录与代次，不复刻收藏和排序草稿 | 替代入口及重试已实现；真实交互待验收 |
| [modules/project-explorer/internal/ui/picker/ProjectPickerActionPanel.tsx](../../react/src/modules/project-explorer/internal/ui/picker/ProjectPickerActionPanel.tsx) | 无需独立操作侧栏 | 保留的新建/打开/最近入口由 welcome、文件菜单与项目表单承接；扫描/清理/收藏/回收站已不在当前流程 | 已审查；无需对应原生组件 |
| [modules/project-explorer/internal/ui/picker/ProjectPickerChrome.tsx](../../react/src/modules/project-explorer/internal/ui/picker/ProjectPickerChrome.tsx) | 复用主窗口标题栏与共享设置 | 欢迎页和工作台沿用 window_chrome/menus/settings，不另建 Picker 标题与语言 owner；主题能力在设置批次继续核对 | 结构已复用；主题及窗口交互待验收 |
| [modules/project-explorer/internal/ui/picker/ProjectPickerFeedbackDetails.tsx](../../react/src/modules/project-explorer/internal/ui/picker/ProjectPickerFeedbackDetails.tsx) | 复用原生反馈并完善失败分类 | projects/feedback 直接分类原 Application 错误与 Project code，保留 recovery_required 和部分提交指导；原恢复路径可查看/复制并沿原入口打开，不显示诊断正文 | 代码已覆盖；真实失败与恢复待验收 |
| [modules/project-explorer/internal/ui/picker/ProjectPickerPageIssueAlert.tsx](../../react/src/modules/project-explorer/internal/ui/picker/ProjectPickerPageIssueAlert.tsx) | 按当前入口迁移失败与重试 | 欢迎页/最近弹窗复用原读取重试，错误按当前语言呈现；旧记录不能绕过失败状态；扫描/清理空结果已无入口 | 重试及生命周期原因已接入；真实交互待验收 |
| [modules/project-explorer/internal/ui/picker/ProjectPickerScreen.tsx](../../react/src/modules/project-explorer/internal/ui/picker/ProjectPickerScreen.tsx) | 以现有工作台欢迎页替代页面容器 | Workbench 直接路由类型化项目操作，最近记录/输入/提交分别由原 owner 管理，不复制路由页面与 UIStore 全局进度 | 结构替代已实现；子流程缺口分别记录 |
| [modules/project-explorer/internal/ui/picker/projectPickerContextMenu/buildProjectPickerContextMenuSections.tsx](../../react/src/modules/project-explorer/internal/ui/picker/projectPickerContextMenu/buildProjectPickerContextMenuSections.tsx) | 无需迁移已移除的项目库菜单 | 保留的打开/创建在当前 welcome 和文件菜单中；资源侧栏菜单为其他组件，不借此恢复项目库管理动作 | 已审查；无需对应原生组件 |

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
| [modules/results/internal/ui/panel/ResultContent.tsx](../../react/src/modules/results/internal/ui/panel/ResultContent.tsx) | 迁移加载、失败与类型化内容 | results/reading/render 复用自动租约并拒绝关闭后的迟到回复；已接纳内容独立于普通重跑 | 面板与独立展开已接入；交互待验收 |
| [modules/results/internal/ui/panel/ResultInspector.tsx](../../react/src/modules/results/internal/ui/panel/ResultInspector.tsx) | 迁移数值/报告切换并复用同一实体 | ResultPanel 保留报告实体和局部章节状态；显式追加完整替换同一引用下的数值与报告 | 代码已覆盖；切换与追加验收待完成 |
| [modules/results/internal/ui/panel/ResultPanel.tsx](../../react/src/modules/results/internal/ui/panel/ResultPanel.tsx) | 复用原生 ResultPanel 与 DockArea | 面板只拥有显示状态和 Application 租约；移动重挂载保留同一实体，实际关闭释放租约 | 代码已覆盖；生命周期验收待完成 |
| [modules/results/internal/ui/plot/PlotWindow.tsx](../../react/src/modules/results/internal/ui/plot/PlotWindow.tsx) | 迁移原生独立结果窗口 | 结果标签工具栏打开原生窗口，直接复用 ResultPanel/PlotView；交接保留原租约，每窗独立控件，原会话结束关闭 | 已接入，人工验收待完成 |
| [modules/results/internal/ui/source-inspector/SourceInspectorWindow.tsx](../../react/src/modules/results/internal/ui/source-inspector/SourceInspectorWindow.tsx) | 迁移原生独立结果窗口 | 复用数值/报告、分页、失败重试和追加流程；重复打开读取窗口当前引用，来源关闭不回收窗口结果，无 Tauri URL | 已接入，人工验收待完成 |

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
| [shared/ui/PageAlert.tsx](../../react/src/shared/ui/PageAlert.tsx) | 复用原生通知与所属表单反馈 | 原文件组合标题/详情、语义图标、操作和关闭；项目错误/重试由现有 owner 承接，其他调用方随自身迁移，不新建通用告警状态 | 项目调用方已接入；真实交互待验收 |
| [shared/ui/ProgressOverlay.tsx](../../react/src/shared/ui/ProgressOverlay.tsx) | 迁移当前项目操作进度 | projects/progress 与原 worker 步骤交付；共享阶段/目标、未知进度和保存前置提示；无取消契约的流程不显示取消，扫描/清理入口已移除 | 代码已覆盖当前流程；真实交互待验收 |
| [shared/ui/Select.tsx](../../react/src/shared/ui/Select.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/ToolbarIconButton.tsx](../../react/src/shared/ui/ToolbarIconButton.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/WindowChrome.tsx](../../react/src/shared/ui/WindowChrome.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/WindowChromeControls.tsx](../../react/src/shared/ui/WindowChromeControls.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/WindowTitleBar.tsx](../../react/src/shared/ui/WindowTitleBar.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/actionMenu/ActionMenu.tsx](../../react/src/shared/ui/actionMenu/ActionMenu.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
| [shared/ui/markdownRendering.tsx](../../react/src/shared/ui/markdownRendering.tsx) | 待查 | 待逐项阅读源码 | 待审查 |
