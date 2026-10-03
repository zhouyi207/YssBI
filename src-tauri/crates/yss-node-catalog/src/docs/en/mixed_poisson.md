# Poisson mixed model

Connect one grouping column and a nonnegative integer count response. Uses a log link with a Gaussian random intercept; conditional variance equals the mean. No exposure or offset input is provided.

Connect aligned Y, X₁, X₂, … and groups. Numeric predictors are optional; classify/encode categorical predictors explicitly. Group labels accept numbers, text, identifiers or categorical/binary/ordinal series and retain their original values. The fixed design must have full rank and more rows than coefficients. Missing/nonfinite values are rejected; no rows are silently dropped. Each grouping factor needs at least two observed groups and identifiable covariance components. All inputs must have equal lengths and pair by current position, including mixed database and in-memory inputs.

Parameters: constant=true (fixed intercept), max_iterations=500 (positive integer), tolerance=1e-7 (1e-12–0.01). Failure to converge fails execution.

## Method

Estimate fixed coefficients and random-intercept variance by maximum likelihood with a one-dimensional Laplace approximation per group:

$$
g(\mu_{gi})=x_{gi}^T\beta+b_g,\quad b_g\sim N(0,\tau^2),\qquad
\ell\approx\sum_g\left[\log p(y_g\mid\hat u_g)-\frac{\hat u_g^2}{2}-\frac12\log H_g\right].
$$

Here $b_g=\tau u_g$, $\hat u_g$ is the conditional mode and $H_g$ is the negative conditional log-posterior curvature in standard-normal coordinates. This approximation is not adaptive quadrature or PQL. Wald standard errors use the observed information of the approximate likelihood, including nuisance-parameter uncertainty. Random effects are conditional modes. Near-zero variance is flagged by boundary; small groups or rare events can reduce approximation accuracy.

## Output

The single structured result opens in Inspect. It contains coefficients, coefficient_covariance, observations, group_counts, iterations, scale, applicable likelihood/AIC, variance_components and random_effects. Fixed coefficient tests use $H_0:\beta_j=0$ versus $H_1:\beta_j\ne0$, $z=\hat\beta_j/SE_j$, a standard normal reference and 95% intervals. Undefined statistics are null.

group_labels preserves original labels in first-appearance order. grouping and level in random_effects use one-based positions; term=0 identifies the random intercept. Fixed coefficients are intercept (when constant=true), x1, x2, etc. in predictor-port order. Row arrays preserve input order: fixed_fitted sets random effects to zero (for a GLMM this is not an integrated population mean); fitted_values includes estimated group effects, and residuals=response−fitted_values.

[Method reference](https://lme4.github.io/lme4/reference/glmer.html)
