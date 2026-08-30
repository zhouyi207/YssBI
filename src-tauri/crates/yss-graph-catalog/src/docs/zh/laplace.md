# Laplace

拉普拉斯（双指数）分布 $\mathrm{Laplace}(\mu, b)$：

$$
f(x)=\frac{1}{2b}\exp\!\left(-\frac{|x-\mu|}{b}\right)
$$

## 用法

设置 **Location**、**Scale** 与 **N** 后执行图。**Samples** 输出 `DataSeries<Float64>`。适用于具有尖峰厚尾特征的误差项、稳健统计演示及与正态分布的对比。
