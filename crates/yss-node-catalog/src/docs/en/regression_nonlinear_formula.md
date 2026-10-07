# Regression Nonlinear Formula

Connect aligned finite observations; missing values are rejected. Unless specified below, Y is numeric and inputs `X₁, X₂, …` accept one or more numeric columns in port order, named x1,x2,… . Encode categorical predictors explicitly. Unpenalized models require a full-rank design and positive residual degrees.

## Method and options

`formula` defaults to `b1 + b2*x1`; x1,x2,… follow predictor ports, b1,b2,… follow `initial_values` (default `[0,1]`). Supports `+ - * / ^`, parentheses, `exp ln sqrt abs sin cos min max`. Each nonempty `lower_bounds`/`upper_bounds` list gives one finite bound per parameter; require lower<upper and starts within bounds. Empty means unbounded on that side. Free parameters continue optimizing at active bounds. Default 500 iterations, tolerance 1e-7. Require n>p and full-rank J. Interior inference uses Student t(n−p); at active bounds covariance/inference is null. Nondifferentiability and local minima can prevent fitting.

$$
\hat\theta=\arg\min_\theta\sum_i[y_i-f(x_i,\theta)]^2,\quad\widehat{\operatorname{Cov}}(\hat\theta)=\frac{RSS}{n-p}(J^TJ)^{-1}.
$$

## Output

The only output is structured `result`, viewed as values or a report in Inspect. A model retains coefficients, covariance, fitted/residual arrays, `statistics`, iteration facts and method-specific `details`. Defined coefficient inference includes standard errors, two-sided tests of H0: reported coefficient=0, and 95% intervals. Undefined/inapplicable inference is `null`; unavailable arrays are empty. Workflows retain models in `stages` and explicit selection facts. Nonconvergence/numerical breakdown fails execution.

[Method reference](https://docs.scipy.org/doc/scipy/reference/generated/scipy.optimize.least_squares.html)
