# Regression Nonlinear

Connect aligned finite observations; missing values are rejected. Unless specified below, response is numeric and `predictors` accepts 1–16 numeric columns in port order, named x1,x2,… . Encode categorical predictors explicitly. Unpenalized models require a full-rank design and positive residual degrees.

## Method and options

Single `predictor`. Models: exponential b1·exp(b2·x) (default), logistic growth b1/[1+exp(−b2·(x−b3))], Michaelis–Menten b1·x/(b2+x), Gompertz b1·exp[−exp(−b2·(x−b3))]. `initial_values` follows b1,b2,…; empty uses automatic starts. Damped local least squares, maximum iterations 500, tolerance 1e-7. J is the fitted Jacobian, p the parameter count; require full-rank J and n>p. Inference uses local Student t approximation with n−p degrees. No global minimum guarantee.

$$
\hat\theta=\arg\min_\theta\sum_i[y_i-f(x_i,\theta)]^2,\quad\widehat{\operatorname{Cov}}(\hat\theta)=\frac{RSS}{n-p}(J^TJ)^{-1}.
$$

## Output

The only output is structured `result`, viewed as values or a report in Inspect. A model retains coefficients, covariance, fitted/residual arrays, `statistics`, iteration facts and method-specific `details`. Defined coefficient inference includes standard errors, two-sided tests of H0: reported coefficient=0, and 95% intervals. Undefined/inapplicable inference is `null`; unavailable arrays are empty. Workflows retain models in `stages` and explicit selection facts. Nonconvergence/numerical breakdown fails execution.

[Method reference](https://docs.scipy.org/doc/scipy/reference/generated/scipy.optimize.least_squares.html)
