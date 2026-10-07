# Logistic 功效（二元预测变量）

当前支持一个二元预测变量、两个独立等人数组且无其他协变量的 Logistic Wald 规划。`baseline_probability` 默认 0.2，须在 (0,1)；`odds_ratio` 默认 1.5，须为正。sample_size 是每组人数 n，至少 1。

H0 为 OR=1，备择由 alternative 指定。由 `logit(p1)=logit(p0)+log(OR)` 得到另一组概率；log(OR) 的方差近似为 `[1/{p0(1−p0)}+1/{p1(1−p1)}]/n`。用 log(OR) 除以该标准误作为非中心正态偏移。小样本、稀有结果和分离可能使近似不准确；不包括多变量调整、连续预测变量或预测模型开发的样本量。

节点无需数据端口，参数描述计划研究。`solve_for=power` 默认按 `sample_size`（默认 100）计算功效；`sample_size` 模式按 `target_power`（默认 0.8，严格在 0 与 1 之间）求达到目标的最小整数。另一模式的样本量／目标功效参数不参与计算。`alpha` 默认 0.05，严格在 0 与 1 之间。

有 alternative 参数时，默认 `two_sided`，也可选 `greater` 或 `less`。双侧功效包含两个拒绝尾，即使其中一个方向与规划效应相反。功效是指定备择下拒绝零假设的概率，不是给定数据的 p 值，也不是“零假设为假的概率”。

**result** 报告 model、method、alpha、alternative、sample_size、sample_unit、total_observations、power、type_ii_error（1−power）、求样本量时的 target_power，以及生存设计的 expected_events。样本量按该设计单位取整数后重新计算 achieved power（字段 power）。不可达目标、无效设计或数值不收敛会报错。样本计数受整数精确表示与数值运算能力约束，不会读取或截断观测数据。规划效应应来自研究假设或可信外部资料；不从已拟合显著性反推“观察功效”。

[Reference](https://www.ncss.com/software/pass/regression-in-pass/)
