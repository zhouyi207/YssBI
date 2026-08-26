# Exp（指数）

逐元素自然指数：

$$
\text{Result} = e^x
$$

对所有实数 $x$ 有定义。接受 `Int64`、`Float64` 或数值 `DataSeries`；输出为 `Float64` 或 `DataSeries<Float64>`。

## 用法

做对数逆变换或计算增长因子。$|x|$ 过大可能溢出为无穷大。
