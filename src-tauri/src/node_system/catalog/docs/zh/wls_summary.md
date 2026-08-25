# WLS Summary

与 **WLS** 相同输入；估计完成后输出完整结果并打开报告窗口。

## 输入

| Pin | 说明 |
|-----|------|
| **In** | 执行流入口 |
| **Y** | 因变量 |
| **X** | 一个或多个自变量 |
| **Weights** | 与 **Y** 等长的正数权重 `Float64` `DataSeries` |
| **Time** | 可选时间索引 |
| **Config** | 可选 **OLS & WLS Configure**（截距、VCE） |

## 输出

| Pin | 说明 |
|-----|------|
| **Result** | 完整 **OLSResult**（含 WLS 估计与所选 VCE） |
| **Out** | 执行流出口 |

运行后自动打开 **OLS Summary** 结果窗口。若只需 **Model** 句柄而不打开窗口，请使用 **WLS** 节点。
