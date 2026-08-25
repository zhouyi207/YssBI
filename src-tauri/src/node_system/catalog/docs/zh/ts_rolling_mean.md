# TS Rolling Mean

对 **DataSeries** 计算长度为 **Window** 的滚动均值；前 **Window − 1** 个值为 null。

## Pin

| Pin | 方向 | 说明 |
|-----|------|------|
| **DataSeries** | 输入 | `DataSeries<Float64>` |
| **Window** | 输入 | 窗口长度，须为正整数 |
| **Rolling Mean** | 输出 | 滚动均值序列 |

## 用法

用于平滑或移动平均。**Window** 须大于 0；窗口内 null 按 Polars 滚动规则处理。
