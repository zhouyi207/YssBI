# Seemingly unrelated regression (SUR)

Fits multiple linear equations on the same observations, allowing their errors to be correlated.

## Inputs and parameters

Connect at least two aligned numeric `responses` and optional repeated numeric `predictors`. Every equation uses the same rows; missing/nonfinite values are rejected.

`constant=true` includes an intercept in every equation. `equation_predictors` is empty by default, meaning all predictors in every equation. To select different designs, enter one semicolon-separated list per response, using comma-separated one-based predictor positions. For example, `1,2;1,3` assigns predictors 1 and 2 to response 1, and 1 and 3 to response 2. `-` selects no predictors for an intercept-only equation.
Lists must match the response count, contain valid indices without duplicates, and leave each equation identified with positive residual degrees of freedom. Without an intercept, each equation needs at least one predictor. Input and equation counts have no fixed caps; the execution budget admits the complete system.

## Estimator and inference

Initial equation OLS residuals estimate $\hat\Sigma_{ab}=n^{-1}\sum_i\hat u_{ia}\hat u_{ib}$. A single feasible GLS step then solves

$$
\hat\beta=(X'\hat\Omega^{-1}X)^{-1}X'\hat\Omega^{-1}y,
\qquad \hat\Omega=\hat\Sigma\otimes I_n,
$$

using equation-major stacked responses and a block-diagonal design. $\hat\Sigma$ must be positive definite. This is two-stage SUR, not iterated SUR.

Covariance is $(X'\hat\Omega^{-1}X)^{-1}$, retaining cross-equation blocks without residual degrees-of-freedom scaling. Under independent observations and constant within-observation cross-equation covariance, coefficient tests use $H_0:\beta_j=0$ versus $H_1:\beta_j\ne0$, normal Wald statistics, two-sided p-values and 95% intervals. No heteroskedastic or clustered sandwich is provided.

## Result

`equations` retains each predictor-index list, coefficients, fitted values and residuals. `error_covariance` is the initial OLS error covariance used for FGLS; `coefficient_covariance` is ordered by equation and then coefficient, including each intercept.
Identical predictor designs yield the equation-by-equation OLS point estimates; distinct designs permit efficiency gains.
