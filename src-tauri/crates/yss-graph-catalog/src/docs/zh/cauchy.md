# Cauchy

柯西分布 $\mathrm{Cauchy}(\mu, \gamma)$ 具有重尾且无有限均值：

$$
f(x)=\frac{1}{\pi\gamma\left[1+\left(\frac{x-\mu}{\gamma}\right)^2\right]}
$$

## 用法

在 **Detail → 配置** 中设置分布参数和样本数，画布上保留 **Samples** 数据输出。

设置 **Location**、**Scale** 与 **N** 后执行图。**Samples** 输出 `DataSeries<Float64>`。适用于重尾现象、稳健性演示及极端值模拟（注意均值不存在）。
