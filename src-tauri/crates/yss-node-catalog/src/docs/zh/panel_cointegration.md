# 面板协整检验（Fisher–Engle–Granger）

输入同一行域的有限数值 **response**、1–5 个 **predictors**、数值实体编号 **entity** 和整数期次 **time**，不接受缺失值。至少两个实体；每个实体的期次必须连续、键唯一，允许实体样本长度和起止期不同。输入变量应为一阶单整 I(1)，节点不自动确认单整阶数。

**参数**：lags 默认 1，为非负整数，指定残差 ADF 的差分滞后阶数；regression 默认 constant，可选 trend（协整方程包含常数和线性趋势）。

每个实体先估计 $y_{it}=a_i+b_it+x_{it}'\beta_i+e_{it}$，是否加入 $b_it$ 由 regression 决定，再对残差估计无常数、无趋势的 ADF。个体 p 值使用 Engle–Granger 的 MacKinnon 响应面，校准维数为因变量加自变量数，并保留第一阶段确定性项的影响；它不同于普通残差 ADF 的 p 值。

原假设为所有实体均不存在协整关系，备择为至少部分实体存在协整关系。独立实体下使用

$$
P=-2\sum_{i=1}^{N}\log p_i \ \sim\ \chi^2_{2N}.
$$

小 p 值拒绝原假设，不说明所有实体均协整，也不估计共同协整向量。

唯一 **result** 包含 method、deterministic、lags、observations、statistic、degrees_of_freedom、p_value 和 entity_tests。每个实体保留残差 ADF 统计量、校准 p 值、有效样本数及 cointegrating_coefficients，系数顺序为常数、可选趋势、输入顺序的自变量。若个体 p 值下溢到零，合并 statistic 为 null（正无穷极限），p_value 为 0。

需有足够的时序长度、正残差自由度和满秩协整设计；完全确定关系或退化残差不能给出有限 ADF 统计量，计算会失败。该检验假定横截面独立，不提供 Pedroni、Kao、Westerlund 或自助修正。因变量方向会影响有限样本结果。

个体方法见 [statsmodels Engle–Granger 说明](https://www.statsmodels.org/stable/generated/statsmodels.tsa.stattools.coint.html)。
