# 相关性功效（Fisher z）

`null_correlation` 默认 0，`correlation` 默认 0.3，均须严格处于 (−1,1)。sample_size 是成对观测数 n，至少 4；每个观测包含两个变量。H0 为 ρ=ρ0，备择由 alternative 指定。

假定独立的二元正态观测，Fisher 变换后的标准化均值偏移为 `(atanh(ρ)−atanh(ρ0))√(n−3)`，近似使用标准正态临界值和尾概率。没有加入小样本偏差修正，不是精确相关系数分布，也不用于比较两个依赖相关系数。

节点无需数据端口，参数描述计划研究。`solve_for=power` 默认按 `sample_size`（默认 100）计算功效；`sample_size` 模式按 `target_power`（默认 0.8，严格在 0 与 1 之间）求达到目标的最小整数。另一模式的样本量／目标功效参数不参与计算。`alpha` 默认 0.05，严格在 0 与 1 之间。

有 alternative 参数时，默认 `two_sided`，也可选 `greater` 或 `less`。双侧功效包含两个拒绝尾，即使其中一个方向与规划效应相反。功效是指定备择下拒绝零假设的概率，不是给定数据的 p 值，也不是“零假设为假的概率”。

**result** 报告 model、method、alpha、alternative、sample_size、sample_unit、total_observations、power、type_ii_error（1−power）、求样本量时的 target_power，以及生存设计的 expected_events。样本量按该设计单位取整数后重新计算 achieved power（字段 power）。不可达目标、无效设计或数值不收敛会报错。样本计数受整数精确表示与数值运算能力约束，不会读取或截断观测数据。规划效应应来自研究假设或可信外部资料；不从已拟合显著性反推“观察功效”。
