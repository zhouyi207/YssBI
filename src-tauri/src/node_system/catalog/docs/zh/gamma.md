# Gamma

Gamma 分布 $\mathrm{Gamma}(\alpha, \beta)$（形状–速率参数化）：

$$
f(x)=\frac{\beta^\alpha}{\Gamma(\alpha)}x^{\alpha-1}e^{-\beta x},\quad x > 0
$$

## 用法

设置 **Shape**、**Rate** 与 **N** 后执行图。**Samples** 输出正的 `DataSeries<Float64>`。适用于等待时间总和、Bayesian 共轭先验及作为 **Erlang** 的连续推广。
