# Pareto

帕累托分布 $\mathrm{Pareto}(\alpha, x_m)$ 刻画幂律尾部（$x \ge x_m$）：

$$
f(x)=\frac{\alpha x_m^\alpha}{x^{\alpha+1}},\quad x \ge x_m
$$

## 用法

设置 **Shape**、**Scale** 与 **N** 后执行图。**Samples** 输出 $\ge x_m$ 的 `DataSeries<Float64>`。适用于财富/城市规模等重尾现象及极端值建模。
