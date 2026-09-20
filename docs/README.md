# YssBI Documentation

> Status: Current
> Scope: 文档分类、事实源优先级和维护入口
> Canonical owners: 本文件拥有文档路由；各链接文档拥有其声明的专项内容
> Update when: 新增、移动、归档或改变维护中文档的职责时

YssBI 文档按稳定责任、生命周期和 authority 边界组织。语言、框架和当前文件清单不是拆分依据；同一知识只在一个 canonical owner 中维护，其他位置只写摘要并链接。

## 事实源优先级

1. **Code / tests / manifests**：可执行事实、版本、路径、常量、依赖和 command registry。
2. **`docs/architecture/`**：当前架构模型与稳定 contract；已接受的目标契约必须显式区分设计约束与生产实现。
3. **`.rules`**：coding agent 行为和不可破坏的跨系统 guardrails。
4. **`docs/development/`**：如何修改、检查和交付。
5. **`docs/decisions/`**：为什么采用当前设计，以及被拒绝的替代方案。
6. **`docs/roadmap/`**：尚未实现或尚未接入生产的目标。
7. **`docs/version/`**：历史状态和版本记录，不是当前实现 authority。

发生冲突时先核对代码、测试和 manifest，再在同一变更中修正文档。Roadmap 和历史记录不能用来证明当前行为。

## Current architecture

- [架构文档入口](architecture/README.md)：专项架构的阅读方式与状态说明。
- [系统架构总览](architecture/ARCHITECTURE.md)：系统上下文、authority、依赖方向和主要运行链路。
- [Graph 与 Execution](architecture/GRAPH_AND_EXECUTION.md)：Draft、Projection、Save、运行准备、Execute、Problems、Results 与运行失败反馈。
- [Workbench FlexLayout](architecture/WORKBENCH_LAYOUT_ARCHITECTURE.md)：布局 authority、panel identity、close/reset/replacement 与持久化。
- [Runtime Signals](architecture/RUNTIME_SIGNALS.md)：logging、operational diagnostics、错误、反馈和各类运行信号的语义边界。
- [Statistical Harness](architecture/STATISTICAL_HARNESS.md)：当前 Harness、Gateway、Rig、SQLite、Tauri 和 Assistant 投影。

## Accepted architecture contracts

- [数据引擎迁移分析](architecture/迁移.md)：基于源码的目标边界、实施顺序和验收，优先细化下面两份方向讨论。
- [DataFusion 目标方向](architecture/DATAFUSION.md)：关系计算和数据集存储的目标职责；实际迁移范围由迁移分析收敛。
- [Polars 边界分析](architecture/POLARS.md)：移除宿主重复 DataFrame 计算层的理由；引擎选择由 DataFusion 目标方向更新。

- [Plugin 架构与契约](architecture/PLUGIN.md)：通用插件宿主、独立进程、声明式 UI、Webview、IPC、数据、生命周期和验收；状态为已接受的目标设计，不代表当前生产已全部实现。

## Focused implementation contracts

- [Application 用例与会话](../src-tauri/crates/yss-application/README.md)：职责、依赖分组与跨子系统调用流程。
- [Tauri / IPC transport](../src-tauri/crates/yss-application/src/ipc/README.md)
- [结构化日志与运行观测](../src-tauri/crates/tauri-plugin-tracing/README.md)
- [Project runtime authority](../src-tauri/crates/yss-project/README.md)
- [Filesystem primitives and watcher lifecycle](../src-tauri/crates/yss-filesystem/README.md)
- [Node definitions, registry and catalog](../src-tauri/crates/yss-node-catalog/README.md)
- [Node kernel contracts and implementations](../src-tauri/crates/yss-node-kernel/README.md)
- [Database runtime](../src-tauri/crates/yss-database-runtime/README.md)
- [Dataset snapshot store](../src-tauri/crates/yss-database-store/README.md)
- [SCI neutral contracts](../src-tauri/crates/yss-sci-contract/README.md)
- [SCI numerical models](../src-tauri/crates/yss-sci/README.md)
- [SCI synchronous runtime](../src-tauri/crates/yss-sci-runtime/README.md)
- [Linear algebra backend boundary](../src-tauri/crates/yss-sci-linalg/README.md)
- [Julia Bayes worker protocol](../plugins/julia/runtime/julia/README.md)

## Development

- [仓库 Agent 规则](development/AGENT_RULES.md)：变更纪律、跨系统边界和验证要求，由根 `.rules` 加载。
- [本地开发工作流](development/LOCAL_WORKFLOW.md)：环境和唯一命令矩阵。
- [变更流程](development/CHANGE_PROCESS.md)：feature、fix、refactor 和行为变更的设计与交付问题。
- [架构门禁](development/ARCHITECTURE_GATES.md)：production source discovery、分类、依赖审计和 semantic checks。

## Decisions, roadmap, and reference

- [SCI 节点整理与实施计划](sci/README.md)：[方法对照表](sci/METHOD_INVENTORY.csv)、[节点台账](sci/NODE_INVENTORY.csv)、[线性回归规格](sci/OLS_SPEC.md)及分批实施与验收。
- [节点组件与图操作 API 重构要求](sci/NODE_REFACTOR_REQUIREMENTS.md)：独立组件与节点装配、基础与复合类型、定义与展示分离、端口级直接操作、Command 提交与 undo/redo 的目标契约。
- [Decision 0001：Statistical Harness](decisions/0001-statistical-harness.md)
- [Statistical Harness roadmap](roadmap/STATISTICAL_HARNESS.md)
- [v0.3 roadmap](roadmap/v0_3.md)
- [v1.0 roadmap](roadmap/v1_0.md)
- [Graph 编辑与执行后续工作](roadmap/v0_3.md)：条件性后续项按 release backlog 维护。
- [组件化架构重构计划（实施中）](draft/component-plan.md)：组件边界、七类代码审计、执行遗留清理、结果缓存与租约，以及解析/缓存驱动的图状态和撤销恢复；按阶段记录剩余工作。
- [Open cross-domain backlog](../TODO.md)：未归入专项计划的其他开放事项。
- [Generated module map](reference/MODULE_MAP.md)
- [数据引擎测量记录](benchmark/DATA_ENGINE_BENCHMARK.md)：百万行查询、编辑、压实与 OLS 的本地观察。

## History

- [2026-09-19 节点执行链路修复](reviews/2026-09-19-node-execution-fixes.md)：必要性判断、共享载体、关系入口、资源控制、调用契约和验证边界。

- [2026-09-15 Graph 与 JSON 报告实施](reviews/2026-09-15-motion-json-driver-implementation.md)：语义报告试点、同一真实 OLS 样本的 renderer 对比，以及人工验收结果与明确跳过项。

- [2026-09-15 Graph 实时解析优化](reviews/2026-09-15-graph-resolution-optimization.md)：语义快照复用、Schema 增量缓存、批次解析与投影索引的实现和测量边界。
- [2026-09-15 当前图数据编辑实施记录](reviews/2026-09-15-current-graph-editing.md)：直接编辑后端当前文档、统一历史、显式保存与增量投影的实现、测试和性能测量边界。
- [2026-09-15 Graph 草稿后端化与 JSON 界面评估](reviews/2026-09-15-motion-json-driver-analysis.md)：两份方案的源码对照、合成传输测量、后端草稿与报告 JSON 的实施建议及性能验收边界。
- [Version history](version/README.md)
- [2026-09-10 数据引擎迁移验收](reviews/2026-09-10-data-engine-migration.md)：DuckDB/宿主 Polars 替换的逐项证据与验证边界。
- [2026-09-07 深度清理审计](reviews/2026-09-07-deep-cleanup-audit.md)：该次源码检查、复现证据和清理建议的快照。
- [2026-09-07 Tolerance 分析](reviews/2026-09-07-tolerance-analysis.md)：删除全局近似比较配置后的取舍、数值问题与后续改造边界。

## 文档状态

维护中文档在开头声明以下元数据：

```text
Status: Current | Accepted Decision | Planned | Historical
Scope: 本文负责什么
Canonical owners: 哪个文档或源码拥有具体事实
Update when: 什么变化要求更新本文
```

`Current` 只描述当前生产实现；`Accepted Decision` 记录已接受的设计；`Planned` 不代表已实现；`Historical` 仅供追溯。架构目录中的 `Accepted Decision` 文档必须额外声明 `Contract: Target Architecture`，明确其约束未来实现，不能作为已实现功能的证据。开发工作流文档保持 `Current`，设计理由记录仍可放在 `docs/decisions/`。容量、阈值、版本和完整模块列表等易变化事实应引用源码或由脚本生成，不手工复制到总架构文档。

Current 文档应明确已确认的实现缺口，并链接到 Planned/TODO 中的修复目标。模块边界、DTO 或前端测试已具备，不能据此声称生产 producer、端到端恢复或全部语义覆盖已完成；旧提交的审查结论也必须先按当前实现重新核对。
