# 架构文档

> Status: Current
> Scope: 当前架构和已接受目标契约的阅读入口
> Canonical owners: 各专项文档拥有其架构内容；总路由由 docs/README.md 维护
> Update when: 架构文档的职责、状态或阅读入口改变时

先读[系统架构总览](ARCHITECTURE.md)，理解状态归属和依赖方向，再按任务查阅专项文档。

| 文档                                                     | 状态              | 负责内容                               |
| -------------------------------------------------------- | ----------------- | -------------------------------------- |
| [系统架构](ARCHITECTURE.md)                              | Current           | 系统上下文、状态归属、依赖方向和主链路 |
| [Graph 与 Execution](GRAPH_AND_EXECUTION.md)             | Current           | 编辑、解析、保存、执行、诊断和结果     |
| [Workbench FlexLayout](WORKBENCH_LAYOUT_ARCHITECTURE.md) | Current           | 工作台布局、面板身份和生命周期         |
| [Runtime Signals](RUNTIME_SIGNALS.md)                    | Current           | 日志、运行观测、错误与反馈             |
| [Statistical Harness](STATISTICAL_HARNESS.md)            | Current           | Agent runtime、能力网关、持久化和投影  |
| [Plugin](PLUGIN.md)                                      | Accepted Decision | 插件目标契约；不表示已全部实现         |

具体 crate 与传输层说明见[参考索引](../reference/README.md)，重构和后续工作见[路线图](../roadmap/README.md)。完成摘要见 [v0.3](../roadmap/v0_3.md#已完成)，旧分析通过 Git 追溯。

[返回文档索引](../README.md)
