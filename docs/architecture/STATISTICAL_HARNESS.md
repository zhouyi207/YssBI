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
    ├─ CapabilityGatewayPort → yss-application
    └─ persistence ports → yss-statistical-harness-sqlite
```

`src-tauri/src/lib.rs` 构造 SQLite store、Application capability gateway、configurable Rig driver、builtin knowledge 和 `HarnessHost`，再把 runtime state 注入 `yss-api`。Harness Core 不依赖 Tauri、Rig、SQLite、ProjectState、Graph runtime 或 concrete Database owner。

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

默认 `ToolRegistry` 只注册当前 read-only foundation：

| Capability                | 作用                                              |
| ------------------------- | ------------------------------------------------- |
| `inspect_project`         | 读取 bounded project metadata/resource identities |
| `inspect_graph`           | 读取 bounded Graph snapshot                       |
| `search_node_catalog`     | 查询 localized node catalog                       |
| `inspect_dataset_schema`  | 读取 schema 和 current revision facts             |
| `inspect_dataset_profile` | 读取 bounded data-quality/profile facts           |
| `inspect_result`          | 读取 bounded structured result                    |

Model-facing schema 来自 typed capability contract；Harness 内部不以任意 JSON 代替 request/result 类型。每次调用先写 running ledger record，再通过 Gateway 执行，最后持久化成功 result 或 structured failure。idempotency 命中已有 terminal record 时返回既有 outcome，而不是重复执行。

`apply_graph_edit` 已有 closed typed contract、Application staged batch、approval/ledger 和 commit receipt 基础，但默认 Tool Registry 不注册它，桌面 Assistant、MCP 和普通 Tauri Harness path 均不能调用。它仍属于 gated roadmap，不是当前生产写能力。

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

## 9. Tauri transport and frontend projection

`yss-api` 当前暴露 Harness runtime status/provider configuration、session create/close、event subscribe/unsubscribe、turn submit/cancel、memory list/delete，以及 dataset-quality workflow plan/advance/pause/resume/cancel。Command 只做 DTO mapping 和 transport delivery；完整注册表以 `yss-api` 源码为准，不在本文复制。

有序事件通过 Tauri Channel 进入 `src/services/assistant/harnessService.ts`，由 `harnessContract.ts` 严格解析。`src/features/application/assistant/assistantHarnessRuntime.ts` 维护可重建的 projection、last sequence 和 reconnect；assistant-ui ExternalStore 只渲染 messages、plan、tool cards、memory 和 composer actions。

React 不生成 authoritative turn/workflow transition，不直接调用 Rig/Gateway，也不在 Zustand 建立 conversation authority。Project/session replacement 或 provider unavailable 只改变 projection/action availability，不能保留旧 backend handle 继续提交。

## 10. MCP status

`yss-mcp-server` workspace crate 已实现 read-only `McpCapabilityServer` adapter，并把与默认 registry 对应的 inspection tools映射到同一个 `CapabilityGatewayPort`。该 crate 不依赖 Project/Graph/Database concrete owner。

桌面 composition root 当前没有启动、监听或发布这个 MCP server，因此“外部 MCP 可连接到运行中的 YssBI”不是现行产品能力。MCP Client 也尚未实现。生产暴露、transport/authentication、Tasks mapping 和外部 tool trust 属于 [roadmap](../roadmap/STATISTICAL_HARNESS.md)。内部 Assistant 始终直接调用 Capability Gateway，不经 loopback MCP。

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

- Project mutation、chart/report write 或 external write tools；
- unknown commit outcome reconciliation；
- external MCP client/server process exposure；
- persistent User Memory 或 portable Project Memory；
- vector/hybrid Knowledge retrieval；
- remote Skill install/signing；
- autonomous background or multi-agent execution。

这些限制是当前边界，不应在 current architecture 中展开为拟议 interface。实施顺序和验收条件只在 [Harness roadmap](../roadmap/STATISTICAL_HARNESS.md) 维护。
