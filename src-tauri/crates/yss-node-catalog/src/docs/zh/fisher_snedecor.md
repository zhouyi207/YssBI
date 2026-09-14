# Fisher–Snedecor F

F 分布 $\mathrm{F}(d_1, d_2)$ 为两个独立卡方变量之比（各自除以自由度）：

$$
X=\frac{\chi^2(d_1)/d_1}{\chi^2(d_2)/d_2}
$$

## 用法

在 **Detail → 配置** 中设置分布参数和样本数，画布上保留 **Samples** 数据输出。

设置 **D1**、**D2** 与 **N** 后执行图。**Samples** 输出非负的 `DataSeries<Float64>`。适用于方差比检验、ANOVA 中的 F 统计量及回归整体显著性模拟。
