# 单样本 t 检验

检验一个总体的均值是否等于给定值。

## 输入与参数

连接 `series` 数值序列，至少 2 个有限观测；不自动删除空值。`null_mean` 默认 `0`，须为有限值。`alternative` 默认 `two_sided`，另可选 `greater`、`less`。

## 假设与统计量

$H_0:\mu=\mu_0$，其中 $\mu_0$ 为 `null_mean`。备择分别为 $\mu\ne\mu_0$、$\mu>\mu_0$ 或 $\mu<\mu_0$。

$$
t=\frac{\bar x-\mu_0}{s/\sqrt n},\qquad t\overset{H_0}{\sim}t_{n-1}.
$$

$n$ 为样本量，$\bar x$ 为均值，$s$ 为样本标准差。观测应独立；小样本推断要求总体近似正态，标准误必须大于零。

## 输出与判读

`result` 为结构化结果。`statistic` 为 t 值，`degrees_of_freedom` 为 `[n−1]`，`p_value` 对应所选备择。`estimate` 是 $\bar x-\mu_0$，`standard_error` 是 $s/\sqrt n$，`sample_sizes` 为 `[n]`。

$p<\alpha$ 时拒绝给定均值假设；否则表示证据不足以拒绝。
