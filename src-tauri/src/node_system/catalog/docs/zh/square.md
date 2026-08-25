# Square（平方）

逐元素平方：

$$
\text{Result} = x^2
$$

对所有实数 $x$ 有定义。接受 `Int64`、`Float64` 或数值 `DataSeries`；输出为 `Float64` 或 `DataSeries<Float64>`。

## 输入

| Pin | 说明 |
|-----|------|
| **X** | 标量或 `DataSeries` |

## 输出

| Pin | 说明 |
|-----|------|
| **Result** | 逐元素 $x^2$ |

## 用法

构造多项式项或方差代理。对单一输入求平方时优先使用 **Square** 而非 **Multiply**。
