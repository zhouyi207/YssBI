# OLS & WLS Configure

将回归选项组合为 **OLSConfigure** 结构体，供 **OLS**、**WLS** 或 **OLS Summary** / **WLS Summary** 使用。

## 常见 VCE 接法

- **VCE: NonRobust** — 经典 $ \hat{\mathrm{Var}}(\hat\beta) = \hat\sigma^2 (X'X)^{-1} $
- **VCE: HC0–HC3** — 异方差稳健（White 类）估计
- **OLS Cluster Config** — 按聚类 ID 的聚类稳健标准误
- **OLS HAC Config** — HAC / Newey–West（ivreg2 核函数集）
- **VCE: Newey** — Stata `newey` 风格（Bartlett + $n/(n-k)$）
