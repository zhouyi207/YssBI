# Decompose DataFrame

将 **DataFrame** 按列拆分为多个 **DataSeries** 输出。连接 **DataFrame** 后，根据输入表 schema 动态生成与列数对应的输出 pin。

## Pin

| Pin | 方向 | 说明 |
|-----|------|------|
| **DataFrame** | 输入 | 源表（通常来自 **Get DataFrame**） |
| *（列名）* | 输出 | 每列一个动态 pin，类型与列 dtype 一致 |

## 用法

先连接 **DataFrame**，列输出 pin 会自动出现。将各列接到变换、比较或 **Combine DataFrame** 等节点。输出 pin 名称与源列名一致，元素类型（`Float64`、`Int64`、`Boolean`、`Categorical` 等）自动匹配。
