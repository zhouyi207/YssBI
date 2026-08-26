# Log2（以 2 为底的对数）

逐元素以 2 为底的对数：

$$
\text{Result} = \log_2 x
$$

**定义域：** $x > 0$。接受 `Int64`、`Float64` 或数值 `DataSeries`；输出为 `Float64` 或 `DataSeries<Float64>`。

## 用法

适用于以“翻倍”或比特尺度衡量增长的情形。正数约束与 **Ln** 相同。
