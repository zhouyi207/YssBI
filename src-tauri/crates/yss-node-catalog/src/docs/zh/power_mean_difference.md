# 均值差功效（t 检验）

`effect_size` 默认 0.5，是真实均值差除以总体标准差 d，保留符号。`design=independent` 默认比较两个独立、等样本量、等方差正态总体；sample_size 是每组 n（至少 2），总人数 2n。非中心 t 自由度为 2n−2，非中心参数为 `d√(n/2)`。

`design=one_sample` 检验单一正态总体均值，d 为真实均值减零假设值再除以总体标准差；sample_size 是总观测数 n（至少 2），自由度 n−1，非中心参数 `d√n`。H0 为标准化均值差等于 0，备择由 alternative 指定。按中心 t 临界值计算非中心 t 拒绝概率。独立组设计不包含 Welch 异方差或不等组大小。

节点无需数据端口，参数描述计划研究。`solve_for=power` 默认按 `sample_size`（默认 100）计算功效；`sample_size` 模式按 `target_power`（默认 0.8，严格在 0 与 1 之间）求达到目标的最小整数。另一模式的样本量／目标功效参数不参与计算。`alpha` 默认 0.05，严格在 0 与 1 之间。

有 alternative 参数时，默认 `two_sided`，也可选 `greater` 或 `less`。双侧功效包含两个拒绝尾，即使其中一个方向与规划效应相反。功效是指定备择下拒绝零假设的概率，不是给定数据的 p 值，也不是“零假设为假的概率”。

**result** 报告 model、method、alpha、alternative、sample_size、sample_unit、total_observations、power、type_ii_error（1−power）、求样本量时的 target_power，以及生存设计的 expected_events。样本量按该设计单位取整数后重新计算 achieved power（字段 power）。不可达目标、无效设计或数值不收敛会报错。样本计数受整数精确表示与数值运算能力约束，不会读取或截断观测数据。规划效应应来自研究假设或可信外部资料；不从已拟合显著性反推“观察功效”。

[Reference](https://www.stat.ethz.ch/R-manual/R-devel/library/stats/html/power.t.test.html)
