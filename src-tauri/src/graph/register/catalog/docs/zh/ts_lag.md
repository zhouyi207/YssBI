# TS Lag

严格时间对齐滞后（Stata `L.` 语义）。将 **Value Series** 按 **Time Series** 对齐后滞后 **Lag** 阶。

时间列已标记 **Aligned** 且与数值列等长时跳过重新对齐；否则自动推断 **Interval** 并对齐。

## Pin

| Pin | 方向 | 说明 |
|-----|------|------|
| **Time Series** | 输入 | `Int64` 或 `Date` 时间序列 |
| **Value Series** | 输入 | `DataSeries<Float64>` |
| **Lag** | 输入 | 滞后阶数，默认 1 |
| **Time** | 输出 | 对齐后的完整时间网格 |
| **Lagged** | 输出 | 滞后后的数值序列 |

## 用法

前 **Lag** 个观测为 null。重复时间键会报错；建议先用 **TS Align** 获得规则网格。
