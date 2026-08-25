# OLS HAC Config

`cov_type = 'HAC'` 的异方差自相关稳健（HAC）协方差配置（ivreg2 风格）。

## 参数

| Pin | 默认 | 选项 |
|-----|------|------|
| **Kernel** | Bartlett | Bartlett、Parzen、Quadratic Spectral |
| **Bandwidth** | 自动 | 可选滞后/带宽（`Int64`） |

时间序列数据需在 **OLS & WLS Configure**（或回归节点）上提供 **Time** 索引。

**Config** → **OLS & WLS Configure** → **VCE**。
