# Uniform

连续均匀分布 $\mathrm{Uniform}(a,b)$，在区间 $[a,b)$ 上密度恒定：

$$
f(x)=\frac{1}{b-a},\quad a \le x < b
$$

## 用法

设置 **Low**、**High** 与 **N** 后执行图。**Samples** 输出 `DataSeries<Float64>`。适用于无信息先验、随机化基准及 Monte Carlo 中的均匀随机数生成。
