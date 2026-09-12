# yss-project

> Status: Current
> Scope: Project runtime authority、资源 revision、持久化与 publication 边界
> Canonical owners: 本 crate 的源码与测试拥有可执行事实；Graph 生命周期由 Graph 架构文档维护
> Update when: Project authority、revision 使用者、事务或 publication 边界改变时

`yss-project` 是项目运行期权威状态的唯一 owner。它负责 `ProjectState`、项目会话与实例身份、资源 revision、持久化事务，以及磁盘提交后的 publication。

该 crate 组合 `yss-project-model`、`yss-project-history`、`yss-project-operation`、`yss-resource-lifecycle` 与 `yss-project-filesystem` 等更低层 crate，但不依赖 Tauri、Commands、IPC schema、Application 工作流或 Database runtime。

边界约束：

- `ProjectData` 是 resident project facts 的权威聚合；
- 项目默认名称与名称规范化由 `yss-project-model` 统一拥有，创建、注册和空项目模型共用同一规则；
- `yss-project-filesystem` 只拥有安全文件系统原语，`yss-project` 拥有 session/revision 校验与 publication；
- 对外返回 Project-owned typed facts，由 Application 和 API 层投影为事件与 DTO；
- `test-support` 只暴露跨 crate 测试所需的 fixture 与故障注入 seam。

项目元数据的 `exportTime` 使用不带时区的本机钟面时间。Manifest 构造与读取时遇到
旧的带偏移日期时间会保留其原日期、钟面和小数精度并去掉偏移，不换算到另一个时区。
日历展示不再依赖浏览器的本地时区转换；registry 的 Unix 秒计数保持数值时间点语义。

Graph 撤销/重做由前端 Graph Draft 管理，数据库编辑历史由 Database runtime 管理。Project 提交发布资源版本和 delta，不维护项目级撤销栈。`yss-project-history` 保留共享的资源身份、变更请求、函数文档、delta、错误及图驻留状态契约；文件事务回滚与失败恢复继续由 Project 和 filesystem owner 负责。

## Graph resource revisions

项目格式版本以 [`CURRENT_PROJECT_SCHEMA_VERSION`](src/manifest.rs) 为准。命名常量随 Event/Function 的 `GraphDocument.constants` 保存，属于 Graph 资源事务，不再维护全局变量、变量 revision、作用域迁移或 `variables.yssbi-vars` 的日常读写。

项目尚未发布，只支持当前格式。打开其他格式版本的项目会在 manifest 校验时失败，不执行旧变量资源或节点的兼容转换，也不改写原文件。新建和保存项目继续写入当前 `schemaVersion`。

常量增删改使用 Graph Draft、Save 与图历史，项目查询不再发布独立变量集合。复制图时常量及 Get 引用同时生成新身份。格式、类型和执行语义由 [Graph 与 Execution](../../../docs/architecture/GRAPH_AND_EXECUTION.md) 维护。

`graph_resource_revisions` 是 Project-owned `GraphResourcePath → ResourceRevision` 索引，不是 editor projection 的请求计数器。它仍有生产读写方：

| 使用方                                                                                                                           | 作用                                                                                       |
| -------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| [Project activation](src/project_activation.rs) 与 [Graph lifecycle](src/project_state/graph_lifecycle.rs)                       | 安装活动资源的 revision，校验 rename 等操作捕获的资源版本，并在提交后更新 identity/version |
| [Graph document commit](src/project_state/graph_operation.rs)                                                                    | 与 committed document 的更新一起推进资源 revision                                          |
| [Function mutation](src/project_state/function_mutation.rs) 与 [resource publication](src/project_state/resource_publication.rs) | 检查提交身份、预期版本与 patch before-state，按已提交变更推进资源版本                      |
| [Execution authority](src/execution_authority.rs)                                                                                | 将 Graph resource revision 映射为执行资源 version/grant，参与资源 currentness 校验         |

因此当前不能直接删除此索引。移除 Graph Projection Channel 仅移除了那条传输链的 request-generation 协调；Save 不再接受 frontend `expectedRevision`，也没有移除 Rust 事务内部的版本校验。

以下字段不能互相替代：`ResourceRevision` 标识已提交资源版本；frontend lifecycle token 拒绝旧 editor 请求；Draft document 是未保存意图；compiled source hash 标识语义内容与 catalog 对应的 artifact。单独的 revision 也不能替代 Project instance/session identity。若以后合并 revision 的存储位置，必须同时迁移以上使用方，保持提交前重验与事务/执行资源校验语义。

Graph Draft、Compile、Save 和 Execute 的当前流程见 [Graph 与 Execution](../../../docs/architecture/GRAPH_AND_EXECUTION.md)。

## Resource publication and external files

Event/Function 创建、复制、删除、重命名复用标准 ProjectDataPatch 的提交与 lifecycle/move delta；资源操作推进
publication revision，而不仅是 authority generation。纯生命周期增删不伪造缺失的图投影，
重命名回执保留真实 move delta、递增发布版本及受影响图的恢复声明。

Graph 与 Chart writers 共用 WriterSnapshot 和 transaction context。取得文件系统 lease 后与暂存完成后，
均检查捕获的 authority generation、受影响资源版本以及源/目标路径存在性；重命名复用相同的 ownership lease
和 patch 发布入口。函数签名的 before-state、函数版本与所属 Graph 版本也在事务内校验，文件落盘成功后才发布，
发布失败则回滚文件。Graph revision 不随驻留状态重置：卸载/重新加载保留版本，删除与移动后的旧路径保留 tombstone。

Watcher 的 rescan 在 filesystem lease 下读取文件，重验项目身份后同步驻留 graph/chart，
预先校验全部版本推进，再发布变更。无内容变化的重复 rescan 不再次推进版本。
未打开的 graph 保持按需加载；本地未保存 Graph Draft 不属于 Rust 驻留快照。
ProjectIndex 的文件成员来自磁盘扫描，内存只提供适用的权威版本，不得复活已删除的 chart。

Chart 文档和文件不包含资源 revision；`chart_revisions` 是其唯一版本 authority，在项目激活时从初始版本开始，
与 Graph 的资源版本生命周期一致。Chart 的创建、保存、复制、移动和删除复用资源 patch 的标准回执，
writer 不再重复构造 delta。刷新文档时可绑定 Project publication revision，在同一次一致读取中验证。
图表文件采用 `yss-chart-document::CURRENT_CHART_SCHEMA_VERSION` 定义的严格格式，不读取含旧文档版本字段的格式，也不自动迁移。
