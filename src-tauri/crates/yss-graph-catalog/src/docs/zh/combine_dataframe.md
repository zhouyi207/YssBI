# Combine DataFrame

将多列 **DataSeries** 合并为一张 **DataFrame**。较短序列会以 null 补齐至最长列的长度。

## 用法

至少连接一个 **Column** pin，需要时可继续添加 **Column** 输入。列名默认使用各序列名称，无名序列记为 `col_i`。常用于 **Decompose DataFrame** 或手工构造序列之后，重建表以供 **Filter DataFrame** 或计量节点使用。
