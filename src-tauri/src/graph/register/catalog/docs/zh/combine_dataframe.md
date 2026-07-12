# Combine DataFrame

将多列 **DataSeries** 合并为一张 **DataFrame**。较短序列会以 null 补齐至最长列的长度。

## Pin

| Pin | 方向 | 说明 |
|-----|------|------|
| **Column** | 输入（可增删） | 一列或多列 **DataSeries**，按列堆叠 |
| **DataFrame** | 输出 | 合并后的表；列名取自序列名（否则为 `col_0`、`col_1` …） |

## 用法

至少连接一个 **Column** pin，需要时可继续添加 **Column** 输入。列名默认使用各序列名称，无名序列记为 `col_i`。常用于 **Decompose DataFrame** 或手工构造序列之后，重建表以供 **Filter DataFrame** 或计量节点使用。
