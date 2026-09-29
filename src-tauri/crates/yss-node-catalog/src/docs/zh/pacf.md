# PACF 偏自相关函数

连接按时间顺序排列的有限、无空值 `series`，至少 4 个观测。PACF 描述去除中间滞后项的线性作用后，当前观测与某阶滞后之间的关联。行顺序就是时间顺序，节点不自动排序或补齐时间。

## 参数与计算公式

`lags` 范围 1–40，默认 1。实际最大阶数 $h=\min(\mathrm{lags},\lfloor n/2\rfloor-1,40)$。先计算未经滞后样本数修正的 ACF：

$$
\hat\rho_k=
\frac{\sum_{t=k+1}^{n}(x_t-\bar x)(x_{t-k}-\bar x)}
{\sum_{t=1}^{n}(x_t-\bar x)^2}.
$$

再进行 Yule–Walker / Durbin–Levinson 递推：

$$
\phi_{1,1}=\hat\rho_1,\qquad
\phi_{k,k}=
\frac{\hat\rho_k-\sum_{j=1}^{k-1}\phi_{k-1,j}\hat\rho_{k-j}}
{1-\sum_{j=1}^{k-1}\phi_{k-1,j}\hat\rho_j},
$$

$$
\phi_{k,j}=\phi_{k-1,j}-\phi_{k,k}\phi_{k-1,k-j},
\quad j=1,\ldots,k-1,\qquad
\widehat{\mathrm{PACF}}(k)=\phi_{k,k}.
$$

$\phi_{k,j}$ 是递推中 $k$ 阶线性预测的第 $j$ 个系数。当前方法不是 Burg 估计，也不是逐阶使用不同样本的 OLS。递推背景见 [statsmodels levinson_durbin](https://www.statsmodels.org/stable/generated/statsmodels.tsa.stattools.levinson_durbin.html)。

## 输出与判读

`result` 和 `report` 相同，含 `function=pacf`、`observations=n`、`values`。数组**从 1 阶开始**，`values[0]` 对应 $\phi_{1,1}$，通常长度为 $h$；不含 0 阶。

例如 2 阶 PACF 接近零，表示控制 1 阶的线性作用后，2 阶没有明显额外样本线性关联。对平稳 AR 模型，PACF 的截尾形态可辅助选阶，但样本波动、趋势和季节性都可能干扰判断。

本节点是相关结构估计，**没有原假设、备择假设或 p 值输出**，也不提供置信带；“接近零”不是正式的不显著结论。通常应先明确等间隔、平稳的分析序列。

常量序列返回空数组；这表示无法计算各阶 PACF。递推分母非正、相关系数数值异常、空值或非有限输入会失败，不填充为零。
