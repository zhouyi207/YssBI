# String to Float64（字符串转浮点）

通过 Polars cast 逐元素将 `DataSeries<String>` 解析为 `DataSeries<Float64>`。

## 用法

在数学或 **OLS** 前导入数值文本列。上游清理非数字字符，避免静默 null。
