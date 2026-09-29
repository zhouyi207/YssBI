# Levene 方差齐性检验

以各组均值为中心，比较独立总体的方差。

## 输入与参数

在 `groups` 添加 2–16 个数值序列，每组至少 2 个有限观测，无空值。当前节点要求各列等长；关系数列须共享行域。无配置参数。

## 假设与统计量

$H_0:\sigma_1^2=\cdots=\sigma_k^2$；$H_1$：至少一个总体方差不同。

令 $c_i=\bar x_i$ 为第 $i$ 组均值，取绝对偏差 $z_{ij}=|x_{ij}-c_i|$。

记 $n_i$ 为组大小，$N=\sum_i n_i$，$\bar z_i$ 为组内偏差均值，$\bar z$ 为所有偏差的均值：

$$
F=\frac{\sum_i n_i(\bar z_i-\bar z)^2/(k-1)}
{\sum_i\sum_j(z_{ij}-\bar z_i)^2/(N-k)}
\overset{H_0}{\approx}F_{k-1,N-k}.
$$

组内偏差变异必须大于零。样本应相互独立。

该方法比 Bartlett 检验对非正态性更稳健，但均值中心仍受极端值影响；可用 Brown–Forsythe 节点选择中位数中心。

## 输出与判读

`result`、`report` 相同。`statistic` 为 $F$，`degrees_of_freedom` 依次给出分子、分母自由度 `[k−1, N−k]`，`p_value` 为 F 分布右尾概率，`sample_sizes` 为各组样本量。估计值和标准误为空。

$p<\alpha$ 时拒绝方差齐性；不显著表示证据不足，不能证明各方差完全相同。
