# YssBI Open Backlog

> Status: Planned
> Scope: 尚未完成且未归入专项 roadmap 的跨领域工作
> Canonical owners: 本文件只拥有开放任务；代码和 Current docs 定义当前行为
> Update when: 添加、完成、取消、澄清或迁移开放任务时

只在这里记录仍需完成的工作。完成记录由 Git 和 `docs/version/` 保存；release/subsystem 计划放在 `docs/roadmap/`；实现细节和验证输出不追加到本文件。

## Active tasks

- [ ] 清理 Rust Clippy 基线：2026-09-07 验证发现 `yss-sci` 有 84 项既有诊断，`yss-api` 有 13 项既有诊断。数值循环/模型参数重构需结合 SCI golden tests；传输参数和 wire 枚举须保持 IPC 契约，不能为消除 lint 随意改协议。
- [ ] 评估前端既有的 14 条 Oxlint 警告；涉及遍历集合副本和测试 observer 的条目应先确认快照/回调语义，再决定简化或注明必要原因。
- [ ] 按需核对 [legacy TODO snapshot](docs/version/legacy-todo-2026-09-04.md)，只把经当前代码验证仍有效的条目迁回本文件或对应 roadmap；不要把历史 change summary 重新标为开放任务。

## Routed roadmaps

- [Statistical Harness](docs/roadmap/STATISTICAL_HARNESS.md)
- [v0.3](docs/roadmap/v0_3.md)
- [v1.0](docs/roadmap/v1_0.md)
