# Triangular

三角分布由最小值 **A**、最大值 **B** 与众数 **C** 确定（$A \le C \le B$），在 **C** 处密度最高：

$$
f(x)=\begin{cases}
\frac{2(x-A)}{(B-A)(C-A)} & A \le x < C \\
\frac{2(B-x)}{(B-A)(B-C)} & C \le x \le B
\end{cases}
$$

## 用法

在 **Detail → 配置** 中设置分布参数和样本数，画布上保留 **Samples** 数据输出。

设置 **A**、**B**、**C** 与 **N** 后执行图。**Samples** 输出 $[A,B]$ 内的 `DataSeries<Float64>`。适用于专家估计、项目工期不确定性及有界但非均匀的随机输入。
