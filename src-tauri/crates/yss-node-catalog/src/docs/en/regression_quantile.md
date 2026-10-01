# Regression Quantile

Connect aligned finite observations; missing values are rejected. Unless specified below, response is numeric and `predictors` accepts one or more numeric columns in port order, named x1,x2,… . Encode categorical predictors explicitly. Unpenalized models require a full-rank design and positive residual degrees.

## Method and options

`quantile=0.5` is the conditional median, strictly between 0 and 1; `constant=true`. IRLS uses a small residual floor, 5000 iterations, tolerance 1e-7. `details.check_loss` retains original check loss. IID covariance is τ(1−τ)(XᵀX)⁻¹/f̂(0)² using Gaussian residual-density kernel and Silverman bandwidth; tests/95% intervals use standard normal approximation. Density-degenerate inference is null. No clustered/heteroskedastic quantile covariance; RSS metrics do not replace check loss.

$$
\hat\beta_\tau=\arg\min_\beta\sum_i\rho_\tau(y_i-x_i^T\beta),\quad\rho_\tau(u)=u[\tau-\mathbf1(u<0)].
$$

## Output

The only output is structured `result`, viewed as values or a report in Inspect. A model retains coefficients, covariance, fitted/residual arrays, `statistics`, iteration facts and method-specific `details`. Defined coefficient inference includes standard errors, two-sided tests of H0: reported coefficient=0, and 95% intervals. Undefined/inapplicable inference is `null`; unavailable arrays are empty. Workflows retain models in `stages` and explicit selection facts. Nonconvergence/numerical breakdown fails execution.

[Method reference](https://www.statsmodels.org/stable/generated/statsmodels.regression.quantile_regression.QuantReg.fit.html)
