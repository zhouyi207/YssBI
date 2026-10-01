# Fixed effects (FE)

Connect aligned finite numeric series: **response**, one or more **predictors**, **entity** IDs and **time**. Missing values are rejected and entity-time pairs must be unique. Rows are sorted by panel keys. Encode textual entity IDs before connecting them.

**Parameters:** constant defaults to true; effects defaults to entity, with time and two_way alternatives; covariance defaults to cluster, with nonrobust and HC0–HC3 alternatives.

The entity model is $y_{it}=\alpha_i+x_{it}'\beta+\varepsilon_{it}$, estimated by the within transformation. time absorbs time effects; two_way absorbs both dimensions. Absorbed or collinear regressors appear in omittedTerms. cluster uses entity clusters.

The only output, **model**, connects to Panel Summary and estimation-scale Panel Predict. It retains coefficients, covariance, effect statistics and the estimation response/design/fitted/residual sample with zero-based source-row groups. FE uses the within scale; these values are not original-scale predictions for new entities.

Coefficient tests compare $H_0:\beta_j=0$ with $H_1:\beta_j\ne0$, using estimate divided by standard error and the fitted model's t inference/degrees of freedom. Sufficient within variation and positive residual degrees of freedom are required. Robust covariance does not resolve endogeneity.
