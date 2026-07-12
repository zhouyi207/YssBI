# OLS Cluster Config

Cluster-robust covariance for `cov_type = 'cluster'`.

## Inputs

| Pin | Description |
|-----|-------------|
| **Cluster ID** | Group labels as a `DataSeries` (`Categorical` or `Int64`, same length as $Y$); observations sharing a label form one cluster |

## Formula

$$
\widehat{\mathrm{Var}}(\hat\beta) = (X'X)^{-1} \left(\sum_g X_g' \hat\varepsilon_g \hat\varepsilon_g' X_g \right) (X'X)^{-1}
$$

Standard errors allow arbitrary within-cluster correlation.

## Output

| Pin | Description |
|-----|-------------|
| **Config** | Cluster VCE config handle |

Wire **Config** → **OLS & WLS Configure** → **VCE**, then to **OLS** / **WLS** / Summary nodes.
