# Int64 to String（整数转字符串）

通过 Polars cast 将 `DataSeries<Int64>` 格式化为 `DataSeries<String>`。

## 输入

| Pin | 说明 |
|-----|------|
| **DataSeries** | 输入 `DataSeries<Int64>` |

## 输出

| Pin | 说明 |
|-----|------|
| **DataSeries** | 逐元素十进制字符串 |

## 用法

将整数键导出为标签，或供 **String to Categorical** 使用。null 保持 null。
