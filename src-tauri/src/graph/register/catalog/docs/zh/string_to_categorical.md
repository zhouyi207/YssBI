# String to Categorical（字符串转分类）

使用 Polars 分类编码，将 `DataSeries<String>` 转为 `DataSeries<Categorical>`。

## 输入

| Pin | 说明 |
|-----|------|
| **DataSeries** | 输入 `DataSeries<String>` |

## 输出

| Pin | 说明 |
|-----|------|
| **DataSeries** | 输出 `DataSeries<Categorical>`，类别池由 distinct 字符串构建 |

## 用法

将自由文本编码转为因子型列，供 **Logit**、**Probit** 或面板回归使用。null 字符串保持 null。
