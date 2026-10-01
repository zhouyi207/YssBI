# Regression Complementary Log-log

Connect aligned finite observations; missing values are rejected. Unless specified below, response is numeric and `predictors` accepts one or more numeric columns in port order, named x1,x2,… . Encode categorical predictors explicitly. Unpenalized models require a full-rank design and positive residual degrees.

## Method and options

Binary 0/1 or false/true response with both outcomes. `constant=true`; 500 iterations, tolerance 1e-7. Fitted values are probabilities. Coefficient inference uses model Fisher information and standard normal reference. This asymmetric link does not automatically define a survival model; no exposure/time offset.

$$
\log[-\log(1-\pi_i)]=x_i^T\beta,\quad\pi_i=1-\exp[-\exp(x_i^T\beta)].
$$

## Output

The only output is structured `result`, viewed as values or a report in Inspect. A model retains coefficients, covariance, fitted/residual arrays, `statistics`, iteration facts and method-specific `details`. Defined coefficient inference includes standard errors, two-sided tests of H0: reported coefficient=0, and 95% intervals. Undefined/inapplicable inference is `null`; unavailable arrays are empty. Workflows retain models in `stages` and explicit selection facts. Nonconvergence/numerical breakdown fails execution.

[Method reference](https://www.statsmodels.org/stable/generated/statsmodels.genmod.families.links.CLogLog.html)
