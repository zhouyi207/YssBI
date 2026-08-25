# Bernoulli

伯努利分布 $\mathrm{Bernoulli}(p)$ 描述单次试验成功（1）或失败（0）：

$$
P(X=1)=p,\quad P(X=0)=1-p
$$

从 $\mathrm{Bernoulli}(p)$ 独立抽样 **N** 次。

## Pin

| Pin | 说明 |
|-----|------|
| **P** | 成功概率 $p$，需满足 $0 \le p \le 1$ |
| **N** | 样本量（非负整数） |

## 用法

连接 **P** 与 **N** 后执行图即可。**Samples** 输出长度为 **N** 的 `DataSeries<Int64>`，元素为 0 或 1。适用于蒙特卡洛模拟、二值结果生成，或与 **Binomial** 等节点组合做随机实验。
