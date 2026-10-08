# Chart application

> Status: Current
> Scope: 图表资源操作、预览投影、保存和前端配置草稿的交付契约
> Canonical owners: 本模块编排图表用例；Project 拥有持久化和资源版本，前端拥有配置草稿
> Update when: 图表查询、保存、预览或版本身份改变时

## 资源与预览

[resources.rs](resources.rs) 协调 Project 的图表文档操作；[query.rs](query.rs) 捕获及重验项目、数据库版本；[projection.rs](projection.rs) 只接收数值和轴格式并生成有界绘图点，不访问会话或执行器。采样保留原有步长规则，输出点数组受请求上限约束，不再先复制全部有效点。

复制的可选名称直接交给 Project writer，在同一次文件事务中分配实际名称和新路径；GUI 未指定时沿用默认副本命名。

[原生宿主](../../../yss-desktop-gpui/README.md)直接消费本模块的完整图表文档、原数据库分布和有界列对，
使用 GPUI 绘制独立直方图、散点与折线。配置草稿及绘图坐标由原生视图拥有，保存、资源版本、分布和数值投影仍由 Rust 原 owner 拥有。
预览绑定项目、配置与数据库资源 revision；旧查询不能安装到新配置，目录重命名/删除继续校验资源版本。
原生操作与验收范围见宿主 README，完整图结果图形仍待迁移；不能以独立图表通过替代结果租约与报告验收。
保留的 React 参考实现中，`ChartPreview` 与 `PlotResultView` 复用 `ChartRenderer`，不参与当前原生构建。

直方图列分布与散点/折线列对读取都携带前端捕获的数据库资源 revision；没有该版本时不发出查询。
原生直方图显式传入所选列，Application/Runtime 在统计之前使用原 Engine 列投影，保留分箱、类别排序与读取 gate。
图表预览可以借用已接纳且数据库 ID/revision 匹配的元数据；原数据读取和返回版本重验仍由 Application 执行。
列对查询按数据库 ID 复用 Project 授权校验，检查 Runtime 列对的声明版本，并在投影完成后重验，
不为单个数据库查询构造完整项目索引。Application 会话检查继续保留；前端返回接纳和缓存失效仍按原读取身份处理。

## 保存与版本

GUI 的独立 Chart Save 按提交的完整内容覆盖资源，不接受 frontend `expectedRevision`。
Harness 的图表设置编辑携带读取时的资源 revision，Application 将它交给同一 Project writer，
在捕获事务基线时比较并在提交处重验，拒绝覆盖期间发生的新修改。
Harness 的 `inspect_chart` 返回当前类型化配置；`update_chart` 只合并显式设置的字段，再通过上述 writer 立即持久化。
省略字段保持原值，轴的 null 清空对应配置，空 databaseId 断开数据源；这两个工具不触发数据计算或图表渲染。
`ChartDocument` 和图表文件只保存图表配置与格式版本，不携带资源 `revision`；资源版本由 Rust Project 单独管理。
`yss-chart-document::ChartType` 是图表类型的唯一 Rust 定义；文档、Project 索引和变更状态、Harness 设置
共用 histogram/scatter/line 枚举及其序列化，Application 不再维护字符串与类型之间的映射。

前端用 operation ID 和资源路径确认保存回执，通过文档内容判断保存期间是否产生新编辑，成功才清除 dirty。干净图表的刷新依据资源索引；GUI 初次视图读取和索引刷新都必须携带捕获的 Project publication revision，由 Rust 校验快照一致性。前端在途请求也以发布版本区分，返回时保留原项目、资源修订及读取令牌检查。Application 的原生/Harness 读取仍可使用已捕获会话的可选版本入口。重命名、删除等资源操作和 Rust 内部事务继续校验资源版本。

前端 Chart Service 在 IPC 边界检查当前 schema version 4，并复用资源回执的 Chart 状态 guard。
列对响应的坐标必须是有限数值，轴格式为 number/date/datetime，两个轴标签必填且可为 null；
显式传入点数上限时，响应不能超过该上限。模型转换只将 null 标签转为呈现层的缺省值，不重复校验。

资源持久化见 [Project](../../../yss-project/README.md)，结果持有与读取见 [Results](../../../../react/src/features/application/results/README.md)。
