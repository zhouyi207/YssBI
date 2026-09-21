# YssBI Open Backlog

> Status: Planned
> Scope: 尚未完成且未归入专项 roadmap 的跨领域工作
> Canonical owners: 本文件只拥有开放任务；代码和 Current docs 定义当前行为
> Update when: 添加、完成、取消、澄清或迁移开放任务时

只在这里记录仍需完成的工作。完成摘要使用 `- [x]` 记录在对应版本路线图，详细历史由 Git 保存；release/subsystem 计划放在 `docs/roadmap/`；实现细节和验证输出不追加到本文件。

在这里有个很明显的问题，那就是 节点 catelog ai 获取不到，因为目前都是注册之后直接解析的，ai 不知道要创建哪些节点，这个是个大问题

## Active tasks

- [ ] 完善线性回归报告的变量与配置元数据：当前 `yss-sci-runtime/src/regression/report.rs` 使用固定的 response 名称，报告仅携带协方差类型，需按实际展示需求传递训练列名及 HAC/Newey 等配置；沿用现有模型和报告契约局部扩展。
- [ ] 核对并修正线性回归配置对 Newey lag=0 的限制：`yss-node-catalog/src/statistics/mod.rs` 当前使用最小值为 1 的正整数参数，应与 SCI 支持的非负滞后范围一致。

- [ ] clippy::too_many_arguments 需要处理
- [ ] 按 [Node Kernel 当前边界](src-tauri/crates/yss-node-kernel/README.md) 测量完整静态调度缓存、紧凑数值缓冲及目录重复装配的收益；矩阵分解中途取消与工作区硬限额需在 SCI/Linalg 所有者内另行设计。
- [ ] 核对兼容节点目录对 Union 类型的保守匹配与 `function_signature_resolves_stable_projected_call_ports` 断言；旧审查在 `a8f3b74c` 复现了目录保留 `yssbi.logic.not` 的失败，继续处理前需按当前代码重新核实。
- [ ] 按产品需求补齐 DataFrame Series 的 `length`、`count`、`sum`、`mean` 执行实现；当前仅有定义，不进入 GUI 创建目录与 AI 节点搜索，已有图通过缺少内核诊断阻断执行。
- [ ] 按 [SCI](src-tauri/crates/yss-sci/README.md) 与 [Linalg](src-tauri/crates/yss-sci-linalg/README.md) 当前契约继续核对其余模型的秩不足策略、模型参数与报告，并评估 SVD 重复计算。
- [ ] 清理 Rust Clippy 基线：统计代码的既有诊断分布在 `yss-sci` 和迁出的 `yss-sci-runtime::data`，`yss-application::ipc` 也有既有诊断。数值循环/模型参数重构需结合 SCI golden tests；传输参数和 wire 枚举须保持 IPC 契约，不能为消除 lint 随意改协议。
- [ ] 评估前端既有的 14 条 Oxlint 警告；涉及遍历集合副本和测试 observer 的条目应先确认快照/回调语义，再决定简化或注明必要原因。

## Routed roadmaps

- [Statistical Harness](docs/roadmap/STATISTICAL_HARNESS.md)
- [v0.3](docs/roadmap/v0_3.md)
- [v1.0](docs/roadmap/v1_0.md)
- [JSON Driver](docs/roadmap/jsonDriver.md)
- [motion](docs/roadmap/motion.md)

immer, zod, zustand, json patch

FlexLayout 我认为还有好多功能没有用上，请分析目前项目还有哪些是可以让 Flexlayout 来管理的，无论是已实现的
还是未实现的都列出来
