# Results views

> Status: Current
> Scope: Result 面板、报告组件和语义页面呈现
> Canonical owners: 本模块拥有呈现；查询与租约由 Application results 维护，数值由 Rust ResultStore 拥有
> Update when: 结果视图、报告渲染或页面布局接入改变时

结构化报告的数组以数据引用呈现，无论数组大小。默认只展示行数，用户展开“查看数据”
才通过现有 Results 查询协调器读取有界数据页；折叠时卸载读取组件。概览、分页与
嵌套数据使用同一结果引用和租约，不新建数据存储，也不自动读取引用中的数组。

[public.ts](public.ts) 提供结果面板入口，报告组件位于 [internal/ui/info](internal/ui/info/)。
读取、分页、分析和结果生命周期见 [Results application](../../features/application/results/README.md)；
组件目录、受控动作与增量协议见 [UI contract](../../../src-tauri/crates/yss-ui-contract/README.md)。

## Inspect 与 JSON 报告

可视化节点通过同一 Result 引用与租约提供 `plot.data`。19 类图形共用
`PlotResultView`、严格 payload parser、`ChartRenderer` 和现有 D3 主题/尺寸适配。
散点/折线还消费参考线与气泡面积；箱线/小提琴、区间、柱线、矩阵和词云由对应
renderer 绘制。前端不计算统计量、核密度、AUC 或置信区间；抽样与总体统计来自 Rust。
计算后的 ROC 曲线直接绘制，普通折线保留自身的显示控制。

统计节点只输出一份结构化 `result`。Result 面板与独立 Inspect 窗口共用右上角的“数值 / 报告”切换，默认查看数值。两种视图绑定同一个结果引用和租约，切换不重新执行节点；已经打开的报告保持挂载，保留布局、分页和显隐状态。独立报告也使用 `/inspect`，不再注册 `/info` 或按统计方法分派专用页面。

所有报告使用 `ResultReportPage` 和 `UiPageRenderer`。普通结果的 JSON 页面绑定 `structured` 组件，由它使用通用键值表、数据表和可折叠章节呈现对象、数组及矩阵；数组保留为结果引用，展开后由 `StructuredResult` 每次请求至多 100 项的有界数据页，嵌套数组继续按需读取，空值、布尔值与宽整数文本保留原意。它不推断统计方法或执行计算。

线性回归的 `LinearResultBindings` 只接入原生结果的系数、观测、图形和分析查询，页面组合仍由同一个 JSON renderer 完成。其余统计结果不要求符合旧的专用报告载荷。

## 页面布局与数据绑定

内容选择与检验参数属于 Summary 节点的 Parameters → Configure。报告的“添加内容”只提交新增选项，调用 Application 更新同一份节点参数并执行；忙碌或失败期间继续显示原报告。已选检验直接展示本次执行结果，图表缩放、分页和布局显隐仅影响查看。报告布局只能绑定本次已选内容，不能通过布局导入启用额外计算。

线性回归报告接入 Rust 拥有的 JSON 页面。GUI 的排序、显隐、重置、导入和 Harness 修改共用 Application，前端通过快照与稳定元素增量呈现容器、文本、报告章节及受控按钮。
页面结构、组件目录、修订校验、回执和会话恢复见 [JSON 页面与界面意图](../../../src-tauri/crates/yss-ui-contract/README.md)；不再维护独立的前端报告布局权威或旧 Spec 转换。

通用展示组件集中在 [ui-presentation](../../components/ui-presentation/)：Section、Equation、KeyValue、DataTable、StatCard、CoefficientTable 和 Chart 各自按文件维护，表格框架、公式映射和控件就近复用。组件只接收展示数据；查询、参数和结果生命周期保留在 Application results 与报告协调组件。

通用组件与数值格式函数从各自 owner 直接导入，报告 `shared` 仅保留线性报告使用的 ACF/PACF、序列相关和假设检验结果视图。检验选项由 Summary 配置，视图通过结果引用读取已计算值，不再保留内联样本计算表单或按统计方法拆分的旧报告组件。公式映射表自行使用统一数值格式；假设检验的公式转换留在检验组件内部。系数表、系数图和公式只消费当前线性系数页，不保留无调用方的分类列、赔率比、z 统计量或 AR(1) 展示选项。

UiPageRenderer 按组件类型呈现 `presentation` 中的键值条目、列和行、指标；分页、公式、图表及分析通过稳定的 Results 绑定接入。章节组合由 Rust 模板拥有，前端不再逐个分发 ModelSummary、ANOVA 等章节枚举。Results 与查询协调器拥有统计值和能力校验，数值不来自 Spec；折叠区展开后才挂载查询组件。
系数分页由报告内的窄 Context 承载，系数表、系数图和公式订阅同一页；系数图与表格在模板中独立组合，隐藏元素仍保留页码。其他章节以稳定数据和绑定隔离渲染，布局忙碌状态及系数翻页不重建无关统计图和检验区。分页快照与图表输入保持引用稳定，页面 Hook 只订阅项目身份。
布局编辑文本保留在前端，完整有效配置经后端提交后才安装。页面绑定当前结果引用，不创建新的数据所有者或结果租约。
同一 Application session 内重开恢复页面；项目或执行会话结束后不跨结果重绑定。隐藏章节不释放真实面板的结果租约，保留快照的失效与回收仍遵循 Results 契约。
模板落盘、更多页面类型和人工验收仍由 [JSON Driver 计划](../../../docs/roadmap/jsonDriver.md) 跟踪。

结构化统计报告还可消费后端的 `report_display.sections` 展示元数据：具名表格、
纯文本方程和稳定性单位圆沿用同一 UiPageRenderer、结果引用及有界分页，不恢复
按统计方法拆分的旧 React 报告链。数值、系数变换、边际效应、特征根和密度均由
后端计算。单位圆显示当前分页的根，超过一页时明确提示，不能据局部图代替完整
稳定性结论。原始结构化值仍可折叠查看。
原生线性报告的诊断章节读取执行时已计算的检验和 leverage 密度；残差图支持
相邻残差坐标及按全样本 leverage 排序的高亮，前端只控制查看参数。
