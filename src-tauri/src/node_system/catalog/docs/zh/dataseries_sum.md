# DataSeries Sum

对数值 **DataSeries** 求和，输出 **Float64** 标量。

## Pin

| Pin | 方向 | 说明 |
|-----|------|------|
| **DataSeries** | 输入 | 数值序列 |
| **Sum** | 输出 | 总和（`Float64`） |

## 用法

连接 **Get DataSeries** 提取的数值列或其它 **Float64** / **Int64** 序列。null 值在 Polars 聚合中按默认规则处理。
