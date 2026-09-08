# yss-project

> Status: Current
> Scope: Project runtime authority、资源 revision、持久化与 publication 边界
> Canonical owners: 本 crate 的源码与测试拥有可执行事实；Graph 生命周期由 Graph 架构文档维护
> Update when: Project authority、revision 使用者、事务或 publication 边界改变时

`yss-project` 是项目运行期权威状态的唯一 owner。它负责 `ProjectState`、项目会话与实例身份、资源 revision、持久化事务，以及磁盘提交后的 publication。

该 crate 组合 `yss-project-model`、`yss-project-history`、`yss-project-operation`、`yss-resource-lifecycle` 与 `yss-project-filesystem` 等更低层 crate，但不依赖 Tauri、Commands、IPC schema、Application 工作流或 Database runtime。

边界约束：

- `ProjectData` 是 resident project facts 的权威聚合；
- `yss-project-filesystem` 只拥有安全文件系统原语，`yss-project` 拥有 session/revision 校验与 publication；
- 对外返回 Project-owned typed facts，由 Application 和 API 层投影为事件与 DTO；
- `test-support` 只暴露跨 crate 测试所需的 fixture 与故障注入 seam。

Graph 撤销/重做由前端 Graph Draft 管理，数据库编辑历史由 Database runtime 管理。Project 提交发布资源版本和 delta，不维护项目级撤销栈。`yss-project-history` 保留共享的资源身份、变更请求、函数文档、delta、错误及图驻留状态契约；文件事务回滚与失败恢复继续由 Project 和 filesystem owner 负责。

## Graph resource revisions

项目格式版本为 4。命名常量随 Event/Function 的 `GraphDocument.constants` 保存，属于 Graph 资源事务，不再维护全局变量、变量 revision、作用域迁移或 `variables.yssbi-vars` 的日常读写。

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
