# Exponential

指数分布 $\mathrm{Exp}(\lambda)$ 以速率参数 $\lambda$ 刻画无记忆等待时间：

$$
f(x)=\lambda e^{-\lambda x},\quad x \ge 0
$$

期望 $\mathbb{E}[X]=1/\lambda$，方差 $\mathrm{Var}(X)=1/\lambda^2$。

## Pin

| Pin | 说明 |
|-----|------|
| **Rate** | 速率参数 $\lambda > 0$ |
| **N** | 样本量 |

## 用法

连接 **Rate** 与 **N** 后执行图。**Samples** 输出非负的 `DataSeries<Float64>`。适用于寿命分析、到达间隔时间、Poisson 过程的事件间隔模拟。
