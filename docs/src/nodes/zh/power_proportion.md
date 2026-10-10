# 单比例功效（正态近似）

`null_proportion` 默认 0.5，须在 (0,1)；`proportion` 默认 0.6，须在 [0,1]。sample_size 为总观测数 n，至少 1。H0 为 p=p0，备择由 alternative 指定。

使用未作连续性修正的正态近似：零假设标准误为 `√[p0(1−p0)/n]`，备择比例的均值为 p、标准误为 `√[p(1−p)/n]`。以零假设标准误构造拒绝域，在备择分布下计算其概率。边界 p=0 或 1 使用退化分布。小样本或稀有事件可能使近似不准确；不声称是精确二项检验。

节点无需数据端口，参数描述计划研究。`solve_for=power` 默认按 `sample_size`（默认 100）计算功效；`sample_size` 模式按 `target_power`（默认 0.8，严格在 0 与 1 之间）求达到目标的最小整数。另一模式的样本量／目标功效参数不参与计算。`alpha` 默认 0.05，严格在 0 与 1 之间。

有 alternative 参数时，默认 `two_sided`，也可选 `greater` 或 `less`。双侧功效包含两个拒绝尾，即使其中一个方向与规划效应相反。功效是指定备择下拒绝零假设的概率，不是给定数据的 p 值，也不是“零假设为假的概率”。

**result** 报告 model、method、alpha、alternative、sample_size、sample_unit、total_observations、power、type_ii_error（1−power）、求样本量时的 target_power，以及生存设计的 expected_events。样本量按该设计单位取整数后重新计算 achieved power（字段 power）。不可达目标、无效设计或数值不收敛会报错。样本计数受整数精确表示与数值运算能力约束，不会读取或截断观测数据。规划效应应来自研究假设或可信外部资料；不从已拟合显著性反推“观察功效”。
