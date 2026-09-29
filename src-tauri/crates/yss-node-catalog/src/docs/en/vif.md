# Variance inflation factors

Connect a linear `model` to diagnose predictor multicollinearity. There are no parameters. OLS, WLS and GLS use the original design without weighting or whitening.

## Formula

$$
\mathrm{VIF}_j=\frac1{1-R_j^2},\qquad
\mathrm{Tolerance}_j=1-R_j^2.
$$

$R_j^2$ is the centered coefficient of determination from regressing predictor $j$ by OLS on the remaining design columns. Higher VIF indicates stronger linear overlap with other predictors. No p-value is provided.

## Outputs and usage

`result` and `report` are identical: `test=vif` with inner `result` entries `vif` and `tolerance` in original column order. The intercept retains its position with both fields Null. No mean VIF is output.

The sample must satisfy $n\geq k+2$, where $k$ is the column count. Near-perfect collinearity is represented by `1e99` and zero tolerance; singular auxiliary designs fail. Intercept-free VIF can be below one and does not support the usual intercept-based interpretation.

Method details: [VIF](https://www.statsmodels.org/dev/generated/statsmodels.stats.outliers_influence.variance_inflation_factor.html).
