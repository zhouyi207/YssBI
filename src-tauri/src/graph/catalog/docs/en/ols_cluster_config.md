# OLS Cluster Config

Cluster-robust covariance for `cov_type = 'cluster'`.

## Formula

$$
\widehat{\mathrm{Var}}(\hat\beta) = (X'X)^{-1} \left(\sum_g X_g' \hat\varepsilon_g \hat\varepsilon_g' X_g \right) (X'X)^{-1}
$$

Standard errors allow arbitrary within-cluster correlation.
