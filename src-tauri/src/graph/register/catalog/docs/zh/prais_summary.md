# Prais Summary

与 **Prais** 相同输入；估计完成后输出结果并打开 Prais 报告窗口。

## 输入

| Pin | 说明 |
|-----|------|
| **In** | 执行流入口 |
| **Y** | 因变量 |
| **X** | 一个或多个自变量 |
| **Time** | 强烈建议提供（观测顺序） |
| **Config** | 可选 **Prais Configure**（**Transform**：`prais` 或 `corc`） |

## 输出

| Pin | 说明 |
|-----|------|
| **Result** | **OLSResult**（含 $\hat\rho$、变换类型等诊断） |
| **Out** | 执行流出口 |

运行后打开 Prais 报告窗口。若只需 **PraisModel** 句柄，请使用 **Prais** 节点。
