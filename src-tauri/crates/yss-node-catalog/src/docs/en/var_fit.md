# VAR Fit

Connect at least two aligned finite series in time order. The input order determines equation, shock and response ordering. Parameter lags selects consecutive lags 1 through p; the fitted model includes an intercept.

Output model retains coefficients, covariance, equation statistics, design and residuals. VAR Summary computes selected lag-exclusion, residual LM and stability analyses. Granger, IRF and FEVD have separate nodes; IRF/FEVD provide a configurable horizon. These analyses reuse the fitted model and are not computed during Fit.

No single-series fitted/residual output is provided for this multivariate model.

Additional controls: `constant=true`, `dfk=false`, and optional comma-separated `selected_lags` (e.g. `1,3`). A nonempty selected list overrides `lags`; it must be strictly increasing positive integers ≤1000, below sample size. Optional repeated `exogenous` inputs are contemporaneous aligned columns. Results retain actual variable/exogenous names, innovation covariance sigma and zero-based complete estimation rows. Summary shows named equation coefficients, equations and optional companion-root unit-circle data; IRF/FEVD tables identify horizon, response and impulse. All inputs remain finite complete data; no new missing-value imputation is introduced.
