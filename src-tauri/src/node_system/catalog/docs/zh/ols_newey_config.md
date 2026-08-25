# VCE: Newey

Stata **`newey`** 风格协方差（`cov_type = 'newey'`）：Bartlett 核 + $n/(n-k)$ 小样本修正。与 **OLS HAC Config**（ivreg2 HAC 实现）不同。

## 参数

| Pin | 说明 |
|-----|------|
| **Lag** | 可选最大滞后（`Int64`）；未连接时使用默认值 |

需在 **OLS & WLS Configure** 上提供 **Time**。**Config** → Configure 节点的 **VCE**。
