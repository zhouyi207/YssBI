# Probit Summary

与 **Probit** 相同输入；估计完成后输出摘要并打开报告窗口。

## 输入

| Pin | 说明 |
|-----|------|
| **In** | 执行流入口 |
| **Y** | 二元因变量 |
| **X** | 一个或多个自变量 |
| **Time** | 可选时间索引 |
| **Config** | 可选 **Probit Configure** |

## 输出

| Pin | 说明 |
|-----|------|
| **Result** | 摘要 **OLSResult** |
| **Out** | 执行流出口 |

运行后打开 Probit 报告窗口。若只需 **ProbitModel** 句柄，请使用 **Probit** 节点。
