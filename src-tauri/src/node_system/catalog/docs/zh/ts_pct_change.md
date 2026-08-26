# TS Pct Change

计算百分比变化：

$$
\frac{y_t - y_{t-\text{lag}}}{y_{t-\text{lag}}}
$$

前 **lag** 个观测及分母为零处为 null。

## 用法

用于收益率或增长率分析。位置滞后，不依赖时间列；严格时间语义请用 **TS Diff** 并自行除以前值。
