# Statistical Harness 当前架构

> Status: Current
> Scope: 当前生产 Harness、typed capability gateway、persistence、Rig、Tauri channel 和 Assistant projection
> Canonical owners: Harness/Application/API/Frontend 源码与测试拥有可执行事实；本文拥有当前跨模块 contract
> Update when: Harness authority、已注册 capabilities、持久化、事件流或生产接入状态改变时

Statistical Harness 是 YssBI 的 Rust-authoritative statistical agent runtime。它不是一个 frontend chat store，也不把 Tauri commands 或 MCP 当作内部业务总线。设计理由见 [Decision 0001](../decisions/0001-statistical-harness.md)，未完成能力见 [Harness roadmap](../roadmap/STATISTICAL_HARNESS.md)。

## 1. Current production path

```text
Assistant UI
    ↓ strict frontend Harness contract
yss-api commands + ordered/replayable channel
    ↓
yss-statistical-harness
    ├─ session / turn
    ├─ statistical plan / workflow
    ├─ tool registry / invocation ledger
    ├─ approval
    ├─ session memory
    ├─ skill resolution
    ├─ lexical knowledge / citations
    └─ ordered event sequence
    ↓ typed ports
    ├─ AgentDriverPort → yss-agent-rig
    ├─ CapabilityGatewayPort → yss-api blocking adapter → yss-application
    └─ persistence ports → yss-statistical-harness-sqlite
```

`src-tauri/src/lib.rs` 构造 SQLite store、Application capability gateway 的调度适配器、configurable Rig driver、builtin knowledge 和 `HarnessHost`，再把 runtime state 注入 `yss-api`。Harness Core 不依赖 Tauri、Rig、SQLite、ProjectState、Graph runtime 或 concrete Database owner。

## 2. Authority

| 事实                                                 | Authority                                            |
| ---------------------------------------------------- | ---------------------------------------------------- |
| Project、Graph、Database、Execution、Result 和 SCI   | 原有业务 owners，不转移给 Harness                    |
| Harness session 和 turn                              | `yss-statistical-harness` + session persistence port |
| conversation transcript / final turn state           | persisted turn/event records                         |
| Statistical Plan 和 Workflow run/step                | Harness workflow runtime + workflow store            |
| Tool invocation state、idempotency 和 result receipt | Harness tool ledger                                  |
| approval grant lifecycle                             | Harness approval service/store                       |
| Session Memory record                                | Harness memory service/store                         |
| Skill identity/version/source                        | Harness skill registry/source port                   |
| Knowledge source/citation                            | knowledge source store；lexical index 是 projection  |
| ordered Assistant stream                             | persisted Harness events + Rust sequence             |
| rendered conversation/workflow cards                 | React projection，可从 replay 重建                   |

Harness 只保存业务资源的 opaque references、project/session binding、captured revisions、bounded capability results 和 receipts。Result payload 仍由 Execution `ResultStore` 拥有；Harness 不能复制完整 DataFrame 或成为 Project history。

## 3. Stable contracts and ports

`yss-automation-contract` 是 Pure Leaf，拥有 Harness、Application Gateway、Rig 和 MCP adapter 共享的 stable typed contracts：identities、project binding、capability request/result、tool descriptor、workflow records、approval、memory、knowledge citation、event、cancellation/deadline 和 structured failure。

Harness 只通过 constructor-injected ports 使用外部能力：

- `AgentDriverPort`：provider-neutral model turn；
- `CapabilityGatewayPort`：唯一业务 capability seam；
- session/event/workflow/tool-ledger/approval/memory/knowledge/skill stores；
- clock 和 ID generator。

adapter 不得把 framework type 带入 Core，也不得拥有 policy。Application Gateway 每次调用根据 principal、Harness session、Project instance/session、resource currentness、approval、deadline、cancellation 和 invocation identity 验证请求。

## 4. Registered capabilities

桌面默认使用 `ToolRegistry::graph_assistant`；只读 foundation 仍可供独立检查型调用方使用：

| Capability                | 作用                                                       |
| ------------------------- | ---------------------------------------------------------- |
| `inspect_project`         | 读取 bounded project metadata/resource identities          |
| `inspect_graph`           | 读取当前草稿、版本/hash、参数、列绑定、端口约束和诊断      |
| `search_node_catalog`     | 查询 localized node catalog                                |
| `inspect_dataset_schema`  | 读取 schema 和 current revision facts                      |
| `inspect_dataset_profile` | 读取 bounded data-quality/profile facts                    |
| `inspect_result`          | 读取 structured result，支持数列/表格的 bounded 分页预览   |
| `apply_graph_edit`        | 原子应用可撤销的草稿编辑批次                               |
| `compile_graph`           | 编译匹配 hash 的草稿，返回 artifact 或阻断诊断             |
| `execute_graph`           | 执行匹配的 artifact，返回实际 run 状态、失败位置与结果 IDs |
| `list_graph_results`      | 查询图当前保留的结果 IDs，包括手动运行产物                 |
| `save_graph`              | 用户要求保存时调用正常的独立 Save                          |

Model-facing schema 来自 typed capability contract；Harness 内部不以任意 JSON 代替 request/result 类型。每次调用先写 running ledger record，再通过 Gateway 执行，最后持久化成功 result 或 structured failure。idempotency 命中已有 terminal record 时返回既有 outcome，而不是重复执行。

Application 的 `invoke_automation_capability` 是同步业务入口。`yss-api` 的 `ApplicationCapabilityGateway` 使用 blocking worker 调用它，避免在 Tokio async worker 中嵌套 DataFusion 的 `Runtime::block_on`。`CapabilityControl` 携带单次调用的 monotonic deadline、turn cancellation 和查询取消标记；profile 把同一预算传入数据库/DataFusion 查询。只读任务在取消、超时或调用 future 被丢弃时通知查询停止；worker panic 转为安全的 `InternalFailure`。已开始提交的写操作等待真实 receipt，不将成功提交改写为超时或取消。

工具生命周期事件由持有 ledger identity 的 Harness executor 产生：`ToolInvocationStarted` 后必须有 `ToolInvocationCompleted` 或带 `failureCode` 的 `ToolInvocationFailed`。失败码区分取消与超时；事件不携带数据行、结果或异常原文。Rig 只执行模型工具映射，不生成另一套工具完成状态。

桌面图操作通过 `HarnessGraphClientHub` 与当前草稿 owner 协作。临时 channel 只传 request ID、session/project/graph identity 和 capability ID，不建立 Rust draft store。前端在既有 Graph FIFO 内捕获当前 document/generation，Rust Application 进行 Resolve、批量 mutation、Compile、Execute 或 Save。Rust 保留真实 capability result，前端仅领取和确认已验证的 projection，不能提交自行计算的 tool result。prepared update 必须匹配 capability 和图身份，迟到/过期 update 不能安装。

`apply_graph_edit` 自动应用用户要求的编辑，使用 `baseRevision`（草稿 generation）和完整 `graphHash` 拒绝过期请求。支持创建/删除/移动/复制节点、参数合并、配置、输入 literal、常量及常量引用、连线/断线和用户端口实例增删。`create_constant` 生成基础标量常量及其 Get 节点；创建节点和端口可声明 `clientId`，后续批次内引用使用 `$clientId`，真实 ID 由 Rust 生成。每批只向既有草稿 history 添加一次变更；任一操作失败都不安装部分候选。`clientKey` 在 session/turn 内幂等，重复相同请求返回已有 receipt，复用 key 修改请求会被拒绝。

编辑、编译和执行不会隐式保存。显式 `save_graph` 复用正常 Save，包括保存成功后的草稿历史清理。没有独立的 AI committed graph 或 AI 撤销栈。图工具需要当前桌面的草稿 channel；external/headless graph mutation 不自动获得这条能力。

节点搜索对 node ID、标题、别名、技术词及资源名分词排序，完整匹配优先，混合语言短语允许部分词命中。profile 的 null 指标表示未计算；复杂常量只暴露类型和 metadata，不复制 tabular 数据。已有图的技术编译/运行不要求重新设计统计方案。

## 5. Session, turn, and events

一个 Harness session 绑定明确 principal 和 Project instance/session。每个 session 同时只准入一个 active turn；submit、cancel、close 和 project/session currentness 由 Rust 控制。

Turn 流程是：

```text
validate and persist user turn
  → publish TurnStarted
  → record allowed Session Memory
  → retrieve bounded lexical Knowledge
  → build Statistical Plan/context
  → AgentDriver invokes registered typed tools
  → persist tool ledger and ordered events
  → persist terminal turn state
```

每个 Harness event 包含 stream/session/sequence 和相关 turn/workflow identity。事件先进入 durable store，再交付 live channel。Frontend 订阅从 last seen sequence replay；出现 gap、断线或交付竞态时重新订阅并 replay，不把本地数组当作 durable transcript。

terminal event 和 persisted terminal state 都由 Harness 产生。取消会封锁或忽略 late model/tool output；frontend stop action 不能把已经完成的业务 commit 改写为“取消成功”。

模型取消或超时后，Rig 停止模型请求并等待已准入工具完成 ledger/终态事件收尾，然后 Harness 结束 turn。共享 cancellation token 支持多个等待者。启动恢复结束遗留 running invocation/turn；中断的 mutation 使用 `outcome_unknown`，不推断已经回滚。已保存/已运行的真实 receipt 保留；客户端 adoption 确认丢失时显示状态待确认，后续先检查当前事实，不能盲目重试。完整跨进程 commit reconciliation 仍属于 roadmap。

## 6. Workflow and statistical plan

Harness 生成 typed Statistical Plan，而不是让 model 自由决定数值事实。计划区分 research question、analysis mode、study design、estimands、variable roles、candidate methods、selected workflow、diagnostics、robustness 和 reporting needs。

当前 production workflow 是 versioned `dataset_quality_review`：先读取 dataset schema，再读取 dataset profile。Workflow compiler 校验 step identity、dependency existence、self-dependency、cycle 和 capability request；runtime 持久化 run/step state，并提供 plan、advance、pause、resume 和 cancel 操作。

Workflow run 绑定 exact definition ID/version 和 Project session。恢复或继续前必须重验 binding/currentness；step output 仍是 typed capability result，不允许 model 自行制造 estimate、p-value、standard error 或 confidence interval。

## 7. Skills, knowledge, and memory

当前实现包括：

- builtin、versioned Skill source 和 exact resolution；
- builtin statistical knowledge 安装；
- bounded lexical retrieval 和 source citation；
- Session Memory proposal、policy、list 和 delete；
- SQLite persistence ports for sessions/events/workflows/ledger/approval/memory/knowledge/skills。

Skill 是允许 tools、knowledge scope 和 workflow policy 的版本化方法包，不是任意脚本。Knowledge source 是 authority，search index 可以重建。Memory 是结构化、scoped、带 source/project/sensitivity/retention 的 record，不等同于 transcript 或 vector index。

当前 Assistant 自动使用的持久记忆范围是 Session Memory。Persistent User Memory、portable Project Memory、hybrid/vector retrieval、remote Skill trust 和完整治理 UI 尚未成为 current production contract。

## 8. Rig adapter

`yss-agent-rig` 实现 `AgentDriverPort`，负责 provider/model configuration、Rig message mapping、streaming、tool schema/call mapping 和 provider failure 分类。它不拥有：

- Harness session、workflow 或 event sequence；
- Tool Registry、approval 或 memory policy；
- Project/Graph/Database authority；
- capability authorization/currentness。

Frontend AI settings 通过 explicit Harness configuration command 更新 configurable driver。credential 不写入 Harness SQLite、Project、event transcript 或 diagnostics；provider network use 仍受用户配置和数据共享边界约束。

Rig adapter 显式启用 `rig-core` 的 Reqwest 和 Rustls 功能，以支持 HTTPS、证书校验及系统代理。
Provider 请求有连接/总时限，完整 model turn 也有独立时限；模型任务 panic 和超时均转换为 typed terminal failure，原始 panic/响应内容不进入错误 wire。具体预算由 adapter 配置和源码拥有。
`providerConfigured` 表示本地客户端配置已建立，不表示远端认证已经通过。模型调用按结构化 HTTP 状态区分认证、限流、请求拒绝、服务不可用和连接失败；响应解析失败单独分类，原始响应和凭据不进入错误 wire。

## 9. Tauri transport and frontend projection

`yss-api` 当前暴露 Harness runtime status/provider configuration、session create/close、event subscribe/unsubscribe、turn submit/cancel、memory list/delete，以及 dataset-quality workflow plan/advance/pause/resume/cancel。Command 只做 DTO mapping 和 transport delivery；完整注册表以 `yss-api` 源码为准，不在本文复制。

有序事件通过 Tauri Channel 进入 `src/services/assistant/harnessService.ts`，由 `harnessContract.ts` 严格解析。`src/features/application/assistant/assistantHarnessRuntime.ts` 维护可重建的 projection、last sequence 和 reconnect；assistant-ui ExternalStore 只渲染 messages、plan、tool cards、memory 和 composer actions。

提交携带 active graph reference，并从已持久化的 completed turns 提供有限长度的会话上下文；历史不替代当前工具事实。图工具的运行事件进入原有 Execution/Results 消费者，不复制到聊天日志。

转换为 assistant-ui 消息时，只有 assistant 角色携带 `status`；user 消息不携带该字段。运行时集成回归使用真实的 ExternalStore 消息转换器校验这一边界。

事件 `type` 使用 snake_case，envelope 和 payload 的字段使用 camelCase。Rust 序列化和 TypeScript 解析共用代表性事件夹具，避免两端各自使用不同的手工样本。
Channel 在历史重放期间缓冲实时事件，按 sequence 合并、去重后交付。等待缺号的实时缓冲有容量限制，超限时交付缺号之后的持久事件并关闭旧订阅，让前端依据 sequence gap 重新重放；长历史按顺序排出，不占用整个实时缓冲。前端重连时封锁发送并忽略已替换订阅的迟到回调。工具卡片按 invocation ID 更新，显示完成、失败、取消、超时或中断；旧持久事件中的 `ToolInvocationRequested` 仅作为重放占位，遇到真实 ID 后合并。turn 终止时未收到工具终态的卡片显示中断/取消，不假定成功。面板卸载后才完成创建的 session 会被关闭。
订阅建立后才开放发送。已终止的模型轮次失败保留安全错误引用并允许再次发送，下一次发送清除该错误；会话或传输故障仍阻止发送。提交请求尚未结束时不能再次提交，关闭后的迟到回调不能更新新会话。

React 不生成 authoritative turn/workflow transition，不直接调用 Rig/Gateway，也不在 Zustand 建立 conversation authority。Project/session replacement 或 provider unavailable 只改变 projection/action availability，不能保留旧 backend handle 继续提交。

## 10. MCP status

当前没有 MCP server adapter 或桌面监听入口，也没有 MCP Client。内部 Assistant 直接调用 Capability Gateway。外部 transport、authentication、Tasks mapping 和 tool trust 属于 [roadmap](../roadmap/STATISTICAL_HARNESS.md)。

## 11. Error, safety, and observability

- Core、Gateway 和 adapters 使用 typed failures；Tauri seam 再映射为 stable error wire；
- prompt、transcript、Memory、tool request/result、数据行、SQL、credential 和 model output 不写入 logging/diagnostics；
- Assistant text 只进入 Harness event stream，不进入 Run Output；
- capability result、knowledge hit 和 model text都有明确 size/depth budget；
- external/provider payload 在 adapter 边界完成 schema、size、time 和 failure validation；
- operational ledger 是 durable业务记录，不等同于 lossy diagnostics log。

通用 transport contract 见 [`yss-api` README](../../src-tauri/crates/yss-api/README.md)，技术信号边界见 [Runtime Signals](RUNTIME_SIGNALS.md)。

## 12. Current limits

当前 production Assistant intentionally does not provide：

- chart/report write 或 external write tools；
- unknown commit outcome reconciliation；
- external MCP client/server process exposure；
- persistent User Memory 或 portable Project Memory；
- vector/hybrid Knowledge retrieval；
- remote Skill install/signing；
- autonomous background or multi-agent execution。

这些限制是当前边界，不应在 current architecture 中展开为拟议 interface。实施顺序和验收条件只在 [Harness roadmap](../roadmap/STATISTICAL_HARNESS.md) 维护。
