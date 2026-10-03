# Chart application

> Status: Current
> Scope: 图表资源操作、预览投影、保存和前端配置草稿的交付契约
> Canonical owners: 本模块编排图表用例；Project 拥有持久化和资源版本，前端拥有配置草稿
> Update when: 图表查询、保存、预览或版本身份改变时

## 资源与预览

[resources.rs](resources.rs) 协调 Project 的图表文档操作；[query.rs](query.rs) 捕获及重验项目、数据库版本；[projection.rs](projection.rs) 只接收数值和轴格式并生成有界绘图点，不访问会话或执行器。采样保留原有步长规则，输出点数组受请求上限约束，不再先复制全部有效点。

独立图表的 `ChartPreview` 与图结果的 `PlotResultView` 复用现有 `ChartRenderer`。图表预览按项目、路径、声明和 Rust 投影的数据库 revision 缓存；配置变更或视图离开后，旧请求不再更新该视图。图结果仍由 ResultStore 和报告租约管理。真实界面的切换、编辑与迟到回执验收见[组件计划](../../../../../docs/roadmap/COMPONENT_REFACTOR.md)。

直方图列分布与散点/折线列对读取都携带前端捕获的数据库资源 revision；没有该版本时不发出查询。
列对查询按数据库 ID 复用 Project 授权校验，检查 Runtime 列对的声明版本，并在投影完成后重验，
不为单个数据库查询构造完整项目索引。Application 会话检查继续保留；前端返回接纳和缓存失效仍按原读取身份处理。

## 保存与版本

GUI 的独立 Chart Save 按提交的完整内容覆盖资源，不接受 frontend `expectedRevision`。
Harness 的图表设置编辑携带读取时的资源 revision，Application 将它交给同一 Project writer，
在捕获事务基线时比较并在提交处重验，拒绝覆盖期间发生的新修改。
`ChartDocument` 和图表文件只保存图表配置与格式版本，不携带资源 `revision`；资源版本由 Rust Project 单独管理。
`yss-chart-document::ChartType` 是图表类型的唯一 Rust 定义；文档、Project 索引和变更状态、Harness 设置
共用 histogram/scatter/line 枚举及其序列化，Application 不再维护字符串与类型之间的映射。

前端用 operation ID 和资源路径确认保存回执，通过文档内容判断保存期间是否产生新编辑，成功才清除 dirty。干净图表的刷新依据资源索引；GUI 初次视图读取和索引刷新都必须携带捕获的 Project publication revision，由 Rust 校验快照一致性。前端在途请求也以发布版本区分，返回时保留原项目、资源修订及读取令牌检查。Application 的原生/Harness 读取仍可使用已捕获会话的可选版本入口。重命名、删除等资源操作和 Rust 内部事务继续校验资源版本。

前端 Chart Service 在 IPC 边界检查当前 schema version 4，并复用资源回执的 Chart 状态 guard。
列对响应的坐标必须是有限数值，轴格式为 number/date/datetime，两个轴标签必填且可为 null；
显式传入点数上限时，响应不能超过该上限。模型转换只将 null 标签转为呈现层的缺省值，不重复校验。

资源持久化见 [Project](../../../yss-project/README.md)，结果持有与读取见 [Results](../../../../../src/features/application/results/README.md)。
