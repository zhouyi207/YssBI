# 变更流程

> Status: Current
> Scope: feature、fix、refactor、contract 和行为变更的设计、自审与交付
> Canonical owners: 本文拥有流程问题；具体架构答案和命令由链接的专项文档拥有
> Update when: 变更设计、review 或交付纪律改变时

按风险选择适用条目，不为勾选清单引入额外抽象。纯机械文档/格式修改可以省略不相关部分。

## 1. Problem, scope, and acceptance

- [ ] 用证据描述问题：用户路径、复现、性能数据或明确需求。
- [ ] 写明可观察结果、验收标准和本次不处理的事项。
- [ ] 判断是行为变更、行为保持重构、contract migration，还是纯文档/配置修改。
- [ ] 检查已有 capability/owner，避免第二事实源、临时 facade 和无必要 dependency。
- [ ] 对高风险或不可逆动作说明失败恢复和用户控制。

## 2. Authority, contract, and lifecycle

- [ ] 为每项 durable/runtime/UI state 指定唯一 owner，并写清 projection 或 draft 如何恢复。
- [ ] 保持依赖方向；framework/transport adapter 不拥有业务 workflow 或 domain state。
- [ ] 明确输入/output、identity、currentness、commit point、atomicity 和 late-result behavior。
- [ ] 跨 process/IPC 时选择 command、event 或 ordered channel，并定义 serialization、ordering、bounds、backpressure、gap 和 terminal semantics。
- [ ] 对 project replacement、save/close、cancel/timeout、restart 和 concurrent request 给出行为。
- [ ] 所有 I/O/long work 在锁外执行，commit 前重新验证 session/revision。

专项 owner：

- [系统总览](../architecture/ARCHITECTURE.md)
- [Graph / Execution / Results / Run Output](../architecture/GRAPH_AND_EXECUTION.md)
- [Workbench layout](../architecture/WORKBENCH_DOCKVIEW_ARCHITECTURE.md)
- [Runtime signals / feedback](../architecture/RUNTIME_SIGNALS.md)
- [Statistical Harness](../architecture/STATISTICAL_HARNESS.md)
- [Tauri / IPC transport](../../src-tauri/crates/yss-api/README.md)

## 3. Scale, safety, and errors

- [ ] 写出代表性的 graph/table/result/task 规模；大数据使用 paging/projection/batching，不通过 IPC 复制全量 DataFrame。
- [ ] 高频/长任务有 resource budget、progress、cancellation 和 bounded queue；慢 consumer 不得无限阻塞 producer。
- [ ] domain/application/infrastructure/transport seam 使用 typed failures，只向 UI 交付 stable code 和安全结构化数据。
- [ ] 检查数据最小化：日志、error details 和 external requests 不泄漏 row/cell、document、clipboard、SQL、connection、credential、prompt、memory 或 model/tool payload。
- [ ] 外部 API 明确 network、费用、rate limit、offline behavior、data sharing、credential storage、license 和 distribution impact。

## 4. Tests and documentation

- [ ] 每个新增测试对应一个不同、项目自有的回归风险，并通过 stable public seam 验证。
- [ ] 行为变更先增加最小聚焦回归；行为保持重构优先复用已有覆盖。
- [ ] 跨 wire/adapter 变更覆盖代表性 success/failure parsing 或 exhaustive mapping，不堆叠重复 matrix。
- [ ] 更新唯一 canonical owner；不要把同一规则复制到 `.rules`、总架构、checklist 和专项文档。
- [ ] Current、Accepted Decision、Planned、Historical 内容放入正确目录并更新 `docs/README.md` 索引。
- [ ] 版本、路径、阈值、commands 和 module inventory 尽量引用 manifest/source 或 generated reference。

## 5. Delivery

- [ ] 根据风险从 focused validation 扩展到 stack/cross-stack gate；命令只查 [Local Workflow](LOCAL_WORKFLOW.md)。
- [ ] 保存并报告新鲜验证输出；未运行的相关检查说明原因。
- [ ] 运行 `git diff --check`，复核未跟踪文件、生成文件 drift 和 unrelated user changes。
- [ ] 确认 acceptance criteria 已满足，roadmap/TODO 只保留真正未完成的工作。
