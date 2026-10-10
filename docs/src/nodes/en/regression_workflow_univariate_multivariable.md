# Workflow Regression Univariate Multivariable

Connect aligned finite observations; missing values are rejected. Unless specified below, Y is numeric and inputs `X₁, X₂, …` accept one or more numeric columns in port order, named x1,x2,… . Encode categorical predictors explicitly. Unpenalized models require a full-rank design and positive residual degrees.

## Method and options

Fit one OLS per predictor then one with all connected predictors, without automatic significance screening. `constant=true`. Stage labels 1…predictor count are individual models; final label is predictor count+1. Coefficient terms and `predictors` preserve original one-based positions. Coefficient t tests use each model residual degrees and have no multiple-testing adjustment. All fitted models must be identifiable.

$$
\hat\beta=\arg\min_\beta\sum_i(y_i-x_i^T\beta)^2.
$$

## Output

The only output is structured `result`, viewed as values or a report in Inspect. A model retains coefficients, covariance, fitted/residual arrays, `statistics`, iteration facts and method-specific `details`. Defined coefficient inference includes standard errors, two-sided tests of H0: reported coefficient=0, and 95% intervals. Undefined/inapplicable inference is `null`; unavailable arrays are empty. Workflows retain models in `stages` and explicit selection facts. Nonconvergence/numerical breakdown fails execution.

[Method reference](https://www.statsmodels.org/stable/generated/statsmodels.regression.linear_model.OLS.html)
