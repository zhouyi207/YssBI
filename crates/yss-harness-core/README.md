# Statistical Harness 当前架构

> Status: Current
> Scope: 当前 Harness、typed capability gateway、persistence、Rig、原生事件交付与保留的 Assistant 参考
> Canonical owners: Harness/Application/GPUI 源码与测试拥有可执行事实；本文拥有当前跨模块 contract
> Update when: Harness authority、已注册 capabilities、持久化、事件流或生产接入状态改变时

Statistical Harness 是 YssBI 的 Rust-authoritative statistical agent runtime。会话与工具流程归 Rust，通过类型化 Application 端口调用业务 owner。设计理由见 [Decision 0001](../../docs/decisions/0001-statistical-harness.md)，未完成能力见 [Harness roadmap](../../docs/roadmap/STATISTICAL_HARNESS.md)。

## 1. Current runtime path

```text
GPUI NativeServices
    ↓ ApplicationServices / typed Harness calls
yss-application::harness + host-owned HarnessEventSinkPort
    ↓
yss-harness-core
    ├─ session / turn
    ├─ statistical plan / workflow
    ├─ tool registry / invocation ledger
    ├─ approval
    ├─ skill resolution
    ├─ knowledge access / passages / citations
    └─ ordered event sequence
    ↓ typed ports
    ├─ LanguageModelResolverPort → Application model settings → yss-harness-rig (AgentDriverPort)
    ├─ CapabilityGatewayPort → yss-application::harness blocking adapter → yss-application
    ├─ KnowledgeIndexPort → yss-harness-tantivy (BM25)
    └─ persistence ports → yss-harness-sqlite
```

`yss-application::runtime` 接收平台中立的 ApplicationPaths 与端口组装回调；其中 `runtime/harness.rs` 构造 SQLite store、模型配置服务、时钟和 ID 实现。GPUI 宿主组装 BlockingHarnessGateway 与 HarnessEventSinkPort，再交给已有 HarnessPorts。`yss-application::harness` 拥有知识安装、Host 构造、启动恢复和创建会话时的项目绑定协调；具体状态与恢复规则仍由 Harness Core 实现。原生宿主将已持久化事件投递到有界 NativeEvent 流；持久序列及重放仍归 Harness。共享序列化值归 `yss-ipc-contract`。Harness Core 不依赖 GPUI、Rig、SQLite、ProjectState、Graph runtime 或 concrete Database owner。原生 Assistant 基本界面已接入上述类型化边界；布局、草稿持久化及完整验收仍在迁移中，React 实现保留在 `react/` 作为参考。

## Manager–Worker roles and task lifecycle

桌面使用扁平的六角色编排。角色定义和工具集合由 [agents/mod.rs](src/agents/mod.rs)
拥有，权限检查位于 [agents/policy.rs](src/agents/policy.rs)；[orchestration](src/orchestration/mod.rs) 拥有运行和委派，回执归集与中断恢复分别位于其子模块。Rig 根据
provider-neutral 请求构建模型与工具循环，不另设角色注册表或持久状态。

| 角色         | 职责与写入边界                                                       |
| ------------ | -------------------------------------------------------------------- |
| ManagerAgent | 唯一用户对话入口，发现资源、委派任务、处理阻塞与汇总；可请求 UI 意图 |
| DataAgent    | 数据理解、质量检查及获准的 Database 准备、导入和导出                 |
| StatsAgent   | 复用 StatisticalPlan，选择节点、编辑分析图、执行并读取诊断与结果     |
| PlotAgent    | 读取指定数据/结果，创建和编辑 Chart；图形能力仍由 Chart owner 决定   |
| ReportAgent  | 撰写 Doc 报告，或创建、编辑和保存任务指定的 Mind                     |
| ReviewAgent  | 独立上下文中的只读审查，返回问题和证据；不能修改或委派               |

一个用户 Turn 内创建一个 Manager run 和按需启动的 Worker runs。只有 Manager 的 executor
实现 `delegate_task` 和 `followup_task`；Worker 只能返回 Manager，不能互相调用或递归委派。
模型委派参数只包含目标、约束、完成条件、当前 turn 内已完成的依赖 run IDs，以及精确的资源/操作授权。
创建授权同时列出精确创建参数和新资源后续允许的操作。Core 按任务规范生成内部去重身份，
同一 turn 的相同规范复用已有结果；运行中的相同任务明确拒绝，不自动执行第二次。

面向模型的任务和业务工具输入、结果投影由 Contract 的 [model.rs](../yss-harness-contract/src/model.rs) 及其子模块拥有。
委派 key、资源版本和编辑会话不出现在委派/续接 schema、Worker 任务正文、任务结果或任务历史回放中。
Core 从 Manager 的真实读取回执捕获版本，未读取过的资源通过现有资源工具捕获元数据；
已观察到的旧版本不自动换成最新版本。数据库运行/Schema 版本独立于项目资源版本：
读取过 schema/profile 后，自动绑定使用资源 Owner 的最小分页读取核对数据身份，再取得资源版本，不能直接转换计数器。

[observations.rs](src/orchestration/observations.rs) 是回执的内存投影，随完整事件/账本历史重建，不随模型上下文压缩丢弃，
也不成为资源 authority 或新持久化存储。Worker 改变已读资源时旧观察失效，Manager 必须重新读取；
项目会话更换后，旧观察也必须通过当前项目读取重新建立。Worker 启动前仍在执行门内核对完整授权版本，
业务 Owner 在提交时继续执行原有并发校验。

Core 和 Application Gateway 共用角色策略，按工具、操作、资源种类、具体资源和结果引用
检查权限。Manager 生成的任务不能改变角色边界，例如 Report 获得 `edit_resource` 也不能
编辑 Database。新资源仅从真实回执取得 ID，并继承明确声明的操作。统计图执行另外在
Application 的准备入口核对实际资源需求、语义依赖和资源版本；已授权的图执行继承 Manager 对实际语义依赖的共享读取权限，显式依赖版本仍须匹配，不继承写入权限。项目会话、审批和资源 Owner 的提交校验继续生效。内部图/资源读取回执保留完整版本；模型只看到业务事实。`inspect_resource` 返回身份、名称和 dirty 状态；函数资源另保留既有签名投影，不返回节点或连线正文。

Worker 使用角色规范、单个任务、必要约束和指定依赖的结构化交付，不复制整个用户会话。
报告 Skill 只加载到 Manager/Report，统计计划工具只提供给 Stats。Worker 以最后一条文本或
Markdown 回复收尾，不要求 JSON 包装，也不限制回复为 32 KiB。Core 将原文放入
`WorkerReport.summary`，从真实工具回执归集资源变化、结果引用和证据调用 IDs；模型不能
通过回复伪造这些事实。`Completed` 表示代理正常结束，Manager 仍需结合正文中的限制和
真实回执判断任务目标是否达成。输入检查、依赖过期和文档未保存由 Core 标记 `Blocked`。

“生成／输出／撰写分析报告”默认交付项目内已保存的 Doc；明确要求只在聊天中回答、不创建文件或简短解释时，由 Manager 直接回答。
Manager 为 ReportAgent 提供 Doc 创建或编辑/保存授权和真实证据引用，Core 绑定实际读取版本；ReportAgent 通过现有资源工具写入 Markdown，并在最后一次修改后显式保存。
对于有 Doc 或 Mind 创建、内容编辑或保存授权的 ReportAgent 任务，Core 根据本次任务的成功回执检查交付：至少有一份获准资源保存成功，且所有仍存在的已修改 Doc/Mind 都已保存。仅返回任务摘要、只创建或编辑、保存失败、保存后再修改都不能形成 `Completed`，会返回 `Blocked` 和 `report_resource_not_saved`。Mind 任务可以独立交付已保存的 Mind，不要求另建 Doc。明确的只读检查、重命名或删除任务仍按其操作范围完成。
报告内容的证据质量仍由 Worker 和独立 Review 负责。Manager 收到完成且含 Doc 回执的结果后请求打开文档，聊天正文只汇总结果、文档位置和限制；失败或受阻不能改为粘贴完整报告并宣称交付。

Manager/Worker 的模型循环持续到模型完成、用户取消或真实执行错误，不设置固定调用轮数、
累计 Worker 数、累计业务工具次数或整个 run 的时间配额。输出 token 上限来自用户配置的模型参数；Anthropic 协议要求配置该参数，其他协议未设置时交由服务决定。任务目标、约束、完成条件与授权列表不套用图 JSON 的
64 KiB 预算或独立条目配额；身份、资源版本、操作和角色权限仍正常校验。
最多 4 个 Worker 同时准入，Manager 工具并发为 4、Worker 为 1；共享读写门串行化所有
写 Worker（包括跨会话），业务 Owner 仍拒绝 GUI 并发造成的过期版本。并发限制和单次
业务工具的查询期限不等同于整个代理任务的运行配额。

这里沿用开源 Codex 的自然完成机制：模型请求工具则继续循环，最终回复交回父代理；
参照 [turn loop](https://github.com/openai/codex/blob/afb436df8b70bb5bc57b86d9a3e829968988cd21/codex-rs/core/src/session/turn.rs)
和 [agent status](https://github.com/openai/codex/blob/afb436df8b70bb5bc57b86d9a3e829968988cd21/codex-rs/core/src/agent/status.rs)。
本项目仍由现有 Core/Rig、工具账本和业务 Owner 承担对应职责。

任务状态复用 SQLite-backed Harness 有序事件流：`AgentRunStarted`、`AgentRunOutput`、
`AgentRunFinished` 和 `AgentRunInvalidated`。工具账本携带 `agent_run_id`，图编辑幂等 key
也区分各 run。并行生产者经异步交付锁按序持久化与发布。Worker 正文不会成为 Manager 的
公开正文；主会话重建原生委派调用/结果，Worker 工具记录保留为独立证据。Manager 的正文由现有 Turn/transcript 保存，Manager run 的 `report` 为空；仅 Worker 返回任务报告。

同一 turn 内的后续提交改变已有任务输入时，Core 标记该任务及其依赖链为 stale，并把
`invalidatedRuns` 返回 Manager。依赖未完成或已过期的任务不能启动；输入版本变化在模型
启动前形成 blocked 结果。Manager 应重新安排受影响的分析、图表和报告。
等待执行门后重新核对依赖，过期则形成 blocked 结果；原读写门保持到失效标记、终态事件和
任务结果登记完成，后继任务不会读取已经释放执行门但尚未结算的依赖状态。

取消向所有已准入 Worker 传播并等待业务操作收尾。启动恢复将未结束的 runs 记为
interrupted，保留已经提交的工具证据，不自动重做可能已提交的操作。前端从相同事件流恢复
任务卡片和运行状态，统计计划继续复用现有展示。真实模型与桌面人工验收见
[Harness roadmap](../../docs/roadmap/STATISTICAL_HARNESS.md)。

`followup_task(runId, instruction)` resumes the same durable run ID, including across user turns and after restart. Core rebuilds scope from the original grant and successful receipts, restores the latest context checkpoint plus subsequent history, and binds any fresh Manager observations only to resources already in that scope. It never adds resources or operations. Without a fresh Manager observation, each granted resource must match the worker's current-project read or commit receipt; older session records do not invalidate a later verified receipt. Another project rebind requires current reads again before that baseline can be reused. Dependencies and input versions are rechecked under the existing execution gate. Unknown commit outcomes block continuation until reconciled; committed graph edits retain their owner-level idempotency. Startup recovery closes unfinished resumed runs as interrupted and never automatically repeats their writes.

Recovery, compaction, delivery checks and graph business outcomes have dedicated persisted events. IPC hides checkpoint text while preserving lifecycle facts and exact failure codes. Reopening a conversation therefore retains provider failures and undelivered status. Tool-call completion is distinct from graph execution success. Model observations carry session, turn and agent-run correlation through tracing spans without recording prompts, data or credentials.

## Source organization

Manager 与 Review 可用 `validate_graph` 读取全图或指定依赖范围的就绪事实；它不授予图编辑或执行权限。没有 Worker 任务信封的 Manager 通过原图检查用例捕获本次只读校验基线；Worker 继续核对授权版本，不以新读取覆盖已过期的写入依据。Report 可检查已授权数据库的概览和 schema，以核对报告的数据来源、行列数和变量类型，不能修改数据库。所有 Worker 仍受原资源读取范围约束；共享工具指令提到的其他角色工具不因此获得注册或授权。

Harness crates 按 Core、Contract 与具体适配器分层，公开类型从各自 crate 根导出。内部按实际职责组织：

| 范围          | 源码入口与职责                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| ------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Core Host     | [host/mod.rs](src/host/mod.rs) 构造共享状态；[session](src/host/session.rs)、[turn](src/host/turn.rs)、[workflow](src/host/workflow.rs)、[approval](src/host/approval.rs) 分别实现对应入口                                                                                                                                                                                                                                                                  |
| Core 公共依赖 | [ports.rs](src/ports.rs) 拥有注入端口集合，[error.rs](src/error.rs) 拥有 Core 错误；编排器不依赖 Host 内部实现                                                                                                                                                                                                                                                                                                                                              |
| 上下文与事件  | [conversation.rs](src/conversation.rs) 统一历史重建和消息组装；[events.rs](src/events.rs) 统一持久事件写入、交付及 Agent 输出适配                                                                                                                                                                                                                                                                                                                           |
| 角色与权限    | [agents/mod.rs](src/agents/mod.rs) 定义六角色、工具及并发度，[policy.rs](src/agents/policy.rs) 执行共享授权检查                                                                                                                                                                                                                                                                                                                                             |
| 编排与交付    | [orchestration/mod.rs](src/orchestration/mod.rs) 调度 Manager/Worker；[delegation.rs](src/orchestration/delegation.rs) 绑定模型任务与读取基线；[receipts.rs](src/orchestration/receipts.rs) 从真实回执归集证据、产物并更新任务范围；[recovery.rs](src/orchestration/recovery.rs) 恢复未结束的运行                                                                                                                                                           |
| Rig           | [driver.rs](../yss-harness-rig/src/driver.rs) 执行模型循环，[provider/client.rs](../yss-harness-rig/src/provider/client.rs) 建立原生协议客户端，[stream.rs](../yss-harness-rig/src/stream.rs) 管理流与取消，[messages.rs](../yss-harness-rig/src/messages.rs) 映射消息，[tools](../yss-harness-rig/src/tools/mod.rs) 映射模型工具，[error.rs](../yss-harness-rig/src/error.rs) 分类失败                                                                     |
| Knowledge     | [knowledge.rs](src/knowledge.rs) 处理查询与引用；[knowledge/tools.rs](src/knowledge/tools.rs) 映射公开工具结果与内部引用事件；[knowledge/index.rs](src/knowledge/index.rs) 管理来源快照与索引发布；[knowledge/retrieval.rs](src/knowledge/retrieval.rs) 拥有 Unicode 分段与引用身份；[knowledge/builtins.rs](src/knowledge/builtins.rs) 安装内置来源                                                                                                        |
| Tantivy       | [lib.rs](../yss-harness-tantivy/src/lib.rs) 实现中立索引构建端口；[index.rs](../yss-harness-tantivy/src/index.rs) 拥有字段、查询和摘要；[tokenizer.rs](../yss-harness-tantivy/src/tokenizer.rs) 拥有中英文分词；[collector.rs](../yss-harness-tantivy/src/collector.rs) 在结果限制前按文档去重                                                                                                                                                              |
| SQLite        | [lib.rs](../yss-harness-sqlite/src/lib.rs) 构造唯一 Store/连接池；各持久化 port 分别实现于 session、events、workflow、ledger、approval 和 knowledge 模块；[schema.rs](../yss-harness-sqlite/src/schema.rs) 拥有唯一 schema，[codec.rs](../yss-harness-sqlite/src/codec.rs) 共享编码与错误映射                                                                                                                                                               |
| Contract      | [lib.rs](../yss-harness-contract/src/lib.rs) 只组织模块和导出；[context.rs](../yss-harness-contract/src/context.rs) 拥有调用身份与绑定，[capabilities.rs](../yss-harness-contract/src/capabilities.rs) 拥有能力注册、请求/结果信封与 schema，[gateway.rs](../yss-harness-contract/src/gateway.rs) 拥有调用控制和 port，[inspection.rs](../yss-harness-contract/src/inspection.rs) 拥有读取投影，[graph.rs](../yss-harness-contract/src/graph.rs) 集中图契约 |

角色提示与通用工具提示位于 `src/agents/prompts/*.md`，由 `include_str!` 将原始 Markdown 文本编译进程序，直接作为系统消息使用，不做 Markdown 渲染或解析。
提示词不承担后端授权，编写约束见 [crates 规则](../.rules)。版本化 Skill 仍位于 `skills/`，由 SkillRegistry 解析。
现有测试随模块组织到对应 `tests.rs` 或并发测试文件，测试功能和协议夹具保持一致。

## 2. Authority

| 事实                                                 | Authority                                           |
| ---------------------------------------------------- | --------------------------------------------------- |
| Project、Graph、Database、Execution、Result 和 SCI   | 原有业务 owners，不转移给 Harness                   |
| Harness session 和 turn                              | `yss-harness-core` + session persistence port       |
| conversation transcript / final turn state           | persisted turn/event records                        |
| Statistical Plan 和 Workflow run/step                | Harness workflow runtime + workflow store           |
| Tool invocation state、idempotency 和 result receipt | Harness tool ledger                                 |
| approval grant lifecycle                             | Harness approval service/store                      |
| Skill identity/version/source                        | Harness skill registry                              |
| Knowledge source/citation                            | knowledge source store；Tantivy index 是 projection |
| ordered Assistant stream                             | persisted Harness events + Rust sequence            |
| rendered conversation/workflow cards                 | GPUI projection，可从持久事件 replay 重建            |

Harness 只保存业务资源的 opaque references、project/session binding、captured revisions、完整结果 JSON、分页数据及其他有界 capability results 和 receipts。Result payload 仍由 Execution `ResultStore` 拥有；Harness 不能复制完整 DataFrame 或成为 Project history。

## 3. Stable contracts and ports

`yss-harness-contract` 是 Pure Leaf，拥有 Harness、Application Gateway、Rig 和 MCP adapter 共享的 stable typed contracts：identities、project binding、capability request/result、tool descriptor、workflow records、approval、knowledge citation、event、cancellation/deadline 和 structured failure。

Contract 的不透明字符串身份共用 `context.rs` 的内部构造与反序列化规则：去除空白后不能为空，UTF-8 编码不超过 128 字节；保留原始字符串，不在读取时归一化。各身份仍为独立类型。

Harness 只通过 constructor-injected ports 使用外部能力：

- `AgentDriverPort`：provider-neutral model turn；
- `CapabilityGatewayPort`：项目资源和业务操作的唯一 capability seam；Core 自有的知识查询通过知识 store/index ports 执行；
- session/event/workflow/tool-ledger/approval/knowledge stores；
- `KnowledgeIndexPort` / `KnowledgeIndexReaderPort`：构造不可变检索投影并查询，不授予来源读取权限；
- clock 和 ID generator。

adapter 不得把 framework type 带入 Core，也不得拥有 policy。Application Gateway 每次调用根据 principal、Harness session、Project instance/session、resource currentness、approval、deadline、cancellation 和 invocation identity 验证请求。模型请求只携带角色、并发度、输出模式、用户执行选项、上下文和工具签名；授权身份由 Core executor 与事件输出持有，不重复塞入 Rig 不使用的请求字段。

## 4. Registered capabilities

项目工具和打开资源的界面意图共用 `yss-project-identity::ProjectResourceRef { kind, id }`。
种类为 `event_graph`、`function_graph`、`chart`、`mind`、`doc`、`database`；ID 是各 owner 的
不透明标识。数据库声明中的 DatabaseId 与 publication key 不可互换。图内部编辑与运行继续
共用节点图协议，传入对应的 `resource.id` 作为 `graphPath`。

桌面按 `ToolRegistry::for_agent(role).with_mode(mode)` 选择工具；Workflow step 和批准执行只注册本次所需的单个能力。Registry 保存允许的能力集合，同一 run 的模型工具和执行器复用该集合，仅为模型请求生成一次输入 schema。下表是共享业务能力全集，每个角色只获得其中的子集：

| Capability                                                         | 作用                                                                           |
| ------------------------------------------------------------------ | ------------------------------------------------------------------------------ |
| `list_resources`                                                   | 读取有界项目资源索引，包含没有打开编辑器面板的图文件                           |
| `inspect_resource`                                                 | 统一资源身份、名称与保存状态，保留函数签名，正文通过领域工具读取               |
| `create_resource`                                                  | 按种类和名称创建图、Chart、Mind 或 Doc，返回实际身份及初始业务引用             |
| `import_database`                                                  | 从明确的 CSV、Parquet、Excel 或 SQL 来源创建数据库，可在同一导入事务中指定名称 |
| `rename_resource`                                                  | 重命名并返回实际新资源身份                                                     |
| `duplicate_resource`                                               | 可指定名称的原子复制，沿用 owner 的身份和名称分配                              |
| `delete_resource`                                                  | 删除获准资源并保留实际提交回执                                                 |
| `save_resource`                                                    | 按资源 owner 的保存边界提交，返回已核对的状态                                  |
| `edit_resource`                                                    | 本轮保留的函数签名编辑                                                         |
| `inspect_chart` / `update_chart`                                   | 类型化配置读取与指定字段更新，沿用立即持久化                                   |
| `inspect_document` / `read_document` / `search_document`           | 文档大纲、章节/字符范围及带原文上下文的精确搜索                                |
| `replace_document_text` / `append_document` / `write_document`     | 原子精确替换、追加与明确整篇替换，保留显式保存生命周期                         |
| `inspect_mind` / `find_topics` / `inspect_topics`                  | 有限大纲、子树搜索和定向主题正文、子主题与引用                                 |
| `create_topics` / `update_topics` / `move_topics`                  | 原子建树、正文/引用修改与最终父子关系调整                                      |
| `delete_topics` / `duplicate_topics`                               | 子树删除或复制，返回实际删除范围或全新主题身份映射                             |
| `insert_rows` / `update_cells` / `delete_rows`                     | 按稳定 rowId 原子插入并初始化、更新单元格或删除行；插入回执交付实际 ID         |
| `create_columns` / `rename_columns` / `delete_columns`             | 原子修改列集合与名称，保留现有身份、默认空值和至少一列的规则                   |
| `cast_columns` / `set_column_semantics`                            | 原子转换物理类型或设置业务语义，验证整个批次后提交                             |
| `export_database`                                                  | 将授权数据库导出为 CSV 或 Parquet，返回真实目标路径，工具层保留原读取基线校验  |
| `inspect_graph`                                                    | 读取图计数、就绪、已有运行状态及按字节预算降级的节点配置和连接概览             |
| `browse_nodes`                                                     | 搜索、按类别筛选并分页浏览可创建节点类型                                       |
| `inspect_node_type`                                                | 批量读取指定类型的参数 schema、默认值和 Pin 数量约束                           |
| `find_nodes`                                                       | 按名称、类型或 ID 筛选图中节点，再分页读取身份摘要                             |
| `inspect_nodes`                                                    | 按 ID 读取指定参数、Pin、schema 和选项，Pin/列分别分页                         |
| `find_connections`                                                 | 按节点及 Pin 交集筛选、分页读取连接与真实端点                                  |
| `create_nodes`                                                     | 单批创建并配置参数、Pin 数量及初始连接，返回节点和 Pin 别名映射                |
| `update_nodes`                                                     | 按明确字段批量修改参数、标签、字面量和 Pin 数量，或精确删除指定可变 Pin        |
| `delete_nodes` / `duplicate_nodes` / `move_nodes`                  | 删除、复制或移动节点，沿用同一原子编辑与历史边界                               |
| `create_connections` / `update_connections` / `delete_connections` | 按真实端点批量连接、按 ID 原子重接或删除                                       |
| `find_constants` / `inspect_constants`                             | 按名称查找常量摘要；按 ID 与值范围读取，文本/集合/表格分别分页                 |
| `create_constants` / `update_constants` / `delete_constants`       | 原子修改类型化常量，可同时创建初始引用节点                                     |
| `search_knowledge`                                                 | 按需搜索当前项目可见的知识，返回摘要和可读取的片段引用                         |
| `read_knowledge`                                                   | 核验并读取指定片段，成功后记录可展开的来源引用                                 |
| `inspect_database`                                                 | 读取名称、行列计数及编辑状态，不附完整 schema 或数据行                         |
| `inspect_database_schema`                                          | 选择列并分页读取类型、物理类型和业务语义                                       |
| `profile_database`                                                 | 只计算指定列和完整性、统计摘要或分布指标组                                     |
| `read_database_rows`                                               | 按列投影、AND 条件与多列排序读取行页，返回稳定 rowId 和真实继续位置            |
| `inspect_result`                                                   | 读取标量、结果结构和分页 schema，交付完整表引用，不读取数据行                  |
| `read_result_table`                                                | 按完整表引用、选定列及行范围读取，返回实际行页和列页                           |
| `validate_graph`                                                   | 全图或所选节点实际依赖范围的只读就绪检查，诊断独立分页                         |
| `execute_graph`                                                    | 自动准备当前图文档的计划并执行，返回实际 run 状态、失败位置与结果 IDs          |
| `list_graph_results`                                               | 按图、节点、Pin 或运行筛选结果引用与有效性，支持分页，包括手动运行产物         |
| `undo_resource` / `redo_resource`                                  | 在已授权的 Graph 或 Database 中导航一个原有历史单位                            |
| `inspect_ui_intent`                                                | 按 ID 读取工作台界面操作回执                                                   |
| `request_ui_intent`                                                | 请求打开六类资源/结果、定位图节点或显示允许的面板，返回待执行回执              |

界面意图契约由共享的 `yss-ui-contract` 拥有，GUI 与 Harness 共用 Application presentation。
Harness 的 `openResult` 意图接收结果工具给出的完整 `resultRef`；工具层转换为 UI owner 的内部来源标识。
请求历史、意图返回值和状态查询使用同一业务投影，不向模型暴露执行会话标识或幂等 key。
模型必须区分意图被接受与前端已完成操作，具体校验、回执、恢复与会话边界见 [工作台界面意图](../yss-ui-contract/README.md)。

节点、连接和常量写入均在 Application 适配到现有 Graph 原子事务，提交整个当前图并保留一个撤销步骤。
`createdNodes`、`createdPorts`、`createdConstants` 是提交时实际分配的映射；复制时节点映射以原节点 ID 为键，
Pin 映射以原端口地址文本为键，值仍是可直接传给工具的真实端点。映射随 Project 幂等回执保留，重放不重新分配。
`update_nodes` 省略标签表示保留，显式 null 清空；Pin 缩减按 owner 顺序保留前面的实例并报告断连。
`update_connections` 先在候选中释放本批所选连接，再整体校验新端点，因此可交换已占用输入并保留连接 ID。
常量类型和值复用 Data Contract，整数以带标签的十进制字符串传输。列表不返回大值；定向读取的 `valuePath`
可选择嵌套对象或列表，文本按 Unicode 字符续读，表格按列和行分页。分页内容不能作为完整常量回写。
模型工具和内部编辑批次共用 `ConstantDeclaration`、`ConstantUpdate` 及按常量 ID 删除的契约。
Application 只通过同一类型化常量入口创建和更新，不另接收未标记原始 JSON 值或推断常量类型。
`apply_graph_edit` 与 `save_graph` 已从模型目录、角色工具集及可调用 schema 移除。内部批量编辑与保存用例继续服务明确工具；已存账本仍按原名称投影业务参数，历史可读不代表工具仍可调用。函数签名分支继续保留。

数据库读取参数只接受 `database: {kind: "database", id}`，由 Core 将 Worker 已获准的原读取基线绑定到内部
`DatabaseReadRequest`；首次读取由 Application 的原查询 gate 建立基线。四种读取结果均推进同一个观察表，
Worker 已有基线不会因读取自动变成新版本。实时结果和历史只投影 `database` 与类型化业务内容。
schema 默认 50 列、最大 100 列；行页默认 20 行、最大 1000 行，显式选择至多 100 列。
过滤条件全部为 AND，比较使用 Data Contract 精确字面量；空值判断不带比较值。排序复用原列语义，
并列值按显示顺序及 rowId 定位。过滤后的总数未知时为 null，分页仍返回实际 `hasMore` 与 `nextOffset`。
画像只运行明确选择的指标组；省略的指标组不会计算。旧 `inspect_dataset_schema` / `inspect_dataset_profile`
不再注册给模型，其内部请求仍服务 Workflow 与既存历史。

`inspect_result` 与界面复用 Application 的有界 `query_result_projection` 和共享 `result_encoding` 映射，返回 `ResultValueInspection::Json`。内联结果受投影展开预算及 64 KiB 编码上限约束，超限明确拒绝，不静默裁剪。原生报告使用概览、参数目录与表引用，统计数据按表引用继续读取。普通结果字段、嵌套对象与文本没有 AI 专用白名单或截断规则。

`inspect_result` 接受完整的 opaque `resultRef`；DataFrame、DataSeries 和内存数列只返回表引用与分页 schema，不读取数据行。`read_result_table` 接受完整 `tableRef`，按列选择及行范围读取。行页默认 20、最大 1000，列页默认 50、最大 100；响应分别携带实际行页和列页。关系选列在读取数据前下推至原查询 owner，未知列或重复列明确拒绝。项目会话、结果可用性、取消和 deadline 检查继续生效。

结构化统计结果的数组保持表引用，包括小数组、空数组和分页内的嵌套数组。完整引用在原投影生成位置编码，用户数据中的 `resultRef`、`tableRef`、`part` 等字段保持原值，不做递归键名替换。模型不接触内部会话和表路径，也不拼接引用。编码只包含原 ResultStore 身份与原表选择，不引入身份映射表、结果租约或第二份缓存。

`list_graph_results` 可按节点、输出 Pin、运行筛选并分页；默认包含当前 Pin 的有效值和保留的过时值，指定运行只读取仍被原 owner 持有的结果。每个引用和读取回执区分 `currentValid`、`currentStale`、`retained`。运行状态、过期会话、已回收结果和不存在的表分别反馈；读取永不触发重算。Worker 授权、委派和历史回放使用同一完整业务引用。

Model-facing schema 来自 Contract 的 `model::CapabilityInput` 所用业务参数类型。
`ModelCapabilityExecutor` 接收公开意图；Core 的 [capabilities.rs](src/orchestration/capabilities.rs) 先授权，
再绑定 Worker 已捕获的资源版本、函数签名版本、图文档与语义依据，生成内部 typed request。
模型不填写 revision/sessionId、图 hash、条件查询 token 或幂等 key。内部 Workflow/审批直接使用原有内部契约。
Rig 实时返回与历史回放共用 `model::capability_result`，工具历史参数共用 `CapabilityInput` 投影；
错误投影保留输入 schema 声明的业务字段、参数范围和下一步指引；不透传内部字段名。
内部 `RevisionConflict` / `GraphDraftChanged` 在实时工具、历史回放与 Worker 预检反馈中通过同一公开错误投影转换。
已确认变化返回 `resource_changed`；历史读取尚未在当前项目中核验，或普通工具缺少必要的读取依据，
统一返回 `resource_read_required`，不能据此断言内容发生变化，也不将其误报为参数格式错误。
两者保留具体 `resourceId`，文档章节错误指引重新读取大纲，模型不协商内部版本。
工具卡片的失败码也复用此投影，内部事件与账本仍保留真实内部诊断。
统计结果和用户值不按字段名递归过滤。

Worker 的资源版本沿真实提交回执推进；新建、复制或重命名回执的 `resources` 包含可继续使用的真实身份、名称、dirty 状态和 Mind 根主题引用，内部另保留完整版本。只在缺少所需正文或发生并发变化时重新读取。函数签名使用独立的读取与提交版本。
图初次操作缺少内部 hash 时仅补充最小概览，并核对完整授权版本；已有编辑/保存回执可直接推进，避免仅为刷新版本重复查询。
同一 Worker 的分页图读取核对语义输入，图编辑/校验/执行通过内部调用上下文将语义依据交给 Application，
复用本次打开图的解析投影在操作前检查。资源或语义冲突会停止后续写入，Manager 读取并重新评估后才能续接。
Worker 压缩和续接通过完整账本重建这些依据，Manager 的新读取只能刷新原授权内的资源。
执行可能发现动态列，因此执行回执使旧图结构依据失效，下次操作按原图授权补读；不把自己的执行结果误判为外部冲突。
Harness 内部不以任意 JSON 代替 request/result 类型。每次调用先写 running ledger record，再由 executor 分派到 Core KnowledgeService 或 Application Gateway，最后持久化成功 result 或 structured failure。两条路径共用授权、取消、deadline、计时和事件收尾；idempotency 命中已有 terminal record 时返回既有 outcome，而不是重复执行。
已解码、已授权请求的业务参数校验失败也记录调用账本和开始/失败事件，不进入实际资源操作；保留校验字段与范围，便于模型修正及界面追踪。

`list_resources` 复用 Project 的 `read_resource_catalog`，沿用图、Chart、Mind、Doc 的路径扫描与数据库声明身份，
不复制 ProjectData、读取文件正文或加载关闭的图。按 kinds 及不区分大小写的名称/路径 query 筛选后分页，默认 100 项、范围 1..100；
`page` 返回实际数量、总数、hasMore 和 nextOffset。返回前重验 Project 索引依据，失败或变化返回 typed failure。
模型条目包含 `resource` 和显示名称；内部投影另有资源与 Project publication revision。新的图查询使用返回的完整引用作为 `graph`，
其 schema 仅接受事件图或函数图。显式消息引用解析使用同一完整目录，不受模型分页限制。

`inspect_resource` 也走这条轻量目录；数据库 dirty 状态独立查询其编辑 owner，不读取行数据。
其内部元信息仅附带 Runtime 已捕获的运行和 schema 版本，用于核对工作流或历史观察；模型投影不包含这些字段。
关闭图的元信息允许没有编辑会话身份；首次打开在相同资源 revision 上补齐会话，之后继续严格比较完整依据。
修改回执只附带与实际提交版本一致的元信息，并推进 Core 授权和历史观察。并发变化或查询失败不会把已提交操作改写成失败，
也不会用较新的元信息覆盖提交基线；缺失的状态由后续明确读取补齐。

`inspect_graph` 的公开输入只有 `graph`，结果包含 `ready`、全图数量及现有运行状态；运行来自原 Execution 恢复投影，
并标注是否对应当前语义输入。该工具不附节点、Pin、常量或结果值。
`find_nodes` 先按名称/标题/ID query、精确 typeIds 和 nodeIds 筛选，再分页读取摘要。
`inspect_nodes` 按 nodeIds 与 fields 读取 parameters、ports、schema、options，默认 parameters 和 ports；
portOffset/portLimit 按每个节点应用，columnOffset/columnLimit 按每个 Pin 的 schema 应用。
未请求的 schema 省略，`schemaKnown:false` 表示未知，不返回伪造的空列集合。
`find_connections` 组合节点及精确 Pin 条件，返回它们的交集和实际连接端点。
公开 `page`、`portPage` 和 `schemaPage` 包含实际范围、总数、hasMore 和 nextOffset；未读取内容仍是未知。
Worker 的分页读取由调用层核对资源与语义依据，冲突页不作为成功结果进入模型上下文。

内部 `observationHash` 绑定图文档/编辑/语义身份及完整查询选项（含分页和筛选），模型不接收该字段或 `ifUnchanged` 参数。内部同一查询携带
`ifUnchanged` 且标识匹配时只返回当前基线、数量及 `content.kind: unchanged`，不重复发送正文。
本地未保留该查询内容时不能使用条件读取；改变查询范围、页码、图版本或语义依赖都不会误命中。
它不维护另一份图缓存，也不改变编辑/执行的权限或版本准入。内部 `view: full` 继续为原编辑与函数用例提供完整 `GraphInspection`，
不属于公开 `inspect_graph` 输入。

`inspect_resource` 只接受 resource，不再提供 graphView、metadataOnly 或正文分页参数。
图概览走 `inspect_graph`，局部筛选走 `find_nodes` / `inspect_nodes` / `find_connections`。
函数资源的既有签名通过 `content.kind: function` 返回，仍可用于范围外的签名编辑；运行数据由结果工具独立读取。
Application 的 `automation/graph/inspection.rs` 从同一次捕获的编辑投影生成查询结果，完整快照和
局部查询复用端口/常量映射；Rig 工具说明及角色提示引导按需读取并复用编辑增量。

资源命令由 Application 的 `automation/resources` 统一分发，调用现有 Graph、Chart、Mind、Doc
和 Database 用例。名称、路径、类型、树约束与持久化仍由这些 owner 校验；不另建 Harness 文档。
`inspect_resource` 的内部回执返回 `ResourceVersion`，调用层保留文档编辑会话和资源 revision。写入拒绝过期版本；
函数签名另有自己的 revision，回执用 `revisionKind` 区分，不能拿签名版本代替图资源版本。
图表基于读取版本的修改在 Project writer 内重验，GUI 独立 Save 保留原来的覆盖语义。
Harness 的 `ChartSettings` 复用 `yss-chart-document::ChartType`；工具 schema 和文档序列化共享
同一个 histogram/scatter/line 枚举，资源适配器直接传递该类型，不另设图表类型定义或字符串映射。

Mind 的公开 DTO 归 Contract 的 `model/mind`，领域回执归 `mind.rs`；Application 的
`automation/resources/mind` 只投影原 `MindTree` 查询和翻译原 `MindEdit` 命令。
`inspect_mind` 默认读取相对深度 2、50 项大纲，最多深度 20、200 项；`find_topics`
执行大小写不敏感的正文子串搜索，默认 50、最多 100 项，并给出至多最后 32 个祖先/自身 ID，
通过 `pathComplete` 明示路径是否完整。摘要最多 160 个 Unicode 字符，不返回整棵树正文。
`inspect_topics` 每次最多 20 个主题，正文默认且最多 2048 个 Unicode 字符，子主题默认且最多 50 个；
两种分页各有实际范围与继续位置。读取保留原会话和版本，在返回前重验，模型投影不含同步信息。

五种主题编辑每次最多 200 个请求项，仍通过 Project 的单次 File Edit 提交。
创建先分配整批真实 ID，再解析 `$clientId` 父引用，因此可以引用稍后声明的父主题；
更新中省略 reference 保留引用，null 明确清空；移动按整批最终关系校验，不因暂时父关系拒绝合法交换。
删除包括全部后代且不能删除资源根；复制子树分配全新 ID，外部引用原样保留，失效外部目标不等同于坏树。
`mindEdit` 回执提供实际创建映射、影响主题、删除主题及 dirty；回执体积在提交前核对，非法批次不发布。
Report 可写 Mind，其他角色仍受原只读边界约束。主题工具沿真实读写回执绑定版本，显式 `save_resource`
完成持久化。原 `edit_resource` 的 Mind 参数、`MindOperation` 和通用资源回执的 `createdNodes` 已移除。
`inspect_resource` 的模型参数由 `model::InspectResourceInput` 定义；正文分页由各领域工具声明单位、范围及继续位置。
Markdown 的范围按 Unicode 字符计数；局部编辑使用 `replace_document_text` 的 replacements 列表，
在同一个捕获正文中按顺序匹配唯一原文。空原文、零匹配或多处匹配拒绝整个批次，返回字段与修正指引，
不提交前面的操作；成功后仍通过原 Doc owner 按捕获版本原子提交并显式保存。未读正文保持不变。

六个公开文档工具的 DTO 归 `model/document`，Application 的 `automation/resources/document`
调用原 Doc 快照、`DocView` 和 File Edit。`inspect_document` 默认 50、最多 100 个标题，正文长度
独立于大纲页；`read_document` 默认 8192、最多 16384 个 Unicode 字符，在可行时沿完整块边界结束，
返回实际绝对范围、所选范围、相对分页位置及本页是否覆盖整个所选范围。
`search_document` 是大小写敏感的原文子串搜索，包含重叠命中，默认 20、最多 100 项；
每侧上下文默认 120、最多 500 个字符。若 JSON 转义导致结果超出共享预算，减少本页条目并返回实际继续位置。

章节引用包含标题序号和字符位置，区分重复标题。Core 在现有 ResourceObservations 中从读取回执
记录每个引用的内部文档版本，随历史重放恢复并传入 Worker；正文编辑只推进资源版本，不给旧章节引用
偷偷换基线。旧引用或 GUI 并发修改会被拒绝，重新读取首张完整大纲页才能刷新定位。
模型无需接触版本、会话或校验摘要。该观察投影不拥有正文或持久化 AST。

`write_document` 表达明确的整篇初始化/替换；`append_document` 原样追加，包括调用者提供的换行。
读页大小和原 Graph 的 JSON 输入预算不限制完整文档写入，实际大小仍由 Doc owner 的 `MAX_FILE_BYTES`
约束。`documentEdit` 回执返回每步实际 Unicode 变更范围、最终字符数和 dirty，成功后继续通过
`save_resource` 保存。旧模型 Markdown 操作枚举及 `edit_resource` 的 Doc 分支已移除，Report
不再注册通用内容编辑工具；报告提示和内置写作 Skill 使用这些明确工具。
数据读取按行分页并返回稳定 row IDs、物理类型、列语义及历史状态；复制通过既有导出/导入 owner
保留数据和 Schema，产生新的数据库身份。导出在替换目标前重验版本，发布结果不确定时不自动重试。

Doc/Mind 修改保留现有 Edit/Save 生命周期；`undo_resource` / `redo_resource` 只支持已有历史的 Graph 与 Database，要求 Edit 授权，并由调用层绑定原读取版本。通用模型编辑不再接受 GraphHistory 或数据库 Undo/Redo 分支。账本记录明确的公开工具名；实际历史与保存边界仍归原 owner，Graph 历史导航不自动保存。

数据库行修改使用三个明确工具，`database` 引用在 schema 中只允许 Database。Data 角色还须持有该资源的 Edit 授权，Core 在既有资源 envelope 中补齐此前观察版本；成功回执继续推进同一观察状态。`insert_rows` 按可选 `beforeRowId` 或追加位置初始化行，省略列为 null；`update_cells` 拒绝重复 rowId/column，null 显式清空；`delete_rows` 只接收稳定行 ID。每批最多 200 个请求项，Store 在原准备/提交路径验证整个批次，形成一个 Runtime 历史单位；这不是计算行数限制。公开回执的 `databaseEdit` 返回实际插入 ID、提交项数、dirty 和历史可用状态，不返回行值或内部版本。通用 `edit_resource` 不再提供单行插入、单元格修改或按页位置删除分支。

五个列工具使用同一 Database 授权与回执路径，每批最多 200 个列目标。新增列初值为空，重命名支持最终名称交换且保留身份；删除必须保留至少一列。物理类型名称由原 Arrow owner 解析，每列转换要求显式 `force`：允许无法表示的值转为空，但不能绕过语义约束。语义修改只更新元数据并验证全部当前值。批次失败不提交，成功只产生一个撤销单位；`databaseEdit.columnNames` 返回本次提交的目标名称（重命名后的名称或已删除名称）。原模型 `DatasetOperation` 与 `edit_resource` 的 Database 分支已移除，Data 角色不再注册通用内容编辑工具。
图表设置修改立即持久化。Chart 没有 Rust 未保存缓冲区，`save_resource` 的 Chart 保存
确认已持久化版本，不读取或保存独立的前端配置草稿；后续发布继续保留前端未提交的修改。
资源回执返回实际变化、删除标识、移动和创建的节点 ID，不猜测新名称对应的路径。
Gateway 将真实提交送入既有 Project 事件发布入口，使侧栏和编辑器通过同一发布协调器刷新。
通知交付失败不会把已提交写操作改写为未提交；模型仍根据实际回执继续。

Application 的 `invoke_automation_capability` 是同步业务入口。`yss-application::harness` 的 `ApplicationCapabilityGateway` 使用 Tokio blocking worker 调用它，供桌面及独立测量共用，避免在 async worker 中嵌套 DataFusion 的 `Runtime::block_on`。`CapabilityControl` 携带单次调用的 monotonic deadline、turn cancellation 和查询取消标记；profile 把同一预算传入数据库/DataFusion 查询。只读任务在取消、超时或调用 future 被丢弃时通知查询停止；worker panic 转为安全的 `InternalFailure`。已开始提交的写操作等待真实 receipt，不将成功提交改写为超时或取消。

工具生命周期事件由持有 ledger identity 的 Harness executor 产生：准入后发送 `ToolInvocationStarted`，结束时发送 `ToolInvocationCompleted` 或带 `failureCode` 和结构化 `failureDetails` 的 `ToolInvocationFailed`。普通工具与控制工具均保留分类所需的原因；IPC 使用完整诊断投影公开错误码。失败码区分取消与超时；事件不携带数据行、结果或异常原文。账本或事件持久化失败可能留下待恢复记录；已完成的业务提交仍返回真实结果，不被交付失败改写。Rig 只执行模型工具映射，不生成另一套工具完成状态。

图操作由 `ApplicationCapabilityGateway` 直接调用 Rust Application，读取 Project 当前编辑版本并核对 revision/hash。编辑、只读校验、执行与保存使用同一状态；校验返回就绪状态和诊断，Execute 在后端准备匹配计划。Rust 返回真实 capability result，Graph Activity 通知前端更新编辑投影与运行状态；前端不生成 tool result。

`execute_graph` 使用 graph 与可选 nodeId/mode；未指定节点时全图重算，指定节点时采用 `currentInputs` / `dependencies`
模式，分别消费当前已求值输入或补算必要依赖，节点默认 dependencies。局部执行不要求无关分支就绪。输入缺失或过期返回
`input_result_unavailable` 与源位置；内部查询计划不计作已发布的可查看结果。

同一次图请求复用打开图时捕获的文档、编辑身份和解析投影，图检查、校验及 Harness 执行准入不重复读取和解析同一图。编辑与保存仍由 Project 在提交时重验捕获版本，执行准备继续重验文档及依赖。快照直接以 graphPath、revision、graphHash 和 semanticInputHash 标识；编辑请求的 graphHash、节点端口与参数、快照事实和编辑回执中的必需字段缺失时拒绝解析，不补成空值或空集合。

明确的节点、连接和常量写工具转换为内部 `ApplyGraphEditRequest`；调用层补齐 `baseRevision`、`graphHash` 和幂等身份，Owner 拒绝过期请求。创建节点的参数、Pin 总数和初始连接可一次提交，常量可连同初始引用节点创建。每批只添加一次图历史变更；任一操作失败都不安装部分候选。调用层按业务参数与实际读取依据生成内部 `clientKey`，在 session/turn/run 内幂等；同一依据的重复请求返回已有 receipt，复用 key 修改请求会被拒绝。

Capability invocation identity 由 ledger 已保存的 idempotency key 确定，Application 将 principal、Harness／Project 会话、调用身份及 client key 映射为稳定的内部 operation ID。工具账本从已绑定请求借用 `clientKey` 来生成 session/turn/run 范围的幂等键，不为读取该字段展开操作列表或复制常量值；回执恢复直接移交所构造的编辑请求。Project 将请求指纹、实际提交版本和创建元素映射与图编辑一起提交。若 gateway 回复丢失或 ledger 收尾失败，`recover_graph_edit` 仅查询该回执，不执行编辑；恢复后补写原工具记录并返回原节点／端口 ID。回执有条目及字节上限；未知、过期或会话已结束的结果保留不确定性。此恢复服务所有明确图编辑工具的共同提交内核，不提供执行和保存的跨进程重放。

`create_node` 的 `parameters` 和 `portCounts` 均为必需映射，快速创建传空映射；可直接提交列名列表及可变端口总数。
声明 `clientId` 后，初始用户端口按模板顺序获得 `$clientId.templateKey[0]` 形式的批次内实例别名；
后续连接使用该别名，回执交付真实端口地址。序号只用于本批别名，不进入文档连接身份。
参数合并、未完成编辑、资源绑定及端口数量限制与 GUI 共用 Graph Editor 和 Protocol。

`browse_nodes` 按名称、别名、技术词及类别筛选，返回不含配置的分页类型摘要和资源绑定；
搜索保留 `to_numeric` 与完整 type ID，不拆解标识符；ASCII 词匹配边界，避免 `bin` 命中 `Durbin` 等词中间，中文仍按子串匹配。实际范围与继续位置由 `page` 交付。已知类型可直接用 `inspect_node_type` 批量读取，无须先搜索；复用本轮已读定义。它返回 `configurationSchema`，描述创建请求的
`parameters` 和 `portCounts`，另附端口模板、方向和固定/可变/派生分类。Schema 从冻结 Registry
中的 Node Protocol 生成，Application 补充本地化文本；仅为指定且可创建的类型生成详细定义。
省略参数保留默认值或未完成配置，Null 表示重置；`x-yss-requiredForExecution` 标明运行所需字段，
`x-yss-activeWhen` 标明条件字段，`x-yss-linkedPorts` 表示共享数量的模板。
固定值、枚举、数值范围、列列表和无损筛选字面量都由对应类型投影，旧的参数定义 DTO 不再并行返回。
具体节点 Schema 由目录发现提供，不把整个目录嵌入每次模型请求；后端继续通过 Graph 校验实际实参。
`update_nodes.removePins` 按真实地址删除可变 Pin 并保留其他 Pin 身份，与 portCounts 互斥；连接后的选项使用 `inspect_nodes.fields: ["options"]` 获取。
无需先创建节点、连接或运行才能查询；缺失类型明确返回业务失败。工具集成测试与真实模型任务验收分别记录。

`inspect_graph` 的初次概览复用同一次捕获的文档和解析投影，先移除编辑器说明、参数选项、列 schema 和无关端口详情，再按实际 UTF-8 JSON 字节数检查 32 KiB 的结构预算；使用共享的计数 writer，不额外生成完整编码缓冲区。`overview.detail=configuration` 提供全部节点、已保存参数及当前可见默认值、字面量覆盖和连接；超限时 `topology` 保留全部节点身份和连接，省略参数及字面量；仍超限时 `counts` 省略结构，保留外层计数、就绪和运行状态，转向定向查询。省略层级明确，不截断 JSON，也不以空数组暗示图中无节点；常量正文和运行数据仍单独读取。项目打开不自动读取所有图，只在任务涉及该图且缺少结构事实时读取一次并复用。

内部 `GraphEditReceipt` 保存图修订、graph hash、`clientKey`、创建元素的身份映射及本次提交的 `changes`。节点、端口、参数、连接和常量与内部检查共用完整事实结构，包含派生列和端口，连接包含实际顺序。删除节点、连接和常量明确返回 ID；未变化实体省略。`ready` 和 `diagnostics` 是提交时的完整状态，替换此前诊断。所有图编辑操作统一比较提交前后解析投影，覆盖下游连带变化，不根据模型请求猜测结果。

模型编辑回执由 Contract 统一投影，实时调用与历史回放使用同一结构。变化节点保留 ID、类型、标签、位置、参数键值及所有端口的地址、方向、类型、孤立状态、连接数和字面量；不重复发送节点编辑器元数据、参数选项或列 schema。连接、移除项、实际创建映射和完整诊断仍返回；需要选项或列时使用 `inspect_nodes` 定向读取，省略不表示空集合。内部完整回执仍用于恢复原提交。

参数事实中的 `options: null` 表示候选尚未知或不适用，`options: []` 表示已知空候选集；固定枚举直接返回选项。`contextHint` 提供来自同一语义投影的输入结构提示。配置提交成功不表示可以执行，模型继续读取 `ready` 与诊断。

内部差分绑定 `fromRevision` / `toRevision` 及语义输入 hash；调用层消费这些字段，模型仅获得实际变化和 ready。已有回执足够时直接继续；缺少业务事实或发生冲突时再读取。保存回执的资源版本由调用层用于下一次编辑。历史重放与幂等重试保留原提交事实，不能覆盖较新证据。

Application 复用打开图时的基线和编辑流程的最终解析投影，在提交前构造并验证完整差分；不在提交后独立查询当前图来补回执。Project 将有界的调用方事实与文档、历史、版本一起提交，恢复时由 Application 解码为 typed changes，不重新解析较新的图。工具完整回执和 Project 单次 correlation 的预算均为 1 MiB，后者另受每图 2 MiB 的总保留预算约束。回执超过任一预算时，在写入前返回 `result_too_large`，调用方可缩小批次；不静默裁剪节点、端口或诊断。回执不暴露内部 Project 操作 ID；Harness 的 session、turn、tool 等编号继续使用独立类型的通用 UUID。

内部常量检查和编辑差分都包含覆盖完整内容的 `contentHash`；模型投影只返回名称、类型、展示元数据和可展示值。长文本、列表或表格等值即使以 `valueIncluded=false` 只暴露元数据，内容改变也会产生新的事实和差分，不因展示元数据相同而漏报。

`execute_graph` 从本次 `RunGraphReceipt` 取得已发布结果引用，不在运行后重新查询当前图的结果索引。后续编辑、另一次运行或结果失效不会改变原回执。`resultCount` 是成功运行实际发布的结果数，失败时为 null；`resultsComplete` 明确区分完整列表和受能力条目上限约束的部分引用。结果引用不取得新的租约，后续读取仍检查实际可用性；部分列表不能当成该次运行的全部结果。

运行 `timing` 来自 Execution 的单调时钟，分别给出 admissionMs、runningMs、finalizationMs 和 elapsedMs。
运行阶段包含调度、资源准备及物化，不等同于纯节点计算或工具往返时间；仍在运行时查询给出当前经过时间。
成功、失败、取消后计时固定；准入前拒绝或运行元数据不再保留时为 null。图概览和按 runId 查询结果复用同一记录。

所有改变状态的模型能力按各自 owner 返回事实：

| 请求                       | 返回的提交或执行事实                                          | 后续读取条件                                                       |
| -------------------------- | ------------------------------------------------------------- | ------------------------------------------------------------------ |
| 节点、连接和常量写工具     | 创建 ID、受影响实体、删除 ID、就绪状态和诊断                  | 发生冲突或缺少业务事实                                             |
| `save_resource`            | 原 owner 确认的资源及 dirty 状态                              | 后续状态变化或需要未掌握的内容                                     |
| `execute_graph`            | 实际 run 状态、阶段耗时、失败位置及完整结果引用               | 通过 `inspect_result` 读取概览，`read_result_table` 读取所需数据页 |
| `request_ui_intent`        | 意图身份和实际回执状态                                        | pending/claimed 时查询完成状态，不能把接受当成界面已执行           |
| `propose_statistical_plan` | Harness 校验且持久化成功后返回 accepted 和实际记录的完整 plan | 计划正文可直接继续使用，不代表分析已经执行                         |

节点、连接和常量写工具自动保存整个当前图文档，包括调用前已有的手动编辑，无需打开编辑器。
它复用 Project 的文件事务，将文件、当前文档、保存指纹、可撤销历史及编辑回执作为一次提交；
保存失败返回 `persistence_unavailable`，保留调用前的文件、文档和历史，不产生成功回执。
成功后 dirty 为 false，批次仍可整体撤销；重试读取原回执，不重复修改或保存。
校验和执行不隐式保存。显式 `save_resource` 用于用户要求单独保存现有编辑，复用正常 Save。
图工具与桌面编辑共享 Project 编辑状态和历史，调用始终校验项目会话与编辑版本。

GraphPortInspection 的 declared/instance 字段统一使用 camelCase，序列化、会话持久化读取与 capability schema 使用同一契约，不接受 snake_case 别名。

SQLite adapter 只接受当前 schema，不执行旧记录迁移，也不维护迁移版本字段。数据库文件通过 SQLx 的原生路径入口打开，目录名中的 `%20` 等字面字符不做 URL 解码，也不经有损字符串转换。空数据库在事务中创建全部当前表；已有数据库的 DDL 必须与 adapter 拥有的 schema 一致，JSON 列由 `json_valid` 约束保护，读取再使用当前类型校验。不兼容结构返回 `InvalidRecord`，保留原表与记录，不自动重建。桌面启动错误保留 Harness 初始化阶段和持久化错误码，便于区分结构不兼容与数据库不可用。需要重新初始化时，应先备份并移走应用数据目录下的 `db/statistical-harness.sqlite` 及其 SQLite sidecar 文件，再启动应用；此操作不由初始化代码自动执行。当前诊断词汇统一由 `yss-graph-diagnostics` 提供。

节点搜索对 node ID、标题、别名、技术词及资源名分词排序，完整匹配优先，混合语言短语允许部分词命中。profile 的 null 指标表示未计算；复杂常量只暴露类型和 metadata，不复制 tabular 数据。已有图的校验和运行不要求重新设计统计方案。

## 5. Session, turn, and events

每次提交携带 `HarnessTurnOptions`：`mode` 为 Ask 或 Write，`reasoningEffort` 为默认（null）、Low、Medium 或 High。Core 在 `TurnStarted` 后持久化 `TurnConfigured`，Manager 和该轮所有 Worker 共享不可变选项。Ask 的 Registry 与 executor 只允许效果为 Inspect 的能力，禁止图计算、资源写入、导出和 UI 意图；委派及恢复旧任务也必须满足只读 scope，不能通过 Worker 绕过。Write 沿用既有角色、任务范围和 Gateway 授权，并不扩大权限。

一个 Harness session 绑定明确 principal 和当前 Project instance/session。用于 Assistant 对话的 session 还保存 `HarnessConversationMetadata`：项目根目录的稳定文件系统身份、首条消息生成的标题和最近打开时间；底层 workflow/session 调用可不带对话元数据。每个 session 同时只准入一个 active turn；submit、cancel 和 project/session currentness 由 Rust 控制。Session 状态为 `Active` 或 `Stale`：项目绑定失效时标记 `Stale` 并取消正在运行的 turn；重新打开对话时验证归属并恢复为 `Active`。Session store 的 `load_active_sessions` 只返回当前有效的绑定。

Host 的 `HarnessSessionAccess` 串行化绑定协调、对话选择和 session 写入；Application 持门重验其捕获的项目，门本身不保存当前绑定。普通 session 创建和首次消息的标题写回也使用同一门；submit 在门内读取 session 并核对调用方捕获的项目绑定，失效则返回既有 `SessionNotActive`。会话读取、turn 准入和标题写入后释放门，再创建 turn 和执行模型，因此项目切换仍可取消正在执行的 turn。

对话及其完整事件/工具账本继续保存在应用的 SQLite 中，不跟随 Assistant 面板卸载而关闭或删除。左侧 Assistant 只查询、搜索、新建和重命名当前用户及项目的对话，显示最近打开时间；打开列表不隐式创建或订阅会话。点击条目在 sidebar 右侧、top 工作区左侧的 AI 专用分组打开对应会话面板，该组不显示顶部标签栏，选择和切换由左侧列表驱动，再次点击当前可见条目关闭对应面板。各面板拥有独立的事件投影和 runtime，重新连接始终恢复该面板的明确 `sessionId`。重命名通过 Rust Session owner 更新标题，并与运行准入互斥。执行期间可以切换查看其他会话，关闭面板或切换侧栏不取消后台任务；旧订阅和迟到回调不能混入其他对话。普通草稿、尚未确认提交的原文和待发送消息按会话保存在本地输入存储中，不充当已提交聊天历史。原生布局与面板开关由 [GPUI Workbench](../yss-desktop-gpui/README.md) 拥有；上述独立无标签分组和跨重启输入保留仍是 [React 参考](../../react/src/modules/workbench/README.md) 中待补齐的交互。

重新打开项目时，Application 通过既有 RootBinding 获取项目根目录身份，验证对话归属后重新绑定当前运行期 ProjectSessionBinding；历史 receipt 保持原始身份，不恢复旧授权或执行结果。订阅和发送均验证当前项目归属和运行绑定。无已保存项目时使用仅当前激活有效的临时归属。无持久项目归属的记录保持原状，不自动猜测或迁移到某个项目。

恢复历史后，Core 从已有 ResourceObservations 派生尚需读取的资源标识，在 Manager 首次采样前提供业务提示。
提示只包含资源标识和读取要求，不包含内部版本、会话绑定或 hash。重新读取后该要求消失；忽略提示的委派仍被拒绝，
不能把历史依据直接替换为最新版本以放行操作。

`list_graph_results` / `execute_graph` 的每个结果交付完整 `resultRef`，模型原样传回即可。内部解析仍校验原执行会话与结果 ID；项目重启后的历史编号不能读取到新结果，过期引用返回 `ResultUnavailable` 和 `reference_expired`。

Turn 流程是：

```text
validate and persist user turn
  → publish TurnStarted
  → rebuild context from durable history and explicit resource references
  → AgentDriver invokes registered typed tools, including on-demand knowledge search/read
  → persist tool ledger and ordered events
  → persist terminal turn state
```

每个 Harness event 包含 stream/session/sequence 和相关 turn/workflow identity。事件先进入 durable store，再交付 live channel。Frontend 订阅从 last seen sequence replay；出现 gap、断线或交付竞态时重新订阅并 replay，不把本地数组当作 durable transcript。

`HarnessEventStorePort::append_event` 在一次持久化操作中分配会话内的连续序号并写入事件，返回已提交的 envelope。
SQLite 在 `BEGIN IMMEDIATE` 事务内读取当前最大序号并 INSERT；失败回滚不消耗序号，独立连接使用同一分配规则。
Host 只发布成功提交的 envelope，不维护第二份序号计数器；本 Host 的发布锁保持 live delivery 顺序。
发布失败后的事件仍可重放，前端继续严格检查连续序号，不跳过缺号。

terminal event 和 persisted terminal state 都由 Harness 产生。取消会封锁或忽略 late model/tool output；frontend stop action 不能把已经完成的业务 commit 改写为“取消成功”。

Host 在 active-turn 锁内捕获当前 turn 的 cancellation token，释放锁后执行取消并唤醒等待者，允许同步 wake 回调再次进入 Host。取消只作用于捕获的 token，不影响同一 session 后续准入的 turn。返回值表示本次请求是否首次取消该 token，不表示业务已经停止；首个原因由 token 的 CAS 决定，并发请求不承诺按取得 active-turn 锁的顺序获胜。

模型取消或超时后，Rig 停止模型请求并等待已准入工具完成 ledger/终态事件收尾，然后 Harness 结束 turn。共享 cancellation token 支持多个等待者；future 注册前后检查取消原因，waker 的克隆、释放与唤醒均在 waiters 锁外进行。启动恢复结束遗留 running invocation/turn；中断的 mutation 使用 `outcome_unknown`，不推断已经回滚。已持久化的真实 receipt 保留；内存编辑回执随 Project 编辑会话释放，进程崩溃后不能从当前图内容猜测旧工具是否提交。完整跨进程 commit reconciliation 仍属于 roadmap。

## 6. Workflow and statistical plan

Harness 生成 typed Statistical Plan，而不是让 model 自由决定数值事实。计划区分 research question、analysis mode、study design、estimands、variable roles、candidate methods、selected workflow、diagnostics、robustness 和 reporting needs。

当前 Assistant 的数据质量检查通过 DataAgent 的 `inspect_database_schema` 和 `profile_database` 能力执行。通用 Workflow runtime 接收调用方构造的 versioned definition；每个 `WorkflowStep` 直接包含 typed capability `request` 和依赖列表，compiler 校验 step identity、dependency existence、self-dependency、cycle 和 capability request。Runtime 持久化 run/step state，并提供 plan、advance、pause、resume 和 cancel 操作；桌面没有手动工作流控制入口。

每个 run 的 advance 持有唯一执行租约；并发 advance 返回 `ConcurrentWorkflow`，状态转换使用短时异步互斥，能力调用期间不持有该转换锁。
运行记录携带 revision；`save_run` 的创建仅允许不存在的记录，更新必须匹配预期 revision，成功返回递增版本，冲突不覆盖当前记录。
取消先持久化 `Cancelled`，再取消正在执行的能力所共享的 token；迟到结果不能更新终态或发布步骤完成事件。
暂停阻止新步骤派发，必须显式 resume。已准入步骤可以在重读当前版本并核对执行尝试后收尾，但保持 `Paused` 或恢复后的 `Ready`；再次 advance 才继续派发或确认完成。
恢复跳过仍有执行所有者的 run。中断的 Inspect 步骤可重新调度，并保留已持久化的暂停状态；其他中断步骤标为 `TerminalFailure`，run 标为 `Failed`，避免重复执行可能已提交的操作，实际提交结果以工具账本和业务回执为准。

统计计划未通过校验时，工具反馈具体失败原因与 MethodRegistry 的当前方法卡（方法 ID、研究设计、变量角色和诊断要求），供模型修正后重新提交；schema 解码失败返回字段、枚举、数值/长度范围及对应字段说明，不回显原始参数。`selectedWorkflow` 是不超过 128 UTF-8 字节的短工作流标识，不接收大段分析描述。工具、并发度和输出模式统一由 Core 的角色配置传入 Rig。

Workflow run 绑定 exact definition ID/version 和 Project session。恢复或继续前必须重验 binding/currentness；step output 仍是 typed capability result，不允许 model 自行制造 estimate、p-value、standard error 或 confidence interval。

## 7. Skills and knowledge

当前实现包括：

- builtin、versioned Skill source 和 exact resolution；
- builtin statistical knowledge 安装；
- Tantivy BM25 检索、中英文分词、命中片段摘要和可核验 source citation；
- 六个角色共用按需 `search_knowledge` / `read_knowledge` 工具；
- SQLite persistence ports for sessions/events/workflows/ledger/approval/knowledge。

Skill 是包含指令正文的版本化方法包，manifest 记录 ID、版本和来源哈希。`SkillRegistry` 是当前内置方法包的来源，SQLite 不存储 Skill 安装包。当前内置 Skill 用于上下文预加载；执行权限由实际角色/任务范围和 Gateway 校验。内置 Knowledge source 是 authority；项目来源的正文 authority 仍为 Project，SQLite 只保存显式来源选择和检索快照。搜索索引可以重建。

同一 ID 和版本的 Skill 只有在 manifest 与正文均相同时才视为重复安装；正文变化但沿用原哈希仍是冲突，注册表拒绝该包并保留已安装内容。

Application 装配 `yss-harness-tantivy`，Core 只依赖中立索引端口。Tantivy 使用 BM25，标题与 scope/标签分别加权；英文按单词、中文按相邻双字分词，索引、查询和摘要使用同一 tokenizer，保留原始 UTF-8 偏移。这是字面检索，不承诺同义词或语义召回。Core 按 Unicode 字符切出重叠片段，Tantivy 先过滤可见文档、再选每份文档的最佳片段，最后限制结果数量，避免一份长文档占满结果。若命中在最终核验时失效，Core 排除已检查文档后继续查询，避免旧来源挤占当前有效结果。

普通轮次不预先检索或把命中片段拼入用户消息。模型需要方法背景时调用 `search_knowledge`，再通过 `read_knowledge` 获取完整片段；两者使用原生工具结果并随账本重建历史。查询只接受文本、scope 与结果数量，读取只接受文档/片段定位；project binding、来源版本、hash 和索引缓存标记由后端处理，不进入模型 schema、返回结果或历史。Knowledge 文本始终是参考数据，不能授予权限或替代实际计算结果。

只有成功读取才产生 `KnowledgeCited`，搜索摘要不自动成为引用。完整引用身份保留在内部事件和桌面引用投影中，Manager 与 Worker 的引用均可从主会话展开；读取已删除、已变化或不可见的片段返回可重新搜索的业务失败。来源存储或索引暂时不可用作为工具失败反馈，账本与事件持久化失败仍保留运行错误语义。查询取消或到期通过同一工具生命周期关闭账本，不发布迟到引用。

SQLite 通过 `replace_source` 在一个事务内替换来源及完整文档集合，跨来源的文档 ID 冲突会回滚整次更新。来源写入或删除使本 Store 的内部缓存标记失效，普通会话写入不影响它；标记不持久化，也不出现在模型参数、结果或历史中。Host 持有共享 KnowledgeService，未变化的查询复用已发布索引，只读取命中文档进行最终核验；引用展开直接按文档 ID 读取，不再扫描整个知识库。移除来源在同一事务中删除其文档快照，保留来源删除标记用于拒绝旧引用。索引放在内存中，从来源快照重建；重启后首次查询重建，来源变化后下次查询重建。当前不维护独立磁盘索引或增量段更新。

分段与 Tantivy 构建、查询运行在 blocking pool，来源 I/O 和构建不持有 Core 的索引发布锁。并发的旧构建不能覆盖已发布的新快照；取消后的迟到构建结果不会发布到会话。引用身份绑定文档、来源 hash、片段位置和实际内容；搜索返回前重新核验当前来源和片段，展开也复用同一规则。来源删除、版本变化、正文变化或项目不可见时旧引用不可用；可见性检查先于来源完整性检查，其他项目的失效来源不会阻断当前检索。项目会话变化也使 Core 索引缓存失效，避免将旧会话的可见性投影复用到新项目。

设置中的“项目知识库”通过 [Application knowledge](../yss-application/src/harness/knowledge.rs) 管理明确选定的项目 Markdown 文档。来源身份绑定已有项目根身份与资源路径，项目重开恢复选择；另存为或切换到其他根不会隐式复制来源选择。添加/重建捕获当前 Rust Doc 正文，以内容摘要绑定检索快照；用户修改正文后来源显示需要重建，空文档和不可用文档各自显示状态。重命名后原路径不可用，用户可将新路径重新加入。源文档没有被知识库复制成另一份可编辑正文。

该 Application 服务实现现有 KnowledgeSourceStorePort，在索引载入时筛选当前项目，并把持久来源映射到当前项目会话；每次命中/片段读取通过 Project 的 `read_doc_source` 再核验当前正文、原始资源路径、磁盘保存指纹和读取期间的编辑版本，随后重验 SQLite 来源没有被删除或替换。未保存正文可以显式加入；外部删除或未处理的外部修改使旧来源不可引用，Project 的脏正文恢复能力保留。文件读取与正文摘要在 blocking pool，Project 与 SQLite 的状态锁不跨这些操作持有。管理请求绑定桌面捕获的项目身份，迟到回复不会更新后继项目；内部项目 key、会话、摘要不进入模型协议。

Assistant 展开引用返回片段正文与可选原始资源定位，打开原文前再次校验来源，随后复用已有文档编辑器入口。内置方法没有原始项目文档按钮。折叠后再次展开重新读取，切换会话/项目丢弃未完成的详情请求。桌面人工验收仍见 roadmap。

会话上下文由对话事件、工具账本和上下文压缩检查点重建，用户消息不另存为独立记忆。当前契约没有会话记忆类型、事件、数据库表、读写接口或前端投影；SQLite 和事件解析只接受当前契约，不保留旧记忆数据的兼容分支。

当前注册并加载的 Skill 是 [statistical-report-writing](skills/statistical-report-writing/SKILL.md)，拥有统计证据、公式、表格竖线、显著性标记、金额转义及交付检查规则；Markdown 解析选项和布局仍由共享前端渲染器负责。

Host 初始化时通过内置 `SkillRegistry` 精确解析 `yssbi.statistics.statistical-report-writing@1.0.0`，将 ID、版本和原始规范作为独立 System 消息预加载到每次 ManagerAgent 和 ReportAgent 请求，位于基础工具策略之后、对话历史之前；source hash 仅用于内部校验，不发送给模型。Skill 的适用条件限定为生成、修改或续写统计报告，涵盖 Assistant 正文和 Doc 内容；无需按当前消息关键词猜测，也不会因后续修改省略“报告”一词而丢失规范。ReportAgent 复用同一写作 Skill，Manager 直接撰写统计正文时同样适用。当前没有模型侧 `load_skill` 工具；内置方法包由 Core 加载。新增 Skill 文件必须同时注册并接入上下文，单独添加文件不会生效。

Hybrid/vector retrieval 和 remote Skill trust 尚未成为 current production contract。

## 8. Rig adapter

`yss-harness-rig` 实现 `AgentDriverPort`，负责 provider/model configuration、Rig message mapping、streaming、tool schema/call mapping 和 provider failure 分类。它不拥有：

- Harness session、workflow 或 event sequence；
- Tool Registry 或 approval；
- Project/Graph/Database authority；
- capability authorization/currentness。

Application 的 `harness/models.rs` 是 provider/model 配置唯一 owner，保存到 app-data 的 `settings/language-models.json`。每个 provider 拥有稳定 ID、供应商名称 `name`、可选的独立自定义名称 `customName`、协议、Rig adapter、认证方式、API 根地址和模型目录；模型包含 ID、名称、可选上下文/输出容量、Temperature、Top P 与 JSON 扩展参数。采样参数留空时不覆盖供应商默认值；扩展参数使用该协议的原生字段，只配置生成选项，不能替换 Harness 的消息、工具、认证、流式控制或已提供的采样/输出设置。普通调用和上下文摘要共用配置。默认模型与会话选择都是 provider/model ID 对。配置读取失败只影响模型操作，不阻止应用初始化，也不静默重置文件。

模型的可选 `reasoningEfforts` 是用户设置的档位限制，不是启用选择器的前置条件。未填写或留空时，选定模型即可选择 Low、Medium、High，包括手工创建和服务发现的模型；非空时只列出指定档位，并保留已声明的默认档位。前端选择器和目录刷新复用同一选项投影，刷新未配置档位的模型不会清空当前选择。Rig 仅提前拒绝明确列表之外的显式覆盖值，未配置列表时发送用户所选值，由服务验证实际支持，不根据模型名称猜测能力。显式档位由 Rig 分别映射为 Responses 的 `reasoning.effort`、Chat 的 `reasoning_effort`、Anthropic 的 `output_config.effort` 和 Gemini Interactions 的 `generation_config.thinking_level`，保留其他生成参数。需要单独启用思考的模型由用户填写其原生扩展参数，不对同协议下所有模型自动启用。

Rig 复用上述原生字段映射读取模型扩展参数中明确声明的默认强度，由 Application 模型目录以 provider 状态中的只读 `reasoningDefaults`（模型 ID → 档位）交付。它不写入设置文件，前端不另外解析供应商原生参数或维护模型名称推断表。下拉列表不提供独立的“默认”项，而在对应档位标注“（默认）”；选中该档位或点击恢复按钮时，轮次 `reasoningEffort` 保持 null，继续继承模型配置。没有声明可识别默认值时不标记任何档位，未覆盖时显示“默认强度未知”，仍可选择具体强度或恢复继承设置。

Rig 将供应商公开的推理文本流投影为 `ReasoningDelta`，不制造思考过程，不公开签名等内部字段。每次完成的模型调用通过 `UsageReported` 保存 Rig 归一化后的输入、输出、缓存读写和推理 Token 数，并区分普通响应与上下文摘要。缺失值保持 null；输入已经包含缓存、输出已经包含推理，消费方不能重复相加。供应商未返回用量的调用仍记录未知用量，不从文本长度猜测。

API Key 通过 `keyring` 保存到系统凭据库，配置文件仅保存随机凭据引用，IPC 目录仅返回 `hasApiKey`。更换密钥先创建新凭据、原子提交配置，再删除旧凭据；待删除引用随配置持久化，删除失败会在后续读取或提交时重试。前端只保留表单中的临时密钥输入，偏好设置不再保存 AI 凭据。已存密钥以固定星号占位显示，空的密钥输入保留已存密钥，输入新值后保存则替换；设置页不提供单独删除密钥的控件，星号占位不作为密钥提交。配置和凭据不写入项目、对话事件或日志。

OpenAI 兼容协议另支持 `none` 认证，用于无需密钥的本地服务。该模式不读取凭据、不发送 Authorization；从 API Key 切换过去会通过同一持久清理流程删除原密钥。设置页、默认模型和会话选择按认证方式判断可用性，不能仅依据 `hasApiKey` 禁用本地模型。

每轮准入通过 `LanguageModelResolverPort` 解析一次配置、凭据与具体 `AgentDriverPort`，Manager 和所有 Worker 共用该轮固定驱动。会话模型选择可在运行时修改，影响后续轮次；不会更换运行中的客户端。`TurnStarted.model` 持久保存执行时的 provider/model ID 与显示名；其中 `providerName` 捕获当时的自定义名称，去除首尾空白后为空时使用供应商名称。历史模型提示直接读取该快照，不依赖当前目录，也不随配置改名或删除而变化。队列携带入队时选择，发送旧队列不会覆盖会话的新选择。配置完成不表示远端认证已通过。

模型发现使用当前连接草稿，连接信息填写完整即可请求，无需先保存，也不依赖模型表单是否填写完毕。新输入的密钥仅用于本次请求；没有新输入时由 Application 按供应商 ID 读取已存密钥，`none` 认证不读取密钥。发现操作不保存草稿或临时密钥。请求捕获已存连接与凭据基线，网络返回后由 Application 重验；其他窗口保存、删除或替换该基线后不交付旧目录。前端切换连接参数会使当前发现请求失效并拒绝迟到回复，保存期间禁用表单输入，避免保存完成覆盖随后输入的修改。

Rig adapter 使用 `rig-core`、`rig-agent` 和 `rig-reqwest` 0.43.0，最低 Rust 版本为 1.95。Provider registry 建立原生客户端，`DynModel<Completion>` 进入统一的工具、流式与恢复流程；HTTP transport 由 `rig-reqwest` 提供，显式启用 Rustls，以支持 HTTPS、证书校验及系统代理。生产客户端禁止自动跟随 HTTP 重定向，避免将消息正文或供应商专用认证头转发到未配置的端点；3xx 返回稳定的请求拒绝错误，由用户修正 API 根地址。
协议显式区分 OpenAI Responses、OpenAI Chat Completions、Anthropic Messages 与 Gemini Interactions。Rig 原生 provider registry 负责供应商方言和默认地址；Moonshot/Kimi、DeepSeek、GLM 等保留各自 adapter。Gemini 使用 Rig 的 Interactions wire，保留完整 JSON Schema，并显式设置 `store: false`，对话状态继续由 Harness 管理；其扩展生成参数位于 `generation_config`。认证、请求编码、模型目录和流式解析由 Rig 处理，最终进入同一 Harness 循环。

供应商预设的唯一目录是 [provider/presets.rs](../yss-harness-rig/src/provider/presets.rs)，包含云服务、地域端点、本地服务和四种自定义协议，经 Application 目录回执提供给前端。AI 设置首页在“模型供应商”右侧提供本地化的“配置”按钮与右箭头；点击进入“AI / 供应商”页面查看列表，添加按钮位于该页面面包屑行的最右侧，空列表也保留入口。添加时直接进入配置页，“自定义名称”排在“供应商名称”上方。新建和编辑均使用共享的 shadcn Combobox，在同一控件中输入搜索和选择供应商，由 Base UI 处理筛选、键盘选择与焦点；选中预设后自动填入协议、地址和认证方式。

“自定义名称”在创建和编辑时均可填写，通过独立的 `customName` 字段保存；默认留空，清空后保存为 null。供应商列表标题、配置页面包屑、默认模型与对话模型选择器的前缀统一由 `providerDisplayName` 读取自定义名称，去除首尾空白后为空时使用供应商名称；模型选项显示为“配置显示名称 · 模型名称”。列表次行依次显示实际供应商名称、模型数量与密钥状态；供应商选择字段继续显示实际供应商名称。选择预设时更新供应商名称，保留自定义名称。预设选择使用独立的表单 ID。配置与凭据按稳定 ID 区分，支持同一供应商使用不同 API Key 的多份配置，不以任一名称作为唯一键。切换供应商会清空原临时密钥与模型，并丢弃旧发现请求的迟到结果；重选当前供应商不重置连接草稿。已有密钥的配置改用其他供应商名称或 adapter 后，API Key 认证需要输入新密钥，发现和保存操作不能隐式沿用原供应商的密钥。

设置页支持重复添加供应商、修改端点；从服务获取模型成功后，将全部返回模型直接加入当前草稿，按模型 ID 跳过已有项并保留已有参数，无需再次选择或确认添加。模型也可以逐个手工添加，支持部署 ID。模型型号由服务发现或用户配置，不维护易过期的固定型号清单。所选模型必须支持工具调用；未实现模型列表的服务仍可手工配置。Base URL 填 API 根路径，例如 OpenAI 的 `https://api.openai.com/v1`、Anthropic 的 `https://api.anthropic.com`、Azure 的资源地址加 `/openai/v1`；不得包含凭据、query 或具体接口路径。
模型工具参数均为对象；`yss-harness-contract::capability_input_schema` 在生成 `ToolDescriptor.input_schema` 时统一声明根级 `type: object`，同时保留 typed schema 的 `oneOf`、`$defs` 等约束。Rig adapter 将 descriptor 中的完整 schema 序列化为 function parameters。内部参数解码契约不变。

模型工具签名仅包含 capability identity 与输入 schema。共享 `CapabilityDescriptor` 只保存能力身份、效果和条目上限，并按效果提供调用时限；Core 与图编辑提交前的结果检查共用 `MAX_CAPABILITY_RESULT_BYTES`。实际幂等由工具账本和提交回执实现，权限由角色/任务范围及 Gateway 检查实现；显式批准执行由 `ApprovalService` 校验并消费精确绑定请求的 grant，不使用声明式审批元数据。
Contract 的同一能力元数据声明生成公开描述符列表及按身份匹配的入口，不手写列表索引。
内部 Owner 操作保留自身的描述符，但不加入公开模型能力列表；角色与执行模式继续由 Core 筛选。
Rig streams the first text promptly and coalesces subsequent deltas at 40 ms or 4 KiB. Core owns identified tool lifecycle events. Worker summaries use the final accepted model response as plain text; Manager finalText retains the public transcript. The Host has no 1 MiB final-response admission limit. Cancellation preserves committed receipts and waits for admitted tools to settle.

工具账本的 `ToolInvocationRequest` 区分已绑定的内部执行请求与执行前被拒绝的模型意图。参数解码失败、授权拒绝和基线绑定失败同样记录调用身份、公开能力名称、失败终态及开始/结束时间；绑定时间计入工具总耗时。成功解码的业务输入保留用于重放，畸形原始参数不落账本，重放该调用使用空参数和原结构化错误。被拒绝的调用不产生业务回执、不推进观察基线，启动恢复也不会把它误判为可能已提交的写入。

委派、续接与统计计划的每次模型调用另由 Core 分配调用 ID，通过同一持久事件流记录 `ControlToolStarted` / `ControlToolFinished`，不新增账本或持久化表。Rig 的 [control.rs](../yss-harness-rig/src/tools/control.rs) 在参数解码前开始记录，成功、参数错误、准入失败、计划策略拒绝、取消及 panic 都经过同一终态路径；计划策略与委派业务仍由原 owner 执行。已准入控制任务加入原清理队列，取消等待其收尾。启动恢复将缺少终态的调用标记为 `outcome_unknown`，不重复执行已发生的委派或计划提交。

`ControlToolFinished` 保存失败码与可选的诊断 details。参数错误保留 category、path 和 expected，策略拒绝保留 reason；Rig 将同一失败交给 Core 记录后再返回模型。缺少诊断的事件不推断或补造原因。

控制调用耗时包含绑定、Worker 排队与执行以及结果交付，不能解释为纯模型或节点计算时间；Worker 本身继续由原 AgentRun 事件记录准入、运行和结算。IPC 将控制调用投影成普通工具生命周期，详情从原事件重建开始/结束时间；任务正文、授权、结果与统计计划沿用各自的任务卡及计划展示，不复制到工具参数摘要中。真实图运行的 admission/running/finalization 时间仍由 Execution owner 提供。

实时 capability 返回与历史重放复用相同的工具结果 JSON 编码。资源不存在、参数或业务请求被拒绝、revision/invocation conflict 及 approval_required 等可处理结果，以 `{state: "failed", failure: {code, details}}` 交回模型，使它可以纠正参数、读取当前状态或向用户说明。图校验的 ready/diagnostics 和图执行的 status/failureCode 由各自能力结果表达；执行成功由提交回执确认，运行事件补充取消和失败定位。`outcome_unknown` 保留为失败反馈；模型必须先查询事实，不能盲目重试可能已经提交的修改。Core 的 ledger 与 ToolInvocationFailed 事件仍记录能力失败，不因协议层成功交付反馈而改写为成功。

普通工具和统计计划工具共用 Rig adapter 的参数解码器。反序列化失败以现有 CapabilityFailure 的 InvalidRequest 返回 reason、category、path 和 expected；字段路径只保留 schema 已声明的字段及数组索引，动态 map key 脱敏，预期类型、范围和枚举来自工具自身 schema，并限制诊断长度。原始 Serde 文案、错误参数值和凭据不进入反馈。计划策略拒绝继续提供 reason 和 availableMethods。

模型生成当前未注册或不允许调用的工具名时，Rig 的 `tools/feedback` hook 通过原生 Skip 决策向模型返回 `tool_not_available` 和当前允许的工具名称，不把它直接升级为整轮 `invalid_provider_response`。流式响应中尚未准入的同批工具不会执行，已完成的前序工具不重放；不自动猜测工具别名，也不扩张角色权限。未注册调用没有 Core capability 身份，不伪造业务账本；诊断日志仅记录原因和工具名，不记录参数或历史。协议参数字节不可解析时仍保留原有失败分类。

Rig 0.43 hooks implement the delivery check at the tool-free model completion boundary. Core returns authoritative missing-delivery feedback; Rig continues through ModelTurnRetried. An identical check without new committed artifacts stops with DeliveryBlocked instead of looping on repeated reads. Core also marks the Manager run blocked when a declared report task lacks its final save. Fatal tool signals still stop sampling and await admitted tool cleanup. The model-turn hook rejects output truncation and content filtering before admitting tool calls; a partial response cannot become a completed answer. Provider errors use Rig's structured error reports without logging response bodies or source chains.

Transport recovery retries only the latest sampling boundary, at most five consecutive attempts without an accepted model call. Cancellable exponential backoff respects numeric Retry-After seconds; 402, authentication failures, output truncation and content filtering are not retried. A 300-second idle detector runs only while waiting for model traffic, not during tools. TextRetracted removes an interrupted attempt from live/replayed text before reconnecting; earlier tool receipts and model-call context are retained. No model-call quota or whole-worker deadline is imposed.
`providerConfigured` 由当前模型目录、会话选择、认证方式与凭据存在状态派生；它不代表网络可达或认证成功。认证、限流、请求拒绝、服务不可用、连接与协议错误继续使用稳定错误码。

## 9. Native host boundary and retained frontend reference

原生宿主直接调用 Application 的 HarnessHost、LanguageModelService 和知识服务。它负责订阅已提交事件、按持久序列补齐缺口、释放订阅和本地化类型化错误；会话、轮次与工具状态继续由 Harness 持有。工具详情读取已有 ledger 或控制调用事件，引用查询匹配来源、版本、hash、chunk 与项目可见性；来源删除或版本改变后返回不可用。模型目录不包含密钥。

原生供应商/模型设置已在 [GPUI settings](../yss-desktop-gpui/README.md) 接入 LanguageModelService：
配置草稿、遮蔽密钥输入、模型发现、默认模型和保存全部均留在宿主边界，当前配置与凭据 owner 未变化。
原生会话目录、消息/工具/任务/引用投影与模型目录失效交付已接入；独立会话分组、草稿持久化、完整卡片、项目知识库及外观偏好仍需继续迁移与人工验收。

以下 Assistant 投影和样式说明记录 `react/` 中保留的参考实现，不表示原生 Assistant 已完成迁移。旧 Tauri 命令、Channel hubs 与 IPC 注册表已移除，原生界面应使用上面的类型化服务和事件边界。

参考前端的 `src/services/assistant/harnessService.ts` 曾负责有序事件交付，`harnessContract.ts` 负责严格解析。
前端 `src/features/application/assistant/assistantHarnessSession.ts` 协调会话、模型选择与重连；历史回放先归约到未发布候选，在订阅接通后通过内部
Zustand store 一次发布完整回放投影。实时事件继续顺序接纳。`assistantHarnessProjection.ts` 归约消息、工具、引用和计划；
消息内容是呈现事实的唯一来源，不另外保留按 turn 索引的可写工具、引用或计划副本。
`assistantHarnessRuntime.ts` 只负责 React 生命周期与 assistant-ui ExternalStore 适配。
自有 Context 传稳定的只读订阅接口和动作；工具栏选择状态、会话及消息是否为空，
不因正文的每个 text delta 更新。工具分组通过 assistant-ui 的选择订阅只读取自身范围的调用数、
运行数、异常数和首个活动工具名，浅比较复用未变摘要，不订阅整段消息内容。正文继续由 assistant-ui 的消息订阅呈现。

`assistantModels.ts` 是 Rust 模型目录的共享只读投影。设置页提交完整 provider 配置，模型选择器提交会话选择；保存成功后发布服务端回执，其他窗口收到无敏感数据的失效通知后重载。聚焦时也刷新目录。没有前端配置驱动的防抖 effect，不再在组件挂载时重传 API Key。发送要求事件订阅接通且模型选择已经确认；迟到的模型选择回执受会话 generation 与请求序号保护。

设置界面沿用左侧分类、右侧内容的布局，模型与供应商、项目知识库和外观共用字段行：左侧显示名称及说明，右侧显示控件，窄窗口下自动上下排列。根页面和子页面统一使用 `SettingsPage`，依次呈现面包屑与操作栏、错误提示、独立滚动的页面内容；页头在表单与滚动区之外，错误提示不会改变页头位置。页头高度、按钮尺寸、面包屑文字基线及内容边距统一，切换页面时内容滚动位置重置。面包屑以当前选中的分类为根节点（如“AI”“外观”）；供应商管理使用“AI / 供应商 / 当前草稿名称”，可分别点击“AI”返回首页、点击“供应商”返回列表，关闭编辑或删除成功也返回列表。恢复默认及供应商列表页的添加操作固定在面包屑同一行的最右端，较长的路径文字截断显示。设置页不提供常驻刷新按钮，沿用模型目录与项目知识来源的自动更新入口；只有目录或来源状态加载失败时在错误提示旁提供重试，保存、删除或重建失败不显示重新加载按钮。从服务获取模型与重建知识库仍保留为显式业务操作。页面不再显示单独的大标题与介绍，字段说明随对应控件呈现。

会话初始化根据面板指定的 `sessionId` 打开并重放持久事件，不自动切换到最近会话。重连时复用该 runtime 的只读投影并从 lastSequence 继续；关闭标签后重新挂载从序号 0 重放，完整持久事件不删除。项目范围的对话目录由 Rust Application 生成 Assistant Activity JSON 文档，经 `get_activity_panel_document` 与现有 `sidebarStore` 缓存交给共用模板渲染；查询绑定项目/语言/生命周期并拒绝旧响应。`assistantConversations.ts` 只编排新建、重命名及文档失效，正文投影不复制目录。界面初次显示最近 40 条消息，更早内容按批展开并保持滚动位置，流式滚动仍由 assistant-ui 管理。
订阅回执与事件回调都核对原会话 generation。替换订阅必须保持连续；重放期间再次缺号或解析失败使该订阅失效并进入错误状态，
不把尚有缺口的投影标成就绪，也不无限启动重连。发送准入同时要求当前订阅仍被会话 owner 持有；显式重载继续使用原初始化入口。
Service 清理后忽略迟到的 Channel 回调；订阅应答前若已被 HMR 释放，收到远端 ID 后完成退订并拒绝该应答，
不向 Application 返回已关闭的句柄。HMR 与显式释放共用同一个退订入口，最多提交一次远端释放。

当前请求的具体错误始终在 Assistant 状态区显示，即使会话仍可继续发送；不能因消息已经标记为中断而隐藏 provider 拒绝、认证、限流等原因。消息内的中断提示用于说明已完成操作仍然有效。

输入区提供模型选择器和资源引用入口。用户可点击引用按钮或在词首输入 `@`，按名称或路径搜索项目索引中的 Database、Event/Function Graph、Chart、Mind 和 Doc；已选引用显示为可移除标签，历史中的引用可打开资源。引用草稿、未确认输入与队列属于前端暂态；提交后的引用归 Rust 有序事件。

`submit_harness_turn.resources` 只接收 `{kind,id}`。Core 先登记轮次准入和取消令牌，再通过 `HarnessResourceResolverPort` 调用 Application；后者在 blocking pool 复用资源工具的 Project index，检查项目绑定、真实成员和重复项，并补充权威显示名。资源与模型解析不持有会话选择锁；完成后 Core 重新读取和验证会话。解析期间取消立即结束等待，迟到的只读结果不能启动模型或发布 `TurnStarted`。Core 将 `{resource,name}` 随 `TurnStarted` 持久化，当前请求和历史回放使用同一个 user-input 投影。模型得到资源定位信息，按任务需要调用资源工具读取正文或结果；引用不携带 revision/hash/同步状态，也不提前展开完整图或数据行。没有隐式的 active graph 参数。资源在之后被修改或删除时，读取工具仍检查当前事实；历史引用不会跟随编辑器焦点改变。

运行期间可把后续消息、模型、执行选项和明确选择的资源放入待发送列表，当前会话任务成功结束后顺序提交。切换模型、模式、推理档位或编辑下一条引用不会改变已排队输入和正在运行的请求。失败、停止、重开会话后暂停自动发送；这不修改已运行任务。未确认输入保留正文、引用、模型与执行选项，匹配正文和引用的 `TurnStarted` 才清除其提交草稿；未接纳输入通过 assistant-ui 的 `MessageNotSentError` 恢复，引用也保留。恢复草稿时一并恢复选项与模型；新草稿选项不被历史重放覆盖。已接纳任务失败时保留真实用户消息和回执，继续操作沿用已有任务上下文，重新连接只修复订阅。

结构化输入参考 [Codex UserInput](https://github.com/openai/codex/blob/main/codex-rs/protocol/src/user_input.rs) 对文本和显式 mention 的区分，选择入口参考 [Zed Adding Context](https://zed.dev/docs/ai/agent-panel#adding-context)。YssBI 的引用保持项目资源身份，不把任意本地文件路径当作可读取权限。

模型上下文由 Harness Core 的 `conversation` 从本会话的持久事件流、最新上下文检查点和工具账本重建，以当前 TurnStarted 为边界；当前用户消息只加入一次。检查点只替换模型工作上下文，完整事件与原始回执仍然保留。不限制最近轮数，不按条截断用户或 assistant 文本，也不将失败/取消轮次排除。相邻 TextDelta 合并，工具事件保留其间的顺序；已有流式正文时不重复追加 TurnCompleted.finalText。

AgentMessage 使用 provider-neutral 的文本、工具调用、工具结果和统计计划变体。工具参数与成功/失败结果读取本会话中对应 invocation 的原始记录，未被检查点覆盖的调用由 Rig 映射为成对的原生 tool-call/tool-result 消息。恢复后的成功 receipt 保留成功，失败保留结构化 failure，缺少确定终态的调用标记 outcome_unknown；取消或失败的 turn 附带明确状态，不暗示已经提交的操作被回滚。事件缺口或缺失的工具记录会明确失败，不静默发送不完整历史。

Rig 将历史中重叠执行的业务工具和 Worker 委派合并为同一条 assistant 调用组，随后连续附上与每个调用 ID 对应的结果。组内进度文字在完整结果之后传给模型，不能插入调用与结果之间；持久事件和前端展示顺序保持原样。缺失、重复或不匹配的结果在发给 provider 前明确拒绝，不清空或截断历史来绕过协议错误。

Working context is separate from the complete durable transcript and ledger. The Rig context hook derives a conservative serialized-byte threshold from configured context/output capacity, falling back to 192,000 bytes when capacity is unknown; this triggers summarization, not rejection. `context/source.rs` prepares summary input without rewriting native messages or stored receipts: identical inspections reference the first observation; graph bodies retain identities, versions, hashes, readiness, constants, diagnostics, node/port identities, parameter values, literals and connections. Repeated editor descriptions, option lists, layout and resolved schemas are omitted with an explicit instruction to inspect current details before editing. Statistical result tables, labels, values and committed-write receipts remain complete. Tool JSON is decoded once, avoiding nested string escaping in summary input.

`context/summary.rs` uses the current request byte budget, reserving space for the preceding summary, instructions and provider framing instead of issuing fixed 12,000-character requests. Capacity rejection lowers that budget through the existing recovery path. The model receives a concise checkpoint target, while ordinary execution uses only the configured model output limit. Every completed fragment emits `ContextCompactionProgress` with byte progress and a durable checkpoint of its exact source prefix. Retries and Manager/Worker history reconstruction can reuse that checkpoint only after its SHA-256 prefix identity matches; partial checkpoints never replace unread history. A completed `ContextCompacted` supersedes partial state before reopening Rig at the replacement checkpoint. Role/skill instructions are reinjected and native call/result groups are either retained together or replaced together.

IPC exposes progress counts without checkpoint text or hashes; the main conversation and Worker cards show completion percentage. Compaction logs record source/completed bytes, summary size and per-fragment elapsed time, never source content. The compatible API uses model summarization and does not assume a Responses `/compact` endpoint. Failure to produce a smaller checkpoint remains a typed context error.

历史不替代当前工具事实。图工具的运行事件进入原有 Execution/Results 消费者；对话只复用持久 Harness 事件与工具 receipt，不新增第二份持久聊天状态。

转换为 assistant-ui 消息时，只有 assistant 角色携带 `status`；user 消息不携带该字段。运行时集成回归使用真实的 ExternalStore 消息转换器校验这一边界。
消息投影按事件顺序维护 text/source/data/tool parts，相邻文本片段合并，工具终态更新原位置。完成事件只收尾已有流式内容；仅在没有文本片段时使用 `finalText` 恢复正文。取消、失败和按 sequence 重放不覆盖中间说明或重排工具。

Assistant 面板使用 assistant-ui 的 Thread Viewport、Composer 与 ActionBar。正文和子任务总结共用 Markdown 渲染，支持 GFM、代码和公式。连续工具调用使用 Collapsible 分组，运行时默认展开、结束后默认收起，用户手动选择优先；默认展示本地化名称、目标、状态和耗时，技术标识及摘要 JSON 放进详情。可见的工具卡片按需查询 ledger 摘要，失败原因直接显示，不因缺少原始参数而禁止展开。子任务的阶段、整理进度、重连次数及失败/阻塞原因在折叠区外可见，内部保留各工具调用的状态与时间。

对话采用紧凑标题栏、平铺消息和底部通栏输入区。标题栏提供下一轮只读开关、新建、重命名、最大化/还原及关闭；底部工具栏提供显式资源引用、推理档位、Token 用量、Ask/Write、可搜索模型选择器和发送/停止。输入支持展开及 Ctrl+Enter，窄面板工具栏可换行。控件复用现有 shadcn/Base UI，事件与 runtime 继续使用 assistant-ui ExternalStore，不引入第二套聊天状态。历史回复根据 `TurnConfigured` 显示实际执行模式与档位，公开推理文本单独折叠展示。

用量投影按持久事件 sequence 重放，每轮开始清空本轮累计，累计包括 Manager、Worker 和上下文摘要的已报告消耗。环形图只使用主对话最近普通请求的输入数与该次配置的上下文容量；Worker、摘要用量不会替换这个读数，整理完成后清空旧占用，等待下次真实请求。未配置容量或供应商未报告数值时明确显示未知；部分调用缺失用量时标明累计不完整。切换下一轮模型不会把上一轮用量标成新模型。

整轮回复从 TurnStarted 开始计时，以持久终态事件结束；工具和子任务分别保留自己的开始/结束时间。运行期间局部计时组件每秒刷新，完成、失败或取消后耗时固定，悬浮可查看开始、结束与最近进展时间。同一轮内恢复 Worker 会新增一次执行卡片，保留前次耗时。事件连接失效时显示待同步，不伪造完成时刻或持续展示旧活动；错误、连接恢复和停止状态优先于普通工具阶段。

Agent completion 的资源回执和结果引用生成可重新打开的卡片；资源打开复用原 editor owner，提示后续修改或资源不可用，结果打开复用原查询/租约 owner，已释放的结果明确提示不可用。引用 source part 保留完整身份并支持展开查询对应版本正文。这些展示查询不进入模型上下文。统计计划和输入草稿保持各自 owner，不新增会话 authority。

Assistant Markdown 与文档预览共用 `src/shared/ui/markdownRendering.tsx` 的 GFM、公式布局、表格容器和延迟加载的 Shiki，使用 Typography 的紧凑字号层级及工作台主题变量。表格按内容自适应并居中，宽表格、代码和独立公式在内容区内横向滚动。`$...$` 保持行内，`$$...$$` 无论同一行或分行书写均独立居中，含公式的段落与单元格保留合适行高。Assistant 继续使用 `normalizeMathDelimiters` 处理模型的 LaTeX 替代分隔符，并保留 smooth/defer 流式显示、代码复制和外链打开处理。

桌面排版验收覆盖窄面板中的表格与长公式、行内分式、流式代码块完成前后及未知代码语言；切换浅色、深色和 OLED 主题检查正文与代码颜色，并确认代码复制和外链打开行为。

项目资源链接通过现有资源索引识别并复用 editor 打开入口，不根据文件扩展名猜测资源类型。外链和片段链接复用共享 `MarkdownLink` 的地址解析、片段定位和点击/中键处理，Assistant 通过现有
`MarkdownLinkContext` 提供稳定的系统浏览器打开动作及原失败弹窗。该上下文只传链接动作，
不承载消息；Workbench 的文档参考页打开策略仍由其自身组合入口提供。

事件 `type` 使用 snake_case，envelope 和 payload 的字段使用 camelCase。Rust 序列化和 TypeScript 解析共用代表性事件夹具，避免两端各自使用不同的手工样本。
Channel 在历史重放期间缓冲实时事件，按 sequence 合并、去重后交付。等待缺号的实时缓冲有容量限制，超限时交付缺号之后的持久事件并关闭旧订阅，让前端依据 sequence gap 重新重放；长历史按顺序排出，不占用整个实时缓冲。前端重连时封锁发送并忽略已替换订阅的迟到回调。工具卡片从携带真实 invocation ID 的 `ToolInvocationStarted` 创建，并按同一 ID 更新完成、失败、取消、超时或中断状态；不生成待替换的工具占位身份。turn 终止时未收到工具终态的卡片显示中断/取消，不假定成功。面板卸载使旧回调失效并释放订阅；已创建的对话继续持久化，供重新打开。
订阅建立后才开放发送。已终止的模型轮次展示中断或停止说明，保留已完成操作并允许继续发送；说明与耗时根据持久化事件重放。具体错误在状态区可见，下一次发送清除；会话或订阅传输故障阻止发送并提供重新连接。提交请求尚未结束时不能再次提交，关闭后的迟到回调不能更新新会话。

React 不生成 authoritative turn/workflow transition，不直接调用 Rig/Gateway，也不在 Zustand 建立 conversation authority。Project/session replacement 或 provider unavailable 只改变 projection/action availability，不能保留旧 backend handle 继续提交。

## 10. MCP status

当前没有 MCP server adapter 或桌面监听入口，也没有 MCP Client。内部 Assistant 直接调用 Capability Gateway。外部 transport、authentication、Tasks mapping 和 tool trust 属于 [roadmap](../../docs/roadmap/STATISTICAL_HARNESS.md)。

## 11. Error, safety, and observability

- Core、Gateway 和 adapters 使用 typed failures；宿主按稳定 code 本地化，序列化值保留共享错误契约；
- prompt、transcript、Memory、tool request/result、数据行、SQL、credential 和 model output 不写入 logging/diagnostics；
- Assistant text 只进入 Harness event stream，不进入 Graph 执行事件或 Output 失败摘要；
- capability result 和 knowledge hit 保留各自 size/depth 契约，模型最终回复没有额外字节配额；
- external/provider payload 在 adapter 边界完成 schema、size、time 和 failure validation；
- Rig 区分供应商输出截断、响应流中断、内容过滤、HTTP 402、认证、限流与协议格式错误。仅记录安全错误码、HTTP 状态、JSON 错误类别与位置；模型调用记录序号、归一化结束原因和供应商报告的 token 用量，不记录原始异常、响应正文、供应商任意 reason 文本或工具参数。截断与缺少终态的流不能成为成功回复；已完成工具的真实回执继续保留；
- operational ledger 是 durable业务记录，不等同于 lossy diagnostics log。

原生服务入口见 [Application README](../yss-application/README.md)，原生通知与日志边界见 [GPUI host](../yss-desktop-gpui/README.md)。

## 12. Current limits

当前 production Assistant intentionally does not provide：

- 超出现有 Chart/Doc owner 契约的图形类型或报告导出格式；
- 所有写入种类的通用跨进程自动重试；
- external MCP client/server process exposure；
- vector/hybrid Knowledge retrieval；
- remote Skill install/signing；
- Worker 递归委派、后台自主执行或重启后自动续跑子 Agent。

这些限制是当前边界，不应在 current architecture 中展开为拟议 interface。实施顺序和验收条件只在 [Harness roadmap](../../docs/roadmap/STATISTICAL_HARNESS.md) 维护。

图工具直接通过 Application 操作 Project 当前驻留文档。图活动流负责 UI 通知及执行事件，编辑数据和历史不依赖客户端存活。图编辑 snapshot/delta 和保存语义见 [Graph 与 Execution](../yss-application/src/graph/README.md)。

## 相关模块

[Application](../yss-application/README.md) · [UI 页面契约](../yss-ui-contract/README.md)
