# Prais Configure

为 **Prais** / **Prais Summary** 生成 **PraisConfigure** 结构体，用于修正误差项 AR(1) 序列相关（Stata `prais`）。

## 输入

| Pin | 默认 | 选项 |
|-----|------|------|
| **Constant** | `true` | 截距 |
| **Transform** | `prais` | `prais`（Prais–Winsten）、`corc`（Cochrane–Orcutt） |

## 输出

| Pin | 说明 |
|-----|------|
| **Config** | **PraisConfigure** 句柄 |

**Config** → **Prais** / **Prais Summary** 可选 **Config** pin。
