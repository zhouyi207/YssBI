# 广义模型功效（Poisson 发生率比）

当前明确实现两个独立、等人数、等暴露量组的 Poisson 发生率比 Wald 规划。`baseline_rate` 默认 1（正单位暴露事件率 λ0），`rate_ratio` 默认 1.5（正发生率比 RR），`exposure` 默认 1（每人正暴露量 e）。sample_size 是每组人数 n，至少 1。

H0 为 RR=1，备择由 alternative 指定。令 λ1=RR×λ0，log(RR) 的规划方差为 `(1/λ0+1/λ1)/(ne)`；其标准化均值偏移为 log(RR) 除以该标准误，功效使用正态尾概率。要求 Poisson 均值与方差关系，无过度离散、零膨胀或其他协变量。此节点不代表任意 GLM 的通用功效。

节点无需数据端口，参数描述计划研究。`solve_for=power` 默认按 `sample_size`（默认 100）计算功效；`sample_size` 模式按 `target_power`（默认 0.8，严格在 0 与 1 之间）求达到目标的最小整数。另一模式的样本量／目标功效参数不参与计算。`alpha` 默认 0.05，严格在 0 与 1 之间。

有 alternative 参数时，默认 `two_sided`，也可选 `greater` 或 `less`。双侧功效包含两个拒绝尾，即使其中一个方向与规划效应相反。功效是指定备择下拒绝零假设的概率，不是给定数据的 p 值，也不是“零假设为假的概率”。

**result** 报告 model、method、alpha、alternative、sample_size、sample_unit、total_observations、power、type_ii_error（1−power）、求样本量时的 target_power，以及生存设计的 expected_events。样本量按该设计单位取整数后重新计算 achieved power（字段 power）。不可达目标、无效设计或数值不收敛会报错。样本计数受整数精确表示与数值运算能力约束，不会读取或截断观测数据。规划效应应来自研究假设或可信外部资料；不从已拟合显著性反推“观察功效”。
