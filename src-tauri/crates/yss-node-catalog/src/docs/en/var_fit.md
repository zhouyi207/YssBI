# VAR Fit

Connect at least two aligned finite series in time order. The input order determines equation, shock and response ordering. Parameter lags selects consecutive lags 1 through p; the fitted model includes an intercept.

Output model retains coefficients, covariance, equation statistics, design and residuals. VAR Summary computes selected lag-exclusion, residual LM and stability analyses. Granger, IRF and FEVD have separate nodes; IRF/FEVD provide a configurable horizon. These analyses reuse the fitted model and are not computed during Fit.

No single-series fitted/residual output is provided for this multivariate model.
