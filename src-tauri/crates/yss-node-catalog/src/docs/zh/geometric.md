# Geometric

几何分布 $\mathrm{Geometric}(p)$ 表示首次成功前的试验次数（含成功那次）：

$$
P(X=k)=(1-p)^{k-1}p,\quad k=1,2,3,\ldots
$$

## 用法

在 **Detail → 配置** 中设置分布参数和样本数，画布上保留 **Samples** 数据输出。

连接 **P** 与 **N** 后执行图。**Samples** 输出正整数的 `DataSeries<Int64>`。适用于首次命中时间、重复试验直到成功的等待次数等场景。
