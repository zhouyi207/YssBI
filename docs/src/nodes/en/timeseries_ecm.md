# Error-correction model (ECM)

Estimate a two-step single-equation ECM for a numeric `series` and one or more aligned `X₁, X₂, …`. Rows must be equally spaced and ordered, with finite values and no missing observations. No fixed row or predictor ceiling is imposed.

`ts_lags=0` is the nonnegative number of lagged differences. `constant=true` adds an intercept to both regressions. The design must have full column rank and positive residual degrees of freedom after lagging.

$$
y_t=c+\beta' x_t+u_t,\qquad
\Delta y_t=a+\lambda\widehat u_{t-1}+\sum_{j=1}^{L}\psi_j\Delta y_{t-j}+\sum_{j=0}^{L}\delta_j'\Delta x_{t-j}+e_t.
$$

$L$ is `ts_lags`; both stages use OLS. `long_run` contains the levels equation; `long_run.residuals` is its full equilibrium-error sequence, aligned with the input rows. `short_run` contains the differenced equation, named coefficients, fitted values, residuals and conditional OLS inference. `first_short_run_row=L+1` is the zero-based source position of its first observation.

A negative adjustment coefficient $\lambda$ is consistent with correction towards equilibrium, subject to the full lag dynamics. Establish integration order, cointegration and a suitable weak-exogeneity specification before interpreting the model. Ordinary stationary-regression inference is not valid for cointegrating levels OLS, so long-run standard errors, t statistics, p-values, confidence intervals and covariance are unavailable. Short-run coefficient tests condition on the estimated equilibrium error and valid model specification: $H_0:b_j=0$ versus $H_1:b_j\ne0$, using $t=b_j/SE(b_j)$ and residual degrees of freedom $n_{\mathrm{short}}-k$. These are conventional homoskedastic approximations, not a cointegration test or generated-regressor correction.
