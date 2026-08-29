# TS Rolling Mean

对 **DataSeries** 计算长度为 **Window** 的滚动均值；前 **Window − 1** 个值为 null。

## 用法

用于平滑或移动平均。**Window** 须大于 0；窗口内 null 按 Polars 滚动规则处理。
