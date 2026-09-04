# YssBI Documentation

> Status: Current
> Scope: 文档分类、事实源优先级和维护入口
> Canonical owners: 本文件拥有文档路由；各链接文档拥有其声明的专项内容
> Update when: 新增、移动、归档或改变维护中文档的职责时

YssBI 文档按稳定责任、生命周期和 authority 边界组织。语言、框架和当前文件清单不是拆分依据；同一知识只在一个 canonical owner 中维护，其他位置只写摘要并链接。

## 事实源优先级

1. **Code / tests / manifests**：可执行事实、版本、路径、常量、依赖和 command registry。
2. **`docs/architecture/`**：当前架构模型与稳定 contract。
3. **`.rules`**：coding agent 行为和不可破坏的跨系统 guardrails。
4. **`docs/development/`**：如何修改、检查和交付。
5. **`docs/decisions/`**：为什么采用当前设计，以及被拒绝的替代方案。
6. **`docs/roadmap/`**：尚未实现或尚未接入生产的目标。
7. **`docs/version/`**：历史状态和版本记录，不是当前实现 authority。

发生冲突时先核对代码、测试和 manifest，再在同一变更中修正文档。Roadmap 和历史记录不能用来证明当前行为。

## Current architecture

- [系统架构总览](architecture/ARCHITECTURE.md)：系统上下文、authority、依赖方向和主要运行链路。
- [Graph 与 Execution](architecture/GRAPH_AND_EXECUTION.md)：Draft、Projection、Compile、Save、Execute、Problems、Results 与 Run Output。
- [Workbench Dockview](architecture/WORKBENCH_DOCKVIEW_ARCHITECTURE.md)：布局 authority、panel identity、close/reset/replacement 与持久化。
- [Runtime Signals](architecture/RUNTIME_SIGNALS.md)：logging、operational diagnostics、错误、反馈和各类运行信号的语义边界。
- [Statistical Harness](architecture/STATISTICAL_HARNESS.md)：当前 Harness、Gateway、Rig、SQLite、Tauri 和 Assistant 投影。

## Focused implementation contracts

- [Tauri / IPC transport](../src-tauri/crates/yss-api/README.md)
- [Project runtime authority](../src-tauri/crates/yss-project/README.md)
- [Database runtime](../src-tauri/crates/yss-database-runtime/README.md)
- [SCI neutral contracts](../src-tauri/crates/yss-sci-contract/README.md)
- [SCI synchronous runtime](../src-tauri/crates/yss-sci-runtime/README.md)
- [Julia Bayes worker protocol](../src-tauri/julia/README.md)

## Development

- [本地开发工作流](development/LOCAL_WORKFLOW.md)：环境和唯一命令矩阵。
- [变更流程](development/CHANGE_PROCESS.md)：feature、fix、refactor 和行为变更的设计与交付问题。
- [架构门禁](development/ARCHITECTURE_GATES.md)：production source discovery、分类、依赖审计和 semantic checks。

## Decisions, roadmap, and reference

- [Decision 0001：Statistical Harness](decisions/0001-statistical-harness.md)
- [Statistical Harness roadmap](roadmap/STATISTICAL_HARNESS.md)
- [v0.3 roadmap](roadmap/v0_3.md)
- [v1.0 roadmap](roadmap/v1_0.md)
- [Open cross-domain backlog](../TODO.md)
- [Generated module map](reference/MODULE_MAP.md)

## History

- [Version history](version/README.md)

## 文档状态

维护中文档在开头声明以下元数据：

```text
Status: Current | Accepted Decision | Planned | Historical
Scope: 本文负责什么
Canonical owners: 哪个文档或源码拥有具体事实
Update when: 什么变化要求更新本文
```

`Current` 只描述当前生产实现；`Accepted Decision` 记录设计理由；`Planned` 不代表已实现；`Historical` 仅供追溯。容量、阈值、版本和完整模块列表等易变化事实应引用源码或由脚本生成，不手工复制到总架构文档。
