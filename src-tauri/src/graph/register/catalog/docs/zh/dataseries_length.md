# DataSeries Length

返回 **DataSeries** 的元素个数，输出 **Int64** 标量。

## Pin

| Pin | 方向 | 说明 |
|-----|------|------|
| **DataSeries** | 输入 | 任意元素类型的序列 |
| **Length** | 输出 | 观测数（`Int64`） |

## 用法

用于校验样本量、构造循环条件或与 **Int Range** 等节点配合。不包含 null 的计数逻辑由 Polars 序列长度决定。
