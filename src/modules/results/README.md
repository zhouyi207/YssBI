# Results views

> Status: Current
> Scope: Result 面板、报告组件和语义页面呈现
> Canonical owners: 本模块拥有呈现；查询与租约由 Application results 维护，数值由 Rust ResultStore 拥有
> Update when: 结果视图、报告渲染或页面布局接入改变时

[public.ts](public.ts) 提供结果面板入口，报告组件位于 [internal/ui/info](internal/ui/info/)。
读取、分页、分析和结果生命周期见 [Results application](../../features/application/results/README.md)；
组件目录、受控动作与增量协议见 [UI contract](../../../src-tauri/crates/yss-ui-contract/README.md)。

## 线性回归语义报告布局

内容选择与检验参数属于 Summary 节点的 Parameters → Configure。报告的“添加内容”只提交新增选项，调用 Application 更新同一份节点参数并执行；忙碌或失败期间继续显示原报告。已选检验直接展示本次执行结果，图表缩放、分页和布局显隐仅影响查看。报告布局只能绑定本次已选内容，不能通过布局导入启用额外计算。

线性回归报告接入 Rust 拥有的 JSON 页面。GUI 的排序、显隐、重置、导入和 Harness 修改共用 Application，前端通过快照与稳定元素增量呈现容器、文本、报告章节及受控按钮。
页面结构、组件目录、修订校验、回执和会话恢复见 [JSON 页面与界面意图](../../../src-tauri/crates/yss-ui-contract/README.md)；不再维护独立的前端报告布局权威或旧 Spec 转换。

通用展示组件集中在 [ui-presentation](../../components/ui-presentation/)：Section、Equation、KeyValue、DataTable、StatCard、CoefficientTable 和 Chart 各自按文件维护，表格框架、公式映射和控件就近复用。组件只接收展示数据；查询、参数和结果生命周期保留在 Application results 与报告协调组件。其他报告直接使用同一份系数表、公式和卡片实现。

通用组件与数值格式函数从各自 owner 直接导入，报告 `shared` 只组织报告专用组合。公式映射表自行使用统一数值格式；假设检验的公式转换留在检验组件内部。

UiPageRenderer 按组件类型呈现 `presentation` 中的键值条目、列和行、指标；分页、公式、图表及分析通过稳定的 Results 绑定接入。章节组合由 Rust 模板拥有，前端不再逐个分发 ModelSummary、ANOVA 等章节枚举。Results 与查询协调器拥有统计值和能力校验，数值不来自 Spec；折叠区展开后才挂载查询组件。
系数分页由报告内的窄 Context 承载，系数表、系数图和公式订阅同一页；系数图与表格在模板中独立组合，隐藏元素仍保留页码。其他章节以稳定数据和绑定隔离渲染，布局忙碌状态及系数翻页不重建无关统计图和检验区。分页快照与图表输入保持引用稳定，页面 Hook 只订阅项目身份。
布局编辑文本保留在前端，完整有效配置经后端提交后才安装。页面绑定当前结果引用，不创建新的数据所有者或结果租约。
同一 Application session 内重开恢复页面；项目或执行会话结束后不跨结果重绑定。隐藏章节不释放真实面板的结果租约，保留快照的失效与回收仍遵循 Results 契约。
模板落盘、更多页面类型和人工验收仍由 [JSON Driver 计划](../../../docs/roadmap/jsonDriver.md) 跟踪。
