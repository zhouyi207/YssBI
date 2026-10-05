# Statistical Harness roadmap

> Status: Planned
> Scope: Statistical Harness 尚未成为当前生产能力的 gated work
> Canonical owners: 本文拥有未完成项；当前实现由 [Harness Core README](../../src-tauri/crates/yss-harness-core/README.md) 维护
> Update when: roadmap item 开始、完成、取消或改变 gate/验收条件时

本文件只记录未来工作，不描述当前产品能力。已实现边界见 [Statistical Harness 当前架构](../../src-tauri/crates/yss-harness-core/README.md)，设计依据见 [Decision 0001](../decisions/0001-statistical-harness.md)。

## Baseline

当前 foundation 已提供 Rust-authoritative sessions/turns/events、typed inspections、桌面图编辑/校验/运行与显式 Save、SQLite persistence、Rig driver、Assistant projection、dataset-quality workflow、builtin Skill 和 BM25 Knowledge；会话上下文复用对话事件、工具账本与压缩检查点。

以下能力仍 gated：外部或后台 Project write、external MCP exposure/client、unknown commit reconciliation、vector retrieval、remote Skill 和 autonomous/background execution。

## 1. External and background write capabilities

桌面已有资源写入与报告交付契约见 [Harness Core 的能力目录](../../src-tauri/crates/yss-harness-core/README.md#4-registered-capabilities)。将这些能力扩展到外部客户端、无桌面会话或后台任务之前，必须同时满足：

- closed typed request/result 和 bounded batch；
- exact principal/project/session/revision binding；
- one-time approval grant 绑定 request fingerprint；
- idempotency key、durable invocation ledger 和 commit receipt；
- 一次 staged validation、一次 authority commit、一次 history/publication；
- cancel point-of-no-return 语义；
- crash 后能够区分 not-started、committed 和 unknown outcome；
- Assistant UI 可显示 pending approval、receipt、failure 和 undo/recovery action。

上述 gate 约束 external/headless/background adapters，不把现有桌面写入能力或草稿投影视为外部写授权。具体资源仍由原有业务 owner 提交。

## 2. Commit outcome reconciliation and recovery

- 定义 mutation receipt 的 durable schema 和 Project history correlation；
- 对 process crash、transport loss 和 late response 建立 unknown-outcome 状态；
- 只允许在 authority receipt 证明未提交时 retry；
- project replacement 将 bound runs/turns 标记 stale 或 paused；
- 完成 restart recovery、duplicate delivery 和 point-of-no-return tests。

该阶段是扩展 external/background 持久写能力的前置；桌面当前仅提供实际 receipt 保留和 unknown outcome 标识。

## 3. MCP external integration

### Server exposure

MCP adapter 与桌面监听入口尚未实现。生产暴露需要决定并实现：

- stdio / local transport 与 process lifecycle；
- authentication、principal 和 project-session binding；
- capability/resource/prompt surface；
- request budgets、rate limits 和 cancellation；
- Workflow run 到 MCP Tasks 的映射；
- packaging、permissions 和 explicit user enablement。

### Client

外部 MCP tools 进入独立 untrusted registry。默认 effect 为 external，要求显式 approval、bounded result、network/data-sharing policy，并禁止直接写 Project 或把对话持久化为长期记忆。Remote prompt 不自动成为 trusted Skill，remote resource 不自动进入 Knowledge index。

内部 Assistant 继续直接调用 Capability Gateway，不建立 loopback MCP。

## 4. Context and discovery integration

- [x] 从节点协议生成配置 JSON Schema，目录按需返回参数、默认值、约束与初始 pin 数量。
- [x] 使用 Tantivy 替换知识检索计分，配置中英文分词与可重建缓存；重复查询复用索引，按文档读取并核验引用。
- [x] 来源与文档集合原子替换；删除和更新使缓存失效，构建期间的删除不能产生有效旧引用。
- [ ] 以可选 Rig FastEmbed 适配器补充本地语义召回，验证模型下载、离线使用和 Windows 打包。
- [ ] 评估 Rig Memory 的上下文窗口策略与当前增量压缩的组合，保留取消、持久化检查点和工具回执恢复。

对话事件仍是历史的唯一权威来源，不重新引入对话自动写入 User/Project Memory 的流程。
Embedding 只索引明确选定的文档、节点说明及 Skill 发现信息，不将检索命中自动视作执行授权。
MCP Client 可复用 Rig MCP 工具适配，Server 使用相同 rmcp SDK 并复用既有 Application Gateway。

## 显式上下文与知识库验收

- [x] 在对话输入中按名称/路径引用项目资源（点击入口或 `@`）。
- [x] Rust 校验资源成员与项目绑定，引用随轮次保存并重放，历史可打开资源。
- [x] 队列、未确认输入与重开会话保留引用；引用不展开整图或数据内容。
- [x] 资源解析与模型准备纳入 Core 取消流程，迟到引用结果不能启动已停止的轮次。
- [ ] 桌面人工验收：多选、移除、同名资源、删除后重试、重开会话和窄面板。
- [x] 显式管理项目文档知识来源，显示索引状态、重建与移除入口。
- [x] 中文查询、命中位置摘要与片段引用校验，展开引用只返回对应片段。
- [x] 模型按需检索/读取知识片段；共用工具账本、计时、取消与历史回放，普通消息不自动检索。
- [x] 知识引用可打开原始项目文档。
- [ ] 桌面人工验收：知识来源添加/重建/移除、状态反馈、引用展开及原文打开。
- [x] 来源内容变化、删除及项目切换后，旧索引不得继续产生当前有效引用。
- [x] 委派/续接使用模型专用参数和任务结果投影；Core 捕获真实读取基线、自动去重，压缩和会话重开后继续校验。
- [x] 业务工具使用模型专用参数与实时/历史统一结果投影；资源、函数签名及图语义依据由调用层捕获，冲突停止写入，压缩与续接从账本恢复。

## 5. Knowledge retrieval

- 保留 source document/manifest 为 authority；
- 按知识库规模评估持久化索引与增量 chunk/index 更新；
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

### Manager–Worker 桌面验收

当前扁平六角色实现和执行契约见 [Harness Core](../../src-tauri/crates/yss-harness-core/README.md)。
以下为真实模型与桌面人工验收，自动化契约测试不能替代：

- [ ] 简单改图样式、续写报告只委派必要角色；Worker 不互相调用。
- [ ] 数据准备、分析、绘图、独立审查、报告和最终复核能返回可打开的真实产物。
- [ ] 并行只读任务的进度、取消、重新打开会话及中断提示可正确重放。
- [ ] 数据或图修改后，受影响的交付提示需要更新；Manager 使用新版本重新安排任务。
- [ ] 窄面板及各主题下任务卡片、统计计划和最终正文显示正常。

## 8. Provider and privacy controls

- [x] provider/model 目录、原生 Rig 协议、会话选择与执行时模型归属；
- [x] 系统凭据库、无密钥目录投影、原子配置与旧凭据清理；
- [x] 后端供应商预设、无凭据本地服务、模型发现/批量添加与高级生成参数；
- [ ] Claude、OpenAI Responses、Gemini、Kimi 与本地/兼容服务的真实模型桌面验收；
- [ ] OAuth、云平台签名等其他认证方式按实际需求扩展；
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
7. 更新 [Statistical Harness 当前架构](../../src-tauri/crates/yss-harness-core/README.md)；
8. 从本文件删除已完成项，并在 release history 中记录结果。

## Deferred decisions

以下实现选择由 ports 延后：embedding model、vector index、remote Skill distribution、MCP transport 和 background scheduler。它们不得反转 Harness → ports → adapters 的依赖方向。
