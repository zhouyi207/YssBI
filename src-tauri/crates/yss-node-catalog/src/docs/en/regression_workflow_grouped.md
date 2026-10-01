# Workflow Regression Grouped

Connect aligned finite observations; missing values are rejected. Unless specified below, response is numeric and `predictors` accepts one or more numeric columns in port order, named x1,x2,… . Encode categorical predictors explicitly. Unpenalized models require a full-rank design and positive residual degrees.

## Method and options

Add aligned `groups`. Fit independent OLS in each group using all predictors; `constant=true`. Each group requires full rank and more rows than coefficients. `stages.label` restores original labels; `observation_indices` gives original one-based row positions, which also order fitted/residual arrays. Coefficient t tests use group residual degrees. Independent fits do not test differences between group coefficients and imply no pooling/multilevel structure.

$$
\hat\beta=\arg\min_\beta\sum_i(y_i-x_i^T\beta)^2.
$$

## Output

The only output is structured `result`, viewed as values or a report in Inspect. A model retains coefficients, covariance, fitted/residual arrays, `statistics`, iteration facts and method-specific `details`. Defined coefficient inference includes standard errors, two-sided tests of H0: reported coefficient=0, and 95% intervals. Undefined/inapplicable inference is `null`; unavailable arrays are empty. Workflows retain models in `stages` and explicit selection facts. Nonconvergence/numerical breakdown fails execution.

[Method reference](https://www.statsmodels.org/stable/generated/statsmodels.regression.linear_model.OLS.html)
