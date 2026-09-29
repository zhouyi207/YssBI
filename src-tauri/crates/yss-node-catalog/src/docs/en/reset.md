# Ramsey RESET specification test

Connect an OLS or WLS `model` to test whether added nonlinear powers improve the original mean equation. Fitted observations and WLS precision weights are reused. GLS is unsupported.

## Methods and hypotheses

For an original design $X$ with $k$ columns, including any intercept, and $n$ observations, augment the model:

$$
y=X\beta+Z\gamma+\varepsilon.
$$

**Null $H_0$:** $\gamma=\mathbf0$; the selected powers add no explanatory contribution.

**Alternative $H_1$:** at least one added coefficient is nonzero, suggesting omitted nonlinear structure.

`rhs` defaults to `false`. Each setting tests the coefficients of its own augmentation:

| Setting     | Actual augmentation                                                                                                                      |
| ----------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `rhs=false` | Normalize fitted values as $f_i=(\hat y_i-\min\hat y)/(\max\hat y-\min\hat y)$ and add $f^2,f^3,f^4$; $q=3$                              |
| `rhs=true`  | Normalize every nonconstant, non-0/1 predictor similarly and add its second, third and fourth powers; $q=3m$ for $m$ eligible predictors |

RHS mode adds no cross-variable interactions. Approximately constant and 0/1 columns are excluded using a $10^{-10}$ tolerance. Powers are fixed at 2–4.

## Statistic

Let $\mathrm{RSS}_R$ and $\mathrm{RSS}_U$ be restricted and augmented residual sums of squares:

$$
F=\frac{(\mathrm{RSS}_R-\mathrm{RSS}_U)/q}
{\mathrm{RSS}_U/(n-k-q)},\qquad
F\overset{H_0}{\sim}F_{q,n-k-q},
\qquad p=1-F_{F_{q,n-k-q}}(F).
$$

OLS uses $\sum_i e_i^2$; WLS uses $\sum_iw_i e_i^2$ in both equations, with the original weights. Classical F calibration requires independent, homoskedastic normal errors on the appropriate scale and full column rank. An upstream robust covariance does not switch this node to a robust Wald RESET. See [statsmodels linear_reset](https://www.statsmodels.org/stable/generated/statsmodels.stats.diagnostic.linear_reset.html) for the general method; the table above specifies this node's powers and normalization.

## Outputs and interpretation

`result` and `report` are identical: `test=reset` with inner `result` fields `f_stat`, `df1=q`, `df2=n-k-q` and `p_value`.

For example, $p=0.01$ at a preselected 5% level rejects the joint zero restriction and motivates inspecting functional form or omitted variables. It does not identify the variable to add or a unique cause of misspecification. $p\geq\alpha$ only means insufficient evidence against the selected restrictions, not proof that the original model is correct.

The node requires $n>k+q$, full column rank after augmentation and at least one eligible predictor in RHS mode. Too few observations, collinear powers, constant fitted values or nonfinite computations can fail. If augmented RSS is at most $10^{-300}$, the current calculation returns $F=0$; such degenerate output does not have the usual F-test interpretation.
