# VCE: HC0

White 异方差稳健协方差（HC0，无小样本修正）。

## 公式

$$
\widehat{\mathrm{Var}}(\hat\beta) = (X'X)^{-1} X' \Omega X (X'X)^{-1}, \quad \Omega = \mathrm{diag}(\hat\varepsilon_i^2)
$$

## 输出

| Pin | 说明 |
|-----|------|
| **VCE** | HC0 协方差常量句柄 |

## 用法

**VCE** → **OLS & WLS Configure** → **VCE** → **OLS** / **WLS** / Summary 节点。
