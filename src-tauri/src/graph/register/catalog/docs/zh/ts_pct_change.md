# TS Pct Change

计算百分比变化：

$$
\frac{y_t - y_{t-\text{lag}}}{y_{t-\text{lag}}}
$$

前 **lag** 个观测及分母为零处为 null。

## Pin

| Pin | 方向 | 说明 |
|-----|------|------|
| **DataSeries** | 输入 | `DataSeries<Float64>` |
| **Lag** | 输入 | 滞后阶数，默认 1 |
| **Pct Change** | 输出 | 百分比变化序列 |

## 用法

用于收益率或增长率分析。位置滞后，不依赖时间列；严格时间语义请用 **TS Diff** 并自行除以前值。
