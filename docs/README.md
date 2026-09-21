# YssBI 文档索引

> Status: Current
> Scope: 系统文档、源码旁模块 README、开发规则与计划的导航
> Canonical owners: 本文件只拥有路由；各模块 README、适用的 .rules 和源码拥有具体内容
> Update when: 文档 owner、模块入口、维护规则或计划位置改变时

先读[系统总览](architecture/ARCHITECTURE.md)了解边界，再进入对应模块的 README。模块职责、接口和生命周期写在源码旁；修改模块时需要遵守的开发约束放在适用的 `.rules`。

## 按用途查找

| 位置             | 内容                                           | 入口                                        |
| ---------------- | ---------------------------------------------- | ------------------------------------------- |
| `architecture/`  | 系统上下文、所有权与跨模块关系                 | [系统架构索引](architecture/README.md)      |
| 模块 `README.md` | 该模块的职责、接口、数据流、生命周期与验证方式 | 下方模块入口                                |
| 模块 `.rules`    | 作用于当前目录及后代的开发约束                 | [根规则](../.rules)                         |
| `development/`   | 全仓开发与交付流程                             | [本地工作流](development/LOCAL_WORKFLOW.md) |
| `decisions/`     | 仍适用的设计理由                               | [决策索引](decisions/README.md)             |
| `roadmap/`       | 开放工作、阶段与人工验收                       | [路线图](roadmap/README.md)                 |
| `reference/`     | 生成索引与更多源码旁说明                       | [实现参考](reference/README.md)             |
| `benchmark/`     | 测量方法、样本与结果                           | [基准索引](benchmark/README.md)             |

## 模块契约入口

| 范围                         | 对应 README                                                                                                                                                                                                                            |
| ---------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Application 组装与用例       | [Application](../src-tauri/crates/yss-application/README.md)                                                                                                                                                                           |
| 项目生命周期与数据用例       | [Project application](../src-tauri/crates/yss-application/src/project/README.md)、[Database application](../src-tauri/crates/yss-application/src/database/README.md)                                                                   |
| 图编辑、保存和投影编排       | [Graph application](../src-tauri/crates/yss-application/src/graph/README.md)                                                                                                                                                           |
| 图表资源、预览与保存         | [Chart application](../src-tauri/crates/yss-application/src/chart/README.md)                                                                                                                                                           |
| 图语义与解析缓存             | [Graph analysis](../src-tauri/crates/yss-graph-analysis/README.md)、[Graph runtime](../src-tauri/crates/yss-graph-runtime/README.md)                                                                                                   |
| 执行、运行状态和 ResultStore | [Graph execution](../src-tauri/crates/yss-graph-execution/README.md)                                                                                                                                                                   |
| 共享语义与内核适配           | [Data contracts](../src-tauri/crates/yss-data-contract/README.md)、[Node kernel](../src-tauri/crates/yss-node-kernel/README.md)                                                                                                        |
| 科学计算入口                 | [SCI runtime](../src-tauri/crates/yss-sci-runtime/README.md)                                                                                                                                                                           |
| 工作台和图画布               | [Workbench](../src/modules/workbench/README.md)、[Graph editor](../src/modules/graph-editor/README.md)                                                                                                                                 |
| 结果查询与报告               | [Results application](../src/features/application/results/README.md)、[Results views](../src/modules/results/README.md)                                                                                                                |
| 图诊断与运行失败             | [Problems](../src/modules/problems/README.md)、[Output](../src/modules/output/README.md)                                                                                                                                               |
| JSON 页面与界面意图          | [UI contract](../src-tauri/crates/yss-ui-contract/README.md)                                                                                                                                                                           |
| Harness / Assistant          | [Harness Core](../src-tauri/crates/yss-harness-core/README.md)                                                                                                                                                                         |
| 运行观测和用户反馈           | [Observability](../src/features/application/observability/README.md)                                                                                                                                                                   |
| 日志存储与呈现               | [Tracing plugin](../src-tauri/crates/tauri-plugin-tracing/README.md)、[Logs](../src/modules/logs/README.md)                                                                                                                            |
| Tauri / IPC                  | [Application IPC](../src-tauri/crates/yss-application/src/ipc/README.md)                                                                                                                                                               |
| 插件体系                     | [插件目标契约](../plugins/README.md)；当前实现另见 [Plugin runtime](../src-tauri/crates/yss-plugin-runtime/README.md)、[Plugin protocol](../src-tauri/crates/yss-plugin-protocol/README.md)与 [Julia 插件](../plugins/julia/README.md) |

更多 Project、Database、SCI 与插件内部模块见[实现参考](reference/README.md)和[生成的模块索引](reference/MODULE_MAP.md)。

## 开发与交付

- [Agent 规则](development/AGENT_RULES.md)：根 `.rules` 加载的行为与跨系统约束。
- [本地工作流](development/LOCAL_WORKFLOW.md)：根命令、开发环境和验证范围。
- [变更流程](development/CHANGE_PROCESS.md)：实现与自审要求。
- [架构复核与文档检查](development/ARCHITECTURE_GATES.md)：人工边界复核及独立文档契约。

## 维护约定

代码、测试和 manifests 拥有可执行事实；模块 README 拥有本模块说明，系统总览只描述模块之间的关系。不要把同一契约复制到多个 README 或把大段实现说明放进 `.rules`。

维护中的 Markdown 声明 `Status`、`Scope`、`Canonical owners` 和 `Update when`。`Current` 表示当前实现；`Accepted Decision` 加 `Contract: Target Architecture` 表示已接受目标，不表示全部落地；`Planned` 用于计划；`Historical` 用于有明确历史范围的记录。

跨领域开放工作见 [TODO](../TODO.md)，计划与待验收见[组件重构](roadmap/COMPONENT_REFACTOR.md)、[JSON Driver](roadmap/jsonDriver.md)和 [motion](roadmap/motion.md)。已完成事项简记在 [v0.3](roadmap/v0_3.md)，详细历史由 Git 保留；尚未完成的验收保持开放。

移动文档时更新引用和章节链接，删除旧正文；新增细节先由现有模块 owner 承接。生成索引通过原生成器更新。
