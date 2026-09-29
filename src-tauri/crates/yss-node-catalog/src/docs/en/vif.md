# Variance inflation factors

Connect a fitted linear `model` to measure linear overlap between each predictor and the others. There are no parameters. OLS, WLS and GLS all use the **original design**, without WLS weighting or GLS whitening.

## Calculation

For each non-intercept design column $x_j$, regress it by OLS on the remaining columns $X_{-j}$, obtaining $\hat x_{ij}$:

$$
R_j^2=1-\frac{\sum_{i=1}^n(x_{ij}-\hat x_{ij})^2}
{\sum_{i=1}^n(x_{ij}-\bar x_j)^2},\qquad
\mathrm{VIF}_j=\frac1{1-R_j^2},\qquad
\mathrm{Tolerance}_j=1-R_j^2.
$$

$n$ is the fitted sample size and $\bar x_j$ the column mean. In classical homoskedastic OLS with an intercept and full rank,

$$
\operatorname{Var}(\hat\beta_j\mid X)
=\frac{\sigma^2}{\sum_i(x_{ij}-\bar x_j)^2}\,\mathrm{VIF}_j.
$$

VIF measures the coefficient variance inflation due to linear collinearity; standard errors are inflated by $\sqrt{\mathrm{VIF}_j}$. This is a design diagnostic with **no null/alternative hypotheses, test reference distribution or p-value**. See [statsmodels VIF](https://www.statsmodels.org/dev/generated/statsmodels.stats.outliers_influence.variance_inflation_factor.html).

## Outputs and example

`result` and `report` are identical. For `test=vif`, inner `result` is an array in original design-column order, with `vif` and `tolerance` per entry. A configured intercept retains its position with both fields Null. No mean VIF is output.

For $R_j^2=0.8$, VIF is 5 and tolerance is 0.2; the classical OLS standard error is inflated by about $\sqrt5$ relative to an otherwise comparable uncorrelated design. Values such as 5 or 10 are heuristic attention thresholds, not significance cutoffs or automatic variable-removal rules.

## Conditions and limits

The sample must satisfy $n\geq k+2$, where $k$ is the design column count. Singular auxiliary designs and nonfinite calculations fail. If $R_j^2\geq1-10^{-10}$, the node reports `1e99` and zero tolerance. A target column with numerically negligible variation is assigned $R_j^2=0$; this does not establish a sound design.

An intercept-free model receives no added auxiliary intercept, although the denominator remains centered. Then $R_j^2$ can be negative and VIF below one; usual intercept-based interpretations do not apply. For WLS/GLS the result describes original predictor structure, not the complete variance decomposition of the weighted/GLS estimator.
