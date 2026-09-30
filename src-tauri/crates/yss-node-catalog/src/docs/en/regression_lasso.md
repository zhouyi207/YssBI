# Regression Lasso

Connect aligned finite observations; missing values are rejected. Unless specified below, response is numeric and `predictors` accepts 1–16 numeric columns in port order, named x1,x2,… . Encode categorical predictors explicitly. Unpenalized models require a full-rank design and positive residual degrees.

## Method and options

`lambda` is nonnegative, default 1. `constant=true`, `standardize=true`: center predictors when an intercept is included and scale by population standard deviation. Return original-unit coefficients; the intercept is unpenalized. Regularization permits collinear/wide designs when the penalty identifies the solution. Ridge uses a direct solve; Lasso coordinate descent allows 5000 iterations, tolerance 1e-7. `details` retains objective and effective degrees (Ridge trace or Lasso nonzero count). No automatic tuning, cross-validation, coefficient p-values or intervals.

$$
\hat\beta=\arg\min_\beta\left\{\frac{RSS}{2n}+\lambda\sum_{j\ne0}|\beta_j|\right\}.
$$

## Output

The only output is structured `result`, viewed as values or a report in Inspect. A model retains coefficients, covariance, fitted/residual arrays, `statistics`, iteration facts and method-specific `details`. Defined coefficient inference includes standard errors, two-sided tests of H0: reported coefficient=0, and 95% intervals. Undefined/inapplicable inference is `null`; unavailable arrays are empty. Workflows retain models in `stages` and explicit selection facts. Nonconvergence/numerical breakdown fails execution.

[Method reference](https://scikit-learn.org/stable/modules/generated/sklearn.linear_model.Lasso.html)
