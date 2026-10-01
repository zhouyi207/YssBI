# Regression Negative Binomial

Connect aligned finite observations; missing values are rejected. Unless specified below, response is numeric and `predictors` accepts one or more numeric columns in port order, named x1,x2,… . Encode categorical predictors explicitly. Unpenalized models require a full-rank design and positive residual degrees.

## Method and options

NB2 maximum likelihood jointly estimates count coefficients and dispersion. Nonnegative integer response with positive mean. `constant=true`; 500 iterations, tolerance 1e-7. Last coefficient `alpha` uses original scale with delta covariance. Wald inference uses complete observed information and standard normal reference. No offset/exposure. Numerical dispersion range is exp(−12) to exp(12); a nonidentifiable boundary is not a successful Poisson-limit fit.

$$
\log\mu_i=x_i^T\beta,\quad\operatorname{Var}(Y_i\mid x_i)=\mu_i+\alpha\mu_i^2,\quad\alpha>0.
$$

Positive dispersion/scale rows retain delta standard errors and log-scale transformed 95% intervals. Their zero-boundary statistic/p-value is null.

## Output

The only output is structured `result`, viewed as values or a report in Inspect. A model retains coefficients, covariance, fitted/residual arrays, `statistics`, iteration facts and method-specific `details`. Defined coefficient inference includes standard errors, two-sided tests of H0: reported coefficient=0, and 95% intervals. Undefined/inapplicable inference is `null`; unavailable arrays are empty. Workflows retain models in `stages` and explicit selection facts. Nonconvergence/numerical breakdown fails execution.

[Method reference](https://www.statsmodels.org/stable/generated/statsmodels.discrete.discrete_model.NegativeBinomial.html)
