# OLS Cluster Config

`cov_type = 'cluster'` 的聚类稳健协方差配置。

## 输入

| Pin | 说明 |
|-----|------|
| **Cluster ID** | 与 $Y$ 等长的组标签 `DataSeries`（`Categorical` 或 `Int64`）；同一标签的观测视为一个聚类 |

## 公式

$$
\widehat{\mathrm{Var}}(\hat\beta) = (X'X)^{-1} \left(\sum_g X_g' \hat\varepsilon_g \hat\varepsilon_g' X_g \right) (X'X)^{-1}
$$

标准误允许聚类内任意相关。

## 输出

| Pin | 说明 |
|-----|------|
| **Config** | 聚类 VCE 配置句柄 |

**Config** → **OLS & WLS Configure** → **VCE** → **OLS** / **WLS** / Summary 节点。
