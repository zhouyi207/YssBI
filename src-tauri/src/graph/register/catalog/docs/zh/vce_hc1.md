# VCE: HC1 (robust)

HC1 在 HC0 基础上乘以自由度修正；部分软件中的默认 robust 选项。

## 公式

$$
\widehat{\mathrm{Var}}(\hat\beta) = \frac{n}{n-k} \cdot \widehat{\mathrm{Var}}_{\mathrm{HC0}}(\hat\beta)
$$

## 输出

| Pin | 说明 |
|-----|------|
| **VCE** | HC1 协方差常量句柄 |

## 用法

**VCE** → **OLS & WLS Configure** → **VCE** → **OLS** / **WLS** / Summary 节点。
