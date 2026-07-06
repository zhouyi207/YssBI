# Log-Normal

对数正态分布：若 $\ln X \sim N(\mu, \sigma^2)$，则 $X$ 服从对数正态分布：

$$
f(x)=\frac{1}{x\sigma\sqrt{2\pi}}\exp\!\left(-\frac{(\ln x-\mu)^2}{2\sigma^2}\right),\quad x > 0
$$

## Pin

| Pin | 说明 |
|-----|------|
| **Mu** | 对数尺度均值 $\mu$ |
| **Sigma** | 对数尺度标准差 $\sigma > 0$ |
| **N** | 样本量 |

## 用法

设置 **Mu**、**Sigma** 与 **N** 后执行图。**Samples** 输出正的 `DataSeries<Float64>`。适用于收入/价格等右偏正值数据、乘性过程及非负随机变量建模。
