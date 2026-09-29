# Ljung–Box 白噪声检验

连接按时间顺序排列的残差或待检验 `series`，至少 4 个无空值、有限观测。`lags` 范围 1–40，默认 1；有效阶数为 $h=\min(\mathrm{lags},\lfloor n/2\rfloor-1,40)$。

## 假设与公式

- **原假设 $H_0$：** $\rho_1=\cdots=\rho_h=0$，前 $h$ 阶总体自相关同时为零。
- **备择假设 $H_1$：** 至少一个上述自相关非零。

$$
Q=n(n+2)\sum_{k=1}^h\frac{\hat\rho_k^2}{n-k},
\qquad Q\overset{H_0}{\approx}\chi_h^2.
$$

$n$ 为观测数，$\hat\rho_k$ 为去均值后的 $k$ 阶样本自相关，分母使用全样本平方和。节点按行位置计算滞后。

## 输出与判读

`result`、`report` 相同，含 `stat=Q`、`p_value`、`lags=h`。p 值取卡方右尾；$p<\alpha$ 拒绝前 $h$ 阶联合无相关，不是只检验第 $h$ 阶。

本节点只接收数列，不作模型参数自由度修正；用于拟合残差时应注意这一限制。常量序列无法计算，未拒绝不代表已证明独立性。

方法细节：[Ljung–Box](https://www.statsmodels.org/stable/generated/statsmodels.stats.diagnostic.acorr_ljungbox.html)。
