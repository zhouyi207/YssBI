# 均值等效功效（已知方差）

两个独立、等样本量正态组，共同总体标准差已知。`difference` 默认 0，是真实均值差除以该标准差 δ；`margin` 默认 0.3，是同单位的对称正等效界值 M。sample_size 是每组人数 n，至少 1。

H0 为 δ≤−M 或 δ≥M，H1 为 −M<δ<M。TOST 要求两个显著性水平各为 α 的单侧检验同时拒绝。令 s=√(2/n)、z=z(1−α)，功效为 `max[0, Φ{(M−δ)/s−z}−Φ{(−M−δ)/s+z}]`。与“非显著差异”不同；在 α<0.5 时对应的等效性置信水平是 1−2α。求样本量时 δ 必须严格落在等效区间，除非最小样本量已达到目标。不包含未知方差 t TOST、不对称界值或对数尺度生物等效性。

节点无需数据端口，参数描述计划研究。`solve_for=power` 默认按 `sample_size`（默认 100）计算功效；`sample_size` 模式按 `target_power`（默认 0.8，严格在 0 与 1 之间）求达到目标的最小整数。另一模式的样本量／目标功效参数不参与计算。`alpha` 默认 0.05，严格在 0 与 1 之间。

有 alternative 参数时，默认 `two_sided`，也可选 `greater` 或 `less`。双侧功效包含两个拒绝尾，即使其中一个方向与规划效应相反。功效是指定备择下拒绝零假设的概率，不是给定数据的 p 值，也不是“零假设为假的概率”。

**result** 报告 model、method、alpha、alternative、sample_size、sample_unit、total_observations、power、type_ii_error（1−power）、求样本量时的 target_power，以及生存设计的 expected_events。样本量按该设计单位取整数后重新计算 achieved power（字段 power）。不可达目标、无效设计或数值不收敛会报错。样本计数受整数精确表示与数值运算能力约束，不会读取或截断观测数据。规划效应应来自研究假设或可信外部资料；不从已拟合显著性反推“观察功效”。

[Reference](https://www.statsmodels.org/stable/generated/statsmodels.stats.weightstats.ztost.html)
