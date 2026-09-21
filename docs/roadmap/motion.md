# motion：React / Harness 共用业务入口与投影同步计划

> Status: Planned
> Scope: React 与 Harness 共用 Application 业务能力、后端状态到前端投影的交付、界面意图和后续入口
> Canonical owners: 本文维护整体目标与剩余工作；当前 Graph、Harness、Project 和 IPC 契约由各自文档及源码维护
> Update when: 业务覆盖范围、通知与投影协议、界面意图或验收状态改变时

原始方案见 Git 提交 `1a33722d` 中的 `docs/harness/motion.md`。核心目标是让 React 和 Harness 成为同一组 Application 业务能力的客户端，由后端状态变化驱动前端投影，而不是为 AI 再建一套同步路径。

Graph 当前文档、历史和部分同步链路已有实现。原方案还涉及其他业务状态、增量交付、AI 界面意图及未来入口，不能由 Graph 局部实现推导为整体完成；本计划继续保留。

## 原始目标与当前约束

React 通过传输适配调用 Application，Harness 通过能力网关调用 Application。业务状态继续归 Project、Database、Graph、Execution、Plugin 等既有所有者；Application 编排用例，不把各领域状态合成一个可随意写入的全局容器。

React 保留标签页、面板、视口、选择和临时手势等 UI 状态。后端先交付快照，再按各领域契约通知或交付增量；Harness 提交后，已有 GUI 消费同一套投影安装逻辑。AI 请求聚焦或显示节点属于界面意图，不应伪装成 Project 业务编辑。

原文的 GraphService、AppEvent 和 AppDelta 是示意名称，不要求创建同名类或通用事件总线。当前 Graph、Results、运行状态、Logs 与 Harness 事件仍须保持各自边界。

## 当前实现与缺口

| 能力                             | 当前状态与依据                                                                                                                                                                                                                          |
| -------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Rust 管理当前图和可逆历史        | 已实现于 [Project graph editing](../../src-tauri/crates/yss-project/src/project_state/graph_editing.rs)，没有独立前端 Graph 草稿权威                                                                                                    |
| GUI 与 Harness 修改同一当前图    | 已有 Graph 链路：[GUI 编辑](../../src-tauri/crates/yss-application/src/graph/edit.rs) 与 [Harness 编辑](../../src-tauri/crates/yss-application/src/automation/graph.rs) 经 Application / Project 提交；Harness 批次同时保存并发布图活动 |
| 图投影快照与增量安装             | 已有 [graphEditorSync](../../src/services/nodeSystem/graphEditorSync.ts)，增量是当前图查询/命令响应的交付形式                                                                                                                           |
| 通知直接携带可安装的业务增量     | Graph 活动的 `changed` 回调仍在 [graphActivity](../../src/features/application/graphProjection/graphActivity.ts) 请求刷新；这不等于原文设想的持续直接推送页面增量                                                                       |
| 其他业务域的双入口覆盖           | 尚需逐域核对，Graph 已支持的写能力不能代表 Chart、数据、插件等全部场景都可由 Harness 操作；未接入写能力见 [Harness 路线图](STATISTICAL_HARNESS.md)                                                                                      |
| Harness 发起聚焦、定位等 UI 意图 | 当前 [capability 契约](../../src-tauri/crates/yss-harness-contract/src/lib.rs) 未提供原方案所述的专用界面意图入口                                                                                                                       |
| CLI / MCP 等其他客户端           | 原方案的后续扩展方向；生产 MCP 接入仍在 [Harness 路线图](STATISTICAL_HARNESS.md#3-mcp-external-integration) 跟踪                                                                                                                        |
| 整体端到端验收                   | 本次仅核对源码，没有重新执行桌面操作；跨入口竞争、通知丢失、关闭/重开及会话切换不能据此标为全部通过                                                                                                                                     |

## 剩余工作

- [ ] 按 Graph、Project、数据、Chart、Results 和 Plugin 逐域列出 GUI / Harness 支持的操作、唯一状态所有者、共用用例和投影交付方式；明确尚未接入或无需开放给 AI 的操作。
- [ ] 复核 Graph 的两个入口是否复用应共享的业务校验与提交逻辑，保留 GUI 显式 Save 和 Harness 原子保存批次的既定差异。
- [ ] 对照原方案评估通知后刷新与直接增量交付的取舍，明确适用业务域；若采用推送，定义版本基线、顺序、背压、断线恢复和快照回退，并核实是否减少重复查询。
- [ ] 定义并接入受控 UI 意图，使 Harness 能请求定位、聚焦或展示已有资源；明确目标面板、项目身份、失败与取消，不把 UI 状态写入 Project。
- [ ] 在明确需求下补齐其他业务域的 Harness 能力和后续客户端，复用现有 Application 入口；批准、幂等、提交结果与恢复要求沿用 Harness 专项计划。
- [ ] 人工验收 GUI / Harness 交错编辑、批次失败、重复请求、保存与撤销、关闭重开、项目替换和迟到投影；逐场景记录当前版本的结果。
- [ ] 在需要性能结论时测量代表性业务图的请求次数、传输量、Resolve、投影安装和绘制耗时；既有微基准不能证明端到端同步目标已达成。

以前明确跳过的 release 测量不自动成为本轮交付要求。是否直接推送增量、开放哪些业务写能力和何时接入外部客户端，都要在实现前明确范围，不能仅凭某个类型或 adapter 存在判定完成。

当前稳定边界见 [Graph 与 Execution](../architecture/GRAPH_AND_EXECUTION.md)、[Statistical Harness](../architecture/STATISTICAL_HARNESS.md)与 [IPC](../../src-tauri/crates/yss-application/src/ipc/README.md)。UI 页面描述及其增量另由 [JSON Driver 计划](jsonDriver.md) 维护，不替代业务状态和图投影协议。

[返回专项计划](README.md)
