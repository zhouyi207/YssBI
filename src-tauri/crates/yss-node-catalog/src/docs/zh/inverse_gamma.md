# Inverse Gamma

逆 Gamma 分布 $\mathrm{InvGamma}(\alpha, \beta)$ 为 Gamma 的倒数，常用于方差先验：

$$
f(x)=\frac{\beta^\alpha}{\Gamma(\alpha)}x^{-\alpha-1}e^{-\beta/x},\quad x > 0
$$

## 用法

在 **Detail → 配置** 中设置分布参数和样本数，画布上保留 **Samples** 数据输出。

形状和尺度必须有限且严格为正。Scale 为 exp(-beta / x) 中的 beta，等价于 Gamma(shape, rate = scale) 样本的倒数。输出正 Float64 样本。
