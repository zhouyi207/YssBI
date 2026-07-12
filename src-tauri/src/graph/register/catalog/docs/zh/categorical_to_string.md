# Categorical to String（分类转字符串）

将 `DataSeries<Categorical>`（或 Enum）cast 为 `DataSeries<String>`，输出类别标签文本。

## 输入

| Pin | 说明 |
|-----|------|
| **DataSeries** | 输入 `DataSeries<Categorical>` 或 Enum |

## 输出

| Pin | 说明 |
|-----|------|
| **DataSeries** | 可读的类别标签 |

## 用法

展示因子水平，或配合 **String to Float64** / **String to Int64** 解析标签。输入须为 categorical/enum 类型。
