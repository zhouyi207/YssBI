# Int Range

生成 **Int64** `DataSeries`，元素为等差整数序列：

$$
\text{start},\ \text{start}+1,\ \ldots,\ \text{start}+\text{length}-1
$$

## Pin

| Pin | 方向 | 说明 |
|-----|------|------|
| **Start** | 输入 | 首项（`Int64`） |
| **Length** | 输入 | 元素个数，须非负 |
| **Col Name** | 输入 | 序列名；省略或空字符串时默认为 `id` |
| **DataSeries** | 输出 | `DataSeries<Int64>` |

## 用法

常用于构造行号、索引或面板 ID。**Length** 为 0 时输出空序列。
