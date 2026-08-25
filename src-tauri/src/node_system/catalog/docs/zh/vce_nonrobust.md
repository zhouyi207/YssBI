# VCE: NonRobust

同方差、无自相关假设下的经典 OLS 协方差。

## 公式

$$
\widehat{\mathrm{Var}}(\hat\beta) = \hat\sigma^2 (X'X)^{-1}, \quad \hat\sigma^2 = \frac{RSS}{n-k}
$$

## 输出

| Pin | 说明 |
|-----|------|
| **VCE** | 经典协方差常量句柄 |

## 用法

**VCE** → **OLS & WLS Configure** → **VCE** → **OLS** / **WLS** / Summary 节点。
