# Regression Tobit

Connect aligned finite observations; missing values are rejected. Unless specified below, response is numeric and `predictors` accepts one or more numeric columns in port order, named x1,x2,… . Encode categorical predictors explicitly. Unpenalized models require a full-rank design and positive residual degrees.

## Method and options

Normal Tobit: `censoring=left` (default) or `both`. `lower=0`; `upper=1` only applies to both bounds and must exceed lower. Responses stay inside bounds; exact boundary values are censored. At least one uncensored observation is required. `constant=true`; 500 iterations, tolerance 1e-7. Likelihood uses censored normal tails and uncensored density. Final coefficient is positive `sigma`, with delta covariance and normal Wald inference. Fitted values are latent means, not expected censored responses. This handles censoring, not truncated samples.

$$
Y_i^*=x_i^T\beta+\varepsilon_i,\quad\varepsilon_i\sim N(0,\sigma^2),\quad Y_i=\max(L,Y_i^*)\ \text{or}\ \min(U,\max(L,Y_i^*)).
$$

Positive dispersion/scale rows retain delta standard errors and log-scale transformed 95% intervals. Their zero-boundary statistic/p-value is null.

## Output

The only output is structured `result`, viewed as values or a report in Inspect. A model retains coefficients, covariance, fitted/residual arrays, `statistics`, iteration facts and method-specific `details`. Defined coefficient inference includes standard errors, two-sided tests of H0: reported coefficient=0, and 95% intervals. Undefined/inapplicable inference is `null`; unavailable arrays are empty. Workflows retain models in `stages` and explicit selection facts. Nonconvergence/numerical breakdown fails execution.

[Method reference](https://www.statsmodels.org/stable/examples/notebooks/generated/generic_mle.html)
