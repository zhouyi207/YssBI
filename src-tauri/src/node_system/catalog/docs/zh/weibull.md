# Weibull

Weibull 分布 $\mathrm{Weibull}(k, \lambda)$ 常用于可靠性分析：

$$
f(x)=\frac{k}{\lambda}\left(\frac{x}{\lambda}\right)^{k-1}\exp\!\left[-\left(\frac{x}{\lambda}\right)^k\right],\quad x \ge 0
$$

## Pin

| Pin | 说明 |
|-----|------|
| **Shape** | 形状参数 $k > 0$ |
| **Scale** | 尺度参数 $\lambda > 0$ |
| **N** | 样本量 |

## 用法

设置 **Shape**、**Scale** 与 **N** 后执行图。**Samples** 输出非负的 `DataSeries<Float64>`。适用于寿命/故障时间建模；$k=1$ 时退化为指数分布。
