# 路线图与专项计划

> Status: Planned
> Scope: 纯 Rust / GPUI 专项计划、版本候选、完成摘要与剩余验收导航
> Canonical owners: 各计划拥有进度与验收；源码、Cargo manifests 和模块 README 拥有当前实现事实
> Update when: 计划新增、完成、取消或验收状态改变时

当前架构为 GPUI 宿主 → ApplicationServices / 类型化用例 → Rust 领域。
阻塞业务由 worker 执行，类型化事件与投影回到原生实体；根 DockArea 拥有工作台拓扑，
Project、Graph、Execution 和 Harness 保留各自业务权威。当前入口是仓库根目录 `cargo run`，
命令与平台前置条件见[根 README](../../README.md)和[原生宿主 README](../../crates/yss-desktop-gpui/README.md)。
`react/` 只是不参与原生构建的参考源码，不是当前契约或原生验收的 owner。

| 计划 | 范围与状态 |
| --- | --- |
| [组件化架构重构](COMPONENT_REFACTOR.md) | Rust 组件边界、缓存/租约与阶段摘要；保留十二项界面验收及旁路/Chart 开放项 |
| [架构规则循环复核](ARCHITECTURE_RULES_REVIEW.md) | 当前 owner、历史结论适用范围、跨领域原生验收与独立环境问题 |
| [motion](motion.md) | GPUI/Harness 共用用例、已有领域工具覆盖、类型化交付与开放验收 |
| [原生 DockArea 工作台](flexlayout.md) | 单一拓扑、窗口/面板生命周期、布局扩展候选；保留原文件路径 |
| [Statistical Harness](STATISTICAL_HARNESS.md) | 外部/后台写入、MCP、知识/Skill/Workflow 等开放能力与准入，真实模型和桌面验收 |
| [Harness 领域工具](HARNESS_TOOL_ARCHITECTURE.md) | 资源、图、节点、数据、Mind、文档、图表与结果工具；保留真实测量范围及 Assistant 人工验收 |
| [节点分类调整](node-category.md) | Rust Catalog 主分类/检索标签与 GPUI 导航的待评估方案 |
| [节点配置与分步执行](NODE_AUTHORING_AND_EXECUTION.md) | P1–P7 原功能获用户确认；Rust 契约继续适用，GPUI 完整交互另保留开放项 |
| [GPUI 迁移](GPUI_MIGRATION.md) | 当前原生能力、精确用户回执及项目/图/资源/Assistant/插件/平台剩余验收 |
| [GPUI 组件审查](GPUI_COMPONENT_AUDIT.md) | 逐组件参考行为与原生对应；保留全部待查/待验收及局部证据限制 |
| [v0.3](v0_3.md) | Rust 完成摘要、近期候选、统计观察与剩余原生验收 |
| [v1.0](v1_0.md) | 原生更新与长期工具链候选 |

跨领域开放事项见 [TODO](../../TODO.md)。开始实施前按当前代码核实；`- [x]` 仅表示明确范围完成，
不扩大为整个子系统、全部平台或人工验收通过。后端回归、历史参考界面通过、静态样例、事件注入与
真实物理交互分别记录。用户明确跳过的历史性能验收保留“未测”状态，不自动重开也不算通过。

大型旧实施日志压缩为仍适用的设计、完成摘要与开放工作，详细历史由 Git 保留；文件路径继续保留。
实现变化更新对应 Current owner，计划不复制当前契约或建立第二套文档检查框架。

[返回文档索引](../README.md)
