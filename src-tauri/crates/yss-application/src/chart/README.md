# Chart application

> Status: Current
> Scope: 图表资源操作、预览投影、保存和前端配置草稿的交付契约
> Canonical owners: 本模块编排图表用例；Project 拥有持久化和资源版本，前端拥有配置草稿
> Update when: 图表查询、保存、预览或版本身份改变时

## 资源与预览

[resources.rs](resources.rs) 协调 Project 的图表文档操作；[query.rs](query.rs) 捕获及重验项目、数据库版本；[projection.rs](projection.rs) 只接收数值和轴格式并生成有界绘图点，不访问会话或执行器。采样保留原有步长规则，输出点数组受请求上限约束，不再先复制全部有效点。

独立图表的 `ChartPreview` 与图结果的 `PlotResultView` 复用现有 `ChartRenderer`。图表预览按项目、路径、声明和 Rust 投影的数据库 revision 缓存；配置变更或视图离开后，旧请求不再更新该视图。图结果仍由 ResultStore 和报告租约管理。真实界面的切换、编辑与迟到回执验收见[组件计划](../../../../../docs/roadmap/COMPONENT_REFACTOR.md)。

## 保存与版本

Chart Save 按提交的完整内容覆盖资源，不接受 frontend `expectedRevision`。`ChartDocument` 和图表文件只保存图表配置与格式版本，不携带资源 `revision`；资源版本由 Rust Project 单独管理。

前端用 operation ID 和资源路径确认保存回执，通过文档内容判断保存期间是否产生新编辑，成功才清除 dirty。干净图表的刷新依据资源索引；为索引加载文档时，读取请求绑定同一 Project publication revision，由 Rust 校验快照一致性。重命名、删除等资源操作和 Rust 内部事务继续校验资源版本。

资源持久化见 [Project](../../../yss-project/README.md)，结果持有与读取见 [Results](../../../../../src/features/application/results/README.md)。
