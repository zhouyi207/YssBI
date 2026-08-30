# Float64 to Int64（浮点转整数）

将 `DataSeries<Float64>` cast 为 `DataSeries<Int64>`（向零截断）。

## 用法

将连续序列量化为整数分箱或 ID。NaN/Inf 或超出 `i64` 范围的值为 null。
