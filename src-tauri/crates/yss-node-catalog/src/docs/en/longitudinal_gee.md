# GEE

One grouping column identifies independent subjects/clusters. Select Gaussian identity (default), Bernoulli logit, or Poisson log. Binary responses must be 0/1; count responses must be nonnegative integers. Working correlation defaults to exchangeable; independence is also available.

Connect aligned Y, X₁, X₂, … and groups. Numeric predictors are optional; classify/encode categorical predictors explicitly. Group labels accept numbers, text, identifiers or categorical/binary/ordinal series and retain their original values. The fixed design must have full rank and more rows than coefficients. Missing/nonfinite values are rejected; no rows are silently dropped. At least two observed groups are required. Singleton groups are allowed with independence correlation; exchangeable correlation needs within-group pairs. All inputs must have equal lengths and pair by current position, including mixed database and in-memory inputs.

Parameters: constant=true (fixed intercept), max_iterations=500 (positive integer), tolerance=1e-7 (1e-12–0.01). Failure to converge fails execution.

## Method

Solve the marginal mean estimating equation, with working covariance $V_g=\phi A_g^{1/2}R_gA_g^{1/2}$ for cluster $g$, derivatives $D_g=\partial\mu_g/\partial\beta$, and variance function matrix $A_g$:

$$
\sum_g D_g^T V_g^{-1}(y_g-\mu_g)=0,\qquad \widehat{\operatorname{Var}}(\hat\beta)=B^{-1}\left(\sum_g U_gU_g^T\right)B^{-1}.
$$

Here $U_g=D_g^TV_g^{-1}(y_g-\mu_g)$ and $B=\sum_gD_g^TV_g^{-1}D_g$. Inference uses the uncorrected cluster sandwich and a standard normal reference, relying on sufficiently many independent clusters. Gaussian scale uses Pearson residual squares divided by $n-p$; binary/Poisson scale is 1. Exchangeable correlation is estimated by residual moments with parameter-count correction; an invalid correlation matrix fails rather than being silently repaired.

## Output

The single structured result opens in Inspect. It contains coefficients, coefficient_covariance, observations, group_counts, iterations, scale, applicable likelihood/AIC, variance_components and random_effects. Fixed coefficient tests use $H_0:\beta_j=0$ versus $H_1:\beta_j\ne0$, $z=\hat\beta_j/SE_j$, a standard normal reference and 95% intervals. Undefined statistics are null.

group_labels preserves original labels in first-appearance order. Fixed coefficients follow predictor-port order: intercept (when constant=true), x1, x2, etc. Row arrays preserve input order; fixed_fitted and fitted_values both contain population-mean predictions, and residuals=response−fitted_values. There are no random effects or variance components. Likelihood/AIC are null; correlation and working_correlation identify the estimated working dependence.

[Method reference](https://www.statsmodels.org/stable/gee.html)
