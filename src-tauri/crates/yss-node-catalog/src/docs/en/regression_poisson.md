# Regression Poisson

Connect aligned finite observations; missing values are rejected. Unless specified below, response is numeric and `predictors` accepts 1–16 numeric columns in port order, named x1,x2,… . Encode categorical predictors explicitly. Unpenalized models require a full-rank design and positive residual degrees.

## Method and options

Nonnegative integer counts with positive mean. Fixed log link and dispersion 1. `constant=true`; 500 iterations, tolerance 1e-7. Fitted values are conditional count means. Inference uses model Fisher information and standard normal reference. No exposure/offset or automatic overdispersion correction.

$$
Y_i\sim\operatorname{Poisson}(\mu_i),\quad\log\mu_i=x_i^T\beta,\quad\operatorname{Var}(Y_i\mid x_i)=\mu_i.
$$

## Output

The only output is structured `result`, viewed as values or a report in Inspect. A model retains coefficients, covariance, fitted/residual arrays, `statistics`, iteration facts and method-specific `details`. Defined coefficient inference includes standard errors, two-sided tests of H0: reported coefficient=0, and 95% intervals. Undefined/inapplicable inference is `null`; unavailable arrays are empty. Workflows retain models in `stages` and explicit selection facts. Nonconvergence/numerical breakdown fails execution.

[Method reference](https://www.statsmodels.org/stable/generated/statsmodels.genmod.families.family.Poisson.html)
