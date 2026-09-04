# Decision 0001: Statistical Harness boundaries

> Status: Accepted Decision
> Scope: Statistical Harness 的 authority、ports、Gateway、durability 和 adapter 边界
> Canonical owners: 本文记录设计理由；当前实现由 `architecture/STATISTICAL_HARNESS.md` 描述
> Update when: 该决策被 supersede，或核心 authority/adapter 方向被重新决定时

Date: 2026-09-01

## Context

YssBI 最初只有 frontend Assistant shell。要接入模型、Tools、Memory、Knowledge、Workflow 和 MCP，最危险的捷径是把现有 Tauri command registry 直接暴露给模型，或让 Rig/MCP/frontend 持有 Project 与 conversation 状态。这样会产生多个 authority、绕过 application policy，并把 framework wire 固化为业务 contract。

专业统计 Assistant 还需要比普通聊天更强的可审计性：数值结论必须来自 YssBI computation owner；长流程需要持久状态、currentness、cancellation、approval、idempotency 和 receipt；知识与记忆需要来源、scope 和失效语义。

## Decision

### 1. Provider-neutral Harness Core

`yss-statistical-harness` 独立拥有 session、turn、ordered events、Statistical Plan、Workflow、Tool Registry/ledger、approval、memory proposal 和 knowledge assembly。Core 只依赖 stable automation contracts 与 injected ports，不依赖 Tauri、Rig、MCP、SQLite 或具体业务 owner。

### 2. One business capability gateway

内部 Assistant、MCP Server 和未来 automation client 复用 Application-owned `CapabilityGatewayPort`。Gateway 接受 closed typed requests/results，并执行 principal、project/session、revision、approval、deadline、cancellation 和 bounded-data checks。

Tauri command registry 不是 Tool registry；adapter 不能直接访问 `ProjectState`、Graph runtime 或 Database session。

### 3. Business authority stays in existing owners

Harness 只保存 opaque resource references、captured revisions、Evidence references 和 receipts。Project、Graph、Database、Execution、Result 和 SCI authority 不迁入 Harness。模型只能解释 typed Evidence，不能自行生成统计数值事实。

### 4. Durable behavior is explicit

Session、turn、event、Workflow run/step、Tool invocation、approval、memory 和 knowledge source 通过 persistence ports 保存。每次 tool invocation 具有 identity/idempotency、deadline、terminal result/failure 和 receipt。未知 mutation commit outcome 不能盲目 retry，必须进入显式 reconciliation 设计。

### 5. Frameworks are adapters

- Rig 只实现 `AgentDriverPort`；
- SQLite 只实现 persistence ports；
- `yss-api` 只映射 Tauri commands/channels；
- MCP Server 只把外部 protocol 映射到 Capability Gateway；
- React/assistant-ui 只投影可 replay 的 Harness events。

Framework types 不得进入 Core contract，adapter 不拥有 approval、workflow、project currentness 或 memory policy。

### 6. Memory, Knowledge, Skill, and transcript remain distinct

- transcript 是 session/turn event history；
- Memory 是 scoped、governed、可删除/失效的结构化事实；
- Knowledge source/citation 是 authority，lexical/vector index 是可重建 projection；
- Skill 是 versioned method/workflow/tool-policy package，不是 arbitrary script 或 prompt。

这些概念不能借一个 vector store 或 conversation array合并为单一状态。

### 7. Internal calls do not loop through MCP

内部 Assistant 直接使用 Capability Gateway。MCP 只服务 external protocol integration，不作为进程内 module bus，也不保存 YssBI Harness session/workflow authority。

## Consequences

正面结果：

- Project 和统计计算保留单一 authority；
- 同一 capability 可以被 Assistant/MCP 复用而不复制 policy；
- provider、protocol、persistence 和 UI 可独立替换；
- turn/tool/workflow 可审计并从 durable event/state 恢复；
- write capability 可以在 read-only foundation 稳定后单独 gate。

成本与约束：

- 需要维护 automation contracts 和 adapter 的穷尽映射；
- 所有 long-running/mutating capability 都必须定义 currentness、cancel、approval 和 receipt；
- UI 必须处理 replay/gap，而不能只消费 ephemeral token stream；
- 新 capability 不能通过“临时 command/tool”绕过 Gateway。

## Rejected alternatives

| 方案                                                           | 拒绝原因                                                  |
| -------------------------------------------------------------- | --------------------------------------------------------- |
| 自动把 Tauri commands 转成 model Tools                         | wire 粒度不等于业务 capability，会绕过 policy 和数据预算  |
| Assistant 通过 loopback MCP 调内部能力                         | 增加无价值 transport、身份和失败模式，混淆内部/外部边界   |
| Rig Agent 直接持有 Project/Graph/Database                      | framework adapter 会成为第二业务 authority                |
| React/Zustand 保存 conversation/workflow truth                 | 无法可靠恢复，且与 Rust durable state 产生 reconcile      |
| model 直接计算/返回统计值                                      | 数值事实不可验证，也不能绑定 revision/provenance          |
| transcript 或 vector index 直接作为 Memory/Knowledge authority | 缺少 scope、来源、删除、版本和 invalidation contract      |
| 首阶段开放任意 mutation/external tools                         | approval、receipt 和 unknown outcome 还不足以保证安全恢复 |

## Follow-up ownership

- 当前生产状态：[Statistical Harness](../architecture/STATISTICAL_HARNESS.md)
- 尚未完成的 gated 能力：[Harness roadmap](../roadmap/STATISTICAL_HARNESS.md)
- Transport contract：[`yss-api` README](../../src-tauri/crates/yss-api/README.md)
- Runtime logging/security：[Runtime Signals](../architecture/RUNTIME_SIGNALS.md)

本 decision 不维护当前 command 列表、crate 文件树、phase 完成百分比或未来 interface 草案。
