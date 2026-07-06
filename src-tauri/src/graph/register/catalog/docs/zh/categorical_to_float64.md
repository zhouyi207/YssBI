# Categorical to Float64（分类转浮点）

将类别标签解析为 `DataSeries<Float64>`；标签须可解析为浮点数。

## 输入

| Pin | 说明 |
|-----|------|
| **DataSeries** | 输入 `DataSeries<Categorical>` 或 Enum |

## 输出

| Pin | 说明 |
|-----|------|
| **DataSeries** | `DataSeries<Float64>`；无效标签 → null |

## 用法

将以数值字符串存储的有序因子水平转回连续值，供数学节点使用。
