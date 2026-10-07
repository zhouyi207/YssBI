# 两比例差功效（正态近似）

`proportion1` 默认 0.6，`proportion2` 默认 0.4，均须在 [0,1]，不能同时为 0 或同时为 1。两组独立且等样本量；sample_size 是每组 n（至少 1），总人数 2n。H0 为 p1−p2=0，备择由 alternative 指定。

令 p̄=(p1+p2)/2，零假设规划标准误为 `√[2p̄(1−p̄)/n]`，备择标准误为 `√[{p1(1−p1)+p2(1−p2)}/n]`。用前者构造正态拒绝域，以真实差 p1−p2 和后者计算概率。采用无连续性修正的正态近似，不是精确二项/Fisher 检验；稀有事件、小样本或边界比例需要谨慎使用。

节点无需数据端口，参数描述计划研究。`solve_for=power` 默认按 `sample_size`（默认 100）计算功效；`sample_size` 模式按 `target_power`（默认 0.8，严格在 0 与 1 之间）求达到目标的最小整数。另一模式的样本量／目标功效参数不参与计算。`alpha` 默认 0.05，严格在 0 与 1 之间。

有 alternative 参数时，默认 `two_sided`，也可选 `greater` 或 `less`。双侧功效包含两个拒绝尾，即使其中一个方向与规划效应相反。功效是指定备择下拒绝零假设的概率，不是给定数据的 p 值，也不是“零假设为假的概率”。

**result** 报告 model、method、alpha、alternative、sample_size、sample_unit、total_observations、power、type_ii_error（1−power）、求样本量时的 target_power，以及生存设计的 expected_events。样本量按该设计单位取整数后重新计算 achieved power（字段 power）。不可达目标、无效设计或数值不收敛会报错。样本计数受整数精确表示与数值运算能力约束，不会读取或截断观测数据。规划效应应来自研究假设或可信外部资料；不从已拟合显著性反推“观察功效”。

[Reference](https://www.stat.ethz.ch/R-manual/R-devel/library/stats/html/power.prop.test.html)
