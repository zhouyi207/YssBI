# KPSS 平稳性检验

连接按等间隔时间顺序排列的有限数值 `series`，至少三条观测且无缺失。`ts_deterministic=constant` 检验水平平稳，`trend` 在去除截距与线性趋势后检验趋势平稳。`ts_bandwidth=4` 为非负整数的 Newey–West 截断滞后数，须小于观测数；不设固定行数上限。

$H_0$：序列围绕指定水平或趋势平稳；$H_1$：存在随机趋势。用 OLS 去除确定性项后，定义残差部分和 $S_t=\sum_{i=1}^t e_i$：

$$
\mathrm{KPSS}=\frac{\sum_{t=1}^n S_t^2}{n^2\widehat\lambda^2}.
$$

$\widehat\lambda^2$ 为 Bartlett/Newey–West 残差长期方差，须为正。参考分布是对应的 KPSS 渐近分布，不是卡方分布；统计量越大，越支持拒绝平稳原假设。

`critical_values` 列出 10%、5%、2.5%、1% 临界值。`p_value` 在已发表临界值表中插值，范围仅为 [0.01,0.10]，须与 `p_value_kind` 合并解读：`less_than` 表示实际尾概率小于 0.01；`greater_than` 表示大于 0.10；`interpolated` 表示表内插值，包括恰好落在边界的数值。不能将截断后的边界当作精确尾概率。报告还包含 `statistic`、`long_run_variance`、`observations`、`effective_observations`、`bandwidth`、`deterministic`。KPSS 与 PP/ADF 原假设相反，可结合用于考察平稳性。

[KPSS 参考](https://www.statsmodels.org/stable/generated/statsmodels.tsa.stattools.kpss.html)
