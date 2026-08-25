# Sqrt（平方根）

逐元素平方根：

$$
\text{Result} = \sqrt{x}
$$

**定义域：** $x \geq 0$。负值在序列中为 null，标量可能为 NaN/错误。接受 `Int64`、`Float64` 或数值 `DataSeries`。

## 输入

| Pin | 说明 |
|-----|------|
| **X** | 标量或 `DataSeries` |

## 输出

| Pin | 说明 |
|-----|------|
| **Result** | $\sqrt{x}$，`Float64` 或 `DataSeries<Float64>` |

## 用法

将方差类或平方量还原到原单位。必要时在上游截断负值。
