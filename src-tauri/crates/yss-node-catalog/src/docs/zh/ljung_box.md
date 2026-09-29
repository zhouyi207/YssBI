# Ljung–Box 白噪声检验

连接按时间顺序排列的残差或待检查数列 `series`，至少 4 个无空值有限观测。节点按行位置计算滞后，不自动排序、去趋势或补齐时间。

## 参数与假设

`lags` 范围 1–40，默认 1。有效检验阶数为

$$
h=\min(\mathrm{lags},\lfloor n/2\rfloor-1,40).
$$

**原假设 $H_0$：** $\rho_1=\cdots=\rho_h=0$，前 $h$ 阶总体自相关同时为零。

**备择假设 $H_1$：** 至少一个 $\rho_k\ne0$，$1\leq k\leq h$。

“白噪声检验”在这里具体指这些自相关矩限制，不是对独立性、正态性或所有阶数无相关的完整证明。

## 计算公式

对 $x_1,\ldots,x_n$，先减去全样本均值 $\bar x$：

$$
\hat\rho_k=
\frac{\sum_{t=k+1}^n(x_t-\bar x)(x_{t-k}-\bar x)}
{\sum_{t=1}^n(x_t-\bar x)^2},\qquad
Q=n(n+2)\sum_{k=1}^h\frac{\hat\rho_k^2}{n-k}.
$$

当前节点使用

$$
Q\overset{H_0}{\approx}\chi_h^2,\qquad
p=1-F_{\chi_h^2}(Q).
$$

这是联合统计量，不能将它解释为单独第 $h$ 阶的检验。[statsmodels Ljung–Box](https://www.statsmodels.org/stable/generated/statsmodels.stats.diagnostic.acorr_ljungbox.html)说明了该检验及模型自由度修正的区别。

## 输出与判读

`result` 与 `report` 相同，含 `stat=Q`、`p_value`、`lags=h`，只输出一个有效阶数下的联合检验，而不是逐阶结果表。

例如请求 `lags=10` 但 $n=12$ 时，实际使用 $h=5$，p 值对应 $\chi^2_5$。在预设 $\alpha$ 下，$p<\alpha$ 拒绝前五阶全无相关；$p\geq\alpha$ 仅表示未发现充分证据，不能证明序列独立或模型正确。

该数列节点不接收拟合参数数目，**不进行模型自由度修正**。对 ARMA 拟合残差，常见修正使用 $h-p_{\mathrm{AR}}-q_{\mathrm{MA}}$ 自由度，本节点不会自动采用它。应先确认分析序列近似平稳、等间隔，并结合估计背景判断渐近 p 值是否合适。

常量序列没有可定义的样本自相关，本节点报错；观测不足、空值、非有限输入或结果同样失败。
