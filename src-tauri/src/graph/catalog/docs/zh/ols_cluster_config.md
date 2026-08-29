# OLS Cluster Config

`cov_type = 'cluster'` 的聚类稳健协方差配置。

## 公式

$$
\widehat{\mathrm{Var}}(\hat\beta) = (X'X)^{-1} \left(\sum_g X_g' \hat\varepsilon_g \hat\varepsilon_g' X_g \right) (X'X)^{-1}
$$

标准误允许聚类内任意相关。
