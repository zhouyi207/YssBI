# PACF 偏自相关函数

连接按时间顺序排列的 `series`，至少 4 个无空值、有限观测。`lags` 范围 1–40，默认 1；实际最大阶数为 $h=\min(\mathrm{lags},\lfloor n/2\rfloor-1,40)$。

## 计算公式

使用样本 ACF 的 Yule–Walker / Durbin–Levinson 递推：

$$
\phi_{1,1}=\hat\rho_1,\qquad
\phi_{k,k}=
\frac{\hat\rho_k-\sum_{j=1}^{k-1}\phi_{k-1,j}\hat\rho_{k-j}}
{1-\sum_{j=1}^{k-1}\phi_{k-1,j}\hat\rho_j},
$$

$$
\phi_{k,j}=\phi_{k-1,j}-\phi_{k,k}\phi_{k-1,k-j},
\quad j<k,\qquad
\widehat{\mathrm{PACF}}(k)=\phi_{k,k}.
$$

$\hat\rho_k$ 为 $k$ 阶样本自相关，$\phi_{k,j}$ 为递推系数。PACF 衡量控制中间滞后项后的线性关联。

## 输出与判读

`result`、`report` 相同，含 `function=pacf`、`observations=n`、`values`。数组从 **1 阶**开始，`values[0]` 为 1 阶 PACF，通常长度为 $h$；常量序列返回空数组。

节点按行位置计算滞后，不自动排序或补齐时间。输出可辅助探索时序结构，但不提供置信带或 p 值；递推不可计算时失败。

方法细节：[Durbin–Levinson 递推](https://www.statsmodels.org/stable/generated/statsmodels.tsa.stattools.levinson_durbin.html)。
