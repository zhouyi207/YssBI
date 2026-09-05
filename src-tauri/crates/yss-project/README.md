# yss-project

> Status: Current
> Scope: Project runtime authority、资源 revision 与持久化/历史边界
> Canonical owners: 本 crate 的源码与测试拥有可执行事实；Graph 生命周期由 Graph 架构文档维护
> Update when: Project authority、revision 使用者、事务或 publication 边界改变时

`yss-project` 是项目运行期权威状态的唯一 owner。它负责 `ProjectState`、项目会话与实例身份、资源 revision、历史、持久化事务，以及磁盘提交后的 publication。

该 crate 组合 `yss-project-model`、`yss-project-history`、`yss-project-operation`、`yss-resource-lifecycle` 与 `yss-project-filesystem` 等更低层 crate，但不依赖 Tauri、Commands、IPC schema、Application 工作流或 Database runtime。

边界约束：

- `ProjectData` 是 resident project facts 的权威聚合；
- `yss-project-filesystem` 只拥有安全文件系统原语，`yss-project` 拥有 session/revision 校验与 publication；
- 对外返回 Project-owned typed facts，由 Application 和 API 层投影为事件与 DTO；
- `test-support` 只暴露跨 crate 测试所需的 fixture 与故障注入 seam。

## Graph resource revisions

`graph_resource_revisions` 是 Project-owned `GraphResourcePath → ResourceRevision` 索引，不是 editor projection 的请求计数器。它仍有生产读写方：

| 使用方                                                                                                     | 作用                                                                                       |
| ---------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| [Project activation](src/project_activation.rs) 与 [Graph lifecycle](src/project_state/graph_lifecycle.rs) | 安装活动资源的 revision，校验 rename 等操作捕获的资源版本，并在提交后更新 identity/version |
| [Graph document commit](src/project_state/graph_operation.rs)                                              | 与 committed document/history 的更新一起推进资源 revision                                  |
| [History hydration](src/history_hydration.rs) 与 [history commit](src/project_state/history.rs)            | 捕获受影响 Graph 的预期版本，在锁外准备完成后重新校验，拒绝过期提交                        |
| [Execution authority](src/execution_authority.rs)                                                          | 将 Graph resource revision 映射为执行资源 version/grant，参与资源 currentness 校验         |

因此当前不能直接删除此索引。移除 Graph Projection Channel 仅移除了那条传输链的 request-generation 协调；Save 不再接受 frontend `expectedRevision`，也没有移除 Rust 事务内部的版本校验。

以下字段不能互相替代：`ResourceRevision` 标识已提交资源版本；frontend lifecycle token 拒绝旧 editor 请求；Draft document 是未保存意图；compiled source hash 标识语义内容与 catalog 对应的 artifact。单独的 revision 也不能替代 Project instance/session identity。若以后合并 revision 的存储位置，必须同时迁移以上使用方，保持提交前重验与历史/执行资源校验语义。

Graph Draft、Compile、Save 和 Execute 的当前流程见 [Graph 与 Execution](../../../docs/architecture/GRAPH_AND_EXECUTION.md)。
