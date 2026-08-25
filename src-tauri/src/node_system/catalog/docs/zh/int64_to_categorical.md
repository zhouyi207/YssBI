# Int64 to Categorical（整数转分类）

经字符串表示将 `DataSeries<Int64>` 编码为 `DataSeries<Categorical>`（类别池与其他 cat 转换一致）。

## 输入

| Pin | 说明 |
|-----|------|
| **DataSeries** | 输入 `DataSeries<Int64>` |

## 输出

| Pin | 说明 |
|-----|------|
| **DataSeries** | `DataSeries<Categorical>`，水平来自十进制字符串形式 |

## 用法

将整数编码当作无序因子用于回归。不同整数对应不同类别。
