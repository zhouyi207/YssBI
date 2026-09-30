# Transform RCS

Connect aligned finite observations; missing values are rejected. Unless specified below, response is numeric and `predictors` accepts 1–16 numeric columns in port order, named x1,x2,… . Encode categorical predictors explicitly. Unpenalized models require a full-rank design and positive residual degrees.

## Method and options

Response plus single predictor. Build bases x,d1,…,d(K−2), then OLS with intercept and linear tails. `(u)+` means max(u,0). `knot_mode=auto`, `knot_count=4` (3–8): three knots use quantiles 0.1/0.5/0.9; otherwise equally spaced from 0.05 to 0.95 with linear interpolation. Manual uses 3–8 finite increasing `knots`. Reject repeated quantiles/rank-deficient bases. Details retain knots, basis names and full columns in input order; x1 is linear, subsequent terms nonlinear. This is spline regression with its numerical design, not a general outcome-family transform. Coefficient t tests use n−K degrees; no joint nonlinearity/knot-selection test.

$$
d_j(x)=\frac{(x-t_j)_+^3-\frac{t_K-t_j}{t_K-t_{K-1}}(x-t_{K-1})_+^3+\frac{t_{K-1}-t_j}{t_K-t_{K-1}}(x-t_K)_+^3}{(t_K-t_1)^2},\quad j=1,\ldots,K-2.
$$

## Output

The only output is structured `result`, viewed as values or a report in Inspect. A model retains coefficients, covariance, fitted/residual arrays, `statistics`, iteration facts and method-specific `details`. Defined coefficient inference includes standard errors, two-sided tests of H0: reported coefficient=0, and 95% intervals. Undefined/inapplicable inference is `null`; unavailable arrays are empty. Workflows retain models in `stages` and explicit selection facts. Nonconvergence/numerical breakdown fails execution.

[Method reference](https://search.r-project.org/CRAN/refmans/Hmisc/html/rcspline.eval.html)
