# Chi-Squared

卡方分布 $\chi^2(\nu)$ 为 $\nu$ 个独立标准正态变量平方之和：

$$
f(x)=\frac{x^{\nu/2-1}e^{-x/2}}{2^{\nu/2}\Gamma(\nu/2)},\quad x > 0
$$

## 用法

连接 **DF** 与 **N** 后执行图。**Samples** 输出非负的 `DataSeries<Float64>`。适用于方差检验、拟合优度统计量及作为 **FisherSnedecor** 分布的组成部分。
