# Normal

正态（高斯）分布 $N(\mu, \sigma^2)$：

$$
f(x)=\frac{1}{\sqrt{2\pi\sigma^2}}\exp\!\left(-\frac{(x-\mu)^2}{2\sigma^2}\right)
$$

## Pin

| Pin | 说明 |
|-----|------|
| **Mean** | 均值 $\mu$ |
| **Std** | 标准差 $\sigma > 0$ |
| **N** | 样本量 |

## 用法

设置 **Mean**、**Std** 与 **N** 后执行图。**Samples** 输出 `DataSeries<Float64>`。适用于测量误差、中心极限定理演示、随机噪声生成及作为其他连续分布的近似基准。
