# Workflow Regression Baseline

Connect aligned finite observations; missing values are rejected. Unless specified below, response is numeric and `predictors` accepts 1–16 numeric columns in port order, named x1,x2,… . Encode categorical predictors explicitly. Unpenalized models require a full-rank design and positive residual degrees.

## Method and options

Fit an explicit OLS baseline with all connected predictors; `constant=true`. Baseline is a workflow role, and this node specifies the actual estimator. Return a single model result. Coefficient tests use Student t(n−p); AIC/BIC count coefficients and one Gaussian variance parameter. No unspecified alternative, model selection or causal identification.

$$
\hat\beta=\arg\min_\beta\sum_i(y_i-x_i^T\beta)^2.
$$

## Output

The only output is structured `result`, viewed as values or a report in Inspect. A model retains coefficients, covariance, fitted/residual arrays, `statistics`, iteration facts and method-specific `details`. Defined coefficient inference includes standard errors, two-sided tests of H0: reported coefficient=0, and 95% intervals. Undefined/inapplicable inference is `null`; unavailable arrays are empty. Workflows retain models in `stages` and explicit selection facts. Nonconvergence/numerical breakdown fails execution.

[Method reference](https://www.statsmodels.org/stable/generated/statsmodels.regression.linear_model.OLS.html)
