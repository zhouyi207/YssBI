# OLS & WLS Configure

将回归选项组合为 **OLSConfigure** 结构体，供 **OLS**、**WLS** 或 **OLS Summary** / **WLS Summary** 使用。

## 输入

| Pin | 说明 |
|-----|------|
| **Constant** | 是否包含截距项（未连接时使用节点默认值） |
| **VCE** | 协方差估计类型 — 连接 **VCE:** 常量节点或配置节点（**Fixed Scale**、**Cluster**、**HAC**、**Newey**） |
| **Time** | 时间索引（`Int64` 或 `Date` 的 `DataSeries`），部分 VCE 类型需要 |

## 输出

**Config** → 接入回归节点可选 **Config** pin。

## 常见 VCE 接法

- **VCE: NonRobust** — 经典 $ \hat{\mathrm{Var}}(\hat\beta) = \hat\sigma^2 (X'X)^{-1} $
- **VCE: HC0–HC3** — 异方差稳健（White 类）估计
- **OLS Cluster Config** — 按聚类 ID 的聚类稳健标准误
- **OLS HAC Config** — HAC / Newey–West（ivreg2 核函数集）
- **VCE: Newey** — Stata `newey` 风格（Bartlett + $n/(n-k)$）
