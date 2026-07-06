# Float64 to Categorical（浮点转分类）

经字符串表示将 `DataSeries<Float64>` 编码为 `DataSeries<Categorical>`。

## 输入

| Pin | 说明 |
|-----|------|
| **DataSeries** | 输入 `DataSeries<Float64>` |

## 输出

| Pin | 说明 |
|-----|------|
| **DataSeries** | 由各浮点字符串形式构成的 `DataSeries<Categorical>` |

## 用法

将连续值分箱为带标签的类别，用于绘图或类固定效应分组。格式遵循 Polars 浮点转字符串规则。
