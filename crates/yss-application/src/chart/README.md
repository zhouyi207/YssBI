# Chart application

> Status: Current
> Scope: 图表预览投影、Harness 图表设置和前端配置草稿的交付契约
> Canonical owners: 本模块编排图表用例；Project 拥有持久化和资源版本，前端拥有配置草稿
> Update when: 图表查询、保存、预览或版本身份改变时

## 资源与预览

图表文件的读取、编辑、保存、复制、重命名和删除与 Mind/Doc 共用 [file_resources.rs](../file_resources.rs)
的 `read_file`/`apply_file_command`；本模块只负责预览。[query.rs](query.rs) 捕获及重验项目、数据库版本；[projection.rs](projection.rs) 只接收数值和轴格式并生成有界绘图点，不访问会话或执行器。采样保留原有步长规则，输出点数组受请求上限约束，不再先复制全部有效点。

复制的可选名称直接交给 Project 文件命令，在同一次文件事务中分配实际名称和新路径；GUI 未指定时沿用默认副本命名。

[原生宿主](../../../yss-desktop-gpui/README.md)直接消费本模块的完整图表文档、原数据库分布和有界列对，
使用 GPUI 绘制独立直方图、散点与折线。配置草稿及绘图坐标由原生视图拥有，保存、资源版本、分布和数值投影仍由 Rust 原 owner 拥有。
预览绑定项目、配置与数据库资源 revision；旧查询不能安装到新配置，目录重命名/删除继续校验资源版本。
原生操作与验收范围见宿主 README，完整图结果图形仍待迁移；不能以独立图表通过替代结果租约与报告验收。

直方图列分布与散点/折线列对读取都携带前端捕获的数据库资源 revision；没有该版本时不发出查询。
原生直方图显式传入所选列，Application/Runtime 在统计之前使用原 Engine 列投影，保留分箱、类别排序与读取 gate。
图表预览可以借用已接纳且数据库 ID/revision 匹配的元数据；原数据读取和返回版本重验仍由 Application 执行。
列对查询按数据库 ID 复用 Project 授权校验，检查 Runtime 列对的声明版本，并在投影完成后重验，
不为单个数据库查询构造完整项目索引。Application 会话检查继续保留；前端返回接纳和缓存失效仍按原读取身份处理。

## 保存与版本

GUI 保存把配置草稿作为 `ChartEdit::SetDocument` 提交到捕获的文件版本，再执行 Save；
文件在磁盘上被外部修改或版本已变化时保存被拒绝，草稿保留，由编辑器的冲突状态决定重新载入或保留草稿后覆盖。
Harness 的 `inspect_chart` 返回当前类型化配置和文件版本（revision 与文件会话）；`update_chart` 校验读取时的版本，
只合并显式设置的字段，以同样的 Edit 与 Save 立即持久化，两次提交都会发布资源事件。
省略字段保持原值，轴的 null 清空对应配置，空 databaseId 断开数据源；这两个工具不触发数据计算或图表渲染。
`ChartDocument` 和图表文件只保存图表配置与格式版本，不携带资源 `revision`；资源版本由 Project 的文件状态管理。
`yss-chart-document::ChartType` 是图表类型的唯一 Rust 定义；文档和 Harness 设置共用 histogram/scatter/line
枚举及其序列化，Application 不再维护字符串与类型之间的映射。

编辑器从文件快照开始，资源事件按修订号决定是否重新读取；自身保存回执已包含的修订不会再读。
数据集目录由工作台已接纳的项目索引提供，图表读取不再为单个文件查询完整项目索引。

图表格式与字段校验由 `yss-chart-document` 和 Project 拥有，原生宿主直接消费类型化文档和查询结果。
列对投影只交付有限坐标及 number/date/datetime 轴格式，可选轴标签由呈现层显示；
显式传入点数上限时，Application 的有界采样遵守该上限。

资源持久化见 [Project](../../../yss-project/README.md)，结果持有与读取见 [Graph 与 Results](../graph/README.md)，原生预览与结果呈现见 [GPUI host](../../../yss-desktop-gpui/README.md)。
