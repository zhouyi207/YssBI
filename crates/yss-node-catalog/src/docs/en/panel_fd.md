# First differences

Connect aligned numeric response, one or more predictors, entity IDs and integer period indices time. Series must be finite, complete and equal in length, with unique entity-time keys. The node sorts rows and retains only adjacent observations within an entity whose time difference is exactly 1; gaps are not treated as one-period differences.

The model is $\Delta y_{it}=\Delta x_{it}'\beta+\Delta\varepsilon_{it}$, eliminating time-invariant entity intercepts. This entry fixes entity effects and removes the intercept; it does not fit an additional intercept in the difference equation.

**Parameter:** covariance defaults to nonrobust, with HC0–HC3 or entity-clustered cluster alternatives.

The only output, **model**, connects to Panel Summary. Its estimation sample is on the first-difference scale; each sourceRows item identifies the two original zero-based rows used for that difference. Coefficient tests compare $H_0:\beta_j=0$ with $H_1:\beta_j\ne0$, using estimate/standard error and the model's t degrees of freedom. Enough retained differences and an identifiable design are required. The method does not automatically resolve endogeneity from lagged responses.
