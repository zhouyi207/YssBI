# Statistical Harness 当前架构

> Status: Current
> Scope: 当前生产 Harness、typed capability gateway、persistence、Rig、Tauri channel 和 Assistant projection
> Canonical owners: Harness/Application/API/Frontend 源码与测试拥有可执行事实；本文拥有当前跨模块 contract
> Update when: Harness authority、已注册 capabilities、持久化、事件流或生产接入状态改变时

Statistical Harness 是 YssBI 的 Rust-authoritative statistical agent runtime。它不是一个 frontend chat store，也不把 Tauri commands 或 MCP 当作内部业务总线。设计理由见 [Decision 0001](../../../docs/decisions/0001-statistical-harness.md)，未完成能力见 [Harness roadmap](../../../docs/roadmap/STATISTICAL_HARNESS.md)。

## 1. Current production path

```text
Assistant UI
    ↓ strict frontend Harness contract
yss-application::ipc commands + yss-ipc-channel ordered/replayable delivery
    ↓
yss-harness-core
    ├─ session / turn
    ├─ statistical plan / workflow
    ├─ tool registry / invocation ledger
    ├─ approval
    ├─ session memory
    ├─ skill resolution
    ├─ lexical knowledge / citations
    └─ ordered event sequence
    ↓ typed ports
    ├─ AgentDriverPort → yss-harness-rig
    ├─ CapabilityGatewayPort → yss-application::ipc blocking adapter → yss-application
    └─ persistence ports → yss-harness-sqlite
```

`yss-application::runtime` 接收 Tauri app 并拥有桌面初始化；其中 `runtime/harness.rs` 构造 SQLite store、configurable Rig driver、时钟和 ID 实现。Application 直接构造内部 `ipc::CommandRuntime`，提供 capability gateway 和 `HarnessChannelHub` 的中立端口，再组成已有 `HarnessPorts`。`yss-application::harness` 拥有知识安装、Host 构造、启动恢复和创建会话时的项目绑定协调；具体状态与恢复规则仍由 Harness Core 实现。Application 安装业务服务后直接安装 IPC 上下文，Host/provider 与订阅方共享同一组 Channel hubs。共享 Harness wire DTO 归 `yss-ipc-contract`。Harness Core 不依赖 Tauri、Rig、SQLite、ProjectState、Graph runtime 或 concrete Database owner。

## 2. Authority

| 事实                                                 | Authority                                           |
| ---------------------------------------------------- | --------------------------------------------------- |
| Project、Graph、Database、Execution、Result 和 SCI   | 原有业务 owners，不转移给 Harness                   |
| Harness session 和 turn                              | `yss-harness-core` + session persistence port       |
| conversation transcript / final turn state           | persisted turn/event records                        |
| Statistical Plan 和 Workflow run/step                | Harness workflow runtime + workflow store           |
| Tool invocation state、idempotency 和 result receipt | Harness tool ledger                                 |
| approval grant lifecycle                             | Harness approval service/store                      |
| Session Memory record                                | Harness memory service/store                        |
| Skill identity/version/source                        | Harness skill registry/source port                  |
| Knowledge source/citation                            | knowledge source store；lexical index 是 projection |
| ordered Assistant stream                             | persisted Harness events + Rust sequence            |
| rendered conversation/workflow cards                 | React projection，可从 replay 重建                  |

Harness 只保存业务资源的 opaque references、project/session binding、captured revisions、完整结果 JSON、分页数据及其他有界 capability results 和 receipts。Result payload 仍由 Execution `ResultStore` 拥有；Harness 不能复制完整 DataFrame 或成为 Project history。

## 3. Stable contracts and ports

`yss-harness-contract` 是 Pure Leaf，拥有 Harness、Application Gateway、Rig 和 MCP adapter 共享的 stable typed contracts：identities、project binding、capability request/result、tool descriptor、workflow records、approval、memory、knowledge citation、event、cancellation/deadline 和 structured failure。

Harness 只通过 constructor-injected ports 使用外部能力：

- `AgentDriverPort`：provider-neutral model turn；
- `CapabilityGatewayPort`：唯一业务 capability seam；
- session/event/workflow/tool-ledger/approval/memory/knowledge/skill stores；
- clock 和 ID generator。

adapter 不得把 framework type 带入 Core，也不得拥有 policy。Application Gateway 每次调用根据 principal、Harness session、Project instance/session、resource currentness、approval、deadline、cancellation 和 invocation identity 验证请求。

## 4. Registered capabilities

桌面默认使用 `ToolRegistry::graph_assistant`；只读 foundation 仍可供独立检查型调用方使用：

| Capability                | 作用                                                                  |
| ------------------------- | --------------------------------------------------------------------- |
| `inspect_project`         | 读取有界项目资源索引，包含没有打开编辑器面板的图文件                  |
| `inspect_graph`           | 读取当前图文档、版本/hash、参数、列绑定、端口约束和诊断               |
| `search_node_catalog`     | 查询 localized node catalog                                           |
| `inspect_dataset_schema`  | 读取 schema 和 current revision facts                                 |
| `inspect_dataset_profile` | 读取 bounded data-quality/profile facts                               |
| `inspect_result`          | 读取完整结果 JSON；数列、表格及 JSON 中的 tableRef 按需分页           |
| `apply_graph_edit`        | 原子应用并自动保存可撤销的图编辑批次                                  |
| `validate_graph`          | 只读校验匹配 hash 的当前图，返回可运行性与阻断诊断                    |
| `execute_graph`           | 自动准备当前图文档的计划并执行，返回实际 run 状态、失败位置与结果 IDs |
| `list_graph_results`      | 查询图当前保留的结果 IDs，包括手动运行产物                            |
| `save_graph`              | 用户要求保存时调用正常的独立 Save                                     |
| `inspect_ui`              | 读取组件 Schema、当前结果页面或界面意图回执                           |
| `update_ui`               | 按页面修订原子替换、局部修改、排序、显隐或重置展示                    |
| `request_ui_intent`       | 请求打开图/结果、定位节点或显示允许的面板，返回待执行回执             |

页面与动作契约由共享的 `yss-ui-contract` 拥有，GUI 与 Harness 共用 Application presentation。
页面变更不写入 Project；模型必须区分意图被接受与前端已完成操作，具体校验、回执、恢复与会话边界见 [JSON 页面与界面意图](../yss-ui-contract/README.md)。

`inspect_result` 与界面复用 Application 的有界 `query_result_projection` 和共享 `result_encoding` 映射，返回 `ResultValueInspection::Json`。内联结果受投影展开预算及 64 KiB 编码上限约束，超限明确拒绝，不静默裁剪。原生报告使用概览、参数目录与表引用，统计数据按表引用继续读取。普通结果字段、嵌套对象与文本没有 AI 专用白名单或截断规则。

DataFrame、DataSeries 和内存数列仍通过 `offset`/`limit` 分页，不展开全部数据。JSON 中的数据引用保持原样；AI 可以把 `resultRef.executionSessionId`、`resultRef.resultId` 和 `tableRef.part` 传给同一个 `inspect_result` 继续分页读取。未指定 `part` 时读取完整 JSON；指定 `part` 时读取该结果公开的表。分页响应使用 JSON 行值和 `nextOffset`/`hasMore`，保留行数及字节边界。项目会话、结果可用性、取消和 deadline 检查继续生效。

Model-facing schema 来自 typed capability contract；Harness 内部不以任意 JSON 代替 request/result 类型。每次调用先写 running ledger record，再通过 Gateway 执行，最后持久化成功 result 或 structured failure。idempotency 命中已有 terminal record 时返回既有 outcome，而不是重复执行。

`inspect_project` 复用 Project 的 `read_project_index`，与左侧项目树使用同一资源成员来源，
不从内存中已加载的 GraphDocument 集合推断项目有哪些图。列举资源不改变图的加载状态；返回前重验索引版本，
读取失败或版本变化返回 typed failure。模型使用返回的图 `resourceId` 作为 `graphPath`，
再通过 `inspect_graph` 按需读取当前文档；图编辑和执行均不要求先打开编辑器面板。

Application 的 `invoke_automation_capability` 是同步业务入口。`yss-application::ipc` 的 `ApplicationCapabilityGateway` 使用 blocking worker 调用它，避免在 Tokio async worker 中嵌套 DataFusion 的 `Runtime::block_on`。`CapabilityControl` 携带单次调用的 monotonic deadline、turn cancellation 和查询取消标记；profile 把同一预算传入数据库/DataFusion 查询。只读任务在取消、超时或调用 future 被丢弃时通知查询停止；worker panic 转为安全的 `InternalFailure`。已开始提交的写操作等待真实 receipt，不将成功提交改写为超时或取消。

工具生命周期事件由持有 ledger identity 的 Harness executor 产生：准入后发送 `ToolInvocationStarted`，结束时发送 `ToolInvocationCompleted` 或带 `failureCode` 的 `ToolInvocationFailed`。失败码区分取消与超时；事件不携带数据行、结果或异常原文。账本或事件持久化失败可能留下待恢复记录；已完成的业务提交仍返回真实结果，不被交付失败改写。Rig 只执行模型工具映射，不生成另一套工具完成状态。

图操作由 `ApplicationCapabilityGateway` 直接调用 Rust Application，读取 Project 当前编辑版本并核对 revision/hash。编辑、只读校验、执行与保存使用同一状态；校验返回就绪状态和诊断，Execute 在后端准备匹配计划。Rust 返回真实 capability result，Graph Activity 通知前端更新编辑投影与运行状态；前端不生成 tool result。

同一次图请求复用打开图时捕获的文档、编辑身份和解析投影，图检查、校验及 Harness 执行准入不重复读取和解析同一图。编辑与保存仍由 Project 在提交时重验捕获版本，执行准备继续重验文档及依赖。快照直接以 graphPath、revision、graphHash 和 semanticInputHash 标识；编辑请求的 graphHash、节点端口与参数、快照事实和编辑回执中的必需字段缺失时拒绝解析，不补成空值或空集合。

`apply_graph_edit` 自动应用用户要求的编辑，使用 `baseRevision`（后端图修订）和完整 `graphHash` 拒绝过期请求。支持创建/删除/移动/复制节点、参数合并、配置、输入 literal、常量及常量引用、连线/断线和用户端口实例增删。`create_constant` 生成基础标量常量及其 Get 节点；创建节点和端口可声明 `clientId`，后续批次内引用使用 `$clientId`，真实 ID 由 Rust 生成。每批只向后端图历史 添加一次变更；任一操作失败都不安装部分候选。`clientKey` 在 session/turn 内幂等，重复相同请求返回已有 receipt，复用 key 修改请求会被拒绝。

Capability invocation identity 由 ledger 已保存的 idempotency key 确定，Application 将 principal、Harness／Project 会话、调用身份及 client key 映射为稳定的内部 operation ID。Project 将请求指纹、实际提交版本和创建元素映射与图编辑一起提交。若 gateway 回复丢失或 ledger 收尾失败，`recover_graph_edit` 仅查询该回执，不执行编辑；恢复后补写原工具记录并返回原节点／端口 ID。回执有条目及字节上限；未知、过期或会话已结束的结果保留不确定性。此恢复针对 `apply_graph_edit`，不提供执行和保存的跨进程重放。

`GraphEditReceipt` 保存图修订、graph hash、`clientKey`、创建元素的身份映射及本次提交的 `changes`。节点、端口、参数、连接和常量与 `inspect_graph` 共用事实结构；新增或改变的节点返回完整信息，包含派生列和端口，连接包含实际顺序。删除节点、连接和常量明确返回 ID；未变化实体省略。`ready` 和 `diagnostics` 是提交时的完整状态，替换此前诊断。所有图编辑操作统一比较提交前后解析投影，覆盖下游连带变化，不根据模型请求猜测结果。

差分绑定外层 `fromRevision` / `toRevision` 及 `changes.baseSemanticInputHash` / `semanticInputHash`。`inspect_graph` 同时提供语义 hash 和 ready，模型基线匹配且所需事实齐全时直接使用回执继续编辑或执行；基线缺失、语义依赖变化、版本冲突或缺少所需信息时再读取。保存后的 `resourceRevision` 同样可作为下一次编辑的 `baseRevision`。历史重放或幂等重试保留原提交事实，不能覆盖较新的证据。

Application 复用打开图时的基线和编辑流程的最终解析投影，在提交前构造并验证完整差分；不在提交后独立查询当前图来补回执。Project 将有界的调用方事实与文档、历史、版本一起提交，恢复时由 Application 解码为 typed changes，不重新解析较新的图。回执超过工具或 Project 字节预算时，在写入前返回 `result_too_large`，调用方可缩小批次；不静默裁剪节点、端口或诊断。回执不暴露内部 Project 操作 ID；Harness 的 session、turn、tool 等编号继续使用独立类型的通用 UUID。

常量检查和编辑差分都包含覆盖完整常量内容的 `contentHash`。长文本、列表或表格等值即使以 `valueIncluded=false` 只暴露元数据，内容改变也会产生新的事实和差分，不因展示元数据相同而漏报。

`execute_graph` 从本次 `RunGraphReceipt` 取得已发布结果引用，不在运行后重新查询当前图的结果索引。后续编辑、另一次运行或结果失效不会改变原回执。`resultCount` 是成功运行实际发布的结果数，失败时为 null；`resultsComplete` 明确区分完整列表和受能力条目上限约束的部分引用。结果引用不取得新的租约，后续读取仍检查实际可用性；部分列表不能当成该次运行的全部结果。

所有改变状态的模型能力按各自 owner 返回事实：

| 请求                       | 返回的提交或执行事实                                             | 后续读取条件                                                       |
| -------------------------- | ---------------------------------------------------------------- | ------------------------------------------------------------------ |
| `apply_graph_edit`         | 新 revision/hash、创建 ID、受影响实体、删除 ID、就绪状态和诊断   | 缺少基线、语义基线不匹配、版本冲突或事实不足                       |
| `save_graph`               | 保存前后 revision、graph hash、实际 dirty/canUndo/canRedo        | 后续状态变化或需要未掌握的图信息                                   |
| `execute_graph`            | 实际 run 状态、失败位置、带执行会话的结果引用                    | 通过 `inspect_result` 读取所需内容或数据页，无需先重复列举本次结果 |
| `update_ui`                | 与 GUI 共用的已提交页面差分、新 revision、完整改变元素与删除操作 | 页面基线缺失或修订冲突                                             |
| `request_ui_intent`        | 意图身份和实际回执状态                                           | pending/claimed 时查询完成状态，不能把接受当成界面已执行           |
| `propose_statistical_plan` | Harness 校验且持久化成功后返回 accepted 和实际记录的完整 plan    | 计划正文可直接继续使用，不代表分析已经执行                         |

`apply_graph_edit` 默认自动保存整个当前图文档，包括调用前已有的手动编辑，无需打开编辑器。
它复用 Project 的文件事务，将文件、当前文档、保存指纹、可撤销历史及编辑回执作为一次提交；
保存失败返回 `persistence_unavailable`，保留调用前的文件、文档和历史，不产生成功回执。
成功后 dirty 为 false，批次仍可整体撤销；重试读取原回执，不重复修改或保存。
校验和执行不隐式保存。显式 `save_graph` 用于用户要求单独保存现有编辑，复用正常 Save。
图工具与桌面编辑共享 Project 编辑状态和历史，调用始终校验项目会话与编辑版本。

GraphPortInspection 的 declared/instance 字段统一使用 camelCase，序列化、会话持久化读取与 capability schema 使用同一契约，不接受 snake_case 别名。

SQLite adapter 只接受当前 schema，不执行旧记录迁移，也不维护迁移版本字段。空数据库在事务中创建全部当前表；已有数据库的 DDL 必须与 adapter 拥有的 schema 一致，JSON 列由 `json_valid` 约束保护，读取再使用当前类型校验。不兼容结构返回 `InvalidRecord`，保留原表与记录，不自动重建。需要重新初始化时，应先备份并移走应用数据目录下的 `db/statistical-harness.sqlite` 及其 SQLite sidecar 文件，再启动应用；此操作不由初始化代码自动执行。当前诊断词汇统一由 `yss-graph-diagnostics` 提供。

节点搜索对 node ID、标题、别名、技术词及资源名分词排序，完整匹配优先，混合语言短语允许部分词命中。profile 的 null 指标表示未计算；复杂常量只暴露类型和 metadata，不复制 tabular 数据。已有图的校验和运行不要求重新设计统计方案。

## 5. Session, turn, and events

一个 Harness session 绑定明确 principal 和当前 Project instance/session。用于 Assistant 对话的 session 还保存 `HarnessConversationMetadata`：项目根目录的稳定文件系统身份、首条消息生成的标题和最近打开时间；底层 workflow/session 调用可不带对话元数据。每个 session 同时只准入一个 active turn；submit、cancel、close 和 project/session currentness 由 Rust 控制。

对话及其完整事件/工具账本继续保存在应用的 SQLite 中，不跟随 Assistant 面板卸载而关闭或删除。界面按当前用户和项目查询对话列表，优先恢复最近打开的一项；可新建和切换多个对话。切换后从 sequence 0 重放目标对话，旧订阅和迟到回调不能混入新对话；未发送草稿只在当前挂载界面内按对话分开保存。生成回答期间先停止或等待结束再切换。

重新打开项目时，Application 通过既有 RootBinding 获取项目根目录身份，验证对话归属后重新绑定当前运行期 ProjectSessionBinding；历史 receipt 保持原始身份，不恢复旧授权或执行结果。订阅和发送均验证当前项目归属和运行绑定。无已保存项目时使用仅当前激活有效的临时归属。无持久项目归属的记录保持原状，不自动猜测或迁移到某个项目。

`list_graph_results` / `execute_graph` 的每个结果引用携带 executionSessionId；`inspect_result` 必须同时提供它与 resultId。项目重启后的历史结果 ID 不能因为编号重用而读取到新结果，失效引用返回 ResultUnavailable，模型应重新查询当前结果。

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

模型取消或超时后，Rig 停止模型请求并等待已准入工具完成 ledger/终态事件收尾，然后 Harness 结束 turn。共享 cancellation token 支持多个等待者。启动恢复结束遗留 running invocation/turn；中断的 mutation 使用 `outcome_unknown`，不推断已经回滚。已持久化的真实 receipt 保留；内存编辑回执随 Project 编辑会话释放，进程崩溃后不能从当前图内容猜测旧工具是否提交。完整跨进程 commit reconciliation 仍属于 roadmap。

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

`yss-harness-rig` 实现 `AgentDriverPort`，负责 provider/model configuration、Rig message mapping、streaming、tool schema/call mapping 和 provider failure 分类。它不拥有：

- Harness session、workflow 或 event sequence；
- Tool Registry、approval 或 memory policy；
- Project/Graph/Database authority；
- capability authorization/currentness。

Frontend AI settings 通过 explicit Harness configuration command 更新 configurable driver。credential 不写入 Harness SQLite、Project、event transcript 或 diagnostics；provider network use 仍受用户配置和数据共享边界约束。

Rig adapter 显式启用 `rig-core` 的 Reqwest 和 Rustls 功能，以支持 HTTPS、证书校验及系统代理。
模型调用使用 Rig 多轮 streaming 接口，只发布公开 text 内容；首段立即发布，后续短片段按 40ms 或 4KiB 合并，工具边界和终止前刷新。工具生命周期仍由 Gateway 发布，不能用 Rig 的批次完成事件代替实时工具状态。取消和超时停止读取模型流，保留已产生的文本并等待已接纳工具完成清理。`finalText` 保存整轮公开文本，不在流结束时再发送一份完整 TextDelta。
实时 capability 返回与历史重放复用相同的工具结果 JSON 编码。资源不存在、参数或业务请求被拒绝、revision/invocation conflict 及 approval_required 等可处理结果，以 `{state: "failed", failure: {code, details}}` 交回模型，使它可以纠正参数、读取当前状态或向用户说明。图校验的 ready/diagnostics 和图执行的 status/failureCode 由各自能力结果表达；执行成功由提交回执确认，运行事件补充取消和失败定位。`outcome_unknown` 保留为失败反馈；模型必须先查询事实，不能盲目重试可能已经提交的修改。Core 的 ledger 与 ToolInvocationFailed 事件仍记录能力失败，不因协议层成功交付反馈而改写为成功。

Rig 0.42 默认会把 ToolExecutionError 转成模型反馈，不能以返回该错误作为必然中止的保证。Adapter 因此对取消、超时、项目会话失效、内部错误、持久化故障和工具运行通道异常发出独立的致命信号：流消费者在下一次模型调用前结束，刷新已产生的公开文本，并等待已准入工具完成收尾。计划工具的事件持久化或交付故障也使用此路径。Provider/stream 错误继续由 AgentDriverFailure 终止；ModelTurnRetried 的拒绝策略保持原有语义。

Provider 请求有连接/总时限，完整 model turn 也有独立时限；模型任务 panic 和超时均转换为 typed terminal failure，原始 panic/响应内容不进入错误 wire。具体预算由 adapter 配置和源码拥有。
`providerConfigured` 表示本地客户端配置已建立，不表示远端认证已经通过。模型调用按结构化 HTTP 状态区分认证、限流、请求拒绝、服务不可用和连接失败；响应解析失败单独分类，原始响应和凭据不进入错误 wire。

## 9. Tauri transport and frontend projection

`yss-application::ipc` 当前暴露 Harness runtime status/provider configuration、按项目列举、新建、重新打开及关闭 session、event subscribe/unsubscribe、turn submit/cancel、memory list/delete，以及 dataset-quality workflow plan/advance/pause/resume/cancel。Command 只做 DTO mapping 和 transport delivery；完整注册表以 `yss-application::ipc` 源码为准，不在本文复制。

有序事件通过 Tauri Channel 进入 `src/services/assistant/harnessService.ts`，由 `harnessContract.ts` 严格解析。`src/features/application/assistant/assistantHarnessRuntime.ts` 维护可重建的 projection、last sequence 和 reconnect；assistant-ui ExternalStore 只渲染 messages、plan、tool cards、memory 和 composer actions。

提交携带 active graph reference。模型上下文由 Harness Core 的 `conversation` 从本会话的完整持久事件流和工具账本重建，以当前 TurnStarted 为边界；当前用户消息只加入一次。不限制最近轮数，不按条截断用户或 assistant 文本，也不将失败/取消轮次排除。相邻 TextDelta 合并，工具事件保留其间的顺序；已有流式正文时不重复追加 TurnCompleted.finalText。

AgentMessage 使用 provider-neutral 的文本、工具调用、工具结果和统计计划变体。工具参数与成功/失败结果读取本会话中对应 invocation 的原始记录，Rig 映射为成对的原生 tool-call/tool-result 消息，不将工具证据降为 assistant 摘要。恢复后的成功 receipt 保留成功，失败保留结构化 failure，缺少确定终态的调用标记 outcome_unknown；取消或失败的 turn 附带明确状态，不暗示已经提交的操作被回滚。事件缺口或缺失的工具记录会明确失败，不静默发送不完整历史。

完整结果 JSON 随工具历史传递，DataSeries/DataFrame 仍保持引用或已读取的数据页。Rig 不再为单条历史消息设 1 MiB 准入上限，也不自动裁剪历史；供应商返回结构化 context_length_exceeded 时映射为 assistant_context_window_exceeded，界面提示完整对话超过模型容量，保留会话供用户切换模型或开启新会话。新消息、模型生成和超时准入仍使用各自已有契约。

历史不替代当前工具事实。图工具的运行事件进入原有 Execution/Results 消费者；对话只复用持久 Harness 事件与工具 receipt，不新增第二份持久聊天状态。

转换为 assistant-ui 消息时，只有 assistant 角色携带 `status`；user 消息不携带该字段。运行时集成回归使用真实的 ExternalStore 消息转换器校验这一边界。
消息投影按事件顺序维护 text/source/data/tool parts，相邻文本片段合并，工具终态更新原位置。完成事件只收尾已有流式内容；仅在没有文本片段时使用 `finalText` 恢复正文。取消、失败和按 sequence 重放不覆盖中间说明或重排工具。

Assistant 面板使用 assistant-ui 的 Thread Viewport 管理流式滚动和回到底部，Composer 管理发送/停止，ActionBar 提供回复复制。文本通过配套 MarkdownTextPrimitive 渲染 GFM 表格、列表、代码块和数学公式，代码块提供复制；排版适配工作台主题与窄面板。引用 source part 显示来源标题；连续工具调用使用 Collapsible 分组，运行时默认展开、结束后默认收起，用户手动选择优先。摘要显示调用进度和异常数，每项显示本地化名称与原位更新的状态，详情保留原始工具标识。参数展示支持截断预览、展开与复制；当前 Harness 事件不提供参数，生产记录明确显示未提供参数，不将占位空对象解释为真实参数。统计计划和记忆仍由业务组件展示，不新增会话 authority。

事件 `type` 使用 snake_case，envelope 和 payload 的字段使用 camelCase。Rust 序列化和 TypeScript 解析共用代表性事件夹具，避免两端各自使用不同的手工样本。
Channel 在历史重放期间缓冲实时事件，按 sequence 合并、去重后交付。等待缺号的实时缓冲有容量限制，超限时交付缺号之后的持久事件并关闭旧订阅，让前端依据 sequence gap 重新重放；长历史按顺序排出，不占用整个实时缓冲。前端重连时封锁发送并忽略已替换订阅的迟到回调。工具卡片从携带真实 invocation ID 的 `ToolInvocationStarted` 创建，并按同一 ID 更新完成、失败、取消、超时或中断状态；不生成待替换的工具占位身份。turn 终止时未收到工具终态的卡片显示中断/取消，不假定成功。面板卸载使旧回调失效并释放订阅；已创建的对话继续持久化，供重新打开。
订阅建立后才开放发送。已终止的模型轮次失败保留安全错误引用并允许再次发送，下一次发送清除该错误；会话或传输故障仍阻止发送。提交请求尚未结束时不能再次提交，关闭后的迟到回调不能更新新会话。

React 不生成 authoritative turn/workflow transition，不直接调用 Rig/Gateway，也不在 Zustand 建立 conversation authority。Project/session replacement 或 provider unavailable 只改变 projection/action availability，不能保留旧 backend handle 继续提交。

## 10. MCP status

当前没有 MCP server adapter 或桌面监听入口，也没有 MCP Client。内部 Assistant 直接调用 Capability Gateway。外部 transport、authentication、Tasks mapping 和 tool trust 属于 [roadmap](../../../docs/roadmap/STATISTICAL_HARNESS.md)。

## 11. Error, safety, and observability

- Core、Gateway 和 adapters 使用 typed failures；Tauri seam 再映射为 stable error wire；
- prompt、transcript、Memory、tool request/result、数据行、SQL、credential 和 model output 不写入 logging/diagnostics；
- Assistant text 只进入 Harness event stream，不进入 Graph 执行事件或 Output 失败摘要；
- capability result、knowledge hit 和 model text都有明确 size/depth budget；
- external/provider payload 在 adapter 边界完成 schema、size、time 和 failure validation；
- operational ledger 是 durable业务记录，不等同于 lossy diagnostics log。

通用 transport contract 见 [`yss-application::ipc` README](../yss-application/src/ipc/README.md)，技术信号边界见 [Runtime Signals](../../../src/features/application/observability/README.md)。

## 12. Current limits

当前 production Assistant intentionally does not provide：

- chart/report write 或 external write tools；
- unknown commit outcome reconciliation；
- external MCP client/server process exposure；
- persistent User Memory 或 portable Project Memory；
- vector/hybrid Knowledge retrieval；
- remote Skill install/signing；
- autonomous background or multi-agent execution。

这些限制是当前边界，不应在 current architecture 中展开为拟议 interface。实施顺序和验收条件只在 [Harness roadmap](../../../docs/roadmap/STATISTICAL_HARNESS.md) 维护。

图工具直接通过 Application 操作 Project 当前驻留文档，不经过 Webview prepare/claim/adopt 握手。图活动 Channel 负责 UI 通知及执行事件，编辑数据和历史不依赖客户端存活。图编辑 snapshot/delta 和保存语义见 [Graph 与 Execution](../yss-application/src/graph/README.md)。

## 相关模块

[Application](../yss-application/README.md) · [UI 页面契约](../yss-ui-contract/README.md)
