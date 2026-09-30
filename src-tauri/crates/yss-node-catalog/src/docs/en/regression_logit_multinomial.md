# Regression Logit Multinomial

Connect aligned finite observations; missing values are rejected. Unless specified below, response is numeric and `predictors` accepts 1–16 numeric columns in port order, named x1,x2,… . Encode categorical predictors explicitly. Unpenalized models require a full-rank design and positive residual degrees.

## Method and options

Category response includes text/identifiers/numeric codes. Observed categories follow first appearance; `categories[0]` is the reference. At most 8 categories and 64 parameters, n greater than parameter count. `constant=true`; 500 iterations, tolerance 1e-7. Terms `class[c].xj` index nonreference category c and predictor j. `probabilities` rows follow observations, columns follow `categories`; `fitted_categories` restores original labels. Numeric fitted/residual arrays are empty and RSS/RMSE null. Wald inference uses observed information and standard normal reference; separation/singular information fails.

$$
P(Y=0\mid x)=\frac1{1+\sum_{c=1}^{K-1}e^{x^T\beta_c}},\quad P(Y=c\mid x)=\frac{e^{x^T\beta_c}}{1+\sum_{h=1}^{K-1}e^{x^T\beta_h}}.
$$

## Output

The only output is structured `result`, viewed as values or a report in Inspect. A model retains coefficients, covariance, fitted/residual arrays, `statistics`, iteration facts and method-specific `details`. Defined coefficient inference includes standard errors, two-sided tests of H0: reported coefficient=0, and 95% intervals. Undefined/inapplicable inference is `null`; unavailable arrays are empty. Workflows retain models in `stages` and explicit selection facts. Nonconvergence/numerical breakdown fails execution.

[Method reference](https://www.statsmodels.org/stable/generated/statsmodels.discrete.discrete_model.MNLogit.html)
