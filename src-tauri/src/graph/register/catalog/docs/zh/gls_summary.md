# GLS Summary

与 **GLS** 相同输入；估计完成后输出完整结果并打开报告窗口。

## 输入

| Pin | 说明 |
|-----|------|
| **In** | 执行流入口 |
| **Y** | 因变量（`Float64` `DataSeries`） |
| **X** | 一个或多个自变量（`Float64` 或 `Categorical`） |
| **Sigma** | n×n 误差协方差 `DataFrame` |
| **Time** | 可选时间索引 |
| **Config** | 可选 **GLS Configure** |

## 输出

| Pin | 说明 |
|-----|------|
| **Result** | **OLSResult**（含 GLS 系数与诊断） |
| **Out** | 执行流出口 |

运行后自动打开 **OLS Summary** 结果窗口。若只需 **Model** 句柄供 **Predict** 使用，请用 **GLS** 节点。
