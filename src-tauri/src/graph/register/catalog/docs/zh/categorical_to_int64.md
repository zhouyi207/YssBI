# Categorical to Int64（分类转整数）

将类别标签解析为 `DataSeries<Int64>`；标签须为合法整数。

## 输入

| Pin | 说明 |
|-----|------|
| **DataSeries** | 输入 `DataSeries<Categorical>` 或 Enum |

## 输出

| Pin | 说明 |
|-----|------|
| **DataSeries** | `DataSeries<Int64>`；无效标签 → null |

## 用法

从因子列恢复数值编码。非整数类别名无法解析，结果为 null。
