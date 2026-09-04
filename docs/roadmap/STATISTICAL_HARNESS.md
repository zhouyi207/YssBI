# Statistical Harness roadmap

> Status: Planned
> Scope: Statistical Harness 尚未成为当前生产能力的 gated work
> Canonical owners: 本文拥有未完成项；当前实现以 `architecture/STATISTICAL_HARNESS.md` 为准
> Update when: roadmap item 开始、完成、取消或改变 gate/验收条件时

本文件只记录未来工作，不描述当前产品能力。已实现边界见 [Statistical Harness 当前架构](../architecture/STATISTICAL_HARNESS.md)，设计依据见 [Decision 0001](../decisions/0001-statistical-harness.md)。

## Baseline

当前 foundation 已提供 Rust-authoritative sessions/turns/events、read-only typed capabilities、SQLite persistence、Rig driver、Assistant projection、一个 dataset-quality workflow、builtin Skill、lexical Knowledge 和 Session Memory。

以下能力仍 gated：Project write、external MCP exposure/client、unknown commit reconciliation、persistent User/Project Memory、vector retrieval、remote Skill 和 autonomous/background execution。

## 1. Production write capabilities

候选能力：

- `apply_graph_edit`；
- chart create/update；
- variable annotation；
- reproducible report save。

启用前必须同时满足：

- closed typed request/result 和 bounded batch；
- exact principal/project/session/revision binding；
- one-time approval grant 绑定 request fingerprint；
- idempotency key、durable invocation ledger 和 commit receipt；
- 一次 staged validation、一次 authority commit、一次 history/publication；
- cancel point-of-no-return 语义；
- crash 后能够区分 not-started、committed 和 unknown outcome；
- Assistant UI 可显示 pending approval、receipt、failure 和 undo/recovery action。

`apply_graph_edit` 的 contract/Application 基础已经存在，但在上述 gate 完成前不得加入默认 Tool Registry 或外部 adapter。

## 2. Commit outcome reconciliation and recovery

- 定义 mutation receipt 的 durable schema 和 Project history correlation；
- 对 process crash、transport loss 和 late response 建立 unknown-outcome 状态；
- 只允许在 authority receipt 证明未提交时 retry；
- project replacement 将 bound runs/turns 标记 stale 或 paused；
- 完成 restart recovery、duplicate delivery 和 point-of-no-return tests。

该阶段是所有 production write tools 的硬前置。

## 3. MCP external integration

### Server exposure

现有 `yss-mcp-server` 是 read-only library adapter，尚未由桌面应用启动。生产暴露需要决定并实现：

- stdio / local transport 与 process lifecycle；
- authentication、principal 和 project-session binding；
- capability/resource/prompt surface；
- request budgets、rate limits 和 cancellation；
- Workflow run 到 MCP Tasks 的映射；
- packaging、permissions 和 explicit user enablement。

### Client

外部 MCP tools 进入独立 untrusted registry。默认 effect 为 external，要求显式 approval、bounded result、network/data-sharing policy，并禁止直接写 Memory 或 Project。Remote prompt 不自动成为 trusted Skill，remote resource 不自动进入 Knowledge index。

内部 Assistant 继续直接调用 Capability Gateway，不建立 loopback MCP。

## 4. Memory governance

### Persistent User Memory

- user scope、retention、encryption 和 credential boundary；
- proposal/review/approve/delete/export/disable UI；
- supersede、conflict 和 invalidation；
- 不跨用户、project 或 principal 泄漏 retrieval。

### Portable Project Memory

若需要随项目移动，Project Memory 必须成为 Project-owned explicit resource，具有 schema、revision、history 和 portability contract；不能只依赖 app-data SQLite。

Episodic workflow summary 只有在来源、revision、sensitivity 和删除传播明确后才可持久化。

## 5. Knowledge retrieval

- 保留 source document/manifest 为 authority；
- 增加可重建 chunk/index pipeline；
- 评估 embedding provider 和本地/远程数据共享；
- lexical + vector hybrid ranking 与 deterministic filters；
- citation/source hash/version/license 完整性；
- source delete 后立即拒绝查询，并异步清理 derived index；
- 默认不索引数据行，优先 schema、codebook、统计摘要和显式文档。

embedding model 和 vector store 选择不能改变 Harness Core contract。

## 6. Skills and distribution

- project/user Skill source；
- remote package review、signature 和 install/update UX；
- exact version/source hash resolution；
- permission intersection，禁止 Skill 扩权；
- representative statistical eval fixtures；
- collision 和 silent shadowing prevention。

Skill 仍是版本化方法包，不获得任意脚本或 filesystem execution capability。

## 7. Workflow breadth and scheduling

在每个方法具备 authoritative computation、diagnostic gates 和 reproducible Evidence 后，逐步加入：

- exploratory data analysis；
- OLS model and diagnostics；
- panel model selection；
- time-series stationarity/modeling；
- DID / IV analysis；
- Bayesian model building and convergence review；
- robustness/sensitivity analysis；
- publication report generation。

background scheduling、pause/resume across restart 和 multi-session concurrency 必须先定义 admission、公平性、resource budget、project replacement 和 user-visible control。首阶段不引入 autonomous multi-agent swarm。

## 8. Provider and privacy controls

- provider/model selection 与 capability discovery；
- platform credential store，不把 token 写入 SQLite/Project/logs；
- transcript/prompt/tool payload 的 explicit data-sharing policy；
- offline/unavailable/provider-rate-limit behavior；
- retention/encryption policy；
- adapter-level request/response size、depth、schema 和 timeout validation。

## 9. Promotion checklist

一个 roadmap capability 迁入 current architecture 前必须：

1. 有明确 owner、typed contract 和 authority boundary；
2. 不绕过 Application Gateway；
3. 定义 project/session/revision currentness；
4. 定义 approval、idempotency、deadline、cancel 和 receipt；
5. 定义 bounded payload、隐私与 network policy；
6. 通过 focused contract/behavior/recovery tests 和 architecture gates；
7. 更新 [Statistical Harness 当前架构](../architecture/STATISTICAL_HARNESS.md)；
8. 从本文件删除已完成项，并在 release history 中记录结果。

## Deferred decisions

以下实现选择由 ports 延后：provider/model、embedding model、vector index、Project Memory resource layout、remote Skill distribution、MCP transport 和 background scheduler。它们不得反转 Harness → ports → adapters 的依赖方向。
