# Binomial

二项分布 $\mathrm{Binomial}(n,p)$ 表示 $n$ 次独立伯努利试验中成功次数：

$$
P(X=k)=\binom{n}{k}p^k(1-p)^{n-k},\quad k=0,1,\ldots,n
$$

## Pin

| Pin | 说明 |
|-----|------|
| **N Trials** | 每次抽样的试验次数 $n$ |
| **P** | 单次成功概率 $p$ |
| **N Samples** | 独立抽样次数 |

## 用法

设置 **N Trials**、**P** 与 **N Samples** 后执行图。**Samples** 输出 `DataSeries<Int64>`，每个元素为一次 $n$ 次试验的成功总数。常用于重复实验、质量控制计数及二项比例模拟。
