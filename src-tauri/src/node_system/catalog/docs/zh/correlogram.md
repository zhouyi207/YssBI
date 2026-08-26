# Correlogram (ACF & PACF)

绘制 **Float64** **DataSeries** 的样本 ACF 与 PACF（至 **Lags** 阶，默认 20，实际上限为 $n/2$）。

含累积 Ljung–Box $Q$ 统计量与 p 值（悬停柱条查看）。95% 置信带半宽 $1.96/\sqrt{n}$。

## 用法

执行后打开 **Plot** 窗口展示 ACF / PACF 双面板。须至少 4 个非 null 观测。
