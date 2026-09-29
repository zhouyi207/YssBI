# 概要统计 t 检验

使用样本量、均值和样本标准差执行 t 检验，无需输入原始观测。

## 输入与参数

`series` 的排列由 `design` 决定：

| `design`              | 输入顺序                              |
| --------------------- | ------------------------------------- |
| `independent`（默认） | `[n1, mean1, sd1, n2, mean2, sd2]`    |
| `one_sample`          | `[n, mean, sd]`                       |
| `paired`              | `[n, mean_difference, sd_difference]` |

$n$ 必须为至少 2 的整数；均值和标准差须有限，标准差不得为负。配对设计必须提供配对差值的标准差，不能用前后两组各自的标准差替代。输入不得含空值。

`null_value` 默认为 `0`，表示原假设均值或均值差。`equal_variance` 默认为 `false`，仅独立组设计使用它选择 Welch 或合并方差。`alternative` 默认为 `two_sided`，另可选 `greater`、`less`。

## 假设与统计量

$H_0:\theta=\theta_0$，其中 $\theta_0$ 为 `null_value`；$\theta$ 是单样本均值、配对差值均值或第一组减第二组的总体均值差。备择按所选方向取不等于、大于或小于。

$$
t=\frac{\hat\theta-\theta_0}{SE},\qquad t\overset{H_0}{\sim}t_\nu.
$$

单样本和配对设计使用 $SE=s/\sqrt n$、$\nu=n-1$。独立组 Welch 方法令 $a=s_1^2/n_1$、$b=s_2^2/n_2$：

$$
SE=\sqrt{a+b},\qquad \nu=\frac{(a+b)^2}{a^2/(n_1-1)+b^2/(n_2-1)}.
$$

合并方差方法为：

$$
s_p^2=\frac{(n_1-1)s_1^2+(n_2-1)s_2^2}{n_1+n_2-2},\qquad
SE=s_p\sqrt{1/n_1+1/n_2},\qquad \nu=n_1+n_2-2.
$$

标准误须大于零。样本独立性、配对设计及小样本正态性假设与对应原始数据 t 检验相同。

## 输出与判读

`result`、`report` 返回同一报告。`method` 标明实际设计，`statistic` 为 t，`estimate` 为 $\hat\theta-\theta_0$，`standard_error` 为 $SE$，`degrees_of_freedom` 为 `[ν]`。`sample_sizes` 是一组或两组的样本量。按 `p_value` 判读，$p<\alpha$ 时拒绝 $H_0$。
