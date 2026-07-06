# Multiply (×)

逐元素乘法，标量可广播：

$$
\text{Result} = A \times B
$$

任一输入为 `DataSeries` 时，双方提升为 `DataSeries<Float64>`；标量广播到序列长度。纯标量输出 `Float64`。

## 输入

| Pin | 说明 |
|-----|------|
| **A**（可选） | 第一个因数：`Int64`、`Float64` 或数值 `DataSeries` |
| **B**（可选） | 第二个因数：与 **A** 相同类型 |

## 输出

| Pin | 说明 |
|-----|------|
| **Result** | 积，`Float64` 或 `DataSeries<Float64>` |

## 用法

用标量缩放整条序列，或两条等长序列逐元素相乘（如构造交互项后再回归）。
