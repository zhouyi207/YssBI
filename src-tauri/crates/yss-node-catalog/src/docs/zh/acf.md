# ACF 自相关函数

连接按时间顺序排列的非空有限 `series`，至少 4 个观测。节点按行的位置计算滞后，不读取日期、不自动排序、去趋势或补齐时间间隔；输入应先整理为所需的等间隔序列。

## 参数与公式

`lags` 为请求的最大滞后阶数，范围 1–40，默认 1。实际使用

$$
h=\min(\mathrm{lags},\lfloor n/2\rfloor-1,40).
$$

设序列为 $x_1,\ldots,x_n$、均值为 $\bar x$，样本自相关为

$$
\hat\rho_k=
\frac{\sum_{t=k+1}^{n}(x_t-\bar x)(x_{t-k}-\bar x)}
{\sum_{t=1}^{n}(x_t-\bar x)^2},
\quad k=1,\ldots,h,\qquad \hat\rho_0=1.
$$

所有阶数使用同一个完整样本平方和分母，不对分子额外乘 $n/(n-k)$。描述稳定的相关结构时通常要求弱平稳；趋势和季节性可能造成较高自相关。估计方法背景见 [statsmodels acf](https://www.statsmodels.org/stable/generated/statsmodels.tsa.stattools.acf.html)。

## 输出与判读

`result` 与 `report` 相同，含 `function=acf`、`observations=n` 和 `values`。`values[0]` 是 0 阶，`values[k]` 是 $k$ 阶，通常长度为 $h+1$。

例如 1 阶 ACF 为 0.7 表示相邻观测有较强的正线性关联，不表示有 70% 的概率发生某事件。观察衰减、周期峰值有助于探索动态结构，但不能仅凭 ACF 确定模型阶数或因果关系。

本节点输出相关系数，**不执行原假设/备择假设检验，也不输出置信带或 p 值**。若要联合检验前若干阶相关是否为零，可另用 Ljung–Box；不能把每个样本相关非零都判为显著。

常量序列因方差为零只返回 `values=[1]`，不填充其余阶数；这不表示各阶相关已被估计为零。空值、非有限输入或不可计算的数值结果会失败。
