# Binomial

二项分布 $\mathrm{Binomial}(n,p)$ 表示 $n$ 次独立伯努利试验中成功次数：

$$
P(X=k)=\binom{n}{k}p^k(1-p)^{n-k},\quad k=0,1,\ldots,n
$$

## 用法

在 **Detail → 配置** 中设置分布参数和样本数，画布上保留 **Samples** 数据输出。

试验次数为 [0, 2^53] 内的整数，成功概率在 [0, 1] 内。输出 Int64 成功次数；零试验或 p = 0 返回零，p = 1 返回试验次数。
