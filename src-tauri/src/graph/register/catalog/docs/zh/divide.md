# Divide (÷)

逐元素除法，标量可广播：

$$
\text{Result} = \frac{A}{B}, \quad B \neq 0
$$

任一输入为 `DataSeries` 时，双方提升为 `DataSeries<Float64>`；标量广播到序列长度。除零在运行时出错。

## 输入

| Pin | 说明 |
|-----|------|
| **A**（可选） | 被除数：`Int64`、`Float64` 或数值 `DataSeries` |
| **B**（可选） | 除数：与 **A** 相同类型 |

## 输出

| Pin | 说明 |
|-----|------|
| **Result** | 商，`Float64` 或 `DataSeries<Float64>` |

## 用法

用标量除序列做归一化，或两条等长序列求比率。确保 **B** 在需要定义处不为零。
