# Get DataSeries

从 **DataFrame** 按列名提取一列，输出 **DataSeries**。

**Int64** / **Date** 列会标记为未对齐时间序列（`Unaligned`），供下游 **TS** 节点使用。

## Pin

| Pin | 方向 | 说明 |
|-----|------|------|
| **DataFrame** | 输入 | 源表 |
| **Column Name** | 输入 | 要提取的列名（`String`） |
| **DataSeries** | 输出 | 单列序列，元素类型与源列一致 |

## 用法

连接 **Get DataFrame** 或数据库查询结果，在 **Column Name** 填入列名（或连接 **String** 常量）。输出可接入比较、变换、绘图或计量节点。
