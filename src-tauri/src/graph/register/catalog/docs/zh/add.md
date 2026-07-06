# Add (+)

逐元素加法，标量可广播：

$$
\text{Result} = a + b + \cdots
$$

任一操作数为 `DataSeries` 时，全部操作数提升为 `DataSeries<Float64>`；标量广播到序列长度。纯标量输入输出 `Float64`。

## 输入

| Pin | 说明 |
|-----|------|
| **Operands**（≥2，可选） | `Int64`、`Float64` 或数值 `DataSeries`；可重复的无名引脚 |

## 输出

| Pin | 说明 |
|-----|------|
| **Result** | 求和结果，`Float64` 或 `DataSeries<Float64>` |

## 用法

将两个及以上标量或序列接到操作数引脚。标量与序列混用可对整列加偏移。输出类型随最宽输入（序列优先于标量）。
