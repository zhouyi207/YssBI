# Poisson

泊松分布 $\mathrm{Poisson}(\lambda)$ 建模固定区间内稀有事件的发生次数：

$$
P(X=k)=\frac{e^{-\lambda}\lambda^k}{k!},\quad k=0,1,2,\ldots
$$

## Pin

| Pin | 说明 |
|-----|------|
| **Lambda** | 强度参数 $\lambda > 0$（期望与方差均为 $\lambda$） |
| **N** | 样本量 |

## 用法

连接 **Lambda** 与 **N** 后执行图。**Samples** 输出非负整数的 `DataSeries<Int64>`。适用于单位时间/空间内事件计数、排队到达量及稀有事件模拟。
