# ACF 自相关函数

连接按时间顺序排列的 `series`，至少 4 个无空值、有限观测。`lags` 范围 1–40，默认 1；实际最大阶数为 $h=\min(\mathrm{lags},\lfloor n/2\rfloor-1,40)$。

## 计算公式

$$
\hat\rho_k=
\frac{\sum_{t=k+1}^{n}(x_t-\bar x)(x_{t-k}-\bar x)}
{\sum_{t=1}^{n}(x_t-\bar x)^2},
\quad k=1,\ldots,h,\qquad \hat\rho_0=1.
$$

$n$ 为观测数，$\bar x$ 为序列均值。各阶使用完整样本分母，不作 $n/(n-k)$ 修正。节点按行位置计算滞后，输入需自行排序并处理时间间隔。

## 输出与判读

`result`、`report` 相同，含 `function=acf`、`observations=n`、`values`。数组从 0 阶开始，通常长度为 $h+1$；常量序列仅返回 `[1]`。

正值表示同向线性关联，负值表示反向关联。节点不输出置信带或 p 值；若需联合检验前若干阶相关为零，可使用 Ljung–Box。

方法细节：[ACF](https://www.statsmodels.org/stable/generated/statsmodels.tsa.stattools.acf.html)。
