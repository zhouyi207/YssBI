# 配对 t 检验功效

`effect_size` 默认 0.5，表示真实平均配对差值除以“差值的总体标准差”，不是除以单次测量标准差。sample_size 是完整配对数 n，至少 2；total_observations 为 2n 次测量。

H0 为平均差值等于 0，备择由 alternative 指定。假定不同配对独立、配对差值正态；非中心 t 自由度 n−1，非中心参数 `d√n`。相关性已经包含在差值标准差中，不另外乘相关修正。

节点无需数据端口，参数描述计划研究。`solve_for=power` 默认按 `sample_size`（默认 100）计算功效；`sample_size` 模式按 `target_power`（默认 0.8，严格在 0 与 1 之间）求达到目标的最小整数。另一模式的样本量／目标功效参数不参与计算。`alpha` 默认 0.05，严格在 0 与 1 之间。

有 alternative 参数时，默认 `two_sided`，也可选 `greater` 或 `less`。双侧功效包含两个拒绝尾，即使其中一个方向与规划效应相反。功效是指定备择下拒绝零假设的概率，不是给定数据的 p 值，也不是“零假设为假的概率”。

**result** 报告 model、method、alpha、alternative、sample_size、sample_unit、total_observations、power、type_ii_error（1−power）、求样本量时的 target_power，以及生存设计的 expected_events。样本量按该设计单位取整数后重新计算 achieved power（字段 power）。不可达目标、无效设计或数值不收敛会报错。样本计数受整数精确表示与数值运算能力约束，不会读取或截断观测数据。规划效应应来自研究假设或可信外部资料；不从已拟合显著性反推“观察功效”。

[Reference](https://www.stat.ethz.ch/R-manual/R-devel/library/stats/html/power.t.test.html)
