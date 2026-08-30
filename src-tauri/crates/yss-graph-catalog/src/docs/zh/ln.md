# Ln（自然对数）

逐元素自然对数：

$$
\text{Result} = \ln x
$$

**定义域：** $x > 0$。非正输入在序列中为 null，标量会报错。接受 `Int64`、`Float64` 或数值 `DataSeries`；输出为 `Float64` 或 `DataSeries<Float64>`。

## 用法

对严格为正的序列做对数变换（如收入、价格）。若存在非正值，请在上游过滤或截断。
