# 面板单位根检验（Fisher–ADF）

连接同一行域的有限数值 **series**、实体编号 **entity** 和整数期次 **time**，不接受缺失值。节点按实体、时间排序；每个实体内部必须逐期连续且键唯一。允许实体的起止期或长度不同，至少两个实体。

**参数**：lags 默认 1，为非负整数，为每个实体 ADF 回归的差分滞后阶数；regression 默认 constant，可选 none（无确定性项）、trend（常数与线性趋势）。不自动选择滞后。

对每个实体估计

$$
\Delta y_{it}=a_i+b_it+\gamma_i y_{i,t-1}
+\sum_{j=1}^{p}\delta_{ij}\Delta y_{i,t-j}+u_{it}.
$$

$a_i,b_i$ 是否出现由 regression 决定，$p$ 为 lags。ADF 使用 $\hat\gamma_i/SE(\hat\gamma_i)$，按匹配确定性项的 MacKinnon 单位根分布计算 $p_i$，不使用普通 t 尾概率。

面板原假设为所有实体均存在单位根，备择为至少部分实体平稳。实体间独立时，

$$
P=-2\sum_{i=1}^{N}\log p_i \ \sim\ \chi^2_{2N}.
$$

小 p 值拒绝“全部存在单位根”，不表示每个实体均平稳。

唯一 **result** 包含 method、deterministic、lags、observations（ADF 有效样本总数）、statistic、degrees_of_freedom、p_value 和 entity_tests（实体、有效样本数、ADF 统计量、p 值）。若个体 p 值下溢为零，合并 statistic 为 null 表示正无穷极限，p_value 为 0。

每个实体至少 4 个原始观测，且扣除滞后后需有正残差自由度和可识别设计。常数序列、秩不足或任一实体失败会使整个检验失败。存在横截面相关时，独立性下的合并 p 值不适用；此节点不实施 LLC、IPS 或横截面相关修正。

参见 [Stata 面板单位根检验手册](https://www.stata.com/manuals/xtxtunitroot.pdf)。
