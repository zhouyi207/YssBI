# Bartlett 方差齐性检验

在总体近似正态的前提下，检验多个独立组的方差是否相同。

## 输入与参数

在 `groups` 添加 2–16 个数值序列，每组至少 2 个有限观测，且样本方差严格为正。当前节点要求各列等长；关系数列须共享行域。空值会被拒绝。无配置参数。

## 假设与统计量

$H_0:\sigma_1^2=\cdots=\sigma_k^2$；$H_1$：至少一个方差不同。令 $n_i,s_i^2$ 为各组样本量、样本方差，$N=\sum_i n_i$：

$$
s_p^2=\frac{\sum_i(n_i-1)s_i^2}{N-k},\qquad
C=1+\frac{\sum_i1/(n_i-1)-1/(N-k)}{3(k-1)},
$$

$$
B=\frac{(N-k)\ln s_p^2-\sum_i(n_i-1)\ln s_i^2}{C}
\overset{H_0}{\approx}\chi^2_{k-1}.
$$

观测和各组应独立。该检验对非正态性较敏感；偏态、重尾或异常值可能导致拒绝，未必仅由方差差异造成。

## 输出与判读

`result` 为结构化结果。`statistic_name` 为 `chi_squared`，`statistic` 为 $B$，`degrees_of_freedom` 为 `[k−1]`，`p_value` 为卡方右尾概率，`sample_sizes` 为各组样本量。估计值和标准误为空。

在正态性条件合理时，$p<\alpha$ 支持方差不齐。对明显非正态的数据，可考虑 Levene 或 Brown–Forsythe 检验。
