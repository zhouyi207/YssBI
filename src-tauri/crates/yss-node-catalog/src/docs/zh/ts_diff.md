# TS Diff

对 **Value Series** 做差分：

$$
\Delta y_t = y_t - y_{t-\text{lag}}
$$

可选 **Time Series** 时与 Stata `D.` 一致，仅在相邻 **Interval** 上差分（不跨 gap）；无时间列时使用位置滞后。

## 用法

已对齐面板/时间序列可直接差分；非规则时间请连接 **Time Series** 与 **Interval**，或先用 **TS Align** 对齐。
