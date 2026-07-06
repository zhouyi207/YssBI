# VCE: HC2

HC2 使用杠杆调整后的残差平方。令 $h_i = x_i'(X'X)^{-1}x_i$：

## 公式

$$
\Omega_{ii} = \frac{\hat\varepsilon_i^2}{1 - h_i}
$$

## 输出

| Pin | 说明 |
|-----|------|
| **VCE** | HC2 协方差常量句柄 |

## 用法

**VCE** → **OLS & WLS Configure** → **VCE** → **OLS** / **WLS** / Summary 节点。
