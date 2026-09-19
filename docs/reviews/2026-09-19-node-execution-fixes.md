# 节点执行链路修复记录

> Status: Historical
> Scope: 2026-09-19 节点审查的必要性判断、实施与验证
> Canonical owners: `yss-node-kernel`、`yss-graph-execution`、`yss-datafusion` 及 Application 源码
> Update when: 本次验证结果或实施范围发生变化时

原始审查见 [2026-09-19-review.md](2026-09-19-review.md)。本次优先修复可确认的行为错误、重复分配和边界缺口，复用既有所有者，没有新增 crate、ECS 或并行执行框架。

| 审查项             | 判断与实施                                                                                                                                                             |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 节点间深复制       | 必须处理。List/Record 使用不可变 Arc；调度结果移动原值，结果在发布前完成共享包装。统计输出不再先 clone 数值向量。                                                      |
| 统计预算与取消     | 必须处理。检查形状和总输入大小后再分配，统一 checked arithmetic 与 try_reserve；补齐预测、转换及输出循环检查，增加工作区与输出合计准入估算。GLS 对称矩阵不再重复转置。 |
| 原因被笼统错误吞掉 | 必须处理。维度、参数、行对齐、预算、调用契约和科学计算错误贯穿 Kernel、Execution、IPC 和中英文界面。                                                                   |
| 输入布局与输出契约 | 必须处理。固定输入键、重复组和数量进入注册及指纹，装配和调用均校验；返回时验证输出数量与外层载体，元素语义仍由 Graph 和生产者负责。                                    |
| 指纹混入展示字段   | 应处理。显式语义投影排除标题、文案和布局；保留 Configuration 规则、条件、默认值及资源解释。                                                                            |
| 组件化             | 复用现有模块与中立契约；删除重复预算及标量转换代码，不新增通用组件层。                                                                                                 |
| 静态调度准备       | 本次消除同一次运行中重复的 selection/producer 构建，并共享 GraphAnalysis 快照；完整静态调度与调用元数据缓存留待基准判断。                                              |
| 动态目录字符串泄漏 | 暂缓单独重构。生产组件已在会话替换间复用；尚需先测量重复装配场景，再整体调整消息和描述结构的所有权。不能用新增泄漏式缓存解决。                                         |

另修复了整链路中的 DataFrame 载体分歧：常量表及内存列组合经 `RelationFactory` 导入共享引擎，输出 RelationHandle；选列、筛选、拆列、统计和结果分页使用同一关系路径。字面量没有数据集绑定，不伪造授权，实际数据集的会话/快照检查仍然有效。移除旧 Record 表拆分和无生产调用者的重路由内核。

裸分类列在导入边界建立无序值域；显式值域和有序级别继续保留。日期时间文本复用既有日历转换，去除时区不移动墙钟时间。空列、空值和声明的列顺序均保留。

内存和关系数列进入统计计算时均拒绝有损整数转浮点，不因换一种载体改变精度规则。

## 验证范围

回归覆盖常量表到关系运算、回归和分页；内存列组合、拆列及再组合；结果共享引用；分配前预算、形状、取消和超时；输入顺序和输出载体；语义指纹与配置规则。沿用应用装配、数据转换、关系组合、缓存和结果生命周期的现有测试。

主要验证命令：

```powershell
pnpm test:rs:package -p yss-application --test numeric_execution
pnpm test:rs:package -p yss-application -p yss-datafusion -p yss-tabular-arrow -p yss-node-registry -p yss-graph-analysis -p yss-graph-execution -p yss-node-kernel -p yss-sci-runtime --lib --no-fail-fast
pnpm lint:rs:package -p yss-application -p yss-node-kernel -p yss-graph-execution -p yss-datafusion -p yss-tabular-arrow -p yss-node-registry -p yss-graph-analysis -p yss-relational-contract -p yss-sci-runtime --lib --no-deps
pnpm lint:rs:package -p yss-tabular-arrow --lib --no-deps '--' -D warnings
pnpm check:ts
pnpm lint:ts
git diff --check
```

库测试结果为 **189 通过、0 失败、2 忽略**：Application 94、DataFusion 13、GraphAnalysis 26、GraphExecution 25、NodeKernel 11、NodeRegistry 3、SCI Runtime 7、Tabular Arrow 10。两项忽略项是原有报告导出/性能手工测试，未当作通过。

应用 `numeric_execution` 集成测试 **14 项通过**。`check:ts` 通过；按改动包执行的 Clippy 完成，保留 12 条既有诊断，新增 Arrow 诊断修正后以 `-D warnings` 复查通过。Oxlint 完成，保留 8 条既有诊断。变更文件格式检查和 `git diff --check` 通过。

补充基线核对：`pnpm test:rs:package -p yss-graph-runtime --lib` 的兼容目录测试有一项失败。在独立的 `a8f3b74c` 工作树运行 `pnpm test:rs:package -p yss-graph-runtime --lib function_signature_resolves_stable_projected_call_ports`，得到同一断言失败：目录仍包含 `yssbi.logic.not`。原始实现将 Union 视为非精确类型并保守保留候选，需另行核对目录规则与测试契约。本次保留测试，没有修改该目录逻辑。

IPC 投影的两处旧断言已按其原有 `SemanticType::Numeric` 声明修正，定向复跑通过。

## 保留边界

- 密集统计工作区使用保守准入估算，不是对 faer 内部工作区或进程 RSS 的硬上限。
- 矩阵分解期间仍不能合作式中断；取消在准备、预测、转换及 SCI 前后边界生效。
- 模型的 f64 向量与通用 RuntimeValue 列表仍各有一份表示；紧凑共享数值缓冲需要统一所有消费者后单独实施。
- UI 全量编码、完整静态拓扑缓存和目录所有权调整不纳入本次改动。没有宣称测得吞吐或峰值内存提升比例。

桌面人工验收可使用常量表连接 Project/Filter/Linear Fit，确认分页和回归报告；再触发长度不一致，确认错误提示及节点定位。当前自动验证不替代这一步。
