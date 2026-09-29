# 单样本均值 z 检验

在总体标准差已知时，检验总体均值。

## 输入与参数

`series` 至少包含 1 个有限数值，不得有空值。`null_mean` 默认为 `0`，须有限；`population_sd` 默认为 `1`，须为已知且大于零的总体标准差。`alternative` 默认 `two_sided`，另可选 `greater`、`less`。

## 假设与统计量

$H_0:\mu=\mu_0$；备择为 $\mu\ne\mu_0$、$\mu>\mu_0$ 或 $\mu<\mu_0$。

$$
z=\frac{\bar x-\mu_0}{\sigma/\sqrt n},\qquad z\overset{H_0}{\sim}N(0,1).
$$

$n$ 是观测数，$\sigma$ 是 `population_sd`。观测应独立；正态总体下该参考分布成立，其他分布依赖大样本近似。未知总体标准差时应使用 t 检验，不把样本标准差当成已知 $\sigma$。

## 输出与判读

`result` 和 `report` 相同。`statistic` 为 z，`estimate` 为 $\bar x-\mu_0$，`standard_error` 为 $\sigma/\sqrt n$，`sample_sizes` 为 `[n]`，`degrees_of_freedom` 为空。`p_value` 按所选方向计算，$p<\alpha$ 时拒绝 $H_0$。
