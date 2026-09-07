# Tolerance 分析与后续改造边界

> Status: Historical
> Scope: 2026-09-07 删除全局近似比较配置后的源码分析快照与改造建议
> Canonical owners: 当前代码和 `yss-linalg` README 拥有实现事实；本文记录本次判断，开放工作由 TODO 跟踪
> Update when: 本次分析结论需要补充或纠正；后续实现改变时更新对应 Current owner

## 已确定的取舍

删除面向用户的全局近似比较容差。原 Absolute/Relative tolerance 只参与设置校验、保存和未接入生产的转换，没有影响 Execution 或 SCI 的数值计算。

本轮删除设置页输入与公式、IPC DTO、持久化模型中的 `numeric` 字段、相关校验，以及无调用方的 SCI/Execution 设置镜像和 Application 转换模块。Execution 的精确相等语义和现有算法阈值没有改变。

设置文件升级为 schema v2。读取 v1 文件时只丢弃废弃的 `settings.computation.numeric`，保留设置 revision 和缺失值偏好；下次成功保存写入 v2。当前 IPC 和 v2 文件仍严格拒绝未知字段。

“比较容差已删除”不意味着删除所有名为 tolerance 的字段。测试误差界限、VIF 报告中的 `tolerance = 1 / VIF` 和算法内部阈值有各自含义。

## 保留的数值策略

| 类别         | 作用                                             | 责任归属                                | 配置原则                                 |
| ------------ | ------------------------------------------------ | --------------------------------------- | ---------------------------------------- |
| 判秩与谱截断 | 决定哪些奇异值/特征值参与计算                    | `yss-linalg` 实现；SCI 选择策略与默认值 | 算法显式传参，默认保留自动数值尺度规则   |
| 迭代收敛     | 判断参数变化、目标函数变化或梯度是否满足停止条件 | 具体 SCI 模型                           | 独立于迭代次数上限；只暴露实际支持的指标 |
| 数值保护     | 限制概率范围、避免除零、保护对数与极端权重       | 对应算法或后端                          | 内部常量说明依据，不接受全局比较容差覆盖 |
| 测试误差界限 | 验证模型结果或残差是否符合参考值                 | 对应测试/fixture                        | 不读取用户设置，避免用户配置改变测试标准 |

同样的绝对值/相对值字段不代表同一种策略。近似相等比较以两个数的大小为尺度；SVD 截断通常以最大奇异值为尺度；迭代收敛还需要指定被测量的量及其范数。不要把参数变化、梯度和目标函数变化混成一个无解释的 tolerance。

参考：[SciPy 迭代收敛指标](https://docs.scipy.org/doc/scipy/reference/generated/scipy.optimize.least_squares.html)、[faer 分解接口](https://docs.rs/faer/latest/faer/)。这些资料用于说明能力与语义，不意味着项目改用这些库或默认值。

## 仍存在的问题

### T01：分解失败被转换为有效秩

[OLS](../../src-tauri/crates/yss-sci/src/regression/linear_model/ols/fit.rs)、[WLS](../../src-tauri/crates/yss-sci/src/regression/linear_model/wls.rs)、[GLS](../../src-tauri/crates/yss-sci/src/regression/linear_model/gls.rs) 和部分共线性处理把 `matrix_rank` 的错误转换为 `(0, infinity)`。部分路径随后计算 `rank - 1`。

后端分解失败不等于矩阵的秩为零。参与自由度、删列和求解决策时，应传播计算失败；真正的零秩或观测不足应单独处理。不能让失败继续进入自由度计算或触发无符号下溢。仅用于展示的诊断也应明确表达不可用，不能伪造秩。

优先级：高。验收应覆盖分解失败、零秩和不足自由度，并确认成功样本的统计结果不变。

### T02：判秩与求解决策分离

OLS 先按 SVD 阈值确定秩和自由度，再对完整 `X'X` 进行 Cholesky 求解。若直接开放可调判秩容差，可能判定某些方向应忽略，却仍按全部列计算系数。

建议先让模型明确其秩不足策略：拒绝输入、按模型规则删除共线列，或显式采用最小范数解。`yss-linalg` 提供一致的谱截断与诊断，SCI 负责截距、变量标签、删列顺序和统计自由度。不能在底层默默选择一种统计模型策略。

保留当前自动阈值作为基准；针对显式阈值验证系数、有效秩、自由度和协方差使用一致的处理策略。数据缩放与单位变化也应进入验证。

优先级：高，位于开放新的可调判秩选项之前。

### T03：迭代结束状态不可靠

[Prais](../../src-tauri/crates/yss-sci/src/regression/linear_model/prais.rs) 将 `delta_rho < tol` 与 `iterations >= max_iter` 合并到同一个 `converged` 条件，两个条件都进入成功返回路径。达到迭代上限时可能尚未满足收敛条件。

需要区分满足收敛条件、达到迭代上限和数值失败，并让调用方/报告能够理解实际停止原因。收紧容差不能只增加迭代次数，最终仍静默返回成功。

优先级：高。用严格容差与很小的迭代上限构造未收敛样本，再保留既有正常收敛样本作对照。

### T04：节点声明与模型参数没有贯通

[节点协议](../../src-tauri/crates/yss-graph-catalog/src/statistics/mod.rs) 为 Logit/Probit 声明 `max_iterations = 100` 和 `tolerance = 1e-6`；[Logit](../../src-tauri/crates/yss-sci/src/regression/discrete/logit.rs) 与 [Probit](../../src-tauri/crates/yss-sci/src/regression/discrete/probit.rs) 的算法配置只接受 `constant`，内部容差固定为 `1e-8`。

当前 Graph 的统计执行 kernel 只接入 OLS Fit/Summary，见[执行契约](../architecture/GRAPH_AND_EXECUTION.md)。因此不能把 Logit/Probit 参数可编辑视为其执行参数已经生效。

后续按“节点显式参数 → 模型配置 → 算法实际使用”接通，并统一协议默认值与算法默认值。不要补回一个全局容差结构来代替缺失的模型参数。

优先级：随对应模型执行接入处理。验收必须覆盖实际计算入口，不能只断言配置已保存。

### T05：重复分解与不必要的向量计算

[`matrix_rank`](../../src-tauri/crates/yss-linalg/src/lib.rs) 目前走完整 SVD，但判秩只需要奇异值。[White](../../src-tauri/crates/yss-sci/src/regression/diagnostics/white.rs) 与 [IM](../../src-tauri/crates/yss-sci/src/regression/diagnostics/im_test.rs) 先求奇异值，随后又执行一次 SVD。

后续可让只需诊断的操作仅计算奇异值；需要伪逆/截断求解时复用同一个分解对象和同一组奇异值。保留各模型现有阈值语义，先用数值回归验证，再测量代表性矩阵的时间和内存。

优先级：中，独立于设置字段删除，不把未经测量的性能收益作为已完成结果。

### T06：实际算法参数和停止原因缺乏追溯

当前 [ResultProvenance](../../src-tauri/crates/yss-execution/src/result.rs) 记录结果 ID、Run ID 和时间；模型报告中的有效数值参数与停止原因尚不完整。

后续运行入口解析一次有效模型参数，同一次运行使用固定参数。需要说明哪些配置来自节点、哪些采用算法默认值；结果按操作需要记录有效参数和停止原因。全局比较设置已经删除，不再设计对应的全局配置快照或设置更新触发重算机制。

优先级：随模型参数接入处理。优先复用已有节点参数与报告归属，不给所有结果增加一份无消费方的通用配置镜像。

## 与本轮删除独立的已有缺口

缺失值偏好仍可以保存，但[Graph OLS adapter](../../src-tauri/crates/yss-sci-runtime/src/service.rs) 当前固定采用 Reject，尚未消费该全局偏好。本轮没有改变其算法行为；它需要单独决定接入或移除，不能据设置保存成功声称计算已生效。

## 建议实施顺序

1. 修复 T01 的失败传播和 T03 的停止状态，建立成功/失败对照。
2. 确定 T02 的模型级秩不足策略，再统一判秩、截断与求解所用的阈值。
3. 在对应模型接入时完成 T04 与 T06，验证节点参数确实作用于计算和报告。
4. 单独处理 T05，使用数值回归和性能测量确认收益。

本次只完成全局近似比较配置删除与分析更新。以上 T01–T06 均为待处理项，不是已交付的算法修改。
