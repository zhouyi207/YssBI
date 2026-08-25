# Logit Summary

与 **Logit** 相同输入；估计完成后输出摘要并打开报告窗口。

## 输入

| Pin | 说明 |
|-----|------|
| **In** | 执行流入口 |
| **Y** | 二元因变量（`Float64` / `Int64` / `Boolean` `DataSeries`） |
| **X** | 一个或多个自变量 |
| **Time** | 可选时间索引（元数据） |
| **Config** | 可选 **Logit Configure** |

## 输出

| Pin | 说明 |
|-----|------|
| **Result** | **OLSResult** 形态的摘要（Logit 系数与诊断） |
| **Out** | 执行流出口 |

运行后打开 Summary 结果窗口。若只需 **LogitModel** 供 **Logit Predict** 使用，请用 **Logit** 节点。
