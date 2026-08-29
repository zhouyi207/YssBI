# Hypergeometric

超几何分布描述从有限总体中**无放回**抽取 $n$ 个个体时，成功个体的数量。总体大小 **N**，其中 **K** 个为成功状态：

$$
P(X=k)=\frac{\binom{K}{k}\binom{N-K}{n-k}}{\binom{N}{n}}
$$

## 用法

设置 **N**、**K**、**n** 与 **N Samples** 后执行图。**Samples** 输出 `DataSeries<Int64>`，每个元素为一次无放回抽样中的成功数。适用于质检抽样、有限总体比例推断及与二项分布的对比实验。
