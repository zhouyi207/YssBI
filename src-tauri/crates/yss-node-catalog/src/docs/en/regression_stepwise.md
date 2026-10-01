# Regression Stepwise

Connect aligned finite observations; missing values are rejected. Unless specified below, response is numeric and `predictors` accepts one or more numeric columns in port order, named x1,x2,… . Encode categorical predictors explicitly. Unpenalized models require a full-rank design and positive residual degrees.

## Method and options

`direction=both` (default), forward/backward; `criterion=aic` (default) or bic; `constant=true`. OLS Gaussian likelihood counts k coefficients plus one variance parameter. Forward/both begins at intercept only; without intercept, best single predictor. Backward begins with all predictors. Each step chooses the best strict improvement, excluding rank-deficient candidates. `selection_history` retains actions, original one-based indices and criterion values; `stages` retains the final model. Final t tests are conditional on selection, without search adjustment. No p-value entry/removal criterion or automatic holdout validation.

$$
AIC=-2\ell+2k,\quad BIC=-2\ell+k\log n.
$$

## Output

The only output is structured `result`, viewed as values or a report in Inspect. A model retains coefficients, covariance, fitted/residual arrays, `statistics`, iteration facts and method-specific `details`. Defined coefficient inference includes standard errors, two-sided tests of H0: reported coefficient=0, and 95% intervals. Undefined/inapplicable inference is `null`; unavailable arrays are empty. Workflows retain models in `stages` and explicit selection facts. Nonconvergence/numerical breakdown fails execution.

[Method reference](https://www.statsmodels.org/stable/generated/statsmodels.regression.linear_model.OLS.html)
