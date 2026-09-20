# YssBI Open Backlog

> Status: Planned
> Scope: 尚未完成且未归入专项 roadmap 的跨领域工作
> Canonical owners: 本文件只拥有开放任务；代码和 Current docs 定义当前行为
> Update when: 添加、完成、取消、澄清或迁移开放任务时

只在这里记录仍需完成的工作。完成记录由 Git 和 `docs/version/` 保存；release/subsystem 计划放在 `docs/roadmap/`；实现细节和验证输出不追加到本文件。

在这里有个很明显的问题，那就是 节点 catelog ai 获取不到，因为目前都是注册之后直接解析的，ai 不知道要创建哪些节点，这个是个大问题

## Active tasks

- [ ] clippy::too_many_arguments 需要处理
- [ ] 按 [节点执行链路修复记录](docs/reviews/2026-09-19-node-execution-fixes.md) 测量完整静态调度缓存、紧凑数值缓冲及目录重复装配的收益；矩阵分解中途取消与工作区硬限额需在 SCI/Linalg 所有者内另行设计。
- [ ] 核对兼容节点目录对 Union 类型的保守匹配与 `function_signature_resolves_stable_projected_call_ports` 断言；该失败已在原始 `a8f3b74c` 提交复现，见 [基线验证](docs/reviews/2026-09-19-node-execution-fixes.md#验证范围)。
- [ ] 按产品需求补齐 DataFrame Series 的 `length`、`count`、`sum`、`mean` 执行实现；当前仅有定义，不进入 GUI 创建目录与 AI 节点搜索，已有图通过缺少内核诊断阻断执行。
- [ ] 按 [Tolerance 分析](docs/reviews/2026-09-07-tolerance-analysis.md) 继续核对其余模型的秩不足策略、模型参数与报告，并评估 SVD 重复计算。
- [ ] 清理 Rust Clippy 基线：统计代码的既有诊断分布在 `yss-sci` 和迁出的 `yss-sci-runtime::data`，`yss-application::ipc` 也有既有诊断。数值循环/模型参数重构需结合 SCI golden tests；传输参数和 wire 枚举须保持 IPC 契约，不能为消除 lint 随意改协议。
- [ ] 评估前端既有的 14 条 Oxlint 警告；涉及遍历集合副本和测试 observer 的条目应先确认快照/回调语义，再决定简化或注明必要原因。
- [ ] 按需核对 [legacy TODO snapshot](docs/version/legacy-todo-2026-09-04.md)，只把经当前代码验证仍有效的条目迁回本文件或对应 roadmap；不要把历史 change summary 重新标为开放任务。

## Routed roadmaps

- [Statistical Harness](docs/roadmap/STATISTICAL_HARNESS.md)
- [v0.3](docs/roadmap/v0_3.md)
- [v1.0](docs/roadmap/v1_0.md)

数据驱动：Immer 库处理 json

1. json-render：最贴近你的目标

Vercel Labs 的 json-render 本身就是：

JSON Spec → Component Registry → React Components

而且它明确支持“AI 只能使用你注册过的组件”，非常适合你以后 YssBI 让 AI 生成 UI。

2. Zod：强烈建议一起使用

你的 JSON Renderer 最大风险不是渲染，而是：

JSON 是不是合法？

尤其以后 AI 生成：

3. Zustand + Immer：负责“JSON 局部变化”

这个还是保留。

整个架构建议是：

4. JSON Patch：我也很推荐

如果后端或者 AI 会不断修改页面：
