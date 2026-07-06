# Int64 to Float64（整数转浮点）

将 `DataSeries<Int64>` cast 为 `DataSeries<Float64>`，在 `f64` 精度内整数可精确表示。

## 输入

| Pin | 说明 |
|-----|------|
| **DataSeries** | 输入 `DataSeries<Int64>` |

## 输出

| Pin | 说明 |
|-----|------|
| **DataSeries** | 拓宽为 `DataSeries<Float64>` |

## 用法

为 **Ln**、**Divide** 或 **OLS** 准备整数列。null 保持 null。
