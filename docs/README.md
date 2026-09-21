# YssBI 文档索引

> Status: Current
> Scope: 当前文档入口、事实源优先级和维护约定
> Canonical owners: 本文件拥有文档路由；专项文档和源码拥有各自内容
> Update when: 新增、删除、移动文档或调整文档职责时

文档保留当前架构、规则、重构计划和仍有用途的参考资料。已完成事项简记在 [v0.3](roadmap/v0_3.md)，过时方案和详细变更历史通过 Git 追溯。

## 按用途查找

| 目录            | 内容                                       | 阅读入口                                    |
| --------------- | ------------------------------------------ | ------------------------------------------- |
| `architecture/` | 当前系统架构、稳定契约及明确标注的目标契约 | [架构索引](architecture/README.md)          |
| `development/`  | 仓库规则、开发命令、变更流程和验证要求     | [本地工作流](development/LOCAL_WORKFLOW.md) |
| `decisions/`    | 仍适用的设计理由与取舍                     | [决策索引](decisions/README.md)             |
| `roadmap/`      | 重构计划、开放事项和简要完成记录           | [路线图索引](roadmap/README.md)             |
| `reference/`    | 生成的模块清单和源码旁专项说明             | [参考索引](reference/README.md)             |
| `benchmark/`    | 数据引擎、Graph 同步与解析的测量资料       | [基准索引](benchmark/README.md)             |

跨领域开放工作见根目录 [TODO](../TODO.md)。组件化重构的阶段与验收清单保留在[重构计划](roadmap/COMPONENT_REFACTOR.md)。[JSON Driver](roadmap/jsonDriver.md) 与 [motion](roadmap/motion.md) 的整体目标尚未完成，分别保留专项计划。

## 当前架构与目标契约

先读[系统架构总览](architecture/ARCHITECTURE.md)，再按子系统查阅：

- [Graph 与 Execution](architecture/GRAPH_AND_EXECUTION.md)：图文档、解析、投影、保存、执行、Problems 和 Results。
- [Workbench FlexLayout](architecture/WORKBENCH_LAYOUT_ARCHITECTURE.md)：布局、面板身份、生命周期和持久化。
- [Runtime Signals](architecture/RUNTIME_SIGNALS.md)：结构化日志、运行观测、错误与反馈边界。
- [Statistical Harness](architecture/STATISTICAL_HARNESS.md)：Harness、Gateway、Rig、持久化与 Assistant 投影。
- [Plugin 架构与契约](architecture/PLUGIN.md)：已接受的目标契约；具体接入范围见[插件开发说明](../plugins/julia/README.md)。

各 crate、IPC 和前端模块的说明见[参考索引](reference/README.md)，完整模块列表见[生成的模块索引](reference/MODULE_MAP.md)。

## 开发与交付

- [仓库 Agent 规则](development/AGENT_RULES.md)：根 `.rules` 加载的全仓策略、跨系统约束和验证纪律。
- [本地开发工作流](development/LOCAL_WORKFLOW.md)：环境、根命令和按改动范围验证。
- [变更流程](development/CHANGE_PROCESS.md)：设计、实现、自审和交付问题。
- [架构门禁](development/ARCHITECTURE_GATES.md)：源码分类、依赖审计、语义检查和文档契约。

## 事实源与维护约定

代码、测试和 manifests 拥有可执行事实；`architecture/` 维护当前架构与稳定契约；根 `.rules` 及其加载的规则约束 Agent 行为；`development/` 定义工作流程；`decisions/` 解释设计取舍；`roadmap/` 跟踪计划与完成记录。发生冲突时先核对实现，再更新对应正式文档。

每份 Markdown 在开头声明 `Status`、`Scope`、`Canonical owners` 和 `Update when`。状态含义如下：

| 状态                | 含义                                                           |
| ------------------- | -------------------------------------------------------------- |
| `Current`           | 当前实现、维护中的索引或流程                                   |
| `Accepted Decision` | 已接受的设计；架构目标另标 `Contract: Target Architecture`     |
| `Planned`           | 计划及进度；`- [ ]` 为未完成，`- [x]` 只表示该条明确范围已完成 |
| `Historical`        | 保留环境和方法边界的测量记录，不代表当前性能保证               |

目标契约、局部自动检查和历史完成记录都不能代替当前生产接入或桌面人工验收。尚未完成的验收继续使用 `- [ ]`。

新增内容先由现有文档承接。过时或不适用内容直接删除，已完成工作可在对应版本路线图中留下 `- [x]` 摘要；旧方案的移除不表示其功能已经实现。规则和重构计划继续维护，生成文件通过原生成器更新。删除或移动文件时同步处理链接、命令入口与文档契约，并按[本地工作流](development/LOCAL_WORKFLOW.md)验证。
