# Random-slope model

Connect one grouping column and one or more numeric random_predictors. The model includes a random intercept plus independent random slopes; normally include slope variables in predictors too. Intercept/slope correlations are fixed to zero. No automatic centering is applied.

Connect aligned response, predictors and groups. Numeric predictors are optional; classify/encode categorical predictors explicitly. Group labels accept numbers, text, identifiers or categorical/binary/ordinal series and retain their original values. The fixed design must have full rank and more rows than coefficients. Missing/nonfinite values are rejected; no rows are silently dropped. Each grouping factor needs at least two observed groups and identifiable covariance components. All inputs must share a proven row domain or be equally long materialized columns.

Parameters: constant=true (fixed intercept), max_iterations=500 (positive integer), tolerance=1e-7 (1e-12–0.01). Failure to converge fails execution.

## Method

Fit Gaussian mixed effects by REML (default) or ML via mixed_estimation:

$$
y=X\beta+Zb+\varepsilon,\quad b\sim N(0,G),\quad \varepsilon\sim N(0,\sigma^2I),\quad V=ZGZ^T+\sigma^2I.
$$

Random terms have independent variance components. Fixed coefficients use GLS conditional on estimated variances; their covariance is $(X^TV^{-1}X)^{-1}$. Random effects are BLUPs, $GZ^TV^{-1}(y-X\hat\beta)$. Use ML to compare models with different fixed-effect designs; REML log likelihood is restricted likelihood and AIC is omitted. Near-zero variance components are flagged by boundary. Coefficient inference uses approximate normal Wald statistics, without Satterthwaite/Kenward–Roger corrections; group effects have no individual significance tests.

## Output

The single structured result opens in Inspect. It contains coefficients, coefficient_covariance, observations, group_counts, iterations, scale, applicable likelihood/AIC, variance_components and random_effects. Fixed coefficient tests use $H_0:\beta_j=0$ versus $H_1:\beta_j\ne0$, $z=\hat\beta_j/SE_j$, a standard normal reference and 95% intervals. Undefined statistics are null.

group_labels preserves original labels in first-appearance order. grouping and level in random_effects use one-based positions; term=0 is a random intercept and term=1,2,... follows random_predictors port order. Fixed coefficients are intercept (when constant=true), x1, x2, etc. in predictor-port order. Row arrays preserve input order: fixed_fitted sets random effects to zero (for a GLMM this is not an integrated population mean); fitted_values includes estimated group effects, and residuals=response−fitted_values.

[Method reference](https://lme4.github.io/lme4/reference/lmer.html)
