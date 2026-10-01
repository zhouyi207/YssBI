# Regression Curve

Connect aligned finite observations; missing values are rejected. Unless specified below, response is numeric and `predictors` accepts one or more numeric columns in port order, named x1,x2,… . Encode categorical predictors explicitly. Unpenalized models require a full-rank design and positive residual degrees.

## Method and options

Connect a single `predictor`. Choose polynomial (default `degree=2`, 1–8), logarithmic, inverse, exponential or power. All include an intercept/amplitude. Logarithmic/power require x>0, inverse x≠0, exponential/power y>0. Exponential/power fit log(response); `amplitude` is the exponentiated intercept with delta covariance, fitted curves are conditional medians under lognormal errors, and AIC/BIC include the response Jacobian. Other families fit original response. Coefficient inference uses residual Student t degrees.

$$
y=a+\sum_{j=1}^d b_jx^j,\quad y=a+b\ln x,\quad y=a+b/x,\quad y=ae^{bx},\quad y=ax^b.
$$

Positive amplitude intervals are transformed from the log-intercept scale; its zero-boundary statistic/p-value is null.

## Output

The only output is structured `result`, viewed as values or a report in Inspect. A model retains coefficients, covariance, fitted/residual arrays, `statistics`, iteration facts and method-specific `details`. Defined coefficient inference includes standard errors, two-sided tests of H0: reported coefficient=0, and 95% intervals. Undefined/inapplicable inference is `null`; unavailable arrays are empty. Workflows retain models in `stages` and explicit selection facts. Nonconvergence/numerical breakdown fails execution.

[Method reference](https://www.statsmodels.org/stable/generated/statsmodels.regression.linear_model.OLS.html)
