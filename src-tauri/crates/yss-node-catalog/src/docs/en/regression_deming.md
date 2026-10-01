# Regression Deming

Connect aligned finite observations; missing values are rejected. Unless specified below, response is numeric and `predictors` accepts one or more numeric columns in port order, named x1,x2,… . Encode categorical predictors explicitly. Unpenalized models require a full-rank design and positive residual degrees.

## Method and options

Single predictor, at least four paired observations. Positive `variance_ratio=1` is Var(response error)/Var(predictor error). S denotes centered sums of products; require both variables varying and nonzero covariance. Accounts for measurement errors in both variables under a fixed ratio, with an intercept. Covariance uses delete-one jackknife and standard normal coefficient inference. No likelihood or AIC/BIC.

$$
\hat b=\frac{S_{yy}-\delta S_{xx}+\sqrt{(S_{yy}-\delta S_{xx})^2+4\delta S_{xy}^2}}{2S_{xy}},\quad\hat a=\bar y-\hat b\bar x.
$$

## Output

The only output is structured `result`, viewed as values or a report in Inspect. A model retains coefficients, covariance, fitted/residual arrays, `statistics`, iteration facts and method-specific `details`. Defined coefficient inference includes standard errors, two-sided tests of H0: reported coefficient=0, and 95% intervals. Undefined/inapplicable inference is `null`; unavailable arrays are empty. Workflows retain models in `stages` and explicit selection facts. Nonconvergence/numerical breakdown fails execution.

[Method reference](https://search.r-project.org/CRAN/refmans/deming/html/deming.html)
