# Normal

正态（高斯）分布 $N(\mu, \sigma^2)$：

$$
f(x)=\frac{1}{\sqrt{2\pi\sigma^2}}\exp\!\left(-\frac{(x-\mu)^2}{2\sigma^2}\right)
$$

## 用法

在 Detail 的**分布参数**和**采样设置**两组中设置分布参数和样本数，画布上保留 **Samples** 数据输出。

均值必须有限，标准差必须有限且严格为正。输出 Float64 样本，每次执行重新采样，不缓存结果。
