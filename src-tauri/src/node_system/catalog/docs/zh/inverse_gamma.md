# Inverse Gamma

逆 Gamma 分布 $\mathrm{InvGamma}(\alpha, \beta)$ 为 Gamma 的倒数，常用于方差先验：

$$
f(x)=\frac{\beta^\alpha}{\Gamma(\alpha)}x^{-\alpha-1}e^{-\beta/x},\quad x > 0
$$

## Pin

| Pin | 说明 |
|-----|------|
| **Shape** | 形状参数 $\alpha > 0$ |
| **Scale** | 尺度参数 $\beta > 0$ |
| **N** | 样本量 |

## 用法

设置 **Shape**、**Scale** 与 **N** 后执行图。**Samples** 输出正的 `DataSeries<Float64>`。适用于 Bayesian 中方差/精度先验及正随机倒数量的模拟。
