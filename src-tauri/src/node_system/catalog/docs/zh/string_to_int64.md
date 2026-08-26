# String to Int64（字符串转整数）

通过 Polars cast 逐元素将 `DataSeries<String>` 解析为 `DataSeries<Int64>`。

## 用法

转换以文本存储的 ID 或计数列。含小数的字符串无法解析，结果为 null。
