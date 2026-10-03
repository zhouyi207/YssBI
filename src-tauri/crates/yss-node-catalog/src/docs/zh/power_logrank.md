# Log-rank 功效（Schoenfeld 近似）

`hazard_ratio` 默认 0.7，须为正；`event_fraction` 默认 0.5，须在 (0,1]；`allocation` 默认 0.5，是处理组分配比例 q，须在 (0,1)。sample_size 是两组合计人数 n，至少 1。分配比例用于连续规划近似，不单独向上取整每组人数。

H0 为 HR=1，备择由 alternative 指定。Schoenfeld 近似正态偏移为 `log(HR)√{n×event_fraction×q(1−q)}`，expected_events 为 n×event_fraction。要求比例风险、独立观测和非信息删失。未模拟入组、随访、交叉处理或两组不同的事件过程；极不平衡分配或强效应下事件分配近似可能较差。

节点无需数据端口，参数描述计划研究。`solve_for=power` 默认按 `sample_size`（默认 100）计算功效；`sample_size` 模式按 `target_power`（默认 0.8，严格在 0 与 1 之间）求达到目标的最小整数。另一模式的样本量／目标功效参数不参与计算。`alpha` 默认 0.05，严格在 0 与 1 之间。

有 alternative 参数时，默认 `two_sided`，也可选 `greater` 或 `less`。双侧功效包含两个拒绝尾，即使其中一个方向与规划效应相反。功效是指定备择下拒绝零假设的概率，不是给定数据的 p 值，也不是“零假设为假的概率”。

**result** 报告 model、method、alpha、alternative、sample_size、sample_unit、total_observations、power、type_ii_error（1−power）、求样本量时的 target_power，以及生存设计的 expected_events。样本量按该设计单位取整数后重新计算 achieved power（字段 power）。不可达目标、无效设计或数值不收敛会报错。样本计数受整数精确表示与数值运算能力约束，不会读取或截断观测数据。规划效应应来自研究假设或可信外部资料；不从已拟合显著性反推“观察功效”。

[Reference](https://arxiv.org/abs/2407.03420)
