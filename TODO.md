# YssBI Open Backlog

> Status: Planned
> Scope: 尚未完成且未归入专项 roadmap 的跨领域工作
> Canonical owners: 本文件只拥有开放任务；代码和 Current docs 定义当前行为
> Update when: 添加、完成、取消、澄清或迁移开放任务时

只在这里记录仍需完成的工作。完成记录由 Git 和 `docs/version/` 保存；release/subsystem 计划放在 `docs/roadmap/`；实现细节和验证输出不追加到本文件。

## Active tasks

- [ ] 补齐目录中 DataFrame 选列、求和节点的执行实现及选列类型推断；当前 `yssbi.dataframe.series.select` 输出泛型无法解析，且这两个节点均未注册到执行器。
- [ ] 按 [Tolerance 分析](docs/reviews/2026-09-07-tolerance-analysis.md) 处理数值策略：优先修复判秩失败回退和 Prais 迭代上限被当作成功，再统一模型级秩不足策略、模型参数与报告，最后评估 SVD 重复计算。
- [ ] 单独决定全局缺失值偏好的生产接入或移除；当前仅持久化，Graph OLS adapter 固定采用 Reject。
- [ ] 清理 Rust Clippy 基线：统计代码的既有诊断分布在 `yss-sci` 和迁出的 `yss-sci-runtime::data`，`yss-api` 也有既有诊断。数值循环/模型参数重构需结合 SCI golden tests；传输参数和 wire 枚举须保持 IPC 契约，不能为消除 lint 随意改协议。
- [ ] 评估前端既有的 14 条 Oxlint 警告；涉及遍历集合副本和测试 observer 的条目应先确认快照/回调语义，再决定简化或注明必要原因。
- [ ] 按需核对 [legacy TODO snapshot](docs/version/legacy-todo-2026-09-04.md)，只把经当前代码验证仍有效的条目迁回本文件或对应 roadmap；不要把历史 change summary 重新标为开放任务。

## Routed roadmaps

- [Statistical Harness](docs/roadmap/STATISTICAL_HARNESS.md)
- [v0.3](docs/roadmap/v0_3.md)
- [v1.0](docs/roadmap/v1_0.md)
