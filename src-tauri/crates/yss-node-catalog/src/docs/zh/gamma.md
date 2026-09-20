# Gamma

Gamma 分布 $\mathrm{Gamma}(\alpha, \beta)$（形状–速率参数化）：

$$
f(x)=\frac{\beta^\alpha}{\Gamma(\alpha)}x^{\alpha-1}e^{-\beta x},\quad x > 0
$$

## 用法

在 **Detail → 配置** 中设置分布参数和样本数，画布上保留 **Samples** 数据输出。

形状和率参数必须有限且严格为正。率为尺度的倒数，均值为 Shape / Rate，输出正 Float64 样本。
