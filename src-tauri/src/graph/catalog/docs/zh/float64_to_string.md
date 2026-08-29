# Float64 to String（浮点转字符串）

通过 Polars cast 将 `DataSeries<Float64>` 格式化为 `DataSeries<String>`。

## 用法

以文本展示数值结果，或供 **String to Categorical** 使用。null 与非有限值遵循 Polars cast 规则。
