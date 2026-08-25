# Subtract (−)

逐元素减法，标量可广播：

$$
\text{Result} = A - B
$$

任一输入为 `DataSeries` 时，双方提升为 `DataSeries<Float64>`；标量广播到序列长度。纯标量输出 `Float64`。

## 输入

| Pin | 说明 |
|-----|------|
| **A**（可选） | 被减数：`Int64`、`Float64` 或数值 `DataSeries` |
| **B**（可选） | 减数：与 **A** 相同类型 |

## 输出

| Pin | 说明 |
|-----|------|
| **Result** | 差，`Float64` 或 `DataSeries<Float64>` |

## 用法

连接 **A** 与 **B**（或使用默认值）。用标量减序列做去均值类变换，或用两条等长序列逐元素相减。
