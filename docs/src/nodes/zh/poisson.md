# Poisson

泊松分布 $\mathrm{Poisson}(\lambda)$ 建模固定区间内稀有事件的发生次数：

$$
P(X=k)=\frac{e^{-\lambda}\lambda^k}{k!},\quad k=0,1,2,\ldots
$$

## 用法

在 Detail 的**分布参数**和**采样设置**两组中设置分布参数和样本数，画布上保留 **Samples** 数据输出。

率须有限且位于 [0, 2^53]，零率返回零。输出非负 Int64 样本，超过 2^53 的精确计数范围时报错。
