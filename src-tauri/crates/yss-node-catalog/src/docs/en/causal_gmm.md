# Linear IV GMM

Fits a linear equation using one-step or two-step generalized method of moments.

## Inputs and parameters

Connect aligned numeric `Y`, repeated `X₁, X₂, …` (at least one), and repeated `instruments` (at least one).
Supply the complete instrument set, including any exogenous predictors that instrument themselves. The intercept is added to both designs when `constant=true` (default); do not supply duplicate constant columns.

`gmm_steps=two_step` (default) uses a heteroskedastic moment weight; `one_step` uses the 2SLS weight.
Both designs must have full column rank, with more observations than instruments and at least as many instruments as coefficients. Missing/nonfinite data are rejected; no rows are removed.

## Estimator and inference

With instrument vector $z_i$, regressor vector $x_i$, residual $u_i=y_i-x_i'\beta$, and $\bar g(\beta)=n^{-1}\sum_i z_i u_i$,

$$
\hat\beta=\arg\min_\beta \bar g(\beta)'W\bar g(\beta).
$$

The initial weight is $(Z'Z/n)^{-1}$. Two-step GMM replaces it with the inverse uncentered covariance of first-step moment contributions. Both choices return an HC0 sandwich computed from final residuals; there is no small-sample correction.

Coefficient inference tests $H_0:\beta_j=0$ against $H_1:\beta_j\ne0$ using $z=\hat\beta_j/SE_j$ and the asymptotic standard normal distribution, with two-sided p-values and 95% intervals.

Only overidentified two-step fits return Hansen's $J=n\bar g'W\bar g$. Under $H_0:E[z_i u_i]=0$, $J$ has asymptotic $\chi^2_{q-p}$ reference distribution, where $q$ counts instruments and $p$ counts coefficients, including the intercept. The alternative is violation of at least one restriction. A small p-value rejects the joint restrictions; a large p-value does not establish instrument strength.

## Result

`result` contains `coefficients`, `covariance`, `fitted`, `residuals`, original-unit `moments`, counts, `steps`, and nullable `hansen_j`.
Arrays follow input row/column order. Instrument validity, identification and independent observations are required. This is linear IV GMM; nonlinear moment expressions, clustered inference and dynamic-panel GMM are separate methods.
