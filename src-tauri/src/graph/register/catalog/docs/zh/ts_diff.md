# TS Diff

对 **Value Series** 做差分：

$$
\Delta y_t = y_t - y_{t-\text{lag}}
$$

可选 **Time Series** 时与 Stata `D.` 一致，仅在相邻 **Interval** 上差分（不跨 gap）；无时间列时使用位置滞后。

## Pin

| Pin | 方向 | 说明 |
|-----|------|------|
| **Value Series** | 输入 | `DataSeries<Float64>` |
| **Time Series** | 输入 | 可选；`Int64` 或 `Date` 时间列 |
| **Lag** | 输入 | 滞后阶数，默认 1 |
| **Interval** | 输入 | 可选；与 **Time Series** 联用时的时间步长 |
| **Diff** | 输出 | 差分序列 |

## 用法

已对齐面板/时间序列可直接差分；非规则时间请连接 **Time Series** 与 **Interval**，或先用 **TS Align** 对齐。
